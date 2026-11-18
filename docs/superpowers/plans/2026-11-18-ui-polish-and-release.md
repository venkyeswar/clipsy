# UI Polish and Release Readiness Plan

> **For agentic workers:** use superpowers:subagent-driven-development. Tasks 1-3 touch disjoint files and run in parallel.

**Goal:** Single-click paste, per-row remove/pin buttons, compact sleek popup, one-screen welcome with keycaps, and a shippable `.deb`.

**Architecture:** GTK4/libadwaita, system accent and named colors only (light/dark automatic). Spec: the design approved in chat on 2026-11-18 (popup 380x480, rows 34px, images 44px, 8px radii, keycap shortcut display) plus `docs/PLAN.md` §8.

**Tech Stack:** Rust, gtk4 0.9, libadwaita 0.7 (v1_5 APIs only), cargo-deb.

## Global Constraints
- Only libadwaita named colors (`@accent_bg_color`, `@window_bg_color`, `@card_bg_color`, `@window_fg_color`); no hard-coded hex, so light and dark both work.
- Popup default size 380x480; row height 34px; image rows 44px; radius 8px; no shadows.
- Built-in default toggle shortcut is `<Super><Shift>v`.
- `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` must pass. Never launch the GUI (user is using it); never run git.
- Strings: sentence case, plain verbs.

## Review Focus
- Clicking ✕ on the selected row must not paste it, and selection must move to a neighbour.
- Removing the last item shows the empty state.
- Clicking pin/✕ must not trigger the row's paste.
- Keyboard use still works without hover (buttons visible on selected row).
- Long one-line text, multi-line text and huge text previews stay on one ellipsized line.

### Task 1: Compact popup (files: `crates/clipsy-app/src/popup.rs`, `crates/clipsy-app/resources/style.css`)
- [ ] Enable `list_view.set_single_click_activate(true)` so one click pastes; Enter still pastes.
- [ ] Rebuild the row: kind glyph (left), one-line ellipsized preview, right-aligned dim meta ("2m"), then pin (`starred-symbolic`/`non-starred-symbolic`) and remove (`window-close-symbolic`) flat buttons, shown on hover and on the selected row (CSS opacity, not visibility, so keyboard focus works). Buttons call existing store pin/delete and refresh without triggering row activation.
- [ ] Image rows 44px with 36px thumbnail; text rows 34px.
- [ ] Popup 380x480; slim 32px search; filter chips become small flat text tabs (All, Text, Images, Files, Pinned star); replace hint bar with one-line footer `↵ paste  ⇧↵ plain  ⌫ remove` in `.clipsy-hint`.
- [ ] Keep row context menu (right-click) and all keyboard shortcuts; `style.css` gets the new classes. Add unit tests only for pure helpers (e.g. relative-time "2m"/"1h" formatting).
- [ ] Verify: build, clippy, tests.

### Task 2: Welcome screen + default shortcut (files: `crates/clipsy-app/src/welcome.rs`, `crates/clipsy-core/src/config.rs`)
- [ ] Replace the carousel with one compact `adw::Window` (~380 wide): app icon, "Clipsy", "Everything you copy, one shortcut away.", the current shortcut drawn as keycap labels (`Super` `Shift` `V`, own CSS provider inside welcome.rs, raised surface + 1px bottom border, 6px radius) with a "Change" button reusing the existing shortcut recorder in settings.rs if it is `pub(crate)` (otherwise expose it), rows "Start at login" and "Paste after selecting" (switches), primary button "Start using Clipsy".
- [ ] Remove the broken Super+V gsettings rebind step from the welcome flow (default no longer conflicts); keep `welcome-done` marker behaviour.
- [ ] Change `Shortcuts::default().toggle_popup` to `"<Super><Shift>v"` and update the core test that asserts defaults.
- [ ] Add a pure helper `accel_to_keycaps(&str) -> Vec<String>` (e.g. `"<Super><Shift>v"` -> `["Super","Shift","V"]`, `"<Ctrl><Alt>c"` -> `["Ctrl","Alt","C"]`, `""` -> `[]`) with unit tests.
- [ ] Verify: build, clippy, tests.

### Task 3: Release packaging (files: `packaging/*`, `README.md`, `LICENSE`, `.github/workflows/ci.yml`, `crates/clipsy-app/Cargo.toml` metadata only)
- [ ] Add `[package.metadata.deb]` to clipsy-app (name `clipsy`, depends on `libgtk-4-1, libadwaita-1-0`, assets: binary to `usr/bin/clipsy`, desktop file, metainfo, SVG icon to `usr/share/icons/hicolor/scalable/apps/`). `packaging/build-deb.sh` runs `cargo deb -p clipsy-app`; document `cargo install cargo-deb`.
- [ ] `README.md` (what it is, install from .deb, usage, shortcuts, privacy: local only, build from source with apt packages, known limits: X11 only for now), GPL-3.0 `LICENSE` text, CI workflow (build, test, clippy on ubuntu-24.04 with apt libs).
- [ ] Try building the .deb if cargo-deb installs; report the file size; otherwise say so.

## Self-review
Spec coverage: single click (T1), remove buttons (T1), compact/sleek (T1), landing page (T2), production-ready (T3). Interfaces: T2 may need the recorder from settings.rs made `pub(crate)` — a one-word visibility change, allowed.
