# Windows Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Windows app match Ubuntu: Win+Shift+V shortcut, Linux-style popup with delete/preview/hints, gear and trash buttons, fuller Settings, working Clear all, and the Clipsy logo everywhere.

**Architecture:** Extend the existing Slint UI and Rust glue in `crates/clipsy-windows` only. Reuse `clipsy_core` (`Config`, `Store::clear/count/delete/set_pinned`). No changes to shared crates, so Linux CI stays idle.

**Tech Stack:** Rust, Slint, tray-icon 0.19, windows crate 0.58, Inno Setup.

**Spec:** `docs/superpowers/specs/2026-11-19-windows-parity-design.md`

## Global Constraints

- Windows-only change: do not edit `crates/clipsy-core`, `crates/clipsy-backends`, `crates/clipsy-app`, `gnome-extension/`, `data/` (Linux CI paths). Copy the icon into `crates/clipsy-windows/assets/` instead of referencing `data/`.
- Default shortcut string is `<Super><Shift>v` (parsed by `hotkey::parse_accel`, Super = Windows key). Legacy default `<Ctrl><Shift>v`.
- Fallback shortcut when registration fails: `<Ctrl><Shift>v`.
- Footer hint text: `Enter paste   Shift+Enter plain   Delete remove`.
- Keys: Delete removes the selected row, Ctrl+P toggles pin, Space previews (only when the search box is empty).
- Clear confirm dialog offers "Also remove pinned" (default off); `Store::clear(keep_pinned)` with `keep_pinned = !also_pinned`.
- Do NOT bump the version or release. Release only after the user tests on the EC2 box and says so.
- `cargo clippy -p clipsy-windows --all-targets -- -D warnings` and `cargo test -p clipsy-windows` must pass. Slint and Win32 code only compile on Windows, so the Windows CI run (ci-windows.yml) is the compile gate.
- Image items must paste into Paint, Word, browsers and terminal apps: write registered `PNG` plus `CF_DIBV5` and `CF_DIB`, and paste images with plain Ctrl+V even in terminals.
- Text into classic console windows (cmd.exe, PowerShell in conhost) pastes with Shift+Insert; Windows Terminal (CASCADIA_HOSTING_WINDOW_CLASS) with Ctrl+Shift+V; other apps Ctrl+V.

## Review Focus

- Win+Shift+V is already owned by Windows: app still starts, falls back to Ctrl+Shift+V, tray tooltip names the key actually active.
- User has a custom shortcut in config.toml: it is kept untouched; only the exact legacy default migrates.
- Delete pressed on the last row or on an empty list: selection index stays valid, no panic.
- Clear with zero items or only pinned items: dialog shows correct counts, no error.
- Preview/Space with an image, file or very long multi-line item: shows something sensible and never grows the popup beyond the screen.
- Settings number fields empty or non-numeric: old value kept (existing `clamp_u32` behaviour), no crash.
- Image copied from a browser then pasted into Paint, Word and a terminal; elevated (admin) target window refuses SendInput: log it and show a tray notice, never hang.

---

### Task 1: Shortcut default, migration, fallback

**Files:**
- Modify: `crates/clipsy-windows/src/hotkey.rs`, `crates/clipsy-windows/src/app.rs:57-175`, `crates/clipsy-windows/src/tray.rs`
- Test: unit tests in `hotkey.rs`

**Interfaces:**
- Produces: `pub fn migrate_toggle_default(current: &str) -> String` in `hotkey.rs` (returns `"<Super><Shift>v"` iff `current` equals `"<Ctrl><Shift>v"`, else `current` unchanged); `pub const WINDOWS_DEFAULT: &str = "<Super><Shift>v"` and `pub const FALLBACK: &str = "<Ctrl><Shift>v"`.

- [ ] **Step 1: Write failing tests** in `hotkey.rs`: `migrate_legacy_default_to_super_shift_v` (`migrate_toggle_default("<Ctrl><Shift>v") == "<Super><Shift>v"`), `migrate_keeps_custom_shortcut` (`"<Ctrl><Alt>k"` unchanged, `"<Super><Shift>v"` unchanged), `super_shift_v_parses` (`parse_accel("<Super><Shift>v")` gives win=true, shift=true, ctrl=false).
- [ ] **Step 2: Run** `cargo test -p clipsy-windows hotkey` — expect FAIL (fn missing).
- [ ] **Step 3: Implement** the constants and `migrate_toggle_default` in `hotkey.rs`.
- [ ] **Step 4: Run** the same command — expect PASS.
- [ ] **Step 5: In `app.rs::run`**, after loading config apply `migrate_toggle_default` to `config.shortcuts.toggle_popup` (and save the config if it changed). In the registration loop, if the toggle shortcut fails to bind and it equals `WINDOWS_DEFAULT`, try `FALLBACK`; on success keep the config string as the default but log `warn!` and use the fallback key name in the tooltip and in the notify text ("Win+Shift+V is used by Windows, so Clipsy uses Ctrl+Shift+V. Change it in Settings."). Tooltip must show the key actually bound.
- [ ] **Step 6: In `settings.rs::save`**, after a successful shortcut change, refresh the tray tooltip (`Tray::notify`-style `set_tooltip` with `tray_tooltip(Some(new))`) so it is not stale.
- [ ] **Step 7: Commit** `git add crates/clipsy-windows && git commit -m "feat(windows): Win+Shift+V default shortcut with fallback and migration"`

### Task 2: Logo everywhere

**Files:**
- Create: `crates/clipsy-windows/assets/clipsy.svg` (copy of `data/icons/io.github.clipsy.Clipsy.svg`), `assets/clipsy.ico` (16,32,48,256), `assets/clipsy-64.png`
- Modify: `crates/clipsy-windows/build.rs`, `crates/clipsy-windows/Cargo.toml`, `src/tray.rs` (`icon_rgba` at l.144), `ui/popup.slint`, `ui/settings.slint`, `packaging/windows/clipsy.iss`

**Interfaces:**
- Produces: `assets/clipsy.ico` and `assets/clipsy-64.png`, used by build.rs (exe icon resource via the `winresource` build-dependency, `#[cfg(windows)]`-gated), tray (decode embedded PNG instead of the blue square), Slint `Image { source: @image-url("../assets/clipsy-64.png") }`, and Inno Setup (`SetupIconFile=..\..\crates\clipsy-windows\assets\clipsy.ico`).

- [ ] **Step 1:** Generate the PNG/ICO from the SVG with `rsvg-convert` and ImageMagick `convert` (install with apt if missing); verify with `file assets/clipsy.ico` showing several sizes.
- [ ] **Step 2: Write a test** in `tray.rs`: `tray_icon_rgba_is_32x32_and_not_flat` (decoded RGBA length is 32*32*4 and has more than one distinct colour).
- [ ] **Step 3: Run** `cargo test -p clipsy-windows tray` — expect FAIL.
- [ ] **Step 4: Implement** the embedded-PNG icon (`include_bytes!` plus the `png`/`image` crate already in the tree or `png` decoder added to Cargo.toml), build.rs `winresource` icon, a 20px logo in the popup header and settings header, and `SetupIconFile` + `UninstallDisplayIcon` in the .iss.
- [ ] **Step 5: Run** test, expect PASS; run clippy.
- [ ] **Step 6: Commit** `feat(windows): show the Clipsy logo in tray, popup, settings and exe`

### Task 3: Popup restyle, delete, preview, hints, gear and trash

**Files:**
- Modify: `crates/clipsy-windows/ui/popup.slint` (`ClipPopup`, `RowData`), `crates/clipsy-windows/src/popup.rs` (`Inner::connect` l.140, `push_rows` l.249, `to_row` l.540), `crates/clipsy-windows/src/popup_state.rs`
- Test: `popup_state.rs`

**Interfaces:**
- Produces in `popup_state.rs`: `pub fn selection_after_delete(selected: usize, len_before: usize) -> usize` (clamps: returns `0` when `len_before <= 1`, else `min(selected, len_before - 2)`).
- Produces in `ClipPopup`: callbacks `delete-requested(int)`, `settings-requested()`, `clear-requested()`, `preview-toggled(int)`; property `preview-text: string` (empty = hidden); `RowData` gains no new fields.
- Consumes: `Store::delete(id)`, existing `toggle_pin`, `refresh`.

- [ ] **Step 1: Write failing tests**: `selection_after_delete_single_item_goes_to_zero` (`(0,1) -> 0`), `selection_after_delete_last_row_moves_up` (`(4,5) -> 3`), `selection_after_delete_middle_stays` (`(1,5) -> 1`), `selection_after_delete_empty_is_zero` (`(0,0) -> 0`).
- [ ] **Step 2: Run** `cargo test -p clipsy-windows popup_state` — expect FAIL.
- [ ] **Step 3: Implement** `selection_after_delete`.
- [ ] **Step 4: Run** — expect PASS.
- [ ] **Step 5: Restyle `popup.slint`**: header (logo, "Clipsy", item count, gear button, trash button), search box, pill filter row (active pill = accent #3584e4 background), rows with a 2px accent left edge when pinned and hover-only pin and delete buttons, footer hint bar with the exact text from Global Constraints. Keep colours from the existing light/dark properties; add a preview panel shown when `preview-text` is non-empty (max height 160px, scrollable, text wrap).
- [ ] **Step 6: Key handling** in the search `TextInput` handler: Delete (when query is empty or selection exists) calls `delete-requested(selected)`; Ctrl+P calls `pin-toggled(selected)`; Space with empty query calls `preview-toggled(selected)`.
- [ ] **Step 7: Wire in `popup.rs`**: `on_delete_requested` deletes via the store, then `refresh` and `set_selected(selection_after_delete(..))`; `on_preview_toggled` sets/clears `preview-text` from the item (text as is, truncated to 2000 chars; image/file items show their label); `on_settings_requested` and `on_clear_requested` call callbacks supplied to `Popup::new` (add two `Rc<dyn Fn()>` parameters and update the single caller in `app.rs`).
- [ ] **Step 8: Run** clippy and tests, then commit `feat(windows): Linux-style popup with delete, preview, hints, gear and trash`

### Task 4: Clear confirm dialog (popup trash, tray, Settings)

**Files:**
- Create: `crates/clipsy-windows/ui/clear_dialog.slint` (`ClearDialog inherits Window`), `crates/clipsy-windows/src/clear_dialog.rs`
- Modify: `ui/app.slint` (export the new component), `src/app.rs` (`clear_history` at l.346 and the verb path l.320-327), `src/settings.rs`, `ui/settings.slint`
- Test: `clear_dialog.rs`

**Interfaces:**
- Produces: `pub fn clear_message(total: usize, pinned: usize, also_pinned: bool) -> String` in `clear_dialog.rs`: `(0,0,_) -> "Nothing to clear."`; `(12,3,false) -> "Remove 9 items? 3 pinned items will be kept."`; `(12,3,true) -> "Remove all 12 items, including 3 pinned?"`; singular "1 item" when the count is 1.
- Produces: `pub fn confirm_and_clear(store: &Store, on_done: impl Fn() + 'static)` opening `ClearDialog`; when confirmed it calls `store.clear(!also_pinned)` then `on_done`.
- Consumes: `Store::count`, `Store::clear(keep_pinned)`, a pinned-count query (use `Store::list` with the pinned filter used by popup filter index 4, or `clear_filtered` count helper if present).

- [ ] **Step 1: Write failing tests** for the four `clear_message` cases above.
- [ ] **Step 2: Run** `cargo test -p clipsy-windows clear_dialog` — expect FAIL; implement `clear_message`; run again, expect PASS.
- [ ] **Step 3: Build `ClearDialog`** (message text, "Also remove pinned" checkbox that updates the message live, Cancel and Clear buttons) and `confirm_and_clear`.
- [ ] **Step 4:** Route popup trash (`on_clear_requested`), tray "Clear history" (`on_verb` Clear) and the clear-history hotkey through `confirm_and_clear`; add a "Clear history…" button under Privacy in `settings.slint` that calls the same function.
- [ ] **Step 5: Run** clippy and tests, then commit `feat(windows): confirm dialog for clearing history`

### Task 5: Settings parity

**Files:**
- Modify: `crates/clipsy-windows/ui/settings.slint` (`SettingsDialog`), `crates/clipsy-windows/src/settings.rs` (`load` l.158, `save` l.194)
- Test: `settings.rs`

**Interfaces:**
- Consumes: existing `clamp_u32(text, fallback, min, max)` helper (check exact signature at l.30 before use).
- Produces: new `SettingsDialog` properties `max-total-mb: string`, `max-item-mb: string`, `save-rich-text: bool`; mapped to `Config.general.max_total_mb`, `max_item_mb`, `save_rich_text`.

- [ ] **Step 1: Write failing tests** in `settings.rs` using the pure helpers: `blank_total_size_keeps_old_value` (`clamp_u32("", 500, ..) == 500`) and `non_numeric_item_size_keeps_old_value`, plus a range check test for max_item_mb (min 1).
- [ ] **Step 2: Run** `cargo test -p clipsy-windows settings` — expect FAIL only if a helper must change; otherwise PASS (then keep the tests as regression guards).
- [ ] **Step 3: Add** the three fields to the General section of `settings.slint` and map them in `load` and `save`.
- [ ] **Step 4: Run** clippy and tests, commit `feat(windows): settings fields for size limits and rich text`

### Task 7: Paste fixes (images and terminals)

**Files:**
- Modify: `crates/clipsy-windows/src/clipboard.rs` (`clipboard_payloads` ~l.374, `push_registered` l.394), `crates/clipsy-windows/src/paste.rs` (l.15-24 key choice, `exe_name` l.89), `crates/clipsy-windows/src/terminals.rs`, `crates/clipsy-windows/src/popup.rs` (l.365 always `PasteMode::Normal`), `crates/clipsy-windows/src/backend.rs` (l.286-316 ignores the mode)
- Test: `clipboard.rs`, `paste.rs`, `terminals.rs`

**Interfaces:**
- Produces in `clipboard.rs`: `pub fn png_to_dib(png: &[u8]) -> Option<Vec<u8>>` (BITMAPINFOHEADER + bottom-up 32bpp BGRA pixels, as CF_DIB expects) and `pub fn png_to_dibv5(png: &[u8]) -> Option<Vec<u8>>` (BITMAPV5HEADER, BI_BITFIELDS, 32bpp); `clipboard_payloads` for `image/png` returns the registered `PNG` payload plus CF_DIBV5 and CF_DIB when conversion succeeds.
- Produces in `paste.rs`: `pub enum PasteKeys { CtrlV, CtrlShiftV, ShiftInsert }` and `pub fn keys_for(class: &str, exe: Option<&str>, mode: PasteMode) -> PasteKeys` (pure): `PasteMode::Binary` always gives `CtrlV`; class `ConsoleWindowClass` gives `ShiftInsert`; class `CASCADIA_HOSTING_WINDOW_CLASS` gives `CtrlShiftV`; otherwise terminal exe names (existing `terminals::is_terminal`) give `CtrlShiftV`; everything else `CtrlV`. Add `fn window_class(hwnd) -> Option<String>` using `GetClassNameW` (Windows only).
- Consumes: `clipsy_backends::PasteMode` (Normal, Binary); popup passes `PasteMode::Binary` when the activated row kind is an image (`paste_mode_for(kind, plain)` pure helper in `popup_state.rs`: image kind and not plain gives Binary, else Normal).

- [ ] **Step 1: Write failing tests**: in `clipboard.rs` `png_to_dib_has_info_header_and_pixel_bytes` (a 2x2 PNG built with the `image` crate gives a DIB whose first 4 bytes are 40 as u32 LE, width 2, height 2, bit count 32, length 40 + 16) and `png_to_dibv5_header_size_is_124`; in `paste.rs` `console_window_class_uses_shift_insert`, `cascadia_class_uses_ctrl_shift_v`, `binary_mode_is_plain_ctrl_v_even_in_terminal` (exe `WindowsTerminal.exe`, mode Binary gives CtrlV), `unknown_app_uses_ctrl_v`; in `popup_state.rs` `image_rows_use_binary_paste_mode`.
- [ ] **Step 2: Run** `cargo test -p clipsy-windows paste clipboard popup_state` — expect FAIL.
- [ ] **Step 3: Implement** the converters (decode with the `image` crate, write RGBA as BGRA bottom-up), the extended payload list, `PasteKeys`, `keys_for`, `window_class`, and `paste_mode_for`; send Shift+Insert as a chord of Shift down, Insert down/up, Shift up (same pattern as the existing Ctrl+V chord in `send_chord`).
- [ ] **Step 4:** Thread the mode through `Popup::activate` to `WindowsBackend` paste (backend.rs l.286-316) and use `keys_for` there. If the target is elevated and `SendInput` is blocked (paste.rs l.177-180), also show a tray notice "Windows blocked pasting into that window (it runs as administrator)".
- [ ] **Step 5: Run** tests and clippy, expect PASS; commit `fix(windows): paste images with DIB formats and fix terminal paste keys`
- [ ] **Step 6:** Verify on the EC2 box in Task 6: copy an image in Edge, paste into Paint and into PowerShell; paste text into cmd.exe, PowerShell and Windows Terminal.

### Task 6: Test build on the EC2 Windows machine (no release)

**Files:**
- Modify: `.github/workflows/ci-windows.yml` (add a step that uploads `target/release/clipsy-windows.exe` renamed `clipsy.exe` plus the zip layout as artifact `clipsy-windows-test`; also `workflow_dispatch` trigger)

- [ ] **Step 1:** Run the full Linux gate: `cargo clippy -p clipsy-windows --all-targets -- -D warnings` and `cargo test -p clipsy-windows`.
- [ ] **Step 2:** Add the artifact upload step to ci-windows.yml; commit `ci(windows): upload a test build artifact`.
- [ ] **Step 3:** Push the branch only after the user agrees; wait for the CI Windows run; download the artifact with `gh run download`.
- [ ] **Step 4:** Upload the exe to a private S3 bucket (AWS profile `Venky`, region ap-south-1) and create a presigned URL valid 1 hour; give the user one PowerShell line to run on the EC2 desktop that downloads and starts it.
- [ ] **Step 5:** The user tests: Win+Shift+V, popup look, delete, preview, gear, trash, Settings, logo. Fix what they report. Then ask whether to release (version bump 0.4.4 and tag are a separate step), and clean up S3 and EC2.
