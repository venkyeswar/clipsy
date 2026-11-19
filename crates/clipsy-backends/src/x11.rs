//! X11 backend (pure Rust, x11rb).
//!
//! - Clipboard watching: XFixes `SetSelectionOwner` notifications on CLIPBOARD, then ICCCM
//!   `ConvertSelection` for every useful target (INCR supported on the read side).
//! - Clipboard owning: a per-`set_clipboard` thread owns CLIPBOARD and answers SelectionRequests
//!   (INCR supported on the write side) until ownership is lost.
//! - Hotkeys: `XGrabKey` on the root window, events read on a dedicated connection.
//! - Paste: `_NET_ACTIVE_WINDOW` client message, wait until that window is active, then
//!   Ctrl+V / Ctrl+Shift+V via XTest with short gaps between the key events.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use clipsy_core::{ClipEvent, Format};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, CreateWindowAux,
    EventMask, GrabMode, ModMask, PropMode, Property, SelectionNotifyEvent, Window,
    WindowClass, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::protocol::{Event, ErrorKind};
use x11rb::rust_connection::{ReplyError, RustConnection};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_FROM_PARENT, NONE};

use crate::{Backend, HotkeyAction, PasteMode};

type Atom = u32;
type Owners = Arc<Mutex<HashSet<Window>>>;

/// Per-format cap (both directions).
const MAX_BYTES: usize = 25 * 1024 * 1024;
/// How long to wait for a selection owner / requestor to answer one step.
const STEP_TIMEOUT: Duration = Duration::from_secs(3);
/// Pause between reading TARGETS and requesting content (see read_clipboard).
const SETTLE_DELAY: Duration = Duration::from_millis(20);
const MIME_UTF8: &str = "text/plain;charset=utf-8";
/// Paste: how long to wait for the remembered window to become active again.
const FOCUS_TIMEOUT: Duration = Duration::from_millis(500);
/// Paste: how often to check the active window / held keys while waiting.
const POLL_EVERY: Duration = Duration::from_millis(10);
/// Paste: extra pause once the window is active, so it has processed its focus-in.
const FOCUS_SETTLE: Duration = Duration::from_millis(30);
/// Paste: how long to wait for the user to let go of Shift/Ctrl/Alt/Super.
const MODIFIER_TIMEOUT: Duration = Duration::from_millis(300);
/// Paste: pause between the fake key events. Some terminals (Warp, kitty, VTE under load)
/// track modifiers from separate state updates and miss a chord sent all at once.
const KEY_GAP: Duration = Duration::from_millis(8);
/// Physical V key (evdev and xfree86 keycode), used when no layout has a `v` keysym.
const KEYCODE_V: u8 = 55;
/// Window classes (lower case) that contain one of these belong to a terminal, which pastes
/// with Ctrl+Shift+V. Only names that cannot appear inside an ordinary app's class.
/// Keep in sync with `TERMINAL_SUBSTRINGS` in the GNOME extension (a test checks this).
const TERMINAL_SUBSTRINGS: &[&str] = &[
    "terminal", // gnome-terminal-server, org.gnome.Terminal, xfce4-terminal, warp-terminal...
    "kitty",
    "alacritty",
    "wezterm",
    "konsole",
    "ptyxis",
    "xterm",
    "rxvt",
    "tilix",
    "terminator",
    "ghostty",
    "sakura",
    "guake",
    "yakuake",
    "terminology",
    "cool-retro-term",
];
/// Short or ambiguous terminal classes (lower case): only an exact match counts, so
/// "Warpinator", "stylus" or "Hyperion" are not mistaken for terminals.
/// Keep in sync with `TERMINAL_EXACT` in the GNOME extension (a test checks this).
const TERMINAL_EXACT: &[&str] = &[
    "warp",
    "dev.warp.warp",
    "org.gnome.console",
    "kgx",
    "st",
    "st-256color",
    "foot",
    "footclient",
    "tabby",
    "hyper",
    "contour",
    "rio",
    "blackbox",
    "com.raggesilver.blackbox",
    "tilda",
    "eterm",
    "termite",
    "zutty",
];

// Modifier bits (core protocol).
const M_LOCK: u16 = 1 << 1;
const M_SHIFT: u16 = 1 << 0;
const M_CONTROL: u16 = 1 << 2;
const M_MOD1: u16 = 1 << 3; // Alt
const M_MOD2: u16 = 1 << 4; // NumLock (usually)
const M_MOD3: u16 = 1 << 5;
const M_MOD4: u16 = 1 << 6; // Super
const M_MOD5: u16 = 1 << 7;
/// Modifiers that count for hotkey matching (lock bits are ignored).
const M_MATCH: u16 = M_SHIFT | M_CONTROL | M_MOD1 | M_MOD3 | M_MOD4 | M_MOD5;
/// Lock-modifier combinations every grab is repeated for (CapsLock / NumLock).
const LOCK_COMBOS: [u16; 4] = [0, M_LOCK, M_MOD2, M_LOCK | M_MOD2];

#[derive(Clone)]
struct Atoms {
    clipboard: Atom,
    targets: Atom,
    timestamp: Atom,
    incr: Atom,
    utf8_string: Atom,
    string: Atom,
    text_plain: Atom,
    text_plain_utf8: Atom,
    text_html: Atom,
    image_png: Atom,
    image_jpeg: Atom,
    uri_list: Atom,
    gnome_files: Atom,
    pw_hint: Atom,
    net_active_window: Atom,
    clipsy_data: Atom,
    clipsy_ts: Atom,
}

fn intern(c: &RustConnection, name: &str) -> Result<Atom> {
    Ok(c.intern_atom(false, name.as_bytes())?.reply()?.atom)
}

impl Atoms {
    fn new(c: &RustConnection) -> Result<Self> {
        Ok(Atoms {
            clipboard: intern(c, "CLIPBOARD")?,
            targets: intern(c, "TARGETS")?,
            timestamp: intern(c, "TIMESTAMP")?,
            incr: intern(c, "INCR")?,
            utf8_string: intern(c, "UTF8_STRING")?,
            string: intern(c, "STRING")?,
            text_plain: intern(c, "text/plain")?,
            text_plain_utf8: intern(c, MIME_UTF8)?,
            text_html: intern(c, "text/html")?,
            image_png: intern(c, "image/png")?,
            image_jpeg: intern(c, "image/jpeg")?,
            uri_list: intern(c, "text/uri-list")?,
            gnome_files: intern(c, "x-special/gnome-copied-files")?,
            pw_hint: intern(c, "x-kde-passwordManagerHint")?,
            net_active_window: intern(c, "_NET_ACTIVE_WINDOW")?,
            clipsy_data: intern(c, "CLIPSY_SELECTION_DATA")?,
            clipsy_ts: intern(c, "CLIPSY_TIMESTAMP")?,
        })
    }
}

/// Wait for the next event, polling until `deadline`. `Ok(None)` on timeout.
fn wait_event(c: &RustConnection, deadline: Instant) -> Result<Option<Event>> {
    loop {
        if let Some(ev) = c.poll_for_event()? {
            return Ok(Some(ev));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(2));
    }
}

/// Create the hidden 1x1 window used for selection traffic.
fn create_helper_window(c: &RustConnection, root: Window) -> Result<Window> {
    let win = c.generate_id()?;
    c.create_window(
        0,
        win,
        root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        COPY_FROM_PARENT,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?;
    Ok(win)
}

/// Best-effort application name: WM_CLASS (class part) of the window or its nearest ancestor.
fn source_app(c: &RustConnection, root: Window, mut w: Window) -> Option<String> {
    for _ in 0..8 {
        if w == NONE || w == root {
            break;
        }
        if let Some(reply) = c
            .get_property(false, w, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        {
            let parts: Vec<&[u8]> = reply.value.split(|b| *b == 0).filter(|s| !s.is_empty()).collect();
            if let Some(class) = parts.get(1).or(parts.first()) {
                return Some(String::from_utf8_lossy(class).into_owned());
            }
        }
        match c.query_tree(w).ok().and_then(|cookie| cookie.reply().ok()) {
            Some(tree) => w = tree.parent,
            None => break,
        }
    }
    None
}

/// Accelerator keysyms are looked up in the keyboard mapping: returns (keycode, extra modifiers).
fn keycode_for_keysym(c: &RustConnection, keysym: u32) -> Result<(u8, u16)> {
    let setup = c.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = c.get_keyboard_mapping(min, max - min + 1)?.reply()?;
    let per = map.keysyms_per_keycode as usize;
    if per == 0 {
        bail!("empty keyboard mapping");
    }
    find_keycode(&map.keysyms, per, min, keysym)
        .ok_or_else(|| anyhow!("no key produces keysym 0x{keysym:x} on this keyboard layout"))
}

/// Finds the key that produces `keysym` in a core keyboard mapping (`per` keysyms per key,
/// first key is `min`). Columns 0/1 are the unshifted/shifted level of the first layout;
/// later columns belong to the other layouts (for example "us" after "ru"). Apps match
/// shortcuts like Ctrl+V by physical key in every layout, so a key found there works too.
fn find_keycode(keysyms: &[u32], per: usize, min: u8, keysym: u32) -> Option<(u8, u16)> {
    for col in 0..per {
        let extra = if col % 2 == 1 { M_SHIFT } else { 0 };
        for (i, syms) in keysyms.chunks(per).enumerate() {
            if syms.get(col) == Some(&keysym) {
                return Some((min.checked_add(u8::try_from(i).ok()?)?, extra));
            }
        }
    }
    None
}

/// Calls `done` every `every` until it returns true or `timeout` has passed.
/// Returns whether it became true.
fn wait_until(timeout: Duration, every: Duration, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(every);
    }
}

fn keysym_from_name(name: &str) -> Option<(u32, u16)> {
    let named = match name {
        "Return" => 0xff0d,
        "KP_Enter" => 0xff8d,
        "Escape" => 0xff1b,
        "Tab" => 0xff09,
        "space" => 0x20,
        "BackSpace" => 0xff08,
        "Delete" => 0xffff,
        "Insert" => 0xff63,
        "Home" => 0xff50,
        "End" => 0xff57,
        "Page_Up" => 0xff55,
        "Page_Down" => 0xff56,
        "Left" => 0xff51,
        "Up" => 0xff52,
        "Right" => 0xff53,
        "Down" => 0xff54,
        "Print" => 0xff61,
        "Pause" => 0xff13,
        "Menu" => 0xff67,
        "minus" => 0x2d,
        "equal" => 0x3d,
        "comma" => 0x2c,
        "period" => 0x2e,
        "slash" => 0x2f,
        "semicolon" => 0x3b,
        "apostrophe" => 0x27,
        "grave" => 0x60,
        "backslash" => 0x5c,
        "bracketleft" => 0x5b,
        "bracketright" => 0x5d,
        _ => 0,
    };
    if named != 0 {
        return Some((named, 0));
    }
    if let Some(n) = name.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=12).contains(&n) {
            return Some((0xffbd + n, 0));
        }
    }
    let mut chars = name.chars();
    let c = chars.next()?;
    if chars.next().is_some() || !c.is_ascii() || c.is_ascii_control() {
        return None;
    }
    if c.is_ascii_uppercase() {
        Some((c.to_ascii_lowercase() as u32, M_SHIFT))
    } else {
        Some((c as u32, 0))
    }
}

/// Parse GTK-style accelerators such as "<Super>v", "<Ctrl><Alt>c", "<Shift>Return".
fn parse_accel(accel: &str) -> Result<(u16, u32)> {
    let mut mods = 0u16;
    let mut rest = accel.trim();
    while let Some(stripped) = rest.strip_prefix('<') {
        let end = stripped.find('>').ok_or_else(|| anyhow!("bad accelerator {accel:?}"))?;
        mods |= match stripped[..end].to_ascii_lowercase().as_str() {
            "shift" => M_SHIFT,
            "ctrl" | "control" | "primary" => M_CONTROL,
            "alt" | "mod1" | "meta" => M_MOD1,
            "super" | "mod4" | "hyper" => M_MOD4,
            "mod3" => M_MOD3,
            "mod5" => M_MOD5,
            "lock" => M_LOCK,
            "mod2" => M_MOD2,
            other => bail!("unknown modifier <{other}> in accelerator {accel:?}"),
        };
        rest = &stripped[end + 1..];
    }
    let (keysym, extra) = keysym_from_name(rest.trim())
        .ok_or_else(|| anyhow!("unknown key {:?} in accelerator {accel:?}", rest.trim()))?;
    Ok((mods | extra, keysym))
}

/// True for a known terminal's window class, or one the user listed in `extra`.
fn is_terminal(class: &str, extra: &[String]) -> bool {
    let class = class.trim().to_ascii_lowercase();
    TERMINAL_EXACT.contains(&class.as_str())
        || TERMINAL_SUBSTRINGS.iter().any(|t| class.contains(t))
        || extra.iter().any(|e| e.trim().eq_ignore_ascii_case(&class))
}

// ---------------------------------------------------------------------------------------------
// Clipboard watcher (reader side)
// ---------------------------------------------------------------------------------------------

struct Watcher<'a> {
    c: &'a RustConnection,
    at: &'a Atoms,
    win: Window,
    root: Window,
    owners: Owners,
    /// Set when the clipboard changed while we were in the middle of reading it.
    dirty: Option<u32>,
}

impl Watcher<'_> {
    /// Next event that is not an XFixes notification (those set `dirty` instead).
    fn next_event(&mut self, deadline: Instant) -> Result<Option<Event>> {
        loop {
            let Some(ev) = wait_event(self.c, deadline)? else { return Ok(None) };
            if let Event::XfixesSelectionNotify(e) = &ev {
                if e.selection == self.at.clipboard {
                    self.dirty = Some(e.timestamp);
                }
                continue;
            }
            return Ok(Some(ev));
        }
    }

    /// Read a property and delete it, returning (type, bytes).
    fn fetch(&self, prop: Atom) -> Result<(Atom, Vec<u8>)> {
        let probe = self.c.get_property(false, self.win, prop, AtomEnum::ANY, 0, 0)?.reply()?;
        let total = probe.bytes_after;
        let full = self
            .c
            .get_property(true, self.win, prop, AtomEnum::ANY, 0, total.div_ceil(4))?
            .reply()?;
        Ok((full.type_, full.value))
    }

    /// Convert `target` and return its bytes, or None if unavailable, too big, or timed out.
    fn convert(&mut self, target: Atom, ts: u32) -> Result<Option<Vec<u8>>> {
        self.c
            .convert_selection(self.win, self.at.clipboard, target, self.at.clipsy_data, ts)?;
        self.c.flush()?;
        loop {
            let Some(ev) = self.next_event(Instant::now() + STEP_TIMEOUT)? else { return Ok(None) };
            if let Event::SelectionNotify(e) = ev {
                if e.requestor != self.win || e.target != target || e.selection != self.at.clipboard {
                    continue;
                }
                if e.property == NONE {
                    return Ok(None);
                }
                let (typ, data) = self.fetch(e.property)?;
                if typ != self.at.incr {
                    return Ok((data.len() <= MAX_BYTES).then_some(data));
                }
                return self.read_incr(e.property);
            }
        }
    }

    /// INCR: the owner writes chunks; each NewValue is one chunk, an empty chunk ends it.
    fn read_incr(&mut self, prop: Atom) -> Result<Option<Vec<u8>>> {
        let mut out = Vec::new();
        let mut too_big = false;
        loop {
            let Some(ev) = self.next_event(Instant::now() + STEP_TIMEOUT)? else { return Ok(None) };
            if let Event::PropertyNotify(e) = ev {
                if e.window != self.win || e.atom != prop || e.state != Property::NEW_VALUE {
                    continue;
                }
                let (_, chunk) = self.fetch(prop)?;
                if chunk.is_empty() {
                    return Ok((!too_big).then_some(out));
                }
                if !too_big {
                    if out.len() + chunk.len() > MAX_BYTES {
                        too_big = true;
                        out = Vec::new();
                    } else {
                        out.extend_from_slice(&chunk);
                    }
                }
            }
        }
    }

    /// Read every useful target of the current CLIPBOARD owner. Result may be stale if
    /// `self.dirty` got set; the caller then re-reads.
    fn read_clipboard(&mut self, ts: u32) -> Result<Option<ClipEvent>> {
        let owner = self.c.get_selection_owner(self.at.clipboard)?.reply()?.owner;
        if owner == NONE || self.owners.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&owner) {
            return Ok(None);
        }
        let Some(raw) = self.convert(self.at.targets, ts)? else { return Ok(None) };
        let offered: Vec<Atom> = raw.as_chunks::<4>().0.iter().map(|b| u32::from_ne_bytes(*b)).collect();
        let has = |a: Atom| offered.contains(&a);
        let sensitive = has(self.at.pw_hint);

        // Some owners (seen with xclip) ignore a content request sent right after the TARGETS
        // reply; a short pause before the content conversions makes this reliable.
        thread::sleep(SETTLE_DELAY);

        let mut plan: Vec<(Atom, &'static str)> = Vec::new();
        if has(self.at.utf8_string) {
            plan.push((self.at.utf8_string, MIME_UTF8));
        } else if has(self.at.text_plain_utf8) {
            plan.push((self.at.text_plain_utf8, MIME_UTF8));
        }
        if has(self.at.text_plain) {
            plan.push((self.at.text_plain, "text/plain"));
        } else if plan.is_empty() && has(self.at.string) {
            plan.push((self.at.string, "text/plain"));
        }
        for (atom, mime) in [
            (self.at.text_html, "text/html"),
            (self.at.image_png, "image/png"),
            (self.at.image_jpeg, "image/jpeg"),
            (self.at.uri_list, "text/uri-list"),
            (self.at.gnome_files, "x-special/gnome-copied-files"),
        ] {
            if has(atom) {
                plan.push((atom, mime));
            }
        }

        let mut formats = Vec::new();
        for (target, mime) in plan {
            if let Some(data) = self.convert(target, ts)? {
                if !data.is_empty() {
                    formats.push(Format { mime: mime.to_string(), data });
                }
            }
            if self.dirty.is_some() {
                return Ok(None);
            }
        }
        if formats.is_empty() {
            return Ok(None);
        }
        Ok(Some(ClipEvent {
            formats,
            source_app: source_app(self.c, self.root, owner),
            sensitive,
        }))
    }
}

fn watch(clips: Sender<ClipEvent>, owners: Owners) -> Result<()> {
    let (conn, screen) = x11rb::connect(None).context("watcher: cannot connect to X display")?;
    let root = conn.setup().roots[screen].root;
    let at = Atoms::new(&conn)?;
    let win = create_helper_window(&conn, root)?;
    conn.xfixes_query_version(5, 0)?.reply()?;
    conn.xfixes_select_selection_input(win, at.clipboard, SelectionEventMask::SET_SELECTION_OWNER)?;
    conn.flush()?;

    let mut watcher = Watcher { c: &conn, at: &at, win, root, owners, dirty: None };
    loop {
        if let Event::XfixesSelectionNotify(e) = conn.wait_for_event()? {
            if e.selection != at.clipboard {
                continue;
            }
            let mut ts = e.timestamp;
            for _ in 0..4 {
                watcher.dirty = None;
                let clip = watcher.read_clipboard(ts)?;
                match watcher.dirty {
                    // Clipboard changed mid-read: read again from the newest owner.
                    Some(newer) => ts = newer,
                    None => {
                        if let Some(clip) = clip {
                            if clips.send(clip).is_err() {
                                return Ok(());
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Clipboard owner (writer side)
// ---------------------------------------------------------------------------------------------

struct Transfer {
    target: Atom,
    idx: usize,
    offset: usize,
}

struct Server {
    conn: RustConnection,
    at: Atoms,
    formats: Vec<Format>,
    /// target atom -> index into `formats`
    offers: Vec<(Atom, usize)>,
    time: u32,
    chunk: usize,
    incr: HashMap<(Window, Atom), Transfer>,
}

impl Server {
    fn request(&mut self, e: x11rb::protocol::xproto::SelectionRequestEvent) -> Result<()> {
        let prop = if e.property == NONE { e.target } else { e.property };
        let c = &self.conn;
        let ok = if e.target == self.at.targets {
            let mut list = vec![self.at.targets, self.at.timestamp];
            list.extend(self.offers.iter().map(|(a, _)| *a));
            c.change_property32(PropMode::REPLACE, e.requestor, prop, AtomEnum::ATOM, &list)?;
            true
        } else if e.target == self.at.timestamp {
            c.change_property32(PropMode::REPLACE, e.requestor, prop, AtomEnum::INTEGER, &[self.time])?;
            true
        } else if let Some(&(_, idx)) = self.offers.iter().find(|(a, _)| *a == e.target) {
            let data = &self.formats[idx].data;
            if data.len() <= self.chunk {
                c.change_property(PropMode::REPLACE, e.requestor, prop, e.target, 8, data.len() as u32, data)?;
            } else {
                c.change_window_attributes(
                    e.requestor,
                    &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
                )?;
                c.change_property32(PropMode::REPLACE, e.requestor, prop, self.at.incr, &[data.len() as u32])?;
                self.incr.insert((e.requestor, prop), Transfer { target: e.target, idx, offset: 0 });
            }
            true
        } else {
            false
        };
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: e.time,
            requestor: e.requestor,
            selection: e.selection,
            target: e.target,
            property: if ok { prop } else { NONE },
        };
        c.send_event(false, e.requestor, EventMask::NO_EVENT, notify)?;
        c.flush()?;
        Ok(())
    }

    /// A requestor deleted one of our INCR properties: write the next chunk (or the terminator).
    fn continue_incr(&mut self, window: Window, prop: Atom) -> Result<()> {
        let key = (window, prop);
        let Some(t) = self.incr.get_mut(&key) else { return Ok(()) };
        let data = &self.formats[t.idx].data;
        if t.offset < data.len() {
            let end = (t.offset + self.chunk).min(data.len());
            self.conn
                .change_property(PropMode::REPLACE, window, prop, t.target, 8, (end - t.offset) as u32, &data[t.offset..end])?;
            t.offset = end;
        } else {
            self.conn.change_property(PropMode::REPLACE, window, prop, t.target, 8, 0, &[])?;
            self.incr.remove(&key);
        }
        self.conn.flush()?;
        Ok(())
    }
}

/// Removes the owner window from the shared set when the server thread ends.
struct OwnerGuard {
    owners: Owners,
    win: Window,
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        self.owners.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&self.win);
    }
}

fn serve(formats: Vec<Format>, owners: Owners, ready: &Sender<Result<()>>) -> Result<()> {
    let (conn, screen) = x11rb::connect(None).context("owner: cannot connect to X display")?;
    let root = conn.setup().roots[screen].root;
    let at = Atoms::new(&conn)?;
    let win = create_helper_window(&conn, root)?;
    owners.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(win);
    let _guard = OwnerGuard { owners, win };

    let mut offers: Vec<(Atom, usize)> = Vec::new();
    for (i, f) in formats.iter().enumerate() {
        match f.mime.as_str() {
            MIME_UTF8 => offers.push((at.utf8_string, i)),
            "text/plain" => offers.push((at.string, i)),
            _ => {}
        }
        offers.push((intern(&conn, &f.mime)?, i));
    }

    // Get a real server timestamp: a zero-length append generates a PropertyNotify with time.
    conn.change_property(PropMode::APPEND, win, at.clipsy_ts, AtomEnum::STRING, 8, 0, &[])?;
    conn.flush()?;
    let time = loop {
        match wait_event(&conn, Instant::now() + STEP_TIMEOUT)? {
            Some(Event::PropertyNotify(p)) if p.window == win && p.atom == at.clipsy_ts => break p.time,
            Some(_) => {}
            None => bail!("X server did not send a timestamp"),
        }
    };

    conn.set_selection_owner(win, at.clipboard, time)?;
    conn.flush()?;
    if conn.get_selection_owner(at.clipboard)?.reply()?.owner != win {
        bail!("could not take CLIPBOARD ownership");
    }
    let _ = ready.send(Ok(()));

    let chunk = conn.maximum_request_bytes().saturating_sub(1024).min(256 * 1024);
    let mut srv = Server { conn, at, formats, offers, time, chunk, incr: HashMap::new() };
    loop {
        match srv.conn.wait_for_event()? {
            Event::SelectionClear(e) if e.selection == srv.at.clipboard => return Ok(()),
            Event::SelectionRequest(e) => srv.request(e)?,
            Event::PropertyNotify(p) if p.state == Property::DELETE => srv.continue_incr(p.window, p.atom)?,
            _ => {}
        }
    }
}

fn owner_main(formats: Vec<Format>, owners: Owners, ready: Sender<Result<()>>) {
    if let Err(e) = serve(formats, owners, &ready) {
        // If ownership was already announced this send fails silently; the error is only logged.
        tracing::warn!("x11 clipboard owner: {e:#}");
        let _ = ready.send(Err(e));
    }
}

// ---------------------------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Binding {
    action: HotkeyAction,
    keycode: u8,
    /// Modifiers without lock bits.
    mods: u16,
}

pub struct X11Backend {
    /// Requests for paste / remember / XTest.
    ctl: Arc<RustConnection>,
    ctl_atoms: Atoms,
    /// Connection that owns the hotkey grabs; its events are read by the hotkey thread.
    hk: Arc<RustConnection>,
    root: Window,
    bindings: Arc<Mutex<Vec<Binding>>>,
    active: Mutex<Option<Window>>,
    /// Extra terminal classes from the config.
    terminal_apps: Mutex<Vec<String>>,
    owners: Owners,
    started: bool,
}

impl X11Backend {
    pub fn new() -> Result<Self> {
        let (ctl, screen) = x11rb::connect(None).context("cannot connect to X display")?;
        let root = ctl.setup().roots[screen].root;
        let ctl_atoms = Atoms::new(&ctl)?;
        let (hk, _) = x11rb::connect(None).context("cannot open hotkey connection")?;
        Ok(X11Backend {
            ctl: Arc::new(ctl),
            ctl_atoms,
            hk: Arc::new(hk),
            root,
            bindings: Arc::new(Mutex::new(Vec::new())),
            active: Mutex::new(None),
            terminal_apps: Mutex::new(Vec::new()),
            owners: Arc::new(Mutex::new(HashSet::new())),
            started: false,
        })
    }

    fn grab_combos(&self, binding: &Binding) -> Result<(), ReplyError> {
        for lock in LOCK_COMBOS {
            self.hk
                .grab_key(
                    false,
                    self.root,
                    ModMask::from(binding.mods | lock),
                    binding.keycode,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                )
                .map_err(ReplyError::from)?
                .check()?;
        }
        Ok(())
    }

    fn ungrab_combos(&self, binding: &Binding) {
        for lock in LOCK_COMBOS {
            let _ = self
                .hk
                .ungrab_key(binding.keycode, self.root, ModMask::from(binding.mods | lock));
        }
        let _ = self.hk.flush();
    }

    fn send_key(&self, press: bool, keycode: u8) -> Result<()> {
        let ty = if press { KEY_PRESS_EVENT } else { KEY_RELEASE_EVENT };
        self.ctl.xtest_fake_input(ty, keycode, 0, NONE, 0, 0, 0)?;
        self.ctl.flush()?;
        thread::sleep(KEY_GAP);
        Ok(())
    }

    /// The window the window manager reports as active right now.
    fn current_active_window(&self) -> Option<Window> {
        let reply = self
            .ctl
            .get_property(false, self.root, self.ctl_atoms.net_active_window, AtomEnum::WINDOW, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        reply.value32().and_then(|mut v| v.next()).filter(|&w| w != NONE)
    }

    /// Asks the window manager to activate `w` and waits until it is active.
    /// Returns false when it did not become active in time.
    fn activate_and_wait(&self, w: Window) -> Result<bool> {
        if self.current_active_window() != Some(w) {
            // Source 2 ("pager"): the user asked for this, so the window manager's
            // focus-stealing prevention must not turn it into a "demands attention" hint.
            let msg = ClientMessageEvent::new(32, w, self.ctl_atoms.net_active_window, [2u32, 0, 0, 0, 0]);
            self.ctl.send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                msg,
            )?;
            self.ctl.flush()?;
        }
        let active = wait_until(FOCUS_TIMEOUT, POLL_EVERY, || self.current_active_window() == Some(w));
        thread::sleep(FOCUS_SETTLE);
        Ok(active)
    }

    /// Modifier keys the user is still physically holding (Shift, Ctrl, Alt, Super...).
    fn held_modifiers(&self) -> u16 {
        self.ctl
            .query_pointer(self.root)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .map_or(0, |r| u16::from(r.mask) & M_MATCH)
    }
}

fn hotkey_loop(conn: Arc<RustConnection>, bindings: Arc<Mutex<Vec<Binding>>>, tx: Sender<HotkeyAction>) -> Result<()> {
    loop {
        if let Event::KeyPress(e) = conn.wait_for_event()? {
            let state = u16::from(e.state) & M_MATCH;
            let action = bindings
                .lock().unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .find(|b| b.keycode == e.detail && b.mods == state)
                .map(|b| b.action);
            if let Some(action) = action {
                if tx.send(action).is_err() {
                    return Ok(());
                }
            }
        }
    }
}

impl Backend for X11Backend {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn start(&mut self, clips: Sender<ClipEvent>, hotkeys: Sender<HotkeyAction>) -> Result<()> {
        if self.started {
            bail!("X11 backend already started");
        }
        self.started = true;

        let owners = self.owners.clone();
        thread::Builder::new()
            .name("clipsy-x11-watch".into())
            .spawn(move || {
                if let Err(e) = watch(clips, owners) {
                    tracing::error!("x11 clipboard watcher stopped: {e:#}");
                }
            })
            .context("cannot spawn clipboard watcher")?;

        let hk = self.hk.clone();
        let bindings = self.bindings.clone();
        thread::Builder::new()
            .name("clipsy-x11-hotkeys".into())
            .spawn(move || {
                if let Err(e) = hotkey_loop(hk, bindings, hotkeys) {
                    tracing::error!("x11 hotkey thread stopped: {e:#}");
                }
            })
            .context("cannot spawn hotkey thread")?;
        Ok(())
    }

    fn set_clipboard(&self, formats: &[Format]) -> Result<()> {
        let (tx, rx) = channel();
        let owners = self.owners.clone();
        let formats = formats.to_vec();
        thread::Builder::new()
            .name("clipsy-x11-owner".into())
            .spawn(move || owner_main(formats, owners, tx))
            .context("cannot spawn clipboard owner thread")?;
        rx.recv().context("clipboard owner thread exited")?
    }

    fn remember_active_window(&self) {
        let found = self
            .ctl
            .get_property(false, self.root, self.ctl_atoms.net_active_window, AtomEnum::WINDOW, 0, 1)
            .map_err(anyhow::Error::from)
            .and_then(|cookie| cookie.reply().map_err(anyhow::Error::from));
        match found {
            Ok(reply) => {
                let win = reply.value32().and_then(|mut v| v.next()).filter(|&w| w != NONE);
                *self.active.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = win;
            }
            Err(e) => tracing::warn!("x11: cannot read _NET_ACTIVE_WINDOW: {e}"),
        }
    }

    fn paste(&self, mode: PasteMode) -> Result<()> {
        let win = *self.active.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let class = win.and_then(|w| source_app(&self.ctl, self.root, w));
        let is_term = mode == PasteMode::Terminal
            || class.as_deref().is_some_and(|c| {
                is_terminal(c, &self.terminal_apps.lock().unwrap_or_else(std::sync::PoisonError::into_inner))
            });

        // Keys are only sent once the remembered window is really active again: before
        // that they would go to the popup (still unmapping) or to whatever window the
        // window manager focused in between.
        let focused = match win {
            Some(w) => self.activate_and_wait(w)?,
            None => {
                tracing::warn!("x11 paste: no remembered window, sending keys to the focused one");
                true
            }
        };
        if !focused {
            tracing::warn!("x11 paste: window {:?} did not become active in time, pasting anyway", win);
        }
        // A key the user still holds (Shift from Shift+Enter, Ctrl from Ctrl+1) would be
        // added to our chord, so wait briefly for it to be released.
        let held_before = self.held_modifiers();
        if held_before != 0 {
            wait_until(MODIFIER_TIMEOUT, POLL_EVERY, || self.held_modifiers() == 0);
        }

        let (ctrl, _) = keycode_for_keysym(&self.ctl, 0xffe3)?; // Control_L
        let (shift, _) = keycode_for_keysym(&self.ctl, 0xffe1)?; // Shift_L
        let v = match keycode_for_keysym(&self.ctl, 'v' as u32) {
            Ok((v, _)) => v,
            Err(_) => KEYCODE_V,
        };

        tracing::info!(
            "x11 paste: window={} class={:?} terminal={is_term} focused={focused} held_mods=0x{held_before:x} keys={}",
            win.map_or_else(|| "none".to_string(), |w| format!("0x{w:x}")),
            class.as_deref().unwrap_or("?"),
            if is_term { "Ctrl+Shift+V" } else { "Ctrl+V" },
        );

        if is_term {
            self.send_key(true, shift)?;
        }
        self.send_key(true, ctrl)?;
        self.send_key(true, v)?;
        self.send_key(false, v)?;
        self.send_key(false, ctrl)?;
        if is_term {
            self.send_key(false, shift)?;
        }
        Ok(())
    }

    fn supports_paste(&self) -> bool {
        true
    }

    fn set_terminal_apps(&self, apps: &[String]) {
        *self.terminal_apps.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = apps.to_vec();
    }

    fn register_hotkey(&mut self, action: HotkeyAction, accel: &str) -> Result<()> {
        let (mods, keysym) = parse_accel(accel)?;
        let (keycode, extra) = keycode_for_keysym(&self.hk, keysym)
            .with_context(|| format!("accelerator {accel:?}"))?;
        let binding = Binding { action, keycode, mods: (mods | extra) & M_MATCH };

        // Re-registering an action replaces its old grab.
        self.unregister_hotkey(action);
        if self
            .bindings
            .lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|b| b.keycode == binding.keycode && b.mods == binding.mods)
        {
            bail!("accelerator {accel:?} is already bound to another action");
        }

        if let Err(e) = self.grab_combos(&binding) {
            // Undo partial grabs (the failing combo was never granted, the others were).
            self.ungrab_combos(&binding);
            return Err(match e {
                ReplyError::X11Error(ref x) if x.error_kind == ErrorKind::Access => {
                    anyhow!("accelerator {accel:?} is already in use by another application")
                }
                other => anyhow!("cannot grab accelerator {accel:?}: {other}"),
            });
        }
        self.bindings.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(binding);
        Ok(())
    }

    fn unregister_hotkey(&mut self, action: HotkeyAction) {
        let mut bindings = self.bindings.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let (removed, kept): (Vec<Binding>, Vec<Binding>) =
            bindings.drain(..).partition(|b| b.action == action);
        *bindings = kept;
        drop(bindings);
        for b in &removed {
            self.ungrab_combos(b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_V: u32 = 'v' as u32;
    const KEY_C: u32 = 'c' as u32;
    const KEY_RETURN: u32 = 0xff0d;
    const KEY_F5: u32 = 0xffc2;

    #[test]
    fn parses_single_modifier() {
        assert_eq!(parse_accel("<Super>v").unwrap(), (M_MOD4, KEY_V));
    }

    #[test]
    fn parses_two_modifiers() {
        assert_eq!(parse_accel("<Ctrl><Alt>c").unwrap(), (M_CONTROL | M_MOD1, KEY_C));
    }

    #[test]
    fn parses_named_key() {
        assert_eq!(parse_accel("<Shift>Return").unwrap(), (M_SHIFT, KEY_RETURN));
    }

    #[test]
    fn parses_modifier_aliases_case_insensitively() {
        assert_eq!(parse_accel("<primary>c").unwrap(), (M_CONTROL, KEY_C));
        assert_eq!(parse_accel("<MOD4>v").unwrap(), (M_MOD4, KEY_V));
    }

    #[test]
    fn parses_function_key_without_modifiers() {
        assert_eq!(parse_accel("F5").unwrap(), (0, KEY_F5));
    }

    #[test]
    fn uppercase_letter_means_shift() {
        assert_eq!(parse_accel("V").unwrap(), (M_SHIFT, KEY_V));
    }

    #[test]
    fn ignores_surrounding_spaces() {
        assert_eq!(parse_accel("  <Super>v ").unwrap(), (M_MOD4, KEY_V));
    }

    #[test]
    fn rejects_unknown_modifier() {
        assert!(parse_accel("<Banana>v").is_err());
    }

    #[test]
    fn rejects_unclosed_modifier() {
        assert!(parse_accel("<Super").is_err());
    }

    #[test]
    fn rejects_missing_key() {
        assert!(parse_accel("<Super>").is_err());
        assert!(parse_accel("<Ctrl><Alt>").is_err());
    }

    #[test]
    fn rejects_empty_and_unknown_keys() {
        assert!(parse_accel("").is_err());
        assert!(parse_accel("NoSuchKey").is_err());
        assert!(parse_accel("<Super>vv").is_err());
        assert!(parse_accel("<Super>F13").is_err());
    }

    #[test]
    fn find_keycode_prefers_first_layout_and_reports_shift() {
        // 2 keysyms per key, keys 8..=10: [a A] [v V] [b B]
        let map = [0x61, 0x41, 0x76, 0x56, 0x62, 0x42];
        assert_eq!(find_keycode(&map, 2, 8, 0x76), Some((9, 0)));
        assert_eq!(find_keycode(&map, 2, 8, 0x56), Some((9, M_SHIFT)));
        assert_eq!(find_keycode(&map, 2, 8, 0x7a), None);
    }

    #[test]
    fn find_keycode_falls_back_to_other_layouts() {
        // "ru,us": 4 keysyms per key, Cyrillic first; v only exists in the second layout.
        const CYRILLIC_EM: u32 = 0x6cd;
        const CYRILLIC_EM_CAP: u32 = 0x6ed;
        let map = [0x6c6, 0x6e6, 0x61, 0x41, CYRILLIC_EM, CYRILLIC_EM_CAP, 0x76, 0x56];
        assert_eq!(find_keycode(&map, 4, 54, 0x76), Some((55, 0)));
        assert_eq!(find_keycode(&map, 4, 54, 0x56), Some((55, M_SHIFT)));
    }

    #[test]
    fn find_keycode_does_not_overflow_keycodes() {
        let map = vec![0u32; 300];
        assert_eq!(find_keycode(&map, 1, 8, 0x76), None);
        let mut map = vec![0u32; 300];
        map[299] = 0x76;
        assert_eq!(find_keycode(&map, 1, 8, 0x76), None);
    }

    #[test]
    fn wait_until_returns_as_soon_as_done() {
        let mut calls = 0;
        let start = Instant::now();
        assert!(wait_until(Duration::from_secs(5), Duration::from_millis(1), || {
            calls += 1;
            calls == 3
        }));
        assert_eq!(calls, 3);
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn wait_until_gives_up_after_timeout() {
        let start = Instant::now();
        assert!(!wait_until(Duration::from_millis(30), Duration::from_millis(5), || false));
        assert!(start.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn is_terminal_matches_warp() {
        assert!(is_terminal("dev.warp.Warp", &[]));
    }

    #[test]
    fn is_terminal_matches_common_terminals() {
        for class in [
            "gnome-terminal-server",
            "Gnome-terminal",
            "org.gnome.Terminal",
            "kitty",
            "Alacritty",
            "org.gnome.Console",
            "kgx",
            "org.gnome.Ptyxis",
            "tilix",
            "com.mitchellh.ghostty",
            "org.wezfurlong.wezterm",
            "konsole",
            "XTerm",
            "URxvt",
            "Xfce4-terminal",
            "Terminator",
            "terminology",
            "cool-retro-term",
        ] {
            assert!(is_terminal(class, &[]), "expected {class:?} to be a terminal");
        }
    }

    #[test]
    fn is_terminal_matches_exact_names_only() {
        for class in ["st", "st-256color", "foot", "footclient", "tabby", "Hyper", "contour", "rio", "Warp",
            "com.raggesilver.BlackBox", "Tilda"]
        {
            assert!(is_terminal(class, &[]), "expected {class:?} to be a terminal");
        }
    }

    #[test]
    fn is_terminal_uses_user_list_exactly() {
        let extra = vec!["Code".to_string(), " my-term ".to_string()];
        assert!(is_terminal("code", &extra));
        assert!(is_terminal("Code", &extra));
        assert!(is_terminal("MY-TERM", &extra));
        assert!(!is_terminal("code-insiders", &extra));
        assert!(!is_terminal("Code", &[]));
    }

    #[test]
    fn is_terminal_rejects_normal_apps() {
        for class in [
            "Google-chrome",
            "firefox",
            "code",
            "Code",
            "Gedit",
            "org.gnome.Nautilus",
            "stylus",
            "distro",
            "Steam",
            "Warpinator",
            "Hyperion",
            "footage",
            "jconsole",
            "org.gnome.TextEditor",
            "",
        ] {
            assert!(!is_terminal(class, &[]), "expected {class:?} not to be a terminal");
        }
    }

    /// The GNOME extension (Wayland) must detect the same terminals as this backend.
    #[test]
    fn gnome_extension_uses_the_same_terminal_lists() {
        let js = include_str!("../../../gnome-extension/clipsy@venkyeswar.github.io/extension.js");
        let list = |name: &str| -> Vec<String> {
            let start = js.find(&format!("const {name} = [")).unwrap_or_else(|| panic!("{name} missing"));
            let body = &js[start..];
            let body = &body[body.find('[').unwrap() + 1..body.find(']').unwrap()];
            body.split(',')
                .map(|s| s.split("//").next().unwrap().trim().trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        assert_eq!(list("TERMINAL_SUBSTRINGS"), TERMINAL_SUBSTRINGS);
        assert_eq!(list("TERMINAL_EXACT"), TERMINAL_EXACT);
    }
}
