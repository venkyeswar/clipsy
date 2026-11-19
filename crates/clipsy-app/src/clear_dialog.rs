//! Shared confirm dialog for clearing history (used by the popup and Settings).
//!
//! The dialog counts the items in the current tab and asks before deleting
//! them. A checkbox lets the user also remove pinned items (hidden on the
//! Pinned tab, where pinned items are the whole list).

use std::rc::Rc;

use adw::prelude::*;
use clipsy_core::Filter;

use crate::app::State;

/// Plural noun used in the heading and body for each tab.
pub fn label_for(filter: Filter) -> &'static str {
    match filter {
        Filter::All => "items",
        Filter::Text => "text items",
        Filter::Images => "images",
        Filter::Files => "files",
        Filter::Pinned => "pinned items",
    }
}

/// Number of items the clear would remove. A failed query is logged and counts as 0.
fn count_items(state: &State, filter: Filter, include_pinned: bool) -> i64 {
    match state.store.borrow().count(filter, include_pinned) {
        Ok(n) => n,
        Err(err) => {
            tracing::warn!("could not count history: {err:#}");
            0
        }
    }
}

/// Updates the body text and the "clear" response for the current count.
/// With nothing to clear there is no "clear" response, only the close button.
fn show_count(dialog: &adw::MessageDialog, label: &str, count: i64) {
    if count == 0 {
        dialog.set_body("Nothing to clear.");
        if dialog.has_response("clear") {
            dialog.remove_response("clear");
        }
    } else {
        dialog.set_body(&format!("This removes all {count} {label} and cannot be undone."));
        if !dialog.has_response("clear") {
            dialog.add_response("clear", "Clear");
            dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
        }
    }
}

/// Asks the user to confirm clearing `filter`, then clears it and calls `on_done`.
///
/// Only the "cancel" response and (when there is something to clear) "clear" are
/// offered. `on_done` runs after a clear, whether or not the database call worked.
/// Returns the dialog so the caller can watch when it closes (for example, to keep
/// the popup open while it is up). Connecting to it is optional.
pub fn confirm(
    parent: &impl IsA<gtk::Window>,
    state: Rc<State>,
    filter: Filter,
    on_done: impl Fn() + 'static,
) -> adw::MessageDialog {
    let label = label_for(filter);
    let heading = format!("Clear {label}?");
    let dialog = adw::MessageDialog::new(Some(parent), Some(&heading), None);
    dialog.add_response("cancel", "Cancel");
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    // The checkbox is hidden on the Pinned tab: there, pinned items are already the scope.
    let check = gtk::CheckButton::with_label("Also remove pinned items");
    let show_checkbox = !matches!(filter, Filter::Pinned);
    check.set_visible(show_checkbox);
    dialog.set_extra_child(Some(&check));

    show_count(&dialog, label, count_items(&state, filter, include_pinned(&check, filter)));

    // Recount whenever the checkbox changes. The dialog is held weakly so the
    // checkbox (owned by the dialog) does not keep it alive in a cycle.
    let (weak, st) = (dialog.downgrade(), state.clone());
    check.connect_toggled(move |c| {
        if let Some(dialog) = weak.upgrade() {
            let count = count_items(&st, filter, include_pinned(c, filter));
            show_count(&dialog, label, count);
        }
    });

    let st = state.clone();
    let check_for_response = check.clone();
    dialog.connect_response(None, move |_, response| {
        if response != "clear" {
            return;
        }
        let include_pinned = include_pinned(&check_for_response, filter);
        let result = st.store.borrow().clear_filtered(filter, include_pinned);
        if let Err(err) = result {
            tracing::warn!("could not clear history: {err:#}");
        }
        on_done();
    });

    dialog.present();
    dialog
}

/// Whether pinned items should be removed too. Always true on the Pinned tab,
/// because the Pinned filter ignores this flag anyway.
fn include_pinned(check: &gtk::CheckButton, filter: Filter) -> bool {
    matches!(filter, Filter::Pinned) || check.is_active()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_for_covers_every_filter() {
        assert_eq!(label_for(Filter::All), "items");
        assert_eq!(label_for(Filter::Text), "text items");
        assert_eq!(label_for(Filter::Images), "images");
        assert_eq!(label_for(Filter::Files), "files");
        assert_eq!(label_for(Filter::Pinned), "pinned items");
    }
}
