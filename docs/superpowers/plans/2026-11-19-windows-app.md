# Clipsy Windows App (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A native Windows build of Clipsy (clipboard history, hotkey popup, auto-paste, tray, autostart) with a Slint UI, reusing `clipsy-core`, without breaking the Linux build.

**Architecture:** New crate `crates/clipsy-windows`. Pure logic (hotkey parsing, terminal detection, clipboard-format mapping, popup state) lives in modules compiled on every OS so it is unit-tested on the Ubuntu dev machine. Win32 code (`windows` crate) and Slint UI are `cfg(windows)`. A `WindowsBackend` implements the existing `clipsy_backends::Backend` trait. Linux-only code in `clipsy-backends` gets `cfg(target_os = "linux")`.

**Tech Stack:** Rust, `windows` crate (0.58), Slint 1.x, `tray-icon`, `winreg`, existing `clipsy-core` (rusqlite, image, directories).

**Spec:** `docs/superpowers/specs/2026-11-19-windows-website-release-design.md` (Phase 1). Phases 2 (website) and 3 (release) get their own plans after this one is accepted.

## Global Constraints

- Linux must keep passing: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` (CLAUDE.md requires clippy `-D warnings` and `cargo test` before a phase is called done).
- Windows default hotkey is `<Ctrl><Shift>v` (Win+V is reserved by Windows); accelerator syntax stays the existing GTK-style string (`<Ctrl><Shift>v`).
- Windows paths: config `%APPDATA%\clipsy\config.toml`, data `%LOCALAPPDATA%\clipsy` (via the `directories` crate already in clipsy-core).
- Local only: no network access, no telemetry.
- Auto-paste: Ctrl+V; Ctrl+Shift+V for terminal apps (built-in list plus `general.terminal_apps`, matched on exe name, case-insensitive exact after trimming).
- Password-manager clips are skipped: formats `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory` (DWORD 0) and `CanUploadToCloudClipboard` (DWORD 0) set `ClipEvent.sensitive = true`.
- Autostart via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value name `Clipsy`.
- Single instance via named mutex `Local\Clipsy-Instance`; CLI verbs forwarded over named pipe `\\.\pipe\clipsy-<username>`. Verbs: `toggle`, `settings`, `pause`, `resume`, `clear`, `quit`.
- The Windows app cannot be run on the Ubuntu dev machine. Local verification is unit tests plus `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu`; real behaviour is verified by the user on Windows 10/11 (Task 13 checklist). Explain commands to the user (beginner).
- Do not start Phase 2 until the user confirms the Task 13 acceptance checks.

## Review Focus

- Clipsy writing to the clipboard (when pasting an item) must not be re-captured as a new history item or reorder history. (Task 6)
- Clipboard held open by another app (`OpenClipboard` fails): retry a few times with short delay, never crash or busy-loop. (Task 6)
- Popup invoked while Clipsy itself is the foreground window, or the previous foreground window has closed: paste must not type into the popup or panic. (Task 7)
- Config file with an invalid or already-taken hotkey: app still starts, shows an error toast/tray notice, and other features work. (Task 7)
- Second launch while the first is running, including from a different Windows session or while the first is starting up: exits cleanly after forwarding or failing; never two tray icons. (Task 8)

---

### Task 1: Make core and backends cross-platform

**Files:**
- Modify: `crates/clipsy-backends/Cargo.toml` (move `x11rb`, `serde_json`, `base64` into `[target.'cfg(target_os = "linux")'.dependencies]`)
- Modify: `crates/clipsy-backends/src/lib.rs` (gate `pub mod gnome; pub mod x11;`, `detect()`, `WAYLAND_UNSUPPORTED`, `GNOME_EXTENSION_UUID` helpers with `#[cfg(target_os = "linux")]`; trait, `PasteMode`, `HotkeyAction`, `SetupNeeded`, `NoBackend` stay ungated; on non-Linux `detect()` returns `NoBackend`-style error "no built-in backend on this platform")
- Modify: `crates/clipsy-backends/examples/watch.rs` (gate with cfg linux)
- Modify: `crates/clipsy-core/src/config.rs` (`ensure_private_dir` sets mode 0700 only under `#[cfg(unix)]`; `Shortcuts::default().toggle_popup` is `"<Ctrl><Shift>v"` under `#[cfg(windows)]`, unchanged `"<Super><Shift>v"` otherwise)
- Test: existing tests in `crates/clipsy-core/src/config.rs`

**Interfaces:**
- Produces: `clipsy_backends::{Backend, PasteMode, HotkeyAction, SetupNeeded, NoBackend}` available on all targets; `clipsy_backends::detect()` only meaningful on Linux.

- [ ] **Step 1: Add test** in `config.rs` tests: `windows_default_toggle_is_ctrl_shift_v` annotated `#[cfg(windows)]` asserting `Config::default().shortcuts.toggle_popup == "<Ctrl><Shift>v"`; and keep `defaults_match_plan` asserting `"<Super><Shift>v"` under `#[cfg(not(windows))]`.
- [ ] **Step 2:** Apply the cfg gating described in Files.
- [ ] **Step 3:** Run `rustup target add x86_64-pc-windows-gnu` then `cargo check -p clipsy-core -p clipsy-backends --target x86_64-pc-windows-gnu`. Expected: compiles with no errors.
- [ ] **Step 4:** Run `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`. Expected: all pass (Linux behaviour unchanged).
- [ ] **Step 5: Commit** `git commit -m "refactor: make core and backends compile on Windows"`

### Task 2: Scaffold the `clipsy-windows` crate

**Files:**
- Create: `crates/clipsy-windows/Cargo.toml`, `crates/clipsy-windows/build.rs`, `crates/clipsy-windows/src/lib.rs`, `crates/clipsy-windows/src/main.rs`, `crates/clipsy-windows/ui/app.slint` (placeholder window)
- Modify: root `Cargo.toml` (add member `crates/clipsy-windows`)

**Interfaces:**
- Produces: library crate `clipsy_windows` with ungated modules `hotkey`, `terminals`, `formats`, `popup_state` (empty files now) and gated modules (`#[cfg(windows)]`) `clipboard`, `paste`, `backend`, `single_instance`, `autostart`, `tray`, `app`. Binary `clipsy` (name `clipsy`, `main` body under `#[cfg(windows)]`; on other OS prints "clipsy-windows only runs on Windows" and exits 1).

- [ ] **Step 1:** Cargo.toml: package `clipsy-windows`, deps `clipsy-core`, `clipsy-backends`, `anyhow`, `tracing`; `[target.'cfg(windows)'.dependencies]` `windows` (features: Win32_Foundation, Win32_System_DataExchange, Win32_System_Memory, Win32_System_Ole, Win32_System_Threading, Win32_System_LibraryLoader, Win32_System_Registry, Win32_System_Pipes, Win32_Storage_FileSystem, Win32_Security, Win32_UI_Input_KeyboardAndMouse, Win32_UI_WindowsAndMessaging, Win32_UI_Shell, Win32_Graphics_Gdi), `slint`, `tray-icon`, `winreg`; `[build-dependencies]` `slint-build`. `build.rs` compiles `ui/app.slint` only when `CARGO_CFG_TARGET_OS == "windows"`.
- [ ] **Step 2:** Run `sudo apt install gcc-mingw-w64-x86-64` (user runs it) then `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu` and `cargo build --workspace`. Expected: both succeed.
- [ ] **Step 3: Commit** `git commit -m "feat(windows): scaffold clipsy-windows crate"`

### Task 3: Hotkey accelerator parsing

**Files:**
- Create/fill: `crates/clipsy-windows/src/hotkey.rs`

**Interfaces:**
- Produces: `pub struct Accel { pub ctrl: bool, pub shift: bool, pub alt: bool, pub win: bool, pub vk: u32 }`; `pub fn parse_accel(s: &str) -> Result<Accel, String>` (accepts `<Ctrl>`, `<Control>`, `<Primary>`, `<Shift>`, `<Alt>`, `<Super>`, `<Win>`, case-insensitive; key = one letter/digit, `F1`..`F24`, `Return`, `space`, `Tab`, `Escape`; returns virtual-key codes, letters map to `'A'..='Z'` VK values); error text mirrors x11.rs style (`unknown modifier`, `unclosed modifier`, `missing key`, `unknown key`).

- [ ] **Step 1: Write failing tests** `parses_ctrl_shift_v` (`<Ctrl><Shift>v` -> ctrl, shift, vk 0x56), `parses_aliases_case_insensitively`, `parses_function_key`, `uppercase_letter_means_shift`, `rejects_unknown_modifier`, `rejects_unclosed_modifier`, `rejects_missing_key`, `rejects_empty_and_unknown_keys`, `ignores_surrounding_spaces`.
- [ ] **Step 2:** `cargo test -p clipsy-windows hotkey` Expected: FAIL (not defined).
- [ ] **Step 3: Implement** `parse_accel` as a small hand parser (model on the parser and tests in `crates/clipsy-backends/src/x11.rs`).
- [ ] **Step 4:** Run the tests again. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "feat(windows): hotkey accelerator parser"`

### Task 4: Terminal detection by executable name

**Files:**
- Create/fill: `crates/clipsy-windows/src/terminals.rs`

**Interfaces:**
- Produces: `pub fn is_terminal(exe: &str, extra: &[String]) -> bool` where `exe` is a file name such as `WindowsTerminal.exe` or a full path; compares the lowercase file stem. Built-in exact stems: `windowsterminal`, `cmd`, `powershell`, `pwsh`, `conhost`, `wezterm-gui`, `alacritty`, `mintty`, `putty`, `kitty`, `warp`, `tabby`, `hyper`; built-in substring: `terminal`. Extra entries match the stem exactly (trim, ASCII case-insensitive, with or without `.exe`).

- [ ] **Step 1: Write failing tests** `matches_windows_terminal`, `matches_full_path`, `matches_powershell_variants`, `rejects_normal_apps` (`notepad.exe`, `chrome.exe`, `code.exe`), `user_list_matches_exactly` (`Code` in extra matches `code.exe` but not `code-insiders.exe`), `user_list_ignores_exe_suffix_and_case`.
- [ ] **Step 2:** `cargo test -p clipsy-windows terminals` Expected: FAIL.
- [ ] **Step 3: Implement** `is_terminal` (modeled on `is_terminal` in `x11.rs:344`).
- [ ] **Step 4:** Run tests. Expected: PASS.
- [ ] **Step 5: Commit** `git commit -m "feat(windows): terminal detection by exe name"`

### Task 5: Clipboard format mapping (pure)

**Files:**
- Create/fill: `crates/clipsy-windows/src/formats.rs`
- Read first: `crates/clipsy-core/src/store.rs` (`describe`, `primary_index`, how `Kind::Files` and `Kind::Html` are detected from MIME types) and `crates/clipsy-core/src/model.rs`

**Interfaces:**
- Produces:
  - `pub struct RawClip { pub unicode_text: Option<Vec<u16>>, pub png: Option<Vec<u8>>, pub dib: Option<Vec<u8>>, pub html: Option<Vec<u8>>, pub files: Vec<String>, pub exclude_flag: bool, pub history_disallowed: bool, pub source_exe: Option<String> }`
  - `pub fn build_event(raw: RawClip) -> Option<clipsy_core::ClipEvent>` (returns `None` when nothing usable). Mapping: text -> `text/plain;charset=utf-8` (strip trailing NUL), PNG -> `image/png`, DIB only -> converted to `image/png` with the `image` crate (BMP header prepended), html (CF_HTML, extract fragment between StartFragment/EndFragment) -> `text/html`, files -> `text/uri-list` (file:// URIs, one per line, same encoding the store already stores for files; confirm in store.rs). `sensitive = exclude_flag || history_disallowed`. `source_app = source_exe`.
  - `pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>>`
  - `pub fn files_to_uri_list(paths: &[String]) -> String`
  - `pub fn clip_text_from_formats(formats: &[clipsy_core::Format]) -> Option<String>` (inverse, for paste of text)

- [ ] **Step 1: Write failing tests**: `text_maps_to_plain_mime_and_strips_nul`, `png_passes_through`, `dib_converts_to_png` (generate a tiny 2x2 24-bit DIB in the test, assert PNG magic bytes and decoded size 2x2), `html_fragment_extracted`, `files_become_uri_list_with_percent_encoding` (path with space), `sensitive_flags_set_sensitive`, `empty_clip_returns_none`.
- [ ] **Step 2:** `cargo test -p clipsy-windows formats` Expected: FAIL.
- [ ] **Step 3: Implement** the functions above. Add `image` (png, bmp features) to `clipsy-windows` deps if bmp decoding needs it.
- [ ] **Step 4:** Run tests. Expected: PASS; also run `cargo test -p clipsy-core` to confirm the store accepts events built this way (add one integration test in `clipsy-windows/tests/store_roundtrip.rs` that inserts a `build_event` result into a temp `Store` and lists it).
- [ ] **Step 5: Commit** `git commit -m "feat(windows): clipboard format mapping"`

### Task 6: Clipboard listener and writer (Windows only)

**Files:**
- Create: `crates/clipsy-windows/src/clipboard.rs` (`#[cfg(windows)]`)

**Interfaces:**
- Consumes: `formats::{RawClip, build_event}`, `clipsy_core::{ClipEvent, Format}`.
- Produces: `pub fn spawn_listener(tx: std::sync::mpsc::Sender<clipsy_core::ClipEvent>) -> anyhow::Result<()>` (message-only window on its own thread, `AddClipboardFormatListener`, on `WM_CLIPBOARDUPDATE` reads formats and sends `build_event`); `pub fn write_formats(formats: &[clipsy_core::Format]) -> anyhow::Result<()>` (writes `CF_UNICODETEXT`, registered `PNG`, `HTML Format`, `CF_HDROP` as applicable); `pub fn mark_own_write()` / own-write suppression via `GetClipboardSequenceNumber` recorded after `write_formats` and compared in the listener. `OpenClipboard` retries up to 10 times at 20 ms. Source app = `GetClipboardOwner` -> process image file name.

- [ ] **Step 1:** Implement with the signatures above; keep unsafe Win32 calls inside small helpers with `// SAFETY:` comments.
- [ ] **Step 2:** Add a pure unit test for the retry helper `fn retry<T>(attempts: u32, delay_ms: u64, f: impl FnMut() -> Option<T>) -> Option<T>` (`retry_stops_on_first_success`, `retry_gives_up_after_attempts`) in this file under `#[cfg(test)]` outside the windows gate (place the helper in `formats.rs`-style ungated `util.rs` if needed so it runs on Linux).
- [ ] **Step 3:** Run `cargo test -p clipsy-windows` (PASS) and `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu` (compiles).
- [ ] **Step 4: Commit** `git commit -m "feat(windows): clipboard listener and writer"`

### Task 7: Hotkeys, paste, and `WindowsBackend`

**Files:**
- Create: `crates/clipsy-windows/src/paste.rs`, `crates/clipsy-windows/src/backend.rs` (both `#[cfg(windows)]`)

**Interfaces:**
- Consumes: `hotkey::parse_accel`, `terminals::is_terminal`, `clipboard::{spawn_listener, write_formats}`, `clipsy_backends::{Backend, PasteMode, HotkeyAction}`.
- Produces: `pub struct WindowsBackend` with `pub fn new() -> Self` implementing `Backend`: `name() -> "windows"`; `start` spawns the clipboard listener and a hotkey thread (message loop using `RegisterHotKey`/`UnregisterHotKey`, sends `HotkeyAction`); `set_clipboard` -> `write_formats`; `remember_active_window` stores `GetForegroundWindow()` if it is not a Clipsy window; `paste(mode)` restores focus (`SetForegroundWindow`, with the `AttachThreadInput` workaround), waits for the window to be foreground (poll up to 500 ms, as `wait_until` does on X11), then `SendInput` Ctrl+V or Ctrl+Shift+V (terminal exe via `GetWindowThreadProcessId` + `QueryFullProcessImageNameW`, `is_terminal` with the list from `set_terminal_apps`); `supports_paste() -> true`; `register_hotkey`/`unregister_hotkey`; `set_terminal_apps`. If the remembered window no longer exists (`IsWindow` false) or is Clipsy itself, `paste` returns `Err` with a clear message and does not send keys. Registration failure returns `Err("hotkey <accel> is already in use or invalid")`.
- Produces ungated helper in `paste.rs`: `pub fn key_sequence(terminal: bool) -> Vec<(u16, bool)>` (virtual key, is_key_down) listing the exact Ctrl[+Shift]+V down/up order, unit-tested.

- [ ] **Step 1: Write failing tests** `key_sequence_normal_is_ctrl_v` and `key_sequence_terminal_is_ctrl_shift_v` (down order Ctrl, Shift, V; up in reverse) in `paste.rs`.
- [ ] **Step 2:** Run `cargo test -p clipsy-windows paste` Expected: FAIL; implement `key_sequence`; Expected: PASS.
- [ ] **Step 3:** Implement `WindowsBackend` per the Interfaces.
- [ ] **Step 4:** `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu` Expected: compiles; `cargo clippy -p clipsy-windows --target x86_64-pc-windows-gnu -- -D warnings` Expected: clean.
- [ ] **Step 5: Commit** `git commit -m "feat(windows): WindowsBackend with hotkey and paste"`

### Task 8: Single instance and CLI verbs

**Files:**
- Create: `crates/clipsy-windows/src/single_instance.rs` (`#[cfg(windows)]` for mutex/pipe; ungated `Verb` and `parse_verb`)

**Interfaces:**
- Produces (ungated): `pub enum Verb { Toggle, Settings, Pause, Resume, Clear, Quit }`, `pub fn parse_verb(arg: &str) -> Option<Verb>`, `pub fn verb_name(v: Verb) -> &'static str`. Produces (windows): `pub enum Role { Primary(PrimaryGuard), Secondary }`, `pub fn acquire() -> anyhow::Result<Role>`, `pub fn forward(v: Verb) -> anyhow::Result<()>` (connects to the pipe, 2 s timeout, writes verb line), `pub fn serve(tx: std::sync::mpsc::Sender<Verb>)` (pipe server thread).

- [ ] **Step 1: Write failing tests** `parses_all_verbs`, `unknown_verb_is_none`, `verb_round_trip_names`.
- [ ] **Step 2:** Run tests, expect FAIL; implement; expect PASS.
- [ ] **Step 3:** Implement the Windows mutex/pipe parts; if `forward` fails (primary still starting) retry for 2 s then exit with a message instead of starting a second tray icon.
- [ ] **Step 4:** `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu`. Expected: compiles.
- [ ] **Step 5: Commit** `git commit -m "feat(windows): single instance and CLI verbs"`

### Task 9: Autostart

**Files:**
- Create: `crates/clipsy-windows/src/autostart.rs` (pure part ungated, registry part `#[cfg(windows)]`)

**Interfaces:**
- Produces: `pub fn run_value(exe: &std::path::Path) -> String` (returns `"<exe path>" --background` with the path quoted); windows: `pub fn set_enabled(exe: &std::path::Path, enabled: bool) -> anyhow::Result<()>`, `pub fn is_enabled() -> bool` using `winreg` on `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `Clipsy`.

- [ ] **Step 1: Write failing tests** `run_value_quotes_path_with_spaces`, `run_value_adds_background_flag`.
- [ ] **Step 2:** Run, expect FAIL; implement; run, expect PASS.
- [ ] **Step 3:** Implement the registry functions; `cargo check` for the Windows target.
- [ ] **Step 4: Commit** `git commit -m "feat(windows): autostart via registry"`

### Task 10: Popup state and Slint popup UI

**Files:**
- Create: `crates/clipsy-windows/src/popup_state.rs` (ungated), `crates/clipsy-windows/ui/popup.slint`, modify `crates/clipsy-windows/ui/app.slint` to export `PopupWindow`
- Create: `crates/clipsy-windows/src/popup.rs` (`#[cfg(windows)]`)

**Interfaces:**
- Produces (`popup_state.rs`): `pub struct PopupState { pub query: String, pub filter: clipsy_core::Filter, pub items: Vec<clipsy_core::ClipItem>, pub selected: usize }` with `pub fn new() -> Self`, `pub fn set_items(&mut self, items: Vec<ClipItem>)` (clamps selection), `pub fn move_selection(&mut self, delta: i32)` (wraps, no panic on empty), `pub fn selected_item(&self) -> Option<&ClipItem>`, `pub fn set_filter(&mut self, f: Filter)`, `pub fn type_char(&mut self, c: char)`, `pub fn backspace(&mut self)`.
- Produces (`popup.rs`): `pub struct Popup` with `pub fn new(store: Rc<RefCell<Store>>, config: Rc<RefCell<Config>>, backend: Rc<WindowsBackend>) -> Self`, `pub fn toggle(&self)`, `pub fn refresh(&self)`. UI: frameless always-on-top window without taskbar entry, search box, filter tabs (All/Text/Images/Files/Pinned), list rows with preview text or thumbnail, pin button, Up/Down/Enter/Esc/Shift+Enter (plain paste), hides on focus loss, positioned near the cursor (`GetCursorPos`) clamped to the monitor work area, or screen center per `appearance.popup_position`; light/dark follows `appearance.theme` and Windows setting.

- [ ] **Step 1: Write failing tests** `selection_wraps_both_directions`, `empty_list_selection_is_none_and_move_does_not_panic`, `set_items_clamps_selected`, `typing_updates_query`, `backspace_on_empty_query_is_noop`, `filter_change_resets_selection`.
- [ ] **Step 2:** `cargo test -p clipsy-windows popup_state` expect FAIL; implement; expect PASS.
- [ ] **Step 3:** Write `popup.slint` and `popup.rs` per Interfaces; Enter calls `backend.set_clipboard(formats)` then hides the popup then `backend.paste(PasteMode::Normal)` (or only copies when `general.paste_on_select` is false); `store.touch(id)` after use.
- [ ] **Step 4:** `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu` compiles (Slint build.rs runs for the Windows target).
- [ ] **Step 5: Commit** `git commit -m "feat(windows): Slint popup and popup state"`

### Task 11: Settings window and tray

**Files:**
- Create: `crates/clipsy-windows/ui/settings.slint`, `crates/clipsy-windows/src/settings.rs`, `crates/clipsy-windows/src/tray.rs` (all `#[cfg(windows)]` except the pure validation below)
- Create (ungated) in `settings.rs`: `pub fn validate_hotkey(s: &str) -> Result<(), String>` (wraps `parse_accel`)

**Interfaces:**
- Produces: `pub struct SettingsWindow` with `new(config: Rc<RefCell<Config>>, on_saved: impl Fn() + 'static) -> Self` and `show(&self)`; covers general (autostart, max items, keep days, paste on select, save images/files), appearance (theme, popup position, compact, show source app), shortcuts (toggle popup with `validate_hotkey`), privacy (paused, skip sensitive, ignored apps, clear on exit), and the `terminal_apps` list; writes via `Config::save`. `tray::Tray::new(on_verb: impl Fn(Verb) + 'static) -> anyhow::Result<Tray>` with menu Open, Pause/Resume, Settings, Clear history, Quit.

- [ ] **Step 1: Write failing tests** `valid_hotkey_accepted`, `hotkey_without_key_rejected_with_message`.
- [ ] **Step 2:** Run, expect FAIL; implement `validate_hotkey`; run, expect PASS.
- [ ] **Step 3:** Implement the settings window and tray.
- [ ] **Step 4:** `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu` compiles; clippy clean for that target.
- [ ] **Step 5: Commit** `git commit -m "feat(windows): settings window and tray"`

### Task 12: App wiring

**Files:**
- Create: `crates/clipsy-windows/src/app.rs` (`#[cfg(windows)]`); modify `crates/clipsy-windows/src/main.rs`

**Interfaces:**
- Consumes: everything above. Produces: `pub fn run(args: Vec<String>) -> anyhow::Result<()>`: parse verb from args (`--background` starts hidden); `single_instance::acquire()`; if `Secondary` forward verb and exit; else `Config::load()`, `Store::open(data_dir)`, `WindowsBackend::start(clip_tx, hotkey_tx)`, register `shortcuts.toggle_popup` (on failure show tray notice and continue), pump `ClipEvent`s into `store.insert_event` then `store.prune` then `popup.refresh()` (skipped while `privacy.paused`), prune hourly and on start, handle `HotkeyAction`s and `Verb`s (toggle, settings, pause, resume, clear, quit with `clear_on_exit`), apply autostart setting, first-run welcome toast showing the hotkey.

- [ ] **Step 1:** Implement `run` and call it from `main`.
- [ ] **Step 2:** `cargo build --release -p clipsy-windows --target x86_64-pc-windows-gnu` Expected: produces `target/x86_64-pc-windows-gnu/release/clipsy.exe`.
- [ ] **Step 3:** Run `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` on Linux. Expected: all pass.
- [ ] **Step 4: Commit** `git commit -m "feat(windows): wire the Windows app together"`

### Task 13: Docs and acceptance checklist

**Files:**
- Modify: `docs/PLAN.md` (Windows moves from non-goal to Phase W; note default hotkey Ctrl+Shift+V), `README.md` (Windows build section: Windows needs Rust + MSVC build tools, `cargo build --release -p clipsy-windows`)
- Create: `docs/windows-acceptance.md` with the checklist below

- [ ] **Step 1:** Write the docs. Checklist (user runs on Windows 10/11 with the cross-built or natively built `clipsy.exe`): (1) copy text, image and a file in Explorer, all appear in history; (2) `Ctrl+Shift+V` opens the popup near the cursor; (3) Enter pastes into Notepad and into Windows Terminal (check Ctrl+Shift+V used); (4) pin and search work; (5) history survives restart; (6) tray menu works (pause stops capture); (7) autostart toggle adds/removes the Run registry value; (8) copying from a password manager that sets the exclude flag is skipped; (9) running `clipsy.exe` twice leaves one tray icon; (10) no network connections (check with Resource Monitor).
- [ ] **Step 2:** Tell the user the exact commands to build and test, and ask them to run the checklist. Stop until they confirm.
- [ ] **Step 3: Commit** `git commit -m "docs: Windows plan, README and acceptance checklist"`
