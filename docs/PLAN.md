# Clipsy — Implementation Plan

> A fast, small, beautiful clipboard history manager for Linux (Windows `Win+V`-style).
> Working name "Clipsy" — rename freely (binary, app ID, folders).
>
> **For the implementing model:** read this whole file first. Decisions here are final unless the
> user changes them. Build phase by phase; each phase ends with acceptance criteria the user tests
> manually. Keep the user (a beginner) informed in plain language about what each piece does.

---

## 1. Goals

1. **Reliable history** — text, images, files, rich text (HTML). Nothing silently lost. Survives reboot.
2. **Great UI** — native-looking, keyboard-first, search, previews, pins. Light + dark + follow-system.
3. **Configurable** — hotkeys, theme, history limits, privacy rules — all from a Settings window.
4. **Local & private** — everything lives in the user's home directory. No root, no network, no cloud, no telemetry.
5. **Small** — single binary, target < 6 MB stripped, idle RAM < 50 MB, idle CPU ≈ 0%.
6. **Works across Linux** — X11 first (user's machine), then GNOME Wayland, KDE, wlroots.

### Non-goals (v1)
Cloud/device sync, macOS, plugin system, OCR. Windows is no longer a non-goal: it is built as Phase W (see section 11).

## 2. Target environment

- Dev machine: **Ubuntu 24.04.5, GNOME Shell 46, X11 session**.
- System libs on 24.04: GTK 4.14, libadwaita 1.5 (do not use newer APIs than these).
- Later: Ubuntu on Wayland (GNOME 46+), KDE Plasma 6, Sway/Hyprland.
- Windows (Phase W): Windows 10 and 11, 64-bit. Native Slint UI in `crates/clipsy-windows`, not GTK. The Ubuntu dev machine cannot run it; the user tests it on Windows.

## 3. Tech decisions (final)

| Area | Choice |
|---|---|
| Language | Rust (stable, edition 2021) |
| UI | `gtk4` + `libadwaita` (`gtk4-rs`, features `v4_14`, `adw` `v1_5`) |
| App model | One process = `adw::Application` (app id `io.github.clipsy.Clipsy`), stays running with hidden window. Popup shows instantly. Single-instance via GApplication D-Bus. |
| Clipboard I/O (X11) | GDK clipboard (`gdk::Display::clipboard()`) for read + write (handles INCR/large images, multi-format). Verify early (Phase 1 spike) that `changed` fires while unfocused on X11; fallback = `x11rb` with XFixes for change notification. |
| Global hotkey (X11) | `x11rb` `XGrabKey` on root window, in a background thread, events sent to GTK main loop via `glib::MainContext` channel. |
| Auto-paste (X11) | `x11rb` XTest: remember previously active window (`_NET_ACTIVE_WINDOW`) before showing popup, hide popup, re-activate it, send Ctrl+V (Ctrl+Shift+V for terminals — configurable). |
| Wayland GNOME (later) | GNOME Shell extension (ESM, GNOME 45+ format) talks to app over D-Bus. |
| Wayland KDE/wlroots (later) | `wayland-client` + `ext-data-control-v1` / `wlr-data-control` protocols. |
| IPC | `zbus` (D-Bus) — interface `io.github.clipsy.Clipsy1` |
| Storage | SQLite via `rusqlite` (feature `bundled`) + blob files |
| Hashing | `blake3` (dedupe + blob filenames) |
| Images | `image` crate, only features `png`, `jpeg` (keep size small); thumbnails 256 px PNG |
| Config | TOML (`serde`, `toml`), file watched & hot-reloaded |
| Paths | `directories` crate (XDG) |
| Logging | `tracing` + `tracing-subscriber` (env filter) |
| Errors | `anyhow` (app), `thiserror` (libs) |

**Release profile (size):**
```toml
[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

## 4. Architecture

```
┌──────────────────────── clipsy (single binary) ───────────────────────┐
│                                                                       │
│  UI (GTK4/libadwaita)            Settings window      Welcome dialog  │
│   popup window  ◄──── AppState (Rc<RefCell>) ────►  config (TOML)     │
│        │                     │                                        │
│        ▼                     ▼                                        │
│   Core: History service  ──► Store (SQLite + blobs)                   │
│        ▲                                                              │
│        │ ClipEvent                                                    │
│   Backend trait  ─┬─ X11Backend (GDK clipboard + x11rb hotkey/XTest)  │
│                   ├─ GnomeWaylandBackend (via extension over D-Bus)   │
│                   └─ DataControlBackend (KDE / wlroots)               │
│                                                                       │
│   D-Bus server (zbus): Toggle, Show, Paste(id), Pause, ...            │
└───────────────────────────────────────────────────────────────────────┘
          ▲                                   ▲
   `clipsy toggle` (CLI)            GNOME Shell extension (Wayland)
```

### Backend trait (sketch)
```rust
pub trait Backend {
    fn name(&self) -> &'static str;
    /// Start watching; send each new clipboard content to `tx`.
    fn start(&mut self, tx: glib::Sender<ClipEvent>) -> anyhow::Result<()>;
    /// Put an item back on the clipboard with all its formats.
    fn set_clipboard(&self, item: &ClipItem) -> anyhow::Result<()>;
    /// Paste into the previously focused app (optional capability).
    fn paste(&self, mode: PasteMode) -> anyhow::Result<()>;
    fn supports_paste(&self) -> bool;
    fn register_hotkey(&mut self, action: HotkeyAction, accel: &str) -> anyhow::Result<()>;
}
```
Backend auto-selected at startup from `XDG_SESSION_TYPE`, `XDG_CURRENT_DESKTOP`, and protocol availability.

## 5. Project layout

```
clipsy/
├── Cargo.toml                 # workspace
├── crates/
│   ├── clipsy-core/           # models, store, history service, config (no GTK) — unit tested
│   ├── clipsy-backends/       # Backend trait + x11 (+ later wayland impls)
│   └── clipsy-app/            # binary: GTK UI, D-Bus server, CLI, wiring
│       ├── src/
│       ├── resources/         # .ui/.blp files, style.css, icons (gresource)
├── gnome-extension/           # Phase 4
├── data/
│   ├── io.github.clipsy.Clipsy.desktop
│   ├── io.github.clipsy.Clipsy.metainfo.xml
│   └── icons/
├── packaging/                 # deb, AppImage, flatpak (later)
├── docs/PLAN.md               # this file
└── CLAUDE.md                  # short orientation for coding agents
```

## 6. Data model

### Files (all under user's home, dirs created with mode 0700)
- Config: `~/.config/clipsy/config.toml`
- DB: `~/.local/share/clipsy/history.db`
- Blobs: `~/.local/share/clipsy/blobs/<blake3>.bin` (raw payloads > 64 KB, and all images)
- Thumbnails: `~/.local/share/clipsy/thumbs/<blake3>.png`
- Autostart: `~/.config/autostart/io.github.clipsy.Clipsy.desktop` (written/removed by setting)

### SQLite schema
```sql
CREATE TABLE items (
  id          INTEGER PRIMARY KEY,
  kind        TEXT NOT NULL,          -- 'text' | 'image' | 'files' | 'html' | 'other'
  hash        TEXT NOT NULL UNIQUE,   -- blake3 of canonical content (dedupe)
  preview     TEXT,                   -- first ~300 chars / file names / "Image 1920×1080"
  search_text TEXT,                   -- lowercased text for search
  source_app  TEXT,                   -- WM_CLASS / app id when known
  byte_size   INTEGER NOT NULL,
  width       INTEGER, height INTEGER,-- images
  pinned      INTEGER NOT NULL DEFAULT 0,
  created_at  INTEGER NOT NULL,       -- unix ms
  last_used_at INTEGER NOT NULL
);
CREATE TABLE formats (                -- every MIME type captured for an item
  item_id   INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  mime      TEXT NOT NULL,            -- text/plain;charset=utf-8, text/html, image/png, text/uri-list, x-special/gnome-copied-files ...
  inline    BLOB,                     -- small payloads
  blob_path TEXT,                     -- large payloads
  PRIMARY KEY (item_id, mime)
);
CREATE INDEX idx_items_order ON items(pinned DESC, last_used_at DESC);
PRAGMA journal_mode = WAL;
PRAGMA user_version = 1;              -- migrations keyed on this
```

### Rules
- Re-copying an existing hash → bump `last_used_at` (moves to top), no duplicate.
- Pruning after each insert: keep pinned always; drop oldest unpinned beyond `max_items` or `max_total_mb`; delete orphaned blobs/thumbs.
- Capture **all** offered formats up to `max_item_mb` (default 20 MB); skip formats over the limit except the best image + plain text.
- Image preference order: `image/png` > `image/jpeg` > others (store PNG if conversion needed).
- Sensitive skip: if offered targets include `x-kde-passwordManagerHint` (value `secret`) or source app in `ignored_apps` → don't store.
- Ignore whitespace-only text. Ignore our own writes (flag/hash of last set).

## 7. Config (`config.toml`, with defaults)

```toml
[general]
autostart        = true
max_items        = 500
max_total_mb     = 500
max_item_mb      = 20
save_images      = true
save_files       = true
save_rich_text   = true
watch_primary    = false        # mouse-selection clipboard
paste_on_select  = true         # auto-paste after choosing an item

[appearance]
theme            = "system"     # "system" | "light" | "dark"
popup_position   = "cursor"     # "cursor" | "center"
compact          = false
show_source_app  = true

[shortcuts]
toggle_popup     = "<Super>v"
clear_history    = ""           # empty = unassigned
pause_recording  = ""
paste_plain      = "<Shift>Return"   # inside popup

[privacy]
paused           = false
skip_sensitive   = true
ignored_apps     = ["KeePassXC", "1Password", "Bitwarden"]
clear_on_exit    = false
```
Config loaded at start, written by Settings window, invalid values fall back to defaults with a log warning.

## 8. UI / UX specification

Use libadwaita widgets and named colors only (`@accent_color`, `@window_bg_color`, `@card_bg_color`, etc.) so light and dark both look right automatically. Theme setting maps to `adw::StyleManager::set_color_scheme` (`Default` / `ForceLight` / `ForceDark`). Minimal custom CSS. Rounded corners, 12 px spacing grid. All strings via `gettext` macros (i18n-ready).

### 8.1 Popup (main window)
- Size ~440 × 560 (minimum width 380), undecorated, appears at cursor (clamped to monitor) or center; closes on Esc or focus loss.
- Layout:
  ```
  ┌──────────────────────────────────────────┐
  │ 🔍 Search clipboard…            [⚙]      │  search entry focused on open
  │ [All] [Text] [Images] [Files] [★ Pinned] [🗑]│  filter chips (toggle buttons); 🗑 = clear current tab
  ├──────────────────────────────────────────┤
  │ ★  "Meeting notes for Thursday…"          │  pinned first
  │    Text · Firefox · 2 min ago        [⋯]  │
  │ ▣  [thumbnail]  Image 1920×1080           │
  │    Screenshot · 5 min ago            [⋯]  │
  │ 📄 report.pdf, notes.txt (2 files)        │
  │    Files · Nautilus · 1 h ago        [⋯]  │
  │ …                                         │
  ├──────────────────────────────────────────┤
  │ ↵ Paste  ⇧↵ Plain text  Ctrl+P Pin  Del   │  hint bar (hideable)
  └──────────────────────────────────────────┘
  ```
- List: `gtk::ListView` + `gio::ListStore` with lazy thumbnail loading (fast with 500+ items).
- Row `[⋯]` menu: Paste, Paste as plain text, Pin/Unpin, Copy only (no paste), Delete.
- Clear button (🗑, right end of the chip row): clears only the current tab's items after a confirm dialog with an "Also remove pinned items" checkbox (unchecked by default). The checkbox is hidden on the ★ tab, where the clear removes pinned items.
- Image rows: hover/Space shows a larger preview popover.
- Empty state: `adw::StatusPage` "Nothing copied yet — copy something and it will appear here". Paused state: banner "Recording paused [Resume]".
- Keyboard: ↑/↓ move, Enter paste, Shift+Enter plain-text paste, Ctrl+P pin, Delete remove, Ctrl+1…9 quick paste, Tab cycles filter chips, typing goes to search, Esc closes.
- Search: case-insensitive substring over `search_text` + file names, debounced 80 ms.

### 8.2 Settings window (`adw::PreferencesWindow`)
- **General:** launch at login, paste after selecting, history size (spin), max total storage, max item size, save images / files / rich text, watch mouse selection.
- **Appearance:** theme (System / Light / Dark — live preview), popup position, compact rows, show source app.
- **Shortcuts:** rows for Open clipboard, Clear history, Pause recording. Clicking a row opens a **shortcut recorder dialog** ("Press new shortcut… Esc to cancel, Backspace to clear"), detects conflicts (grab failure → inline error toast "Shortcut already used by another app").
- **Privacy:** pause recording, skip passwords, ignored apps list (add/remove, with "pick from running windows" helper), clear history on exit, **Clear history** (destructive, same confirm dialog with the "Also remove pinned items" checkbox).
- **About:** `adw::AboutWindow` (version, license, links).
- Opened from popup gear button, `clipsy settings`, or app launcher entry.

### 8.3 First-run welcome
`adw::Window` (480 × 600) with a 4-page carousel: (1) welcome — what Clipsy does, (2) how to open it — your shortcut, and **GNOME binds Super+V to the notification list by default**: offer button "Use Super+V for Clipsy" which rebinds GNOME's `org.gnome.shell.keybindings toggle-message-tray` to only `<Super>m` (via `gio::Settings`), or let user pick another shortcut, (3) private by default — history stays on this machine, sensitive items skipped, (4) make it yours — launch at login toggle and paste after selecting. Done.

### 8.4 Notifications / tray
No tray icon on stock GNOME. Use `adw::Toast` inside windows and `gio::Notification` sparingly (e.g. "Paused"). Panel indicator comes with the GNOME extension (Phase 4).

## 9. CLI

```
clipsy                 # start (or focus) app in background
clipsy toggle          # show/hide popup (used by DE custom shortcuts / Wayland)
clipsy settings        # open settings
clipsy pause|resume
clipsy clear
clipsy --version
```
Implemented as GApplication actions / D-Bus calls to the running instance.

## 10. D-Bus interface `io.github.clipsy.Clipsy1` (object `/io/github/clipsy/Clipsy`)
Methods: `Toggle()`, `ShowSettings()`, `SetPaused(b)`, `Clear()`, `PasteItem(x id)`.
For the GNOME extension (Phase 4): `PushClip(a{say} formats, s source_app)`, signal `SetClipboardRequested(x id)`, `PasteRequested()`.

## 11. Phases & tasks

Each phase: implement → `cargo clippy -- -D warnings` + `cargo test` clean → user runs manual checklist → commit.

### Phase 0 — Setup
- [ ] Check/install: `rustup` (stable), `build-essential`, `pkg-config`, `libgtk-4-dev`, `libadwaita-1-dev`, `libsqlite3-dev` (not needed with bundled but harmless), `libxcb1-dev`, `gettext`.
- [ ] Workspace + 3 crates, release profile, `.gitignore`, `git init`, `CLAUDE.md`.
- [ ] Hello-window: `adw::Application` opens empty popup window.
- [ ] **Spike:** log every clipboard change (mime types + size) to terminal using GDK clipboard while window is hidden/unfocused. If GDK doesn't notify when unfocused → switch watcher to x11rb XFixes.
- **Accept:** `cargo run` prints a line for every copy in Firefox, Terminal, Nautilus, and a screenshot (Shift+PrtSc → copy).

### Phase 1 — Capture & store
- [ ] `clipsy-core`: models, config load/save with defaults, Store (schema, migrations, insert w/ dedupe, prune, blobs, thumbnails), unit tests (temp dir).
- [ ] X11 backend: read all formats per change, classify kind, sensitive/ignored filter, self-write suppression.
- [ ] Wire backend → history service → store.
- **Accept:** copy text, HTML from browser, an image, files from Nautilus; quit, reboot; `sqlite3 history.db 'select kind,preview from items'` shows all.

### Phase 2 — Popup UI
- [ ] Popup window per §8.1 with ListView, filter chips, search, empty state, thumbnails, keyboard nav.
- [ ] Selecting → `set_clipboard` with all formats (no auto-paste yet).
- [ ] Global hotkey via XGrabKey from config; `clipsy toggle` CLI.
- [ ] Theme setting applied (temporarily via config file).
- **Accept:** Super+V (or chosen key) opens popup in < 150 ms; choose image → Ctrl+V into LibreOffice/GIMP/Chrome pastes image; files paste in Nautilus; HTML keeps formatting in LibreOffice Writer.

### Phase 3 — Settings, auto-paste, polish
- [ ] Settings window per §8.2 incl. shortcut recorder and live theme switch.
- [ ] Auto-paste (XTest) with terminal detection (WM_CLASS contains `terminal`, `kitty`, `alacritty`, `wezterm`, `konsole` → Ctrl+Shift+V).
- [ ] Pins, delete, clear (confirm), pause, row menu, Ctrl+1…9, plain-text paste.
- [ ] First-run welcome incl. Super+V conflict fix; autostart desktop file.
- [ ] App icon (simple SVG, symbolic + full color), `.desktop`, metainfo.
- **Accept:** full manual checklist §12 passes on X11.

### Phase 4 — Wayland
- [ ] GNOME Shell extension (GNOME 46 ESM): watches `St.Clipboard`, forwards formats to app via D-Bus; sets clipboard on request; registers configurable keybinding; panel indicator (optional toggle); paste via `Clutter` virtual keyboard.
- [ ] `GnomeWaylandBackend` in app (D-Bus based).
- [ ] `DataControlBackend` for KDE / wlroots; hotkey via GlobalShortcuts portal (`ashpd`) or documented DE custom shortcut → `clipsy toggle`; paste via virtual-keyboard protocol where available, else "copy only" mode with toast "Press Ctrl+V to paste".
- **Accept:** checklist §12 on Ubuntu Wayland session; smoke test on KDE (VM ok).

### Phase 5 — Release
- [ ] `.deb` (cargo-deb), AppImage, extension zip for extensions.gnome.org; Flatpak later (needs sandbox holes; evaluate).
- [ ] README with screenshots (light + dark), GIF, install docs, LICENSE (GPL-3.0-or-later recommended, matches GNOME ecosystem).
- [ ] GitHub Actions: build, clippy, test, attach artifacts on tag.
- [ ] Verify size/perf budgets (§13).

### Phase W — Windows (`crates/clipsy-windows`)
Windows is developed in its own crate with a native Slint UI. Manual checks are in `docs/windows-acceptance.md`; the user runs them on Windows 10/11.

- **W1 Windows app (done, untested on Windows).** The code compiles and the app is wired together, but it has only been compile-checked, never run on Windows. Expect bugs; the acceptance checklist exists to find them. What it contains:
  - Clipboard listener: text, images and file lists are saved; password-manager clips are skipped.
  - Global hotkey, default **`Ctrl+Shift+V`** (Windows reserves `Win+V`, so Clipsy cannot use it). Set in the Settings window or `config.toml`.
  - Auto-paste: Ctrl+V, or Ctrl+Shift+V in terminal apps.
  - System tray icon with menu: Open, Pause/Resume capture, Settings, Clear history, Quit.
  - Autostart at login via the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` registry key.
  - Single instance: a second start does not add a second tray icon. CLI verbs (`toggle`, `settings`, `pause`, `resume`, `clear`, `quit`) go to the running copy.
  - Slint popup (search, pins, text/image/file rows, keyboard navigation, closes on focus loss) and Settings window, light/dark/system theme.
  - Data: `%LOCALAPPDATA%\clipsy\data`. Config: `%APPDATA%\clipsy\config\config.toml`.
  - Known limits: the app runs as a console program (a console window stays open, including at login with autostart on); logs go only to that console window.
- **W2 website** (not started; waits until the user confirms W1 on Windows): responsive layout and Windows download/install content.
- **W3 release automation** (not started): installer + portable `.exe` (zip), `SHA256SUMS` on GitHub Releases, website deploy on Cloudflare.

## 12. Manual test checklist
Apps: Firefox, Chrome, Terminal (gnome-terminal / ptyxis), VS Code, LibreOffice Writer, GIMP, Nautilus, Screenshot tool, Text Editor.
- [ ] Plain text, multi-line, emoji/Unicode, 5 MB text.
- [ ] Rich text browser → Writer keeps formatting; plain-text paste strips it.
- [ ] Image from screenshot, from browser "Copy image", from GIMP.
- [ ] Copy file(s) in Nautilus → paste in another folder works.
- [ ] Copy then close source app → item still pastes.
- [ ] Same text copied twice → one entry, moved to top.
- [ ] Password from KeePassXC not stored.
- [ ] Pins survive pruning and reboot.
- [ ] Theme: system/light/dark switch live; follows OS toggle when "system".
- [ ] Change hotkey; conflicting hotkey shows error.
- [ ] Pause → nothing recorded; resume works.
- [ ] 500 items incl. 100 images → popup still opens instantly, scrolling smooth.

## 13. Budgets
| Metric | Target |
|---|---|
| Stripped binary | < 6 MB |
| Idle RAM | < 50 MB |
| Idle CPU | ~0% (event-driven, no polling) |
| Popup open | < 150 ms |
| Capture latency | < 100 ms for text |

## 14. Risks & mitigations
| Risk | Mitigation |
|---|---|
| GDK clipboard doesn't notify when unfocused | Phase 0 spike; fallback x11rb XFixes |
| Super+V taken by GNOME | First-run rebind offer; any shortcut configurable |
| Large images block UI | Read/hash/thumbnail on `gio` background tasks; never block main loop |
| Wayland paste injection blocked | Extension (GNOME), virtual-keyboard (wlroots), else copy-only mode |
| Auto-paste to wrong window | Re-activate saved window, short delay (~50 ms) before XTest |
| DB corruption | WAL mode; on open error, back up and recreate |

## 15. Conventions for the implementing model
- Small commits per task; conventional commit messages.
- No `unwrap()` in non-test code paths that handle clipboard data; log and skip bad items.
- Keep GTK code out of `clipsy-core`.
- Explain to the user, in simple words, what was built and exactly what to run/test after each phase.
