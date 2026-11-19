//! Popup state: the search query, active filter, visible items and selection.
//!
//! This type holds no store or UI handles. Typing, backspace and filter changes only update
//! the state; the UI layer re-queries the store and calls `set_items` with the result.

use clipsy_core::{ClipItem, Filter};

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

#[cfg(test)]
mod tests {
    use super::*;
    use clipsy_core::Kind;

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
}
