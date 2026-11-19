//! The confirm dialog for clearing history.
//!
//! Every clear path (the popup's Clear button, the tray's "Clear history", the clear-history
//! hotkey and the Settings button) goes through `confirm_and_clear`, so the user sees the counts
//! and confirms before anything is deleted. The message text is pure and tested on every OS;
//! the window itself is Windows only.

/// How many items a clear would remove. Pinned items are kept unless `also_pinned` is true.
pub fn clearable_count(total: usize, pinned: usize, also_pinned: bool) -> usize {
    if also_pinned {
        total
    } else {
        total.saturating_sub(pinned)
    }
}

/// The question shown in the dialog for the given counts.
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

#[cfg(windows)]
pub use window::confirm_and_clear;

/// The Slint dialog and the glue that clears the store when the user confirms (Windows only).
#[cfg(windows)]
mod window {
    use std::cell::RefCell;
    use std::rc::Rc;

    use clipsy_core::{Filter, Store};
    use slint::ComponentHandle;

    use super::{clear_message, clearable_count};
    use crate::ui::ClearDialog;

    thread_local! {
        /// The dialog being shown. Kept here so the window lives until the user answers it.
        static OPEN: RefCell<Option<ClearDialog>> = const { RefCell::new(None) };
    }

    /// Opens the confirm dialog. When the user clicks Clear, `Store::clear` removes the items
    /// (pinned ones only when "Also remove pinned" is ticked) and then `on_done` runs. Cancel
    /// or closing the window changes nothing.
    pub fn confirm_and_clear(store: Rc<RefCell<Store>>, on_done: impl Fn() + 'static) {
        let dialog = match ClearDialog::new() {
            Ok(dialog) => dialog,
            Err(e) => {
                tracing::warn!("could not create the clear dialog: {e}");
                return;
            }
        };
        // A failed count is logged and counts as nothing, so the dialog says "Nothing to clear.".
        let (total, pinned) = {
            let store = store.borrow();
            let count = |filter, label| match store.count(filter, true) {
                Ok(n) => usize::try_from(n).unwrap_or(0),
                Err(e) => {
                    tracing::warn!("could not count {label}: {e:#}");
                    0
                }
            };
            (count(Filter::All, "history"), count(Filter::Pinned, "pinned items"))
        };
        refresh(&dialog, total, pinned, false);

        let on_done: Rc<dyn Fn()> = Rc::new(on_done);

        // Weak handles: the dialog owns these closures, so a strong handle would form a cycle.
        let weak = dialog.as_weak();
        dialog.on_also_pinned_changed(move |also| {
            if let Some(dialog) = weak.upgrade() {
                refresh(&dialog, total, pinned, also);
            }
        });

        let weak = dialog.as_weak();
        let clear_store = store.clone();
        let clear_done = on_done.clone();
        dialog.on_clear_requested(move || {
            let Some(dialog) = weak.upgrade() else {
                return;
            };
            let also_pinned = dialog.get_also_pinned();
            // Store::clear takes keep_pinned, the opposite of the checkbox.
            let result = clear_store.borrow().clear(!also_pinned);
            if let Err(e) = result {
                tracing::warn!("could not clear history: {e:#}");
            }
            hide(&dialog);
            clear_done();
        });

        let weak = dialog.as_weak();
        dialog.on_cancel_requested(move || {
            if let Some(dialog) = weak.upgrade() {
                hide(&dialog);
            }
        });

        if let Err(e) = dialog.show() {
            tracing::warn!("could not show the clear dialog: {e}");
            return;
        }
        // Replacing an earlier dialog drops it. It is already answered or hidden by now.
        OPEN.with(|slot| *slot.borrow_mut() = Some(dialog));
    }

    /// Updates the question and whether Clear is allowed for the current checkbox state.
    fn refresh(dialog: &ClearDialog, total: usize, pinned: usize, also_pinned: bool) {
        dialog.set_also_pinned(also_pinned);
        dialog.set_message(clear_message(total, pinned, also_pinned).into());
        dialog.set_can_clear(clearable_count(total, pinned, also_pinned) > 0);
    }

    fn hide(dialog: &ClearDialog) {
        if let Err(e) = dialog.hide() {
            tracing::warn!("could not hide the clear dialog: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
