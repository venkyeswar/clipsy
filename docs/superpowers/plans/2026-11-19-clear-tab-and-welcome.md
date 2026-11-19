# Clear-per-tab, Bigger Popup, New Welcome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-tab Clear button with a "also remove pinned" confirm dialog, enlarge the popup, and replace the first-run welcome with a roomier multi-page carousel.

**Architecture:** Store gets two filter-scoped methods (`count`, `clear_filtered`) reusing the existing filter SQL. A new `clear_dialog.rs` in clipsy-app owns the confirm dialog and is used by both the popup and Settings. Popup and welcome are layout changes in their existing files.

**Tech Stack:** Rust, GTK4 0.14 / libadwaita 1.5 (gtk4-rs, `adw::MessageDialog`, `adw::Carousel`), rusqlite, tempfile (tests).

**Spec:** docs/superpowers/specs/2026-11-19-clear-tab-and-welcome-design.md

## Global Constraints

- Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` before calling any task done (matches CI).
- Pinned items are only ever deleted when the user ticks "Also remove pinned items" (default unchecked), except on the ★ tab where pinned items are the scope.
- Popup default size 440×560; minimum width 380; welcome window 480×600.
- Follow existing test style: inline `#[cfg(test)]`, `assert_eq!`, `unwrap()`, helpers `open()`, `text_ev()`, `tick()` in store.rs.
- Dev machine is Ubuntu 24.04 / GNOME 46 / X11; the user is a beginner, so give exact run/test commands in the final report.

## Review Focus

- Tab has zero items: Clear button disabled (or dialog says "Nothing to clear"); never deletes anything.
- Search text is active when Clear is pressed: it clears the whole tab, not just matches, and the dialog says so ("all N Text items").
- `Html` items belong to the Text tab (`kind IN ('text','html')`); clearing Text must remove them too.
- Image/file blobs and thumbnails are purged after a filtered clear (no orphan files).
- Popup hides on focus loss: opening the confirm dialog must not dismiss the popup or lose the dialog.
- Corrupt or tiny `popup-size` file falls back to the default size.

---

### Task 1: Filter-scoped count and clear in the store

**Files:**
- Modify: `crates/clipsy-core/src/store.rs` (filter match in `list()` ~:197-239; `clear()` ~:310; tests ~:822+)

**Interfaces:**
- Produces: `fn filter_sql(filter: Filter) -> &'static str` (private; extracted from `list()`, same strings), `pub fn count(&self, filter: Filter, include_pinned: bool) -> Result<i64>`, `pub fn clear_filtered(&self, filter: Filter, include_pinned: bool) -> Result<usize>` (returns rows deleted).
- Semantics: for `Filter::Pinned` the scope is `pinned = 1` and `include_pinned` is ignored; for every other filter the scope is `(filter_sql) AND (include_pinned OR pinned = 0)`. `clear_filtered` ends with `self.purge_orphan_files()`.

- [ ] **Step 1: Write failing tests** `count_respects_filter_and_pinned`, `clear_filtered_text_keeps_images_and_pinned`, `clear_filtered_include_pinned_removes_pinned`, `clear_filtered_pinned_tab_only_removes_pinned`, `clear_filtered_all_matches_clear`, `clear_filtered_removes_orphan_blobs` (insert an image via `png_bytes`, clear Images, assert `count_files(blobs)`/`thumbs` is 0). Assertions use exact counts, e.g. 2 text + 1 html + 1 image + 1 pinned text: `count(Filter::Text, false) == 2`, `count(Filter::Text, true) == 3`+html as set up; after `clear_filtered(Filter::Text, false)` the image and pinned text remain.
- [ ] **Step 2: Run** `cargo test -p clipsy-core clear_filtered count_respects` — Expected: FAIL (methods not defined).
- [ ] **Step 3: Implement** `filter_sql`, `count`, `clear_filtered` as specified; make `list()` call `filter_sql`.
- [ ] **Step 4: Run** `cargo test -p clipsy-core` — Expected: PASS.
- [ ] **Step 5: Commit** `git add crates/clipsy-core/src/store.rs && git commit -m "feat(store): filter-scoped count and clear"`

### Task 2: Shared confirm dialog

**Files:**
- Create: `crates/clipsy-app/src/clear_dialog.rs`
- Modify: `crates/clipsy-app/src/main.rs` (or wherever modules are declared) to add `mod clear_dialog;`

**Interfaces:**
- Consumes: `Store::count`, `Store::clear_filtered`, `State` (`state.store: RefCell<Store>`), `Filter`.
- Produces: `pub fn confirm(parent: &impl IsA<gtk::Window>, state: Rc<State>, filter: Filter, on_done: impl Fn() + 'static)` and `pub fn label_for(filter: Filter) -> &'static str` returning "items", "text items", "images", "files", "pinned items" for All/Text/Images/Files/Pinned.

- [ ] **Step 1: Write failing test** `label_for_covers_every_filter` in `clear_dialog.rs` asserting the five labels above.
- [ ] **Step 2: Run** `cargo test -p clipsy-app label_for` — Expected: FAIL.
- [ ] **Step 3: Implement** `confirm` with `adw::MessageDialog` (heading "Clear {label}?", body "This removes all N {label} and cannot be undone." with N from `count(filter, include_pinned)`), responses "cancel" (default, close) and "clear" (Destructive). Add a `gtk::CheckButton` labelled "Also remove pinned items", unchecked, set as the dialog's extra child; hide it when `filter == Filter::Pinned`; on toggle, update the body count. If the count is 0 show body "Nothing to clear." and omit the "clear" response. On "clear": call `clear_filtered`, log failures with `tracing::warn!("could not clear history: {err:#}")`, then `on_done()`.
- [ ] **Step 4: Run** `cargo test -p clipsy-app` and `cargo clippy --workspace --all-targets -- -D warnings` — Expected: PASS.
- [ ] **Step 5: Commit** `git commit -am "feat(app): shared clear confirm dialog"` (add the new file first).

### Task 3: Trash button in the popup tab row

**Files:**
- Modify: `crates/clipsy-app/src/popup.rs` (chips box construction :122-148; `Popup` struct :55-75; `reload` :388-433)

**Interfaces:**
- Consumes: `clear_dialog::confirm(parent, state, filter, on_done)`, `filter_for(index) -> Filter`, `self.filter_index`.
- Produces: `clear_btn: gtk::Button` field (icon `user-trash-symbolic`, tooltip "Clear this tab", css class `flat`), placed at the right end of the chips row (expanding spacer before it).

- [ ] **Step 1: Implement** the button; on click call `clear_dialog::confirm(&self.window, self.state.clone(), filter_for(self.filter_index.get()), move || popup.reload(Focus::Top))`.
- [ ] **Step 2:** In `reload`, set `clear_btn.set_sensitive(!items.is_empty() || search is empty ...)` — simpler: sensitive when `store.count(filter, true) > 0`.
- [ ] **Step 3:** Ensure the popup does not hide while the dialog is open: check how `seen_active` / focus-out hiding works in popup.rs and suppress hide while a `clear_dialog` is open (e.g. a `Cell<bool>` set around the dialog).
- [ ] **Step 4: Run** `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- [ ] **Step 5: Manual check** `cargo run -p clipsy-app` then open the popup: on Text tab press 🗑, confirm; only text disappears; pinned stay unless the box is ticked; ★ tab hides the checkbox.
- [ ] **Step 6: Commit** `git commit -am "feat(popup): per-tab clear button"`

### Task 4: Settings uses the shared dialog

**Files:**
- Modify: `crates/clipsy-app/src/settings.rs` (`confirm_clear` :686-710)

**Interfaces:**
- Consumes: `clear_dialog::confirm(window, state, Filter::All, on_done)`.

- [ ] **Step 1:** Replace the body of `confirm_clear` with a call to `clear_dialog::confirm(window, state, Filter::All, move || window.add_toast(adw::Toast::new("History cleared")))`. Remove the old unconditional `clear(false)`.
- [ ] **Step 2: Run** `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`.
- [ ] **Step 3: Commit** `git commit -am "feat(settings): clear history uses shared dialog"`

### Task 5: Bigger popup and one-time saved-size reset

**Files:**
- Modify: `crates/clipsy-app/src/popup.rs` (`MIN_SIZE` :991, `set_size_request` :93, `load_saved_size` :999-1010, tests ~:1079+), `crates/clipsy-app/src/placement.rs:15`

**Interfaces:**
- Produces: `fn parse_size(s: &str) -> Option<(i32, i32)>` (parses "WxH", clamps to MIN/MAX; None on garbage); `const DEFAULT_SIZE: (i32, i32) = (440, 560)` used by both popup.rs and placement.rs; `MIN_SIZE = (380, 260)`.
- Reset rule: if marker file `data_dir()/popup-size-v2` is absent, ignore the saved size, create the marker, return `DEFAULT_SIZE`.

- [ ] **Step 1: Write failing tests** `parse_size_accepts_wxh` (`"500x600"` → `Some((500,600))`), `parse_size_clamps_to_min` (`"100x100"` → `Some((380,260))`), `parse_size_rejects_garbage` (`"abc"` → `None`).
- [ ] **Step 2: Run** `cargo test -p clipsy-app parse_size` — Expected: FAIL.
- [ ] **Step 3: Implement** `parse_size`, the shared `DEFAULT_SIZE`, `set_size_request(MIN_SIZE.0, MIN_SIZE.1)`, and the marker-based reset in `load_saved_size`.
- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` — Expected: PASS.
- [ ] **Step 5: Manual check** `cargo run -p clipsy-app`; popup opens at ~440×560 even though an old `~/.local/share/clipsy/popup-size` exists.
- [ ] **Step 6: Commit** `git commit -am "feat(popup): 440x560 default, reset old saved size once"`

### Task 6: Welcome carousel

**Files:**
- Modify: `crates/clipsy-app/src/welcome.rs` (`show` :83-208, `finish_welcome` :309)

**Interfaces:**
- Consumes: existing `shortcut_row`/keycaps helpers, `gnome_setup::setup_box(state)`, `autostart::set_enabled`, `update_config`, `finish_welcome(window)`, `mark_done()`.
- Produces: `adw::Window` 480×600 (min 420×520) containing an `adw::Carousel` of 4 pages plus `adw::CarouselIndicatorDots`, a "Back" button and a "Next" button (last page: pill "Start using Clipsy" → `finish_welcome`). Pages: (1) icon, "Welcome to Clipsy", tagline; (2) "Open it anywhere": shortcut keycaps + Change button + GNOME setup box when present; (3) "Private by default": short copy on local-only history and clearing per tab with 🗑; (4) "Make it yours": "Start at login" and "Paste after selecting" switch rows. Closing the window still calls `mark_done()` (existing close_request).

- [ ] **Step 1:** Rebuild `show()` as above reusing the existing widgets; keep `accel_to_keycaps` tests untouched.
- [ ] **Step 2: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` — Expected: PASS.
- [ ] **Step 3: Manual check** `rm ~/.local/share/clipsy/welcome-done && cargo run -p clipsy-app`; welcome opens at 480×600, Next/Back/dots work, toggles persist, closing or finishing recreates `welcome-done`.
- [ ] **Step 4: Commit** `git commit -am "feat(welcome): multi-page carousel"`

### Task 7: Docs sync

**Files:**
- Modify: `docs/PLAN.md` (§8.1 popup size → 440×560; add the per-tab clear button to the layout line; §8.3 welcome described as the 4-page carousel)

- [ ] **Step 1:** Make those three edits.
- [ ] **Step 2: Commit** `git commit -am "docs: sync PLAN with clear button, popup size, welcome"`
