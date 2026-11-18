//! SQLite history store (schema from docs/PLAN.md §6) plus blob and thumbnail files.
//!
//! Layout inside the store directory:
//! - `history.db`      SQLite database (WAL)
//! - `blobs/<hash>-<n>.bin`  payloads that are images or larger than 64 KB
//! - `thumbs/<hash>.png`     256 px PNG thumbnails for image items

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use tracing::{debug, warn};

use crate::config::{ensure_private_dir, Config};
use crate::model::{ClipEvent, ClipItem, Format, Kind};

const DB_FILE: &str = "history.db";
const SCHEMA_VERSION: i64 = 1;
/// Payloads up to this size are stored inline in SQLite (unless they are images).
const INLINE_MAX: usize = 64 * 1024;
const PREVIEW_CHARS: usize = 300;
/// Only the first N characters of text are kept for search.
const SEARCH_MAX_CHARS: usize = 64 * 1024;
const THUMB_SIZE: u32 = 256;

const SCHEMA: &str = "
CREATE TABLE items (
  id          INTEGER PRIMARY KEY,
  kind        TEXT NOT NULL,
  hash        TEXT NOT NULL UNIQUE,
  preview     TEXT,
  search_text TEXT,
  source_app  TEXT,
  byte_size   INTEGER NOT NULL,
  width       INTEGER,
  height      INTEGER,
  pinned      INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,
  last_used_at INTEGER NOT NULL
);
CREATE TABLE formats (
  item_id   INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  mime      TEXT NOT NULL,
  inline    BLOB,
  blob_path TEXT,
  PRIMARY KEY (item_id, mime)
);
CREATE INDEX idx_items_order ON items(pinned DESC, last_used_at DESC);
";

/// Which items `Store::list` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    /// Plain text and rich text (HTML) items.
    Text,
    Images,
    Files,
    Pinned,
}

pub struct Store {
    dir: PathBuf,
    conn: Connection,
}

impl Store {
    /// Open (or create) the store in `dir`. Creates `history.db`, `blobs/` and `thumbs/`.
    pub fn open(dir: &Path) -> Result<Store> {
        ensure_private_dir(dir)?;
        ensure_private_dir(&dir.join("blobs"))?;
        ensure_private_dir(&dir.join("thumbs"))?;

        let db_path = dir.join(DB_FILE);
        let mut conn = Connection::open(&db_path)
            .with_context(|| format!("opening {}", db_path.display()))?;
        // History is private: owner read/write only. The -wal and -shm files that
        // SQLite creates next copy the permissions of this file.
        make_private(&db_path);
        // Files from older versions may still be readable by others: tighten them once.
        for sub in ["blobs", "thumbs"] {
            if let Ok(entries) = fs::read_dir(dir.join(sub)) {
                for entry in entries.flatten() {
                    make_private(&entry.path());
                }
            }
        }
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get::<_, String>(0))?;
        for suffix in ["-wal", "-shm"] {
            let mut side = db_path.clone().into_os_string();
            side.push(suffix);
            make_private(Path::new(&side));
        }

        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        match version {
            0 => {
                let tx = conn.transaction()?;
                tx.execute_batch(SCHEMA)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
                tx.commit()?;
            }
            SCHEMA_VERSION => {}
            v => bail!("history database has unsupported schema version {v}"),
        }
        Ok(Store { dir: dir.to_path_buf(), conn })
    }

    /// Store one clipboard change. Returns `Some(id)` when a new item was stored, and `None`
    /// when it was skipped (paused, sensitive, ignored app, blank, disabled kind, empty) or was
    /// a duplicate (the existing item is moved to the top instead).
    pub fn insert_event(&mut self, ev: &ClipEvent, cfg: &Config) -> Result<Option<i64>> {
        let privacy = &cfg.privacy;
        let general = &cfg.general;

        if privacy.paused {
            debug!("recording paused, ignoring clipboard change");
            return Ok(None);
        }
        if ev.sensitive && privacy.skip_sensitive {
            debug!("sensitive clipboard content skipped");
            return Ok(None);
        }
        if let Some(app) = &ev.source_app {
            if privacy.ignored_apps.iter().any(|a| a.eq_ignore_ascii_case(app)) {
                debug!(app = %app, "source app is ignored");
                return Ok(None);
            }
        }

        let with_files = with_uri_list(&ev.formats);
        let formats = usable_formats(with_files.as_deref().unwrap_or(&ev.formats));
        if formats.is_empty() {
            return Ok(None);
        }
        let kind = classify(&formats);
        let enabled = match kind {
            Kind::Image => general.save_images,
            Kind::Files => general.save_files,
            Kind::Html => general.save_rich_text,
            Kind::Text | Kind::Other => true,
        };
        if !enabled {
            debug!(kind = kind.as_str(), "kind disabled in settings");
            return Ok(None);
        }

        let text = item_text(&formats, kind);
        if text.as_ref().is_some_and(|t| t.trim().is_empty()) {
            debug!("whitespace-only text skipped");
            return Ok(None);
        }

        // Keep formats within max_item_mb. The best image and text/plain are always kept
        // (PLAN §6) so the item itself is never lost.
        let limit = (general.max_item_mb as usize).saturating_mul(1024 * 1024);
        let primary = primary_index(&formats, kind);
        let kept: Vec<(usize, &Format)> = formats
            .iter()
            .enumerate()
            .filter(|(i, f)| Some(*i) == primary || is_plain(f) || f.data.len() <= limit)
            .map(|(i, f)| (i, *f))
            .collect();
        if kept.is_empty() {
            return Ok(None);
        }

        let hash = content_hash(kind, &formats, primary);
        if let Some(id) = self.id_by_hash(&hash)? {
            self.touch(id)?;
            debug!(id, "duplicate content, moved to top");
            return Ok(None);
        }

        let thumbs_dir = self.dir.join("thumbs");
        let mut written: Vec<PathBuf> = Vec::new();
        let desc = describe(kind, &formats, primary, text.as_deref(), &hash, &thumbs_dir, &mut written);

        let result = self.persist(&hash, kind, &desc, &kept, ev.source_app.clone(), &mut written);
        match result {
            Ok(id) => Ok(Some(id)),
            Err(e) => {
                for p in &written {
                    let _ = fs::remove_file(p);
                }
                Err(e)
            }
        }
    }

    /// Items for the popup, pinned first, then most recently used. `query` is a
    /// case-insensitive substring match (empty = no search).
    pub fn list(&self, query: &str, filter: Filter, limit: usize) -> Result<Vec<ClipItem>> {
        let q = query.trim().to_lowercase();
        let filter_sql = match filter {
            Filter::All => "1=1",
            Filter::Text => "kind IN ('text', 'html')",
            Filter::Images => "kind = 'image'",
            Filter::Files => "kind = 'files'",
            Filter::Pinned => "pinned = 1",
        };
        let sql = format!(
            "SELECT id, hash, kind, preview, source_app, byte_size, width, height, pinned, created_at, last_used_at
             FROM items
             WHERE ({filter_sql}) AND (?1 = '' OR instr(search_text, ?1) > 0)
             ORDER BY pinned DESC, last_used_at DESC, id DESC
             LIMIT ?2"
        );
        let thumbs_dir = self.dir.join("thumbs");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![q, limit as i64], |r| {
            let hash: String = r.get(1)?;
            let kind = Kind::from_str(&r.get::<_, String>(2)?);
            let thumb_path = if kind == Kind::Image {
                let p = thumbs_dir.join(format!("{hash}.png"));
                p.exists().then_some(p)
            } else {
                None
            };
            Ok(ClipItem {
                id: r.get(0)?,
                kind,
                preview: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                source_app: r.get(4)?,
                byte_size: r.get(5)?,
                width: r.get(6)?,
                height: r.get(7)?,
                pinned: r.get::<_, i64>(8)? != 0,
                created_at: r.get(9)?,
                last_used_at: r.get(10)?,
                thumb_path,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    /// All stored representations of an item, in capture order.
    pub fn formats(&self, id: i64) -> Result<Vec<Format>> {
        let mut stmt = self
            .conn
            .prepare("SELECT mime, inline, blob_path FROM formats WHERE item_id = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<Vec<u8>>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (mime, inline, blob) = row?;
            if let Some(data) = self.load_payload(inline, blob) {
                out.push(Format { mime, data });
            }
        }
        Ok(out)
    }

    /// The text/plain representation of an item, if it has one.
    pub fn plain_text(&self, id: i64) -> Result<Option<String>> {
        let row = self
            .conn
            .query_row(
                "SELECT inline, blob_path FROM formats
                 WHERE item_id = ?1 AND mime LIKE 'text/plain%'
                 ORDER BY rowid LIMIT 1",
                [id],
                |r| Ok((r.get::<_, Option<Vec<u8>>>(0)?, r.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        Ok(row
            .and_then(|(inline, blob)| self.load_payload(inline, blob))
            .map(|d| String::from_utf8_lossy(&d).into_owned()))
    }

    pub fn set_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        self.conn
            .execute("UPDATE items SET pinned = ?1 WHERE id = ?2", params![pinned as i64, id])?;
        Ok(())
    }

    /// Remove one item and its blob and thumbnail files.
    pub fn delete(&self, id: i64) -> Result<()> {
        let hash: Option<String> = self
            .conn
            .query_row("SELECT hash FROM items WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        let blobs: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT blob_path FROM formats WHERE item_id = ?1 AND blob_path IS NOT NULL")?;
            let rows = stmt.query_map([id], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        self.conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
        for b in blobs {
            remove_quiet(&self.dir.join(b));
        }
        if let Some(h) = hash {
            remove_quiet(&self.dir.join("thumbs").join(format!("{h}.png")));
        }
        Ok(())
    }

    /// Delete all items, or only unpinned ones when `keep_pinned` is true.
    pub fn clear(&self, keep_pinned: bool) -> Result<()> {
        let sql = if keep_pinned {
            "DELETE FROM items WHERE pinned = 0"
        } else {
            "DELETE FROM items"
        };
        self.conn.execute(sql, [])?;
        self.purge_orphan_files()
    }

    /// Mark an item as just used (moves it to the top of the list).
    pub fn touch(&self, id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE items SET last_used_at = ?1 WHERE id = ?2", params![now_ms(), id])?;
        Ok(())
    }

    /// Enforce `max_items` (unpinned items) and `max_total_mb` (all items), oldest unpinned
    /// first. Pinned items are never removed. Returns how many items were removed.
    pub fn prune(&mut self, cfg: &Config) -> Result<usize> {
        let expired = self.delete_expired(false, cfg.general.keep_days)?
            + self.delete_expired(true, cfg.general.keep_pinned_days)?;
        Ok(expired + self.prune_to_limits(cfg)?)
    }

    /// Deletes pinned (or unpinned) items not copied or used for `keep_days`
    /// days. 0 keeps them all. Returns how many were deleted.
    pub fn delete_expired(&mut self, pinned: bool, keep_days: u32) -> Result<usize> {
        if keep_days == 0 {
            return Ok(0);
        }
        let cutoff = now_ms() - i64::from(keep_days) * 24 * 60 * 60 * 1000;
        let removed = self.conn.execute(
            "DELETE FROM items WHERE pinned = ?1 AND last_used_at < ?2",
            rusqlite::params![pinned, cutoff],
        )?;
        if removed > 0 {
            debug!(removed, pinned, keep_days, "deleted old history items");
            self.purge_orphan_files()?;
        }
        Ok(removed)
    }

    /// Keeps the history within the item count and storage limits.
    fn prune_to_limits(&mut self, cfg: &Config) -> Result<usize> {
        let max_items = cfg.general.max_items as usize;
        let max_bytes = cfg.general.max_total_mb as i64 * 1024 * 1024;

        let mut total = self.total_bytes()?;
        let unpinned: Vec<(i64, i64)> = {
            let mut stmt = self.conn.prepare(
                "SELECT id, byte_size FROM items WHERE pinned = 0 ORDER BY last_used_at ASC, id ASC",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut excess = unpinned.len().saturating_sub(max_items);
        let mut victims = Vec::new();
        for (id, size) in unpinned {
            if excess == 0 && total <= max_bytes {
                break;
            }
            victims.push(id);
            total -= size;
            excess = excess.saturating_sub(1);
        }

        if victims.is_empty() {
            return Ok(0);
        }
        let tx = self.conn.transaction()?;
        for id in &victims {
            tx.execute("DELETE FROM items WHERE id = ?1", [id])?;
        }
        tx.commit()?;
        debug!(removed = victims.len(), "pruned history");
        self.purge_orphan_files()?;
        Ok(victims.len())
    }

    /// Sum of `byte_size` over all items.
    pub fn total_bytes(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COALESCE(SUM(byte_size), 0) FROM items", [], |r| r.get(0))?)
    }

    // ----- internals -------------------------------------------------------------------

    fn id_by_hash(&self, hash: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT id FROM items WHERE hash = ?1", [hash], |r| r.get(0))
            .optional()?)
    }

    fn load_payload(&self, inline: Option<Vec<u8>>, blob: Option<String>) -> Option<Vec<u8>> {
        if inline.is_some() {
            return inline;
        }
        let rel = blob?;
        match fs::read(self.dir.join(&rel)) {
            Ok(data) => Some(data),
            Err(e) => {
                warn!(path = %rel, error = %e, "missing payload file, skipping format");
                None
            }
        }
    }

    /// Write blob files (recording them in `written` for cleanup) and insert the item rows.
    fn persist(
        &mut self,
        hash: &str,
        kind: Kind,
        desc: &Description,
        kept: &[(usize, &Format)],
        source_app: Option<String>,
        written: &mut Vec<PathBuf>,
    ) -> Result<i64> {
        let mut rows: Vec<(String, Option<Vec<u8>>, Option<String>)> = Vec::with_capacity(kept.len());
        let mut byte_size: i64 = 0;
        for (idx, f) in kept {
            byte_size += f.data.len() as i64;
            if is_image(&f.mime) || f.data.len() > INLINE_MAX {
                let rel = format!("blobs/{hash}-{idx}.bin");
                let path = self.dir.join(&rel);
                written.push(path.clone());
                fs::write(&path, &f.data).with_context(|| format!("writing {}", path.display()))?;
                make_private(&path);
                rows.push((f.mime.clone(), None, Some(rel)));
            } else {
                rows.push((f.mime.clone(), Some(f.data.clone()), None));
            }
        }

        let now = now_ms();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO items (kind, hash, preview, search_text, source_app, byte_size, width, height,
                                pinned, created_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?9)",
            params![
                kind.as_str(),
                hash,
                desc.preview,
                desc.search_text,
                source_app,
                byte_size,
                desc.width,
                desc.height,
                now
            ],
        )?;
        let id = tx.last_insert_rowid();
        for (mime, inline, blob) in rows {
            tx.execute(
                "INSERT INTO formats (item_id, mime, inline, blob_path) VALUES (?1, ?2, ?3, ?4)",
                params![id, mime, inline, blob],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    /// Delete blob and thumbnail files that no item references anymore.
    fn purge_orphan_files(&self) -> Result<()> {
        let mut keep_blobs: HashSet<String> = HashSet::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT blob_path FROM formats WHERE blob_path IS NOT NULL")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for p in rows {
                if let Some(name) = Path::new(&p?).file_name() {
                    keep_blobs.insert(name.to_string_lossy().into_owned());
                }
            }
        }
        let mut keep_thumbs: HashSet<String> = HashSet::new();
        {
            let mut stmt = self.conn.prepare("SELECT hash FROM items")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for h in rows {
                keep_thumbs.insert(format!("{}.png", h?));
            }
        }
        sweep_dir(&self.dir.join("blobs"), &keep_blobs);
        sweep_dir(&self.dir.join("thumbs"), &keep_thumbs);
        Ok(())
    }
}

/// Fields computed from the content before the database write.
struct Description {
    preview: String,
    search_text: String,
    width: Option<u32>,
    height: Option<u32>,
}

// ----- classification and content helpers ------------------------------------------------

fn base_mime(mime: &str) -> String {
    mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

fn is_image(mime: &str) -> bool {
    base_mime(mime).starts_with("image/")
}

fn is_plain(f: &Format) -> bool {
    base_mime(&f.mime) == "text/plain"
}

/// GNOME's file manager on Wayland can offer copied files only as
/// `x-special/gnome-copied-files` ("copy" or "cut", then one URI per line).
/// When there is no `text/uri-list`, this returns the formats with one added,
/// so the item is stored as Files. The original format is kept for pasting.
fn with_uri_list(formats: &[Format]) -> Option<Vec<Format>> {
    if formats.iter().any(|f| base_mime(&f.mime) == "text/uri-list") {
        return None;
    }
    let gnome = formats.iter().find(|f| base_mime(&f.mime) == "x-special/gnome-copied-files")?;
    let text = String::from_utf8_lossy(&gnome.data);
    let uris: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("://"))
        .collect();
    if uris.is_empty() {
        return None;
    }
    let mut out = vec![Format { mime: "text/uri-list".into(), data: (uris.join("\r\n") + "\r\n").into_bytes() }];
    out.extend(formats.iter().cloned());
    Some(out)
}

/// Drop empty formats and repeated base MIME types (first one wins).
fn usable_formats(formats: &[Format]) -> Vec<&Format> {
    let mut seen = HashSet::new();
    formats
        .iter()
        .filter(|f| !f.data.is_empty() && seen.insert(base_mime(&f.mime)))
        .collect()
}

fn classify(formats: &[&Format]) -> Kind {
    let bases: Vec<String> = formats.iter().map(|f| base_mime(&f.mime)).collect();
    if bases.iter().any(|m| m == "text/uri-list") {
        Kind::Files
    } else if bases.iter().any(|m| m.starts_with("image/")) {
        Kind::Image
    } else if bases.iter().any(|m| m == "text/html") {
        Kind::Html
    } else if bases.iter().any(|m| m == "text/plain") {
        Kind::Text
    } else {
        Kind::Other
    }
}

/// Index of the format whose bytes identify the item (dedupe key, thumbnail source).
fn primary_index(formats: &[&Format], kind: Kind) -> Option<usize> {
    let find = |want: &str| formats.iter().position(|f| base_mime(&f.mime) == want);
    match kind {
        Kind::Files => find("text/uri-list"),
        Kind::Html => find("text/html"),
        Kind::Text => find("text/plain"),
        Kind::Image => formats
            .iter()
            .enumerate()
            .filter(|(_, f)| is_image(&f.mime))
            .min_by_key(|(_, f)| match base_mime(&f.mime).as_str() {
                "image/png" => 0,
                "image/jpeg" | "image/jpg" => 1,
                _ => 2,
            })
            .map(|(i, _)| i),
        Kind::Other => None,
    }
}

/// Plain text for Text items, and plain text or tag-stripped HTML for Html items.
fn item_text(formats: &[&Format], kind: Kind) -> Option<String> {
    let text_of = |want: &str| {
        formats
            .iter()
            .find(|f| base_mime(&f.mime) == want)
            .map(|f| String::from_utf8_lossy(&f.data).into_owned())
    };
    match kind {
        Kind::Text => text_of("text/plain"),
        Kind::Html => text_of("text/plain").or_else(|| text_of("text/html").map(|h| strip_tags(&h))),
        _ => None,
    }
}

/// Canonical hash: kind plus primary payload (or all payloads for Other items).
fn content_hash(kind: Kind, formats: &[&Format], primary: Option<usize>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(kind.as_str().as_bytes());
    hasher.update(&[0]);
    match primary {
        Some(i) => {
            hasher.update(&formats[i].data);
        }
        None => {
            let mut sorted: Vec<&&Format> = formats.iter().collect();
            sorted.sort_by_key(|f| base_mime(&f.mime));
            for f in sorted {
                hasher.update(base_mime(&f.mime).as_bytes());
                hasher.update(&[0]);
                hasher.update(&f.data);
                hasher.update(&[0]);
            }
        }
    }
    hasher.finalize().to_hex().to_string()
}

fn describe(
    kind: Kind,
    formats: &[&Format],
    primary: Option<usize>,
    text: Option<&str>,
    hash: &str,
    thumbs_dir: &Path,
    written: &mut Vec<PathBuf>,
) -> Description {
    match kind {
        Kind::Text | Kind::Html => {
            let text = text.unwrap_or("");
            Description {
                preview: make_preview(text),
                search_text: truncate_chars(text, SEARCH_MAX_CHARS).to_lowercase(),
                width: None,
                height: None,
            }
        }
        Kind::Files => {
            let names = primary
                .map(|i| file_names(&String::from_utf8_lossy(&formats[i].data)))
                .unwrap_or_default();
            Description {
                preview: files_preview(&names),
                search_text: names.join("\n").to_lowercase(),
                width: None,
                height: None,
            }
        }
        Kind::Image => {
            let data: &[u8] = primary.map(|i| formats[i].data.as_slice()).unwrap_or(&[]);
            let dims = write_thumbnail(thumbs_dir, hash, data, written);
            let preview = match dims {
                Some((w, h)) => format!("Image {w}×{h}"),
                None => "Image".to_string(),
            };
            Description {
                preview,
                search_text: String::new(),
                width: dims.map(|d| d.0),
                height: dims.map(|d| d.1),
            }
        }
        Kind::Other => {
            let mut mimes: Vec<String> = formats.iter().map(|f| base_mime(&f.mime)).collect();
            mimes.sort();
            Description {
                preview: make_preview(&format!("Other: {}", mimes.join(", "))),
                search_text: String::new(),
                width: None,
                height: None,
            }
        }
    }
}

/// Restricts a file to its owner (mode 0600). Failure is not fatal: the data
/// folder is already owner-only (0700).
fn make_private(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

/// Decode the image, write a 256 px PNG thumbnail and return the original dimensions.
/// Failures are logged and leave the item without a thumbnail.
fn write_thumbnail(thumbs_dir: &Path, hash: &str, data: &[u8], written: &mut Vec<PathBuf>) -> Option<(u32, u32)> {
    let img = match image::load_from_memory(data) {
        Ok(img) => img,
        Err(e) => {
            debug!(error = %e, "could not decode image, no thumbnail");
            return None;
        }
    };
    let dims = (img.width(), img.height());
    let path = thumbs_dir.join(format!("{hash}.png"));
    written.push(path.clone());
    let thumb = img.thumbnail(THUMB_SIZE, THUMB_SIZE);
    match thumb.save_with_format(&path, image::ImageFormat::Png) {
        Ok(()) => make_private(&path),
        Err(e) => warn!(error = %e, "could not write thumbnail"),
    }
    Some(dims)
}

fn make_preview(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > PREVIEW_CHARS {
        let mut s: String = collapsed.chars().take(PREVIEW_CHARS).collect();
        s.push('…');
        s
    } else {
        collapsed
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Very rough tag stripper used only for previews and search of HTML without plain text.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// File names from a text/uri-list body (`file:///path/to/name%20x.pdf` -> `name x.pdf`).
fn file_names(uri_list: &str) -> Vec<String> {
    uri_list
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let path = l.strip_prefix("file://").unwrap_or(l);
            let last = path.trim_end_matches('/').rsplit('/').next().unwrap_or(path);
            let name = percent_decode(last);
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let decoded = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(b) = decoded {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn files_preview(names: &[String]) -> String {
    match names.len() {
        0 => "Files".to_string(),
        1 => make_preview(&names[0]),
        n => make_preview(&format!("{} ({} files)", names.join(", "), n)),
    }
}

fn remove_quiet(path: &Path) {
    if let Err(e) = fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(path = %path.display(), error = %e, "could not remove file");
        }
    }
}

/// Delete regular files in `dir` whose names are not in `keep`.
fn sweep_dir(dir: &Path, keep: &HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_file && !keep.contains(&name) {
            remove_quiet(&entry.path());
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::{tempdir, TempDir};

    fn text_ev(s: &str) -> ClipEvent {
        ClipEvent {
            formats: vec![Format { mime: "text/plain;charset=utf-8".into(), data: s.as_bytes().to_vec() }],
            source_app: None,
            sensitive: false,
        }
    }

    #[test]
    fn gnome_copied_files_are_stored_as_files() {
        let (_d, mut store) = open();
        let ev = ClipEvent {
            formats: vec![
                Format { mime: "text/plain;charset=utf-8".into(), data: b"notes.txt".to_vec() },
                Format {
                    mime: "x-special/gnome-copied-files".into(),
                    data: b"copy\nfile:///home/u/notes.txt\nfile:///home/u/My%20Doc.pdf".to_vec(),
                },
            ],
            source_app: Some("org.gnome.Nautilus".into()),
            sensitive: false,
        };
        let id = store.insert_event(&ev, &Config::default()).unwrap().unwrap();
        let files = store.list("", Filter::Files, 10).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].id, id);
        assert!(store.list("", Filter::Text, 10).unwrap().is_empty());
        let formats = store.formats(id).unwrap();
        let uri = formats.iter().find(|f| f.mime == "text/uri-list").unwrap();
        assert_eq!(uri.data, b"file:///home/u/notes.txt\r\nfile:///home/u/My%20Doc.pdf\r\n");
        // The original format is kept so pasting into the file manager works.
        assert!(formats.iter().any(|f| f.mime == "x-special/gnome-copied-files"));
    }

    #[test]
    fn old_items_expire() {
        let (_d, mut store) = open();
        let cfg = Config::default(); // 2 days unpinned, 10 days pinned
        let day = 24 * 60 * 60 * 1000;
        let old = store.insert_event(&text_ev("3 days, unpinned"), &cfg).unwrap().unwrap();
        let pinned_recent = store.insert_event(&text_ev("3 days, pinned"), &cfg).unwrap().unwrap();
        let pinned_old = store.insert_event(&text_ev("11 days, pinned"), &cfg).unwrap().unwrap();
        let fresh = store.insert_event(&text_ev("fresh"), &cfg).unwrap().unwrap();
        store.set_pinned(pinned_recent, true).unwrap();
        store.set_pinned(pinned_old, true).unwrap();
        let set_age = |store: &Store, id: i64, days: i64| {
            store
                .conn
                .execute("UPDATE items SET last_used_at = ?1 WHERE id = ?2", [now_ms() - days * day, id])
                .unwrap();
        };
        set_age(&store, old, 3);
        set_age(&store, pinned_recent, 3);
        set_age(&store, pinned_old, 11);

        // 0 means keep forever.
        assert_eq!(store.delete_expired(false, 0).unwrap(), 0);
        assert_eq!(store.delete_expired(true, 0).unwrap(), 0);

        store.prune(&cfg).unwrap();
        let ids: Vec<i64> = store.list("", Filter::All, 10).unwrap().iter().map(|i| i.id).collect();
        assert!(!ids.contains(&old));
        assert!(ids.contains(&pinned_recent));
        assert!(!ids.contains(&pinned_old));
        assert!(ids.contains(&fresh));
    }

    fn open() -> (TempDir, Store) {
        let dir = tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        (dir, store)
    }

    fn tick() {
        // Make sure consecutive last_used_at values differ (they are in milliseconds).
        sleep(Duration::from_millis(3));
    }

    fn count_files(dir: &Path) -> usize {
        fs::read_dir(dir).unwrap().count()
    }

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
        let mut cur = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut cur, image::ImageFormat::Png)
            .unwrap();
        cur.into_inner()
    }

    #[test]
    fn schema_version_and_subdirs() {
        let (dir, _store) = open();
        assert!(dir.path().join("blobs").is_dir());
        assert!(dir.path().join("thumbs").is_dir());
        let conn = Connection::open(dir.path().join(DB_FILE)).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn dedupe_moves_existing_item_to_top() {
        let (_dir, mut store) = open();
        let cfg = Config::default();
        let a = store.insert_event(&text_ev("alpha"), &cfg).unwrap();
        assert!(a.is_some());
        tick();
        store.insert_event(&text_ev("beta"), &cfg).unwrap();
        tick();
        let dup = store.insert_event(&text_ev("alpha"), &cfg).unwrap();
        assert!(dup.is_none());

        let items = store.list("", Filter::All, 10).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, a.unwrap());
        assert_eq!(items[0].preview, "alpha");
    }

    #[test]
    fn whitespace_only_is_skipped() {
        let (_dir, mut store) = open();
        let cfg = Config::default();
        assert!(store.insert_event(&text_ev(" \n\t  "), &cfg).unwrap().is_none());
        assert!(store.list("", Filter::All, 10).unwrap().is_empty());
    }

    #[test]
    fn sensitive_is_skipped_unless_allowed() {
        let (_dir, mut store) = open();
        let mut ev = text_ev("hunter2");
        ev.sensitive = true;
        assert!(store.insert_event(&ev, &Config::default()).unwrap().is_none());

        let mut cfg = Config::default();
        cfg.privacy.skip_sensitive = false;
        assert!(store.insert_event(&ev, &cfg).unwrap().is_some());
    }

    #[test]
    fn paused_ignored_app_and_disabled_kinds_are_skipped() {
        let (_dir, mut store) = open();

        let mut paused = Config::default();
        paused.privacy.paused = true;
        assert!(store.insert_event(&text_ev("x"), &paused).unwrap().is_none());

        let mut ev = text_ev("secret");
        ev.source_app = Some("keepassxc".into());
        assert!(store.insert_event(&ev, &Config::default()).unwrap().is_none());

        let mut no_images = Config::default();
        no_images.general.save_images = false;
        let img = ClipEvent {
            formats: vec![Format { mime: "image/png".into(), data: png_bytes(4, 4) }],
            source_app: None,
            sensitive: false,
        };
        assert!(store.insert_event(&img, &no_images).unwrap().is_none());

        assert!(store.list("", Filter::All, 10).unwrap().is_empty());
    }

    #[test]
    fn prune_respects_max_items() {
        let (_dir, mut store) = open();
        let mut cfg = Config::default();
        cfg.general.max_items = 2;
        for s in ["a", "b", "c", "d"] {
            store.insert_event(&text_ev(s), &cfg).unwrap();
            tick();
        }
        let removed = store.prune(&cfg).unwrap();
        assert_eq!(removed, 2);
        let previews: Vec<String> = store
            .list("", Filter::All, 10)
            .unwrap()
            .into_iter()
            .map(|i| i.preview)
            .collect();
        assert_eq!(previews, vec!["d", "c"]);
    }

    #[test]
    fn prune_respects_total_size_and_deletes_blobs() {
        let (dir, mut store) = open();
        let mut cfg = Config::default();
        cfg.general.max_total_mb = 1;
        store.insert_event(&text_ev(&"a".repeat(600_000)), &cfg).unwrap();
        tick();
        store.insert_event(&text_ev(&"b".repeat(600_000)), &cfg).unwrap();
        assert_eq!(count_files(&dir.path().join("blobs")), 2);

        store.prune(&cfg).unwrap();
        assert!(store.total_bytes().unwrap() <= 1024 * 1024);
        assert_eq!(count_files(&dir.path().join("blobs")), 1);
    }

    #[test]
    fn pin_survives_prune_and_reopen() {
        let dir = tempdir().unwrap();
        let mut cfg = Config::default();
        cfg.general.max_items = 1;
        {
            let mut store = Store::open(dir.path()).unwrap();
            let old = store.insert_event(&text_ev("old"), &cfg).unwrap().unwrap();
            store.set_pinned(old, true).unwrap();
            tick();
            store.insert_event(&text_ev("mid"), &cfg).unwrap();
            tick();
            store.insert_event(&text_ev("new"), &cfg).unwrap();
            store.prune(&cfg).unwrap();
        }
        let store = Store::open(dir.path()).unwrap();
        let previews: Vec<String> = store
            .list("", Filter::All, 10)
            .unwrap()
            .into_iter()
            .map(|i| i.preview)
            .collect();
        assert_eq!(previews, vec!["old", "new"]);
        assert_eq!(store.list("", Filter::Pinned, 10).unwrap().len(), 1);
    }

    #[test]
    fn search_is_case_insensitive_substring() {
        let (_dir, mut store) = open();
        let cfg = Config::default();
        store.insert_event(&text_ev("Hello World"), &cfg).unwrap();
        store.insert_event(&text_ev("rust code"), &cfg).unwrap();
        store.insert_event(&text_ev("Ärger über Büro"), &cfg).unwrap();

        assert_eq!(store.list("hello", Filter::All, 10).unwrap().len(), 1);
        assert_eq!(store.list("WORLD", Filter::All, 10).unwrap().len(), 1);
        assert_eq!(store.list("ÄRGER", Filter::All, 10).unwrap().len(), 1);
        assert_eq!(store.list("nomatch", Filter::All, 10).unwrap().len(), 0);
        assert_eq!(store.list("", Filter::All, 10).unwrap().len(), 3);
    }

    #[test]
    fn filters_by_kind() {
        let (_dir, mut store) = open();
        let cfg = Config::default();
        store.insert_event(&text_ev("plain"), &cfg).unwrap();
        let html = ClipEvent {
            formats: vec![
                Format { mime: "text/html".into(), data: b"<b>bold</b> words".to_vec() },
                Format { mime: "text/plain".into(), data: b"bold words".to_vec() },
            ],
            source_app: None,
            sensitive: false,
        };
        let html_id = store.insert_event(&html, &cfg).unwrap().unwrap();
        let files = ClipEvent {
            formats: vec![Format {
                mime: "text/uri-list".into(),
                data: b"file:///home/u/report%20final.pdf\nfile:///tmp/notes.txt\n".to_vec(),
            }],
            source_app: None,
            sensitive: false,
        };
        store.insert_event(&files, &cfg).unwrap();

        assert_eq!(store.list("", Filter::Text, 10).unwrap().len(), 2);
        assert_eq!(store.list("", Filter::Files, 10).unwrap().len(), 1);
        let f = store.list("", Filter::Files, 10).unwrap();
        assert_eq!(f[0].preview, "report final.pdf, notes.txt (2 files)");
        assert_eq!(store.list("bold", Filter::All, 10).unwrap()[0].id, html_id);
        assert_eq!(store.plain_text(html_id).unwrap().as_deref(), Some("bold words"));
    }

    #[test]
    fn image_gets_thumbnail_and_blob() {
        let (dir, mut store) = open();
        let cfg = Config::default();
        let data = png_bytes(600, 300);
        let ev = ClipEvent {
            formats: vec![Format { mime: "image/png".into(), data: data.clone() }],
            source_app: None,
            sensitive: false,
        };
        let id = store.insert_event(&ev, &cfg).unwrap().unwrap();

        let items = store.list("", Filter::Images, 10).unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(item.kind, Kind::Image);
        assert_eq!(item.width, Some(600));
        assert_eq!(item.height, Some(300));
        assert_eq!(item.preview, "Image 600×300");

        let thumb = item.thumb_path.as_ref().expect("thumbnail path");
        assert!(thumb.exists());
        let t = image::open(thumb).unwrap();
        assert_eq!((t.width(), t.height()), (256, 128));

        assert_eq!(count_files(&dir.path().join("blobs")), 1);
        let formats = store.formats(id).unwrap();
        assert_eq!(formats.len(), 1);
        assert_eq!(formats[0].data, data);
    }

    #[test]
    fn large_text_goes_to_blob_and_roundtrips() {
        let (dir, mut store) = open();
        let text = "x".repeat(100_000);
        let id = store.insert_event(&text_ev(&text), &Config::default()).unwrap().unwrap();
        assert_eq!(count_files(&dir.path().join("blobs")), 1);
        assert_eq!(store.plain_text(id).unwrap().unwrap(), text);
    }

    #[test]
    fn delete_and_clear_remove_files() {
        let (dir, mut store) = open();
        let cfg = Config::default();
        let img = ClipEvent {
            formats: vec![Format { mime: "image/png".into(), data: png_bytes(8, 8) }],
            source_app: None,
            sensitive: false,
        };
        let img_id = store.insert_event(&img, &cfg).unwrap().unwrap();
        let text_id = store.insert_event(&text_ev("keep me"), &cfg).unwrap().unwrap();
        store.set_pinned(text_id, true).unwrap();

        store.delete(img_id).unwrap();
        assert_eq!(count_files(&dir.path().join("blobs")), 0);
        assert_eq!(count_files(&dir.path().join("thumbs")), 0);

        store.insert_event(&img, &cfg).unwrap();
        store.clear(true).unwrap();
        let left = store.list("", Filter::All, 10).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, text_id);
        assert_eq!(count_files(&dir.path().join("blobs")), 0);
        assert_eq!(count_files(&dir.path().join("thumbs")), 0);

        store.clear(false).unwrap();
        assert!(store.list("", Filter::All, 10).unwrap().is_empty());
        assert_eq!(store.total_bytes().unwrap(), 0);
    }
}
