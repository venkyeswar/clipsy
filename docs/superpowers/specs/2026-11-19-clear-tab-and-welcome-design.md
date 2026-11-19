# Clear-per-tab, bigger popup, new welcome — short design

## Intent
Users want an obvious way to clear clipboard history from the popup, per filter tab, without losing pinned items by accident. The popup is also too narrow (~366px in practice) and the first-run welcome window is too small/plain.

## Goals
- A Clear (trash) button at the right end of the filter-tab row, visible on every tab.
- Clear affects only the current tab's items: All = everything, Text = text only, Images = images only, Files = files only, ★ = pinned only.
- Confirm dialog shows the count and a checkbox "Also remove pinned items", unchecked by default. On the ★ tab the checkbox is hidden (the tab is already pinned-only) and the dialog says it removes pinned items.
- Settings → Privacy "Clear…" reuses the same dialog (All scope) instead of calling clear(false) with no choice.
- Default popup size 440×560, minimum width 380; reset the previously saved narrow size once.
- Welcome window 480×600 with 3–4 pages: welcome, how to open (Super+V + shortcut fix), privacy, launch-at-login toggle. Still shown once on first run.

## Non-goals
- No undo, no new hotkeys, no changes to retention/pruning, no GNOME extension changes (it never sets window size).

## Anti-goals
- Accidentally deleting pinned items; a clear button that's easy to hit by mistake; clearing items not visible in the current tab.

## Constraints
- Rust + GTK4 (4.14) / libadwaita 1.5, X11 dev machine. Run `cargo clippy -- -D warnings` and `cargo test` before calling done.
- Existing code: store.rs `clear(keep_pinned)` (:310) clears everything; popup.rs tabs at :122-148, `filter_for` :863, size at :92 / `load_saved_size()`; welcome.rs `show` (:83); settings.rs `confirm_clear` (:686); placement.rs DEFAULT_SIZE (:15, currently out of sync).

## Approaches
1. (Chosen) Trash icon at the end of the tab row + confirm dialog with checkbox; new store method scoped by kind and pinned flag.
2. Clear button in the footer hint bar — rejected: crowds the keyboard hints and is far from the tabs it affects.
3. Clear only inside Settings — rejected: this is what exists today and is not discoverable.

## Decisions made with the user
- Clear is per-tab (user confirmed): only the current tab's contents are deleted.
- Checkbox "Also remove pinned items" unchecked by default.

## Calls I made (user can override)
- Trash icon placement at the right end of the tab row; confirm dialog rather than instant delete.
- Popup 440×560, min width 380; welcome 480×600 carousel with 3–4 pages.
- Hide the checkbox on the ★ tab.
- Fix placement.rs DEFAULT_SIZE to match the new popup size.

## Left to the builder
- New store method (e.g. `clear_filtered(kind, include_pinned)`) and tests; exact kind column/values; icon choice; page copy; how to reset the saved popup-size once (e.g. versioned marker).
