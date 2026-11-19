//! Popup state: the search query, active filter, visible items and selection.
//!
//! This type holds no store or UI handles. Typing, backspace and filter changes only update
//! the state; the UI layer re-queries the store and calls `set_items` with the result.

use clipsy_backends::PasteMode;
use clipsy_core::{ClipItem, Filter, Kind};

pub struct PopupState {
    pub query: String,
    pub filter: Filter,
    pub items: Vec<ClipItem>,
    pub selected: usize,
}

impl Default for PopupState {
    fn default() -> Self {
        Self::new()
    }
}

impl PopupState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            filter: Filter::All,
            items: Vec::new(),
            selected: 0,
        }
    }

    /// Replace the visible items and clamp the selection into range.
    pub fn set_items(&mut self, items: Vec<ClipItem>) {
        self.items = items;
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
    }

    /// Move the selection by `delta`, wrapping at both ends. Does nothing on an empty list.
    pub fn move_selection(&mut self, delta: i32) {
        let len = self.items.len() as i64;
        if len == 0 {
            self.selected = 0;
            return;
        }
        let next = (self.selected as i64 + i64::from(delta)).rem_euclid(len);
        self.selected = next as usize;
    }

    pub fn selected_item(&self) -> Option<&ClipItem> {
        self.items.get(self.selected)
    }

    /// Change the tab filter. The selection goes back to the first row.
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
        self.selected = 0;
    }

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Remove the last character of the query. No-op when the query is empty.
    pub fn backspace(&mut self) {
        self.query.pop();
    }
}

#[cfg(test)]
mod delete_selection_tests {
    use super::selection_after_delete;

    #[test]
    fn selection_after_delete_single_item_goes_to_zero() {
        assert_eq!(selection_after_delete(0, 1), 0);
    }

    #[test]
    fn selection_after_delete_last_row_moves_up() {
        assert_eq!(selection_after_delete(4, 5), 3);
    }

    #[test]
    fn selection_after_delete_middle_stays() {
        assert_eq!(selection_after_delete(1, 5), 1);
    }

    #[test]
    fn selection_after_delete_empty_is_zero() {
        assert_eq!(selection_after_delete(0, 0), 0);
    }
}

/// Selection index after the row at `selected` is deleted from a list of `len_before` rows.
///
/// Keeps the same row position when possible, otherwise moves up to the new last row. An empty
/// result list gives 0.
pub fn selection_after_delete(selected: usize, len_before: usize) -> usize {
    if len_before <= 1 {
        0
    } else {
        selected.min(len_before - 2)
    }
}

/// Paste mode for the row the user activated. Image rows always use `Binary` (plain Ctrl+V even
/// in a terminal, so the target reads the image), also with `plain` (Shift+Enter). Other rows use
/// `Normal`.
pub fn paste_mode_for(kind: Kind, _plain: bool) -> PasteMode {
    if kind == Kind::Image {
        PasteMode::Binary
    } else {
        PasteMode::Normal
    }
}

/// Move a popup's top-left corner so the whole popup lies inside `area`.
///
/// `area` is (left, top, right, bottom). When the popup is larger than the area, it is aligned
/// to the area's left and top edges.
pub fn clamp_to_area(pos: (i32, i32), size: (i32, i32), area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (left, top, right, bottom) = area;
    let (width, height) = size;
    // Largest top-left that still keeps the popup's right/bottom edge inside the area. `min`
    // then `max` means an oversized popup (max < left/top) ends up at the left/top edge.
    let x = pos.0.min(right - width).max(left);
    let y = pos.1.min(bottom - height).max(top);
    (x, y)
}

/// Smallest popup size in logical pixels. Keep in step with the grip logic in src/popup.rs.
pub const POPUP_MIN_SIZE: (f32, f32) = (320.0, 300.0);
/// Largest popup size in logical pixels.
pub const POPUP_MAX_SIZE: (f32, f32) = (900.0, 1000.0);

/// Keep a popup size inside `POPUP_MIN_SIZE` and `POPUP_MAX_SIZE`, each side on its own.
pub fn clamp_popup_size(width: f32, height: f32) -> (f32, f32) {
    (
        width.clamp(POPUP_MIN_SIZE.0, POPUP_MAX_SIZE.0),
        height.clamp(POPUP_MIN_SIZE.1, POPUP_MAX_SIZE.1),
    )
}

/// Popup size during a resize-grip drag: the size where the drag started, plus how far the
/// cursor has moved since then (screen pixels, converted to logical with `scale`), clamped.
/// Measuring from the start, not step by step, means a drag past a limit comes back as soon as
/// the cursor comes back.
pub fn drag_popup_size(start: (f32, f32), cursor_start: (i32, i32), cursor_now: (i32, i32), scale: f32) -> (f32, f32) {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let dx = (cursor_now.0 - cursor_start.0) as f32 / scale;
    let dy = (cursor_now.1 - cursor_start.1) as f32 / scale;
    clamp_popup_size(start.0 + dx, start.1 + dy)
}

/// Cap a popup size (physical px) to the size of the work area, so the popup cannot be bigger
/// than the screen area it has to fit in. `area` is (left, top, right, bottom). Each side is at
/// least 1, so the result is never a zero-sized window.
///
/// Minimum precedence: the 320x300 minimum is applied to the user's size (`clamp_popup_size`)
/// before this runs. When the work area is smaller than the minimum, the work area wins, so the
/// popup stays on screen.
pub fn fit_size_to_area(size: (i32, i32), area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (left, top, right, bottom) = area;
    (size.0.min(right - left).max(1), size.1.min(bottom - top).max(1))
}

/// Size and top-left for the window during a resize-grip drag (physical px).
///
/// The size is capped to the work area first. Then, if the origin is inside the area, the window
/// is not moved; the size is capped to the room left from the origin, so the bottom and right
/// edges stay on screen and the top-left does not creep while the drag goes on and back. If the
/// origin is outside the area, the window is first moved inside (`clamp_to_area`), and the room
/// is taken from the moved position. Each side is at least 1. The work area wins over the
/// minimum, as in `fit_size_to_area`.
pub fn drag_window_fit(
    wanted: (i32, i32),
    origin: (i32, i32),
    area: (i32, i32, i32, i32),
) -> ((i32, i32), (i32, i32)) {
    let (left, top, right, bottom) = area;
    let size = fit_size_to_area(wanted, area);
    let inside = origin.0 >= left && origin.0 < right && origin.1 >= top && origin.1 < bottom;
    let pos = if inside { origin } else { clamp_to_area(origin, size, area) };
    let size = (
        size.0.min(right - pos.0).max(1),
        size.1.min(bottom - pos.1).max(1),
    );
    (size, pos)
}

/// How many items a clear would remove. Pinned items are kept unless `also_pinned` is true.
pub fn clearable_count(total: usize, pinned: usize, also_pinned: bool) -> usize {
    if also_pinned {
        total
    } else {
        total.saturating_sub(pinned)
    }
}

/// The question shown in the clear-history overlay for the given counts.
///
/// `total` is every item in history, `pinned` how many of them are pinned.
/// `(0, 0, _)` gives "Nothing to clear.". With pinned items kept, the message says how many
/// are removed and how many stay.
pub fn clear_message(total: usize, pinned: usize, also_pinned: bool) -> String {
    if total == 0 {
        return "Nothing to clear.".to_string();
    }
    let noun = |n: usize| if n == 1 { "item" } else { "items" };
    if also_pinned {
        return if pinned == 0 {
            format!("Remove all {total} {}?", noun(total))
        } else {
            format!("Remove all {total} {}, including {pinned} pinned?", noun(total))
        };
    }
    let removable = clearable_count(total, pinned, false);
    if removable == 0 {
        return format!("Nothing to clear. {pinned} pinned {} will be kept.", noun(pinned));
    }
    if pinned == 0 {
        format!("Remove {removable} {}?", noun(removable))
    } else {
        format!(
            "Remove {removable} {}? {pinned} pinned {} will be kept.",
            noun(removable),
            noun(pinned)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clipsy_backends::PasteMode;
    use clipsy_core::Kind;

    #[test]
    fn image_rows_use_binary_paste_mode() {
        assert_eq!(paste_mode_for(Kind::Image, false), PasteMode::Binary);
        assert_eq!(paste_mode_for(Kind::Image, true), PasteMode::Binary);
        assert_eq!(paste_mode_for(Kind::Text, false), PasteMode::Normal);
        assert_eq!(paste_mode_for(Kind::Files, false), PasteMode::Normal);
    }

    fn item(id: i64) -> ClipItem {
        ClipItem {
            id,
            kind: Kind::Text,
            preview: format!("item {id}"),
            source_app: None,
            byte_size: 0,
            width: None,
            height: None,
            pinned: false,
            created_at: 0,
            last_used_at: 0,
            thumb_path: None,
        }
    }

    fn items(n: i64) -> Vec<ClipItem> {
        (1..=n).map(item).collect()
    }

    #[test]
    fn selection_wraps_both_directions() {
        let mut s = PopupState::new();
        s.set_items(items(3));
        assert_eq!(s.selected, 0);
        s.move_selection(-1);
        assert_eq!(s.selected, 2);
        s.move_selection(1);
        assert_eq!(s.selected, 0);
        s.move_selection(1);
        assert_eq!(s.selected, 1);
        assert_eq!(s.selected_item().map(|i| i.id), Some(2));
    }

    #[test]
    fn empty_list_selection_is_none_and_move_does_not_panic() {
        let mut s = PopupState::new();
        assert!(s.selected_item().is_none());
        s.move_selection(1);
        s.move_selection(-1);
        assert_eq!(s.selected, 0);
        assert!(s.selected_item().is_none());
    }

    #[test]
    fn set_items_clamps_selected() {
        let mut s = PopupState::new();
        s.set_items(items(5));
        s.move_selection(4);
        assert_eq!(s.selected, 4);
        s.set_items(items(2));
        assert_eq!(s.selected, 1);
        s.set_items(Vec::new());
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn typing_updates_query() {
        let mut s = PopupState::new();
        s.set_items(items(2));
        s.type_char('a');
        s.type_char('b');
        assert_eq!(s.query, "ab");
        // Typing does not fetch: the items stay as the UI last set them.
        assert_eq!(s.items.len(), 2);
    }

    #[test]
    fn backspace_on_empty_query_is_noop() {
        let mut s = PopupState::new();
        s.backspace();
        assert_eq!(s.query, "");
        s.type_char('x');
        s.backspace();
        assert_eq!(s.query, "");
        s.backspace();
        assert_eq!(s.query, "");
    }

    #[test]
    fn filter_change_resets_selection() {
        let mut s = PopupState::new();
        assert_eq!(s.filter, Filter::All);
        s.set_items(items(4));
        s.move_selection(2);
        assert_eq!(s.selected, 2);
        s.set_filter(Filter::Text);
        assert_eq!(s.filter, Filter::Text);
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn clamp_keeps_inside_right_bottom() {
        // Popup 420x520 near the bottom-right corner of a 1000x800 work area.
        let got = clamp_to_area((900, 700), (420, 520), (0, 0, 1000, 800));
        assert_eq!(got, (580, 280));
    }

    #[test]
    fn clamp_keeps_inside_left_top() {
        let got = clamp_to_area((-50, -20), (420, 520), (0, 0, 1000, 800));
        assert_eq!(got, (0, 0));
    }

    #[test]
    fn clamp_leaves_inside_position_unchanged() {
        let got = clamp_to_area((100, 200), (420, 520), (0, 0, 1000, 800));
        assert_eq!(got, (100, 200));
    }

    #[test]
    fn clamp_oversized_popup_aligns_to_top_left() {
        assert_eq!(clamp_to_area((50, 50), (420, 520), (0, 0, 300, 200)), (0, 0));
        assert_eq!(clamp_to_area((500, 500), (420, 520), (10, 20, 310, 220)), (10, 20));
    }

    #[test]
    fn clear_message_nothing_when_empty() {
        assert_eq!(clear_message(0, 0, false), "Nothing to clear.");
        assert_eq!(clear_message(0, 0, true), "Nothing to clear.");
    }

    #[test]
    fn clear_message_keeps_pinned_by_default() {
        assert_eq!(clear_message(12, 3, false), "Remove 9 items? 3 pinned items will be kept.");
    }

    #[test]
    fn clear_message_including_pinned() {
        assert_eq!(clear_message(12, 3, true), "Remove all 12 items, including 3 pinned?");
    }

    #[test]
    fn clear_message_singular_for_one_item() {
        assert_eq!(clear_message(1, 0, false), "Remove 1 item?");
        assert_eq!(clear_message(1, 1, true), "Remove all 1 item, including 1 pinned?");
    }

    #[test]
    fn clear_message_only_pinned_nothing_removable() {
        assert_eq!(clear_message(3, 3, false), "Nothing to clear. 3 pinned items will be kept.");
    }

    #[test]
    fn clearable_count_follows_keep_pinned() {
        assert_eq!(clearable_count(12, 3, false), 9);
        assert_eq!(clearable_count(12, 3, true), 12);
        assert_eq!(clearable_count(0, 0, false), 0);
    }

    #[test]
    fn popup_size_clamps_to_min_and_max() {
        assert_eq!(clamp_popup_size(100.0, 100.0), (320.0, 300.0));
        assert_eq!(clamp_popup_size(5000.0, 5000.0), (900.0, 1000.0));
        assert_eq!(clamp_popup_size(420.0, 520.0), (420.0, 520.0));
    }

    #[test]
    fn popup_size_clamps_each_side_separately() {
        assert_eq!(clamp_popup_size(200.0, 800.0), (320.0, 800.0));
        assert_eq!(clamp_popup_size(700.0, 2000.0), (700.0, 1000.0));
    }

    #[test]
    fn drag_size_is_start_plus_cursor_movement() {
        // Cursor moves 50 px right and 20 px up, so the popup grows by the same amount.
        assert_eq!(drag_popup_size((420.0, 520.0), (100, 100), (150, 80), 1.0), (470.0, 500.0));
    }

    #[test]
    fn drag_size_converts_physical_cursor_movement_to_logical() {
        // At 200% scale, 100 physical px of cursor movement is 50 logical px.
        assert_eq!(drag_popup_size((420.0, 520.0), (0, 0), (100, 100), 2.0), (470.0, 570.0));
    }

    #[test]
    fn drag_size_recovers_as_soon_as_pointer_comes_back_from_the_limit() {
        // Overshooting the maximum does not lose the anchor: moving back in brings the size back.
        let start = (420.0, 520.0);
        assert_eq!(drag_popup_size(start, (0, 0), (1000, 0), 1.0).0, 900.0);
        assert_eq!(drag_popup_size(start, (0, 0), (400, 0), 1.0).0, 820.0);
        // Same for the minimum: overshoot down, then come back.
        assert_eq!(drag_popup_size(start, (0, 0), (-1000, 0), 1.0).0, 320.0);
        assert_eq!(drag_popup_size(start, (0, 0), (-100, 0), 1.0).0, 320.0);
        assert_eq!(drag_popup_size(start, (0, 0), (-10, 0), 1.0).0, 410.0);
    }

    #[test]
    fn fit_size_caps_each_side_to_the_work_area() {
        // Work area 1920x1040 at the origin.
        assert_eq!(fit_size_to_area((1000, 1200), (0, 0, 1920, 1040)), (1000, 1040));
        assert_eq!(fit_size_to_area((2500, 500), (0, 0, 1920, 1040)), (1920, 500));
        assert_eq!(fit_size_to_area((600, 400), (0, 0, 1920, 1040)), (600, 400));
    }

    #[test]
    fn fit_size_handles_a_negative_origin_work_area() {
        // A monitor to the left of the primary one has negative x; its work area is (-1920, -100, 0, 1040).
        assert_eq!(fit_size_to_area((2500, 2000), (-1920, -100, 0, 1040)), (1920, 1140));
        assert_eq!(fit_size_to_area((600, 400), (-1920, -100, 0, 1040)), (600, 400));
    }

    #[test]
    fn remembered_size_is_not_shrunk_by_a_capped_drag() {
        // The user drags to 420x800. A low spot on screen only leaves 200 px, so the window is
        // capped, but the remembered size stays 800 tall and the next show uses it again.
        let remembered = drag_popup_size((420.0, 520.0), (0, 0), (0, 280), 1.0);
        assert_eq!(remembered, (420.0, 800.0));
        let low = (0, 0, 1920, 200);
        assert_eq!(fit_size_to_area((420, 800), low), (420, 200));
        assert_eq!(fit_size_to_area((420, 800), (0, 0, 1920, 1040)), (420, 800));
    }

    #[test]
    fn drag_window_fit_never_gives_a_zero_size() {
        // Origin far beyond the bottom-right corner: the window is pulled back on screen.
        let (size, pos) = drag_window_fit((420, 520), (5000, 5000), (0, 0, 1920, 1040));
        assert!(size.0 >= 1 && size.1 >= 1);
        assert!(pos.0 + size.0 <= 1920 && pos.1 + size.1 <= 1040);
        // A zero-sized work area still gives at least 1x1.
        let (size, _) = drag_window_fit((420, 520), (0, 0), (0, 0, 0, 0));
        assert_eq!(size, (1, 1));
    }

    #[test]
    fn drag_window_fit_keeps_the_minimum_when_there_is_room() {
        let (size, pos) = drag_window_fit((320, 300), (100, 100), (0, 0, 1920, 1040));
        assert_eq!((size, pos), ((320, 300), (100, 100)));
    }

    #[test]
    fn drag_window_fit_lets_a_smaller_area_win_over_the_minimum() {
        // The work area is 300x200, smaller than the 320x300 minimum: the area wins.
        let (size, pos) = drag_window_fit((320, 300), (0, 0), (0, 0, 300, 200));
        assert_eq!((size, pos), ((300, 200), (0, 0)));
    }

    #[test]
    fn drag_window_fit_does_not_move_a_window_inside_the_area() {
        // The bottom edge is near the work area bottom: the size is capped, the top-left stays.
        let (size, pos) = drag_window_fit((420, 520), (100, 900), (0, 0, 1920, 1040));
        assert_eq!((size, pos), ((420, 140), (100, 900)));
    }

    #[test]
    fn drag_window_fit_pulls_an_origin_off_the_top_left_back_in_before_capping() {
        // Origin left of and above the area: it is clamped first, so the window does not overflow.
        let (size, pos) = drag_window_fit((1200, 1200), (-500, -500), (0, 0, 1920, 1040));
        assert_eq!(pos, (0, 0));
        assert_eq!(size, (1200, 1040));
    }

    #[test]
    fn drag_window_fit_handles_a_negative_origin_work_area() {
        // Secondary monitor to the left: work area (-1920, -100, 0, 1040), origin (-1000, 100).
        let (size, pos) = drag_window_fit((1200, 1200), (-1000, 100), (-1920, -100, 0, 1040));
        assert_eq!((size, pos), ((1000, 940), (-1000, 100)));
    }
}
