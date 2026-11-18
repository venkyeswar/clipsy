//! The popup: the window that shows the clipboard history.
//!
//! Layout, top to bottom: search box and settings button, text-tab filters, a
//! "paused" banner, the history list (or an empty-state page) and a one-line
//! footer that lists the main shortcuts.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use adw::prelude::*;
use clipsy_backends::PasteMode;
use clipsy_core::{ClipItem, Filter, Format, Kind};
use gtk::{gdk, gio, glib};

use crate::app::{pad, State};
use crate::placement;

/// How many rows the list shows at most. Search still looks through everything.
const HISTORY_LIMIT: usize = 500;

/// Filter chip labels in order. The index matches `filter_for`.
const FILTER_LABELS: [&str; 5] = ["All", "Text", "Images", "Files", "★"];

/// Time to wait for the popup to hide before sending Ctrl+V to the other window.
const PASTE_DELAY: Duration = Duration::from_millis(60);

/// Time to wait after showing the popup before moving it (see `show`).
const POSITION_DELAY: Duration = Duration::from_millis(30);

/// Time to wait after the last key press before searching (keeps typing smooth).
const SEARCH_DELAY: Duration = Duration::from_millis(80);

/// Where the selection goes after the list is rebuilt.
#[derive(Clone, Copy)]
enum Focus {
    /// The first row.
    Top,
    /// The same item as before (if it still exists), otherwise the same position.
    Keep,
    /// A specific item.
    Item(i64),
}

/// Items in the row's "..." menu.
#[derive(Clone, Copy)]
enum MenuAction {
    Paste,
    PastePlain,
    SetPinned(bool),
    CopyOnly,
    Delete,
}

pub struct Popup {
    state: Rc<State>,
    /// Pointer to ourselves, used by timers and callbacks.
    me: Weak<Popup>,
    window: gtk::Window,
    search: gtk::SearchEntry,
    chips: Vec<gtk::ToggleButton>,
    banner: adw::Banner,
    stack: gtk::Stack,
    empty: adw::StatusPage,
    list_view: gtk::ListView,
    list_store: gio::ListStore,
    selection: gtk::SingleSelection,
    filter_index: Cell<usize>,
    /// True once the window has had focus since it was shown. Used so that a
    /// window which never got focus is not hidden by the focus-loss handler.
    seen_active: Cell<bool>,
    search_timer: RefCell<Option<glib::SourceId>>,
    /// The Space-bar preview, if it is open.
    preview: RefCell<Option<gtk::Popover>>,
}

impl Popup {
    /// Builds the popup window. It starts hidden.
    pub fn new(app: &adw::Application, state: Rc<State>) -> Rc<Popup> {
        // `new_cyclic` gives us a `Weak` pointer to the popup before it exists.
        // Callbacks use it to reach the popup without keeping it alive forever.
        Rc::new_cyclic(|me: &Weak<Popup>| {
            // --- The window -------------------------------------------------
            let window = gtk::Window::new();
            window.set_application(Some(app));
            window.set_title(Some("Clipboard history"));
            window.set_decorated(false);
            // True while the user drags a resize handle. The window manager takes focus
            // for the drag, and that must not count as "clicked away".
            let resizing = Rc::new(std::cell::Cell::new(false));
            let (saved_w, saved_h) = load_saved_size();
            window.set_default_size(saved_w, saved_h);
            window.set_size_request(300, 260);
            window.set_resizable(true);
            // Closing (for example with Alt+F4) hides the window. We keep it for next time.
            window.set_hide_on_close(true);
            window.add_css_class("clipsy-popup");

            // --- Search row ---------------------------------------------------
            let search = gtk::SearchEntry::new();
            search.set_placeholder_text(Some("Search clipboard…"));
            search.set_hexpand(true);
            search.add_css_class("clipsy-search");
            let w = me.clone();
            search.connect_search_changed(move |_| {
                if let Some(p) = w.upgrade() {
                    p.schedule_reload();
                }
            });

            let gear = gtk::Button::from_icon_name("emblem-system-symbolic");
            gear.add_css_class("flat");
            gear.set_tooltip_text(Some("Settings"));
            let a = app.clone();
            gear.connect_clicked(move |_| a.activate_action("settings", None));

            let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            pad(&top, 10, 8);
            top.append(&search);
            top.append(&gear);

            // --- Filter chips ---------------------------------------------------
            // ToggleButtons in one group behave like radio buttons: one is always on.
            let chips_box = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            pad(&chips_box, 10, 2);
            let mut chips: Vec<gtk::ToggleButton> = Vec::new();
            for (index, label) in FILTER_LABELS.iter().enumerate() {
                let chip = gtk::ToggleButton::with_label(label);
                chip.add_css_class("flat");
                chip.add_css_class("clipsy-tab");
                if index + 1 == FILTER_LABELS.len() {
                    chip.set_tooltip_text(Some("Pinned"));
                }
                if let Some(first) = chips.first() {
                    chip.set_group(Some(first));
                }
                let w = me.clone();
                chip.connect_toggled(move |button| {
                    if button.is_active() {
                        if let Some(p) = w.upgrade() {
                            p.set_filter(index);
                        }
                    }
                });
                chips_box.append(&chip);
                chips.push(chip);
            }
            chips[0].set_active(true);

            // --- Paused banner --------------------------------------------------
            let banner = adw::Banner::new("Clipboard recording is paused");
            banner.set_button_label(Some("Resume"));
            banner.set_revealed(false);
            let a = app.clone();
            banner.connect_button_clicked(move |_| a.activate_action("resume", None));

            // --- Empty state ------------------------------------------------------
            let empty = adw::StatusPage::new();
            empty.set_icon_name(Some("edit-paste-symbolic"));
            empty.set_title("Nothing copied yet");
            empty.set_description(Some("Copy something and it will appear here."));
            empty.set_vexpand(true);

            // --- The list ---------------------------------------------------------
            // Each row holds a `ClipItem` inside a `BoxedAnyObject` (a GObject wrapper
            // for any Rust value), so the list store can hold our data.
            let list_store = gio::ListStore::new::<glib::BoxedAnyObject>();
            let selection = gtk::SingleSelection::new(Some(list_store.clone()));

            // The factory builds the widgets for one row. GTK reuses row widgets while
            // scrolling, and only binds the rows that are on screen (lazy loading).
            let factory = gtk::SignalListItemFactory::new();
            let w = me.clone();
            factory.connect_bind(move |_, object| {
                let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else {
                    return;
                };
                let Some(boxed) = list_item
                    .item()
                    .and_then(|item| item.downcast::<glib::BoxedAnyObject>().ok())
                else {
                    return;
                };
                let clip = boxed.borrow::<ClipItem>();
                list_item.set_child(Some(&build_row(&clip, w.clone())));
            });

            let list_view = gtk::ListView::new(Some(selection.clone()), Some(factory));
            // Each row handles its own left click (see `build_row`), which works the
            // same on X11 and Wayland. The list's own activation is used for Enter.
            list_view.set_single_click_activate(false);
            let w = me.clone();
            list_view.connect_activate(move |_, position| {
                if let Some(p) = w.upgrade() {
                    p.use_at(position, false, true);
                }
            });

            let scroll = gtk::ScrolledWindow::new();
            scroll.set_child(Some(&list_view));
            scroll.set_vexpand(true);
            scroll.set_hscrollbar_policy(gtk::PolicyType::Never);

            // The stack switches between the list and the empty-state page.
            let stack = gtk::Stack::new();
            stack.set_vexpand(true);
            stack.add_named(&scroll, Some("list"));
            stack.add_named(&empty, Some("empty"));

            // --- Footer -------------------------------------------------------------
            let hint = gtk::Label::new(Some("↵ paste  ⇧↵ plain  ⌫ remove"));
            hint.add_css_class("clipsy-hint");
            pad(&hint, 12, 6);

            // The popup has no title bar, so the grip bar at the top and the footer
            // act as drag handles. The tab bar is not one: its tabs need every click.
            // A slim bar with a small grip pill at the very top: the obvious place to grab.
            let grab_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            grab_bar.add_css_class("clipsy-grabbar");
            grab_bar.set_halign(gtk::Align::Fill);
            let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            pill.add_css_class("clipsy-pill");
            pill.set_halign(gtk::Align::Center);
            pill.set_valign(gtk::Align::Center);
            pill.set_hexpand(true);
            grab_bar.append(&pill);
            tooltip_for_drag(&grab_bar);
            add_move_handle(&grab_bar, &window, &resizing);
            add_move_handle(&hint, &window, &resizing);

            // --- Put it together ------------------------------------------------------
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            root.append(&grab_bar);
            root.append(&top);
            root.append(&chips_box);
            root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            root.append(&banner);
            root.append(&stack);
            root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
            root.append(&hint);
            // Drag handles on the right edge, bottom edge and corner. An undecorated
            // window has no resize border of its own, so we start the resize ourselves.
            let frame = gtk::Overlay::new();
            frame.set_child(Some(&root));
            frame.add_overlay(&resize_handle(&window, gtk::gdk::SurfaceEdge::East, &resizing));
            frame.add_overlay(&resize_handle(&window, gtk::gdk::SurfaceEdge::South, &resizing));
            frame.add_overlay(&resize_handle(&window, gtk::gdk::SurfaceEdge::SouthEast, &resizing));
            window.set_child(Some(&frame));

            // --- Keyboard -----------------------------------------------------------------
            // The capture phase sees keys before the search box does, so we can use
            // Enter, arrows and Tab for the list even while the search box has focus.
            let keys = gtk::EventControllerKey::new();
            keys.set_propagation_phase(gtk::PropagationPhase::Capture);
            let w = me.clone();
            keys.connect_key_pressed(move |_, keyval, _code, mods| match w.upgrade() {
                Some(p) => p.on_key(keyval, mods),
                None => glib::Propagation::Proceed,
            });
            window.add_controller(keys);

            // --- Close on focus loss ------------------------------------------------------
            let w = me.clone();
            let resizing_flag = resizing.clone();
            window.connect_is_active_notify(move |win| {
                tracing::debug!("popup: window active = {}", win.is_active());
                if let Some(p) = w.upgrade() {
                    if win.is_active() {
                        p.seen_active.set(true);
                        // The drag is over once the window has focus again.
                        resizing_flag.set(false);
                    } else if p.seen_active.get() && !resizing_flag.get() {
                        p.hide();
                    }
                }
            });

            Popup {
                state,
                me: me.clone(),
                window,
                search,
                chips,
                banner,
                stack,
                empty,
                list_view,
                list_store,
                selection,
                filter_index: Cell::new(0),
                seen_active: Cell::new(false),
                search_timer: RefCell::new(None),
                preview: RefCell::new(None),
            }
        })
    }

    // ------------------------------------------------------------------
    // Showing and hiding
    // ------------------------------------------------------------------

    /// Shows the popup, or hides it if it is already open.
    pub fn toggle(&self) {
        if self.window.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn show(&self) {
        // Remember which window has focus right now, so we can paste into it later.
        self.state.backend.borrow().remember_active_window();

        self.search.set_text("");
        self.seen_active.set(false);
        self.filter_index.set(0);
        self.chips[0].set_active(true);
        self.refresh_banner();
        self.reload(Focus::Top);

        // On GNOME Wayland the Shell extension places and focuses the window, so it
        // must know before the window appears. X11 ignores this.
        let position = self.state.config.borrow().appearance.popup_position.clone();
        self.state.backend.borrow().popup_will_show(&position);
        self.window.present();
        self.search.grab_focus();

        // The window manager places a new window when it maps it. Move the window
        // after a short wait, so our position is applied after that.
        let position = self.state.config.borrow().appearance.popup_position.clone();
        let window = self.window.clone();
        glib::timeout_add_local_once(POSITION_DELAY, move || placement::place(&window, &position));
    }

    pub fn hide(&self) {
        tracing::debug!("popup: hide()");
        if self.window.is_visible() {
            save_size(self.window.width(), self.window.height());
        }
        self.close_preview();
        // Reset this first, so the focus-loss handler does not run again.
        self.seen_active.set(false);
        self.window.set_visible(false);
    }

    /// Called when the history or the paused state changed.
    pub fn refresh(&self) {
        self.refresh_banner();
        if self.window.is_visible() {
            self.reload(Focus::Keep);
        }
    }

    fn refresh_banner(&self) {
        let paused = self.state.config.borrow().privacy.paused;
        self.banner.set_revealed(paused);
    }

    // ------------------------------------------------------------------
    // Loading the list
    // ------------------------------------------------------------------

    fn set_filter(&self, index: usize) {
        tracing::debug!("popup: filter tab {index} clicked");
        self.filter_index.set(index);
        self.reload(Focus::Top);
    }

    /// Waits a moment after the last key press, then reloads the list.
    fn schedule_reload(&self) {
        if let Some(id) = self.search_timer.borrow_mut().take() {
            id.remove();
        }
        let me = self.me.clone();
        let id = glib::timeout_add_local(SEARCH_DELAY, move || {
            if let Some(p) = me.upgrade() {
                // The timer has fired, so forget its id before reloading.
                p.search_timer.borrow_mut().take();
                p.reload(Focus::Top);
            }
            glib::ControlFlow::Break
        });
        *self.search_timer.borrow_mut() = Some(id);
    }

    /// Reads the history from the store (search text and filter applied) and fills the list.
    fn reload(&self, focus: Focus) {
        let previous_id = self.selected_clip().map(|clip| clip.id);
        let previous_pos = self.selection.selected();
        let query = self.search.text().to_string();
        let filter = filter_for(self.filter_index.get());

        let items = {
            let store = self.state.store.borrow();
            match store.list(&query, filter, HISTORY_LIMIT) {
                Ok(items) => items,
                Err(err) => {
                    tracing::warn!("could not load the history: {err:#}");
                    return;
                }
            }
        };

        let objects: Vec<glib::BoxedAnyObject> =
            items.into_iter().map(glib::BoxedAnyObject::new).collect();
        let old_count = self.list_store.n_items();
        self.list_store.splice(0, old_count, &objects);
        let count = self.list_store.n_items();

        // Empty state: different text when a search or filter hides everything.
        let filtered = !query.is_empty() || self.filter_index.get() != 0;
        if filtered {
            self.empty.set_title("No matches");
            self.empty.set_description(Some("Try another search or filter."));
        } else {
            self.empty.set_title("Nothing copied yet");
            self.empty.set_description(Some("Copy something and it will appear here."));
        }
        self.stack
            .set_visible_child_name(if count == 0 { "empty" } else { "list" });

        if count > 0 {
            let target = match focus {
                Focus::Top => 0,
                Focus::Item(id) => self.position_of(id).unwrap_or(0),
                Focus::Keep => previous_id
                    .and_then(|id| self.position_of(id))
                    .unwrap_or_else(|| previous_pos.min(count - 1)),
            };
            self.selection.set_selected(target);
        }
    }

    // ------------------------------------------------------------------
    // Reading rows
    // ------------------------------------------------------------------

    fn clip_at(&self, pos: u32) -> Option<ClipItem> {
        let object = self.list_store.item(pos)?;
        let boxed = object.downcast::<glib::BoxedAnyObject>().ok()?;
        let clip = boxed.borrow::<ClipItem>().clone();
        Some(clip)
    }

    fn id_at(&self, pos: u32) -> Option<i64> {
        let object = self.list_store.item(pos)?;
        let boxed = object.downcast::<glib::BoxedAnyObject>().ok()?;
        let clip = boxed.borrow::<ClipItem>();
        Some(clip.id)
    }

    fn position_of(&self, id: i64) -> Option<u32> {
        (0..self.list_store.n_items()).find(|&pos| self.id_at(pos) == Some(id))
    }

    fn selected_clip(&self) -> Option<ClipItem> {
        let pos = self.selection.selected();
        if pos == gtk::INVALID_LIST_POSITION {
            return None;
        }
        self.clip_at(pos)
    }

    fn move_selection(&self, delta: i64) {
        let count = self.list_store.n_items();
        if count == 0 {
            return;
        }
        let current = i64::from(self.selection.selected().min(count - 1));
        let next = (current + delta).clamp(0, i64::from(count) - 1) as u32;
        self.selection.set_selected(next);
        self.list_view.scroll_to(next, gtk::ListScrollFlags::NONE, None);
    }

    // ------------------------------------------------------------------
    // Actions on items
    // ------------------------------------------------------------------

    /// Uses the row at `pos`: copies it to the clipboard and, if asked, pastes it.
    fn use_at(&self, pos: u32, plain: bool, paste: bool) {
        tracing::debug!("popup: row {pos} activated (paste={paste})");
        if let Some(clip) = self.clip_at(pos) {
            self.use_item(clip.id, plain, paste);
        }
    }

    /// Puts an item on the clipboard, hides the popup and optionally pastes.
    /// `plain` puts only the plain-text version on the clipboard.
    fn use_item(&self, id: i64, plain: bool, paste: bool) {
        if let Err(err) = self.put_on_clipboard(id, plain) {
            tracing::warn!("could not copy item {id} to the clipboard: {err:#}");
            return;
        }
        if let Err(err) = self.state.store.borrow().touch(id) {
            tracing::warn!("could not update item {id}: {err:#}");
        }
        self.hide();

        let paste_on_select = self.state.config.borrow().general.paste_on_select;
        if paste && paste_on_select {
            self.paste_later();
        }
    }

    fn put_on_clipboard(&self, id: i64, plain: bool) -> anyhow::Result<()> {
        let store = self.state.store.borrow();
        let backend = self.state.backend.borrow();

        if plain {
            if let Some(text) = store.plain_text(id)? {
                let format = Format {
                    mime: "text/plain;charset=utf-8".to_string(),
                    data: text.into_bytes(),
                };
                return backend.set_clipboard(&[format]);
            }
            // No plain text in this item: fall through and copy everything instead.
        }

        let formats = store.formats(id)?;
        backend.set_clipboard(&formats)
    }

    /// Sends Ctrl+V to the window that had focus before the popup, after a short delay.
    fn paste_later(&self) {
        let state = self.state.clone();
        glib::timeout_add_local(PASTE_DELAY, move || {
            let backend = state.backend.borrow();
            if backend.supports_paste() {
                if let Err(err) = backend.paste(PasteMode::Normal) {
                    tracing::warn!("auto-paste failed: {err:#}");
                }
            }
            glib::ControlFlow::Break
        });
    }

    fn set_pinned(&self, id: i64, pinned: bool) {
        tracing::debug!("popup: pin button for item {id}");
        if let Err(err) = self.state.store.borrow().set_pinned(id, pinned) {
            tracing::warn!("could not pin item {id}: {err:#}");
        }
        // Pinned items move to the top, so keep the selection on this item.
        self.reload(Focus::Item(id));
    }

    fn delete(&self, id: i64) {
        tracing::debug!("popup: remove button for item {id}");
        if let Err(err) = self.state.store.borrow().delete(id) {
            tracing::warn!("could not delete item {id}: {err:#}");
        }
        self.reload(Focus::Keep);
    }

    fn run_menu(&self, id: i64, action: MenuAction) {
        match action {
            MenuAction::Paste => self.use_item(id, false, true),
            MenuAction::PastePlain => self.use_item(id, true, true),
            MenuAction::CopyOnly => self.use_item(id, false, false),
            MenuAction::SetPinned(pinned) => self.set_pinned(id, pinned),
            MenuAction::Delete => self.delete(id),
        }
    }

    // ------------------------------------------------------------------
    // Keyboard
    // ------------------------------------------------------------------

    fn on_key(&self, keyval: gdk::Key, mods: gdk::ModifierType) -> glib::Propagation {
        use glib::Propagation::{Proceed, Stop};

        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = mods.contains(gdk::ModifierType::SHIFT_MASK);
        let search_is_empty = self.search.text().is_empty();

        if keyval == gdk::Key::Escape {
            self.hide();
            return Stop;
        }
        if keyval == gdk::Key::Up {
            self.move_selection(-1);
            return Stop;
        }
        if keyval == gdk::Key::Down {
            self.move_selection(1);
            return Stop;
        }
        // Enter pastes. Shift+Enter pastes as plain text.
        if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
            if let Some(clip) = self.selected_clip() {
                self.use_item(clip.id, shift, true);
            }
            return Stop;
        }
        // Tab moves to the next filter chip, Shift+Tab to the previous one.
        if keyval == gdk::Key::Tab || keyval == gdk::Key::ISO_Left_Tab {
            let count = self.chips.len();
            let current = self.filter_index.get();
            let next = if keyval == gdk::Key::ISO_Left_Tab {
                (current + count - 1) % count
            } else {
                (current + 1) % count
            };
            self.chips[next].set_active(true);
            return Stop;
        }
        // Delete removes the selected item, but only when the search box is empty
        // (otherwise the Delete key is for editing the search text).
        if keyval == gdk::Key::Delete && search_is_empty {
            if let Some(clip) = self.selected_clip() {
                self.delete(clip.id);
            }
            return Stop;
        }
        // Space toggles a larger preview, again only when the search box is empty.
        if keyval == gdk::Key::space && search_is_empty && !ctrl {
            self.toggle_preview();
            return Stop;
        }

        if ctrl {
            if keyval == gdk::Key::p || keyval == gdk::Key::P {
                if let Some(clip) = self.selected_clip() {
                    self.set_pinned(clip.id, !clip.pinned);
                }
                return Stop;
            }
            // Ctrl+1 .. Ctrl+9 paste the first nine rows.
            if let Some(digit) = keyval.to_unicode().and_then(|c| c.to_digit(10)) {
                if (1..=9).contains(&digit) {
                    let pos = digit - 1;
                    if pos < self.list_store.n_items() {
                        self.use_at(pos, false, true);
                    }
                    return Stop;
                }
            }
        }
        Proceed
    }

    // ------------------------------------------------------------------
    // Image / text preview (Space)
    // ------------------------------------------------------------------

    fn toggle_preview(&self) {
        if self.preview.borrow().is_some() {
            self.close_preview();
            return;
        }
        let Some(clip) = self.selected_clip() else {
            return;
        };
        let Some(content) = self.preview_widget(&clip) else {
            return;
        };
        // A popover that does not take focus, so the popup stays open.
        let popover = gtk::Popover::new();
        popover.set_autohide(false);
        popover.set_has_arrow(false);
        popover.set_child(Some(&content));
        popover.set_parent(&self.list_view);
        popover.popup();
        *self.preview.borrow_mut() = Some(popover);
    }

    fn close_preview(&self) {
        if let Some(popover) = self.preview.borrow_mut().take() {
            popover.popdown();
            popover.unparent();
        }
    }

    fn preview_widget(&self, clip: &ClipItem) -> Option<gtk::Widget> {
        let store = self.state.store.borrow();
        if clip.kind == Kind::Image {
            let formats = store.formats(clip.id).ok()?;
            let image = formats.iter().find(|f| f.mime.starts_with("image/"))?;
            let bytes = glib::Bytes::from(&image.data[..]);
            let texture = gdk::Texture::from_bytes(&bytes).ok()?;
            let picture = gtk::Picture::for_paintable(&texture);
            picture.set_size_request(360, 280);
            picture.set_content_fit(gtk::ContentFit::Contain);
            return Some(picture.upcast());
        }
        let text = store.plain_text(clip.id).ok()??;
        // Long texts are cut short so the preview stays small.
        let shown: String = text.chars().take(2000).collect();
        let label = gtk::Label::new(Some(&shown));
        label.set_wrap(true);
        label.set_xalign(0.0);
        label.set_max_width_chars(60);
        pad(&label, 12, 12);
        Some(label.upcast())
    }
}

// ----------------------------------------------------------------------
// Row widgets
// ----------------------------------------------------------------------

/// Builds the widgets for one history row: kind glyph (or thumbnail), the
/// one-line preview, the age, and the pin and remove buttons.
fn build_row(clip: &ClipItem, me: Weak<Popup>) -> gtk::Widget {
    let show_source = me
        .upgrade()
        .map(|p| p.state.config.borrow().appearance.show_source_app)
        .unwrap_or(true);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("clip-row");
    row.set_valign(gtk::Align::Center);
    if clip.kind == Kind::Image {
        row.add_css_class("clip-row-image");
    }

    // Images show a small thumbnail, everything else shows an icon for its kind.
    match (clip.kind, &clip.thumb_path) {
        (Kind::Image, Some(path)) => {
            let picture = gtk::Picture::new();
            picture.set_filename(Some(path));
            picture.set_content_fit(gtk::ContentFit::Cover);
            picture.set_size_request(36, 36);
            picture.add_css_class("clip-thumb");
            row.append(&picture);
        }
        _ => {
            let icon = gtk::Image::from_icon_name(kind_icon(clip.kind));
            icon.set_pixel_size(16);
            icon.add_css_class("dim-label");
            row.append(&icon);
        }
    }

    // One ellipsized line of text. Long text is cut short, never wrapped.
    let title = gtk::Label::new(Some(&one_line(&clip.preview)));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_single_line_mode(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    // Caps the natural width, so a huge preview cannot push the row wider.
    title.set_max_width_chars(48);
    title.add_css_class("clip-row-title");

    let mut details = vec![kind_label(clip.kind).to_string()];
    if show_source {
        if let Some(app_name) = &clip.source_app {
            details.push(app_name.clone());
        }
    }
    title.set_tooltip_text(Some(&details.join("  ·  ")));
    row.append(&title);

    let age = gtk::Label::new(Some(&age_text(clip.last_used_at)));
    age.add_css_class("dim-label");
    age.add_css_class("caption");
    age.add_css_class("clip-age");
    age.set_valign(gtk::Align::Center);
    row.append(&age);

    // Pin and remove. They call the store directly, so clicking them does not
    // paste the row (the list only pastes on a click of the row itself).
    let id = clip.id;
    let pinned = clip.pinned;

    // Pin and remove live in a small pill that floats over the right end of the row
    // and fades in on hover, so idle rows show the full text.
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    actions.add_css_class("clip-actions");
    actions.set_halign(gtk::Align::End);
    actions.set_valign(gtk::Align::Center);
    actions.set_margin_end(6);

    let pin = action_button(
        if pinned { "starred-symbolic" } else { "non-starred-symbolic" },
        if pinned { "Unpin" } else { "Pin" },
    );
    let w = me.clone();
    pin.connect_clicked(move |_| {
        if let Some(p) = w.upgrade() {
            p.set_pinned(id, !pinned);
        }
    });
    actions.append(&pin);

    let remove = action_button("window-close-symbolic", "Remove");
    let w = me.clone();
    remove.connect_clicked(move |_| {
        if let Some(p) = w.upgrade() {
            p.delete(id);
        }
    });
    actions.append(&remove);

    let item = gtk::Overlay::new();
    item.add_css_class("clip-item");
    if pinned {
        item.add_css_class("clip-pinned-row");
    }
    item.set_child(Some(&row));
    item.add_overlay(&actions);

    // One left click pastes the row. The pin and remove buttons claim their own
    // clicks first, so pressing them does not paste. The popup hides right after
    // the paste, so a second click can never land on the row.
    let left = gtk::GestureClick::new();
    left.set_button(1);
    let w = me.clone();
    left.connect_released(move |gesture, n_press, _, _| {
        if n_press != 1 {
            return;
        }
        gesture.set_state(gtk::EventSequenceState::Claimed);
        if let Some(p) = w.upgrade() {
            tracing::debug!("popup: row for item {id} clicked");
            p.use_item(id, false, true);
        }
    });
    item.add_controller(left);

    // Right-click opens the context menu. The menu is built only when it is needed,
    // so rows that are never right-clicked carry no popover.
    let click = gtk::GestureClick::new();
    click.set_button(3);
    let (parent, w) = (item.clone(), me.clone());
    click.connect_pressed(move |_, _, x, y| open_row_menu(&parent, &w, id, pinned, x, y));
    item.add_controller(click);
    item.upcast()
}

/// A small flat icon button used on the row. It is faded out until the row is
/// hovered or selected (the CSS does that, so keyboard users still see it).
fn action_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.add_css_class("flat");
    button.add_css_class("clip-action");
    button.set_tooltip_text(Some(tooltip));
    button.set_valign(gtk::Align::Center);
    button
}

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Text | Kind::Html => "text-x-generic-symbolic",
        Kind::Image => "image-x-generic-symbolic",
        Kind::Files => "folder-documents-symbolic",
        Kind::Other => "edit-paste-symbolic",
    }
}

fn kind_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Text => "Text",
        Kind::Html => "Rich text",
        Kind::Image => "Image",
        Kind::Files => "Files",
        Kind::Other => "Other",
    }
}

/// Maps a filter chip index to the core `Filter`.
fn filter_for(index: usize) -> Filter {
    match index {
        1 => Filter::Text,
        2 => Filter::Images,
        3 => Filter::Files,
        4 => Filter::Pinned,
        _ => Filter::All,
    }
}

/// Replaces line breaks and other control characters, so a preview fits on one line.
fn one_line(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = flat.trim();
    if trimmed.is_empty() {
        "(empty)".to_string()
    } else {
        trimmed.to_string()
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// The age of an item for its row, such as "now", "5m", "3h" or "2d".
/// Timestamps are unix milliseconds.
fn age_text(timestamp_ms: i64) -> String {
    compact_age(((now_ms() - timestamp_ms) / 1000).max(0))
}

/// Formats an age in seconds for the narrow row: "now", "2m", "1h", "3d".
fn compact_age(secs: i64) -> String {
    match secs {
        ..=59 => "now".to_string(),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86_399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

/// Builds and shows the right-click menu for one row, then removes it when it closes.
fn open_row_menu(parent: &gtk::Overlay, me: &Weak<Popup>, id: i64, pinned: bool, x: f64, y: f64) {
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    pad(&menu_box, 4, 4);
    let popover = gtk::Popover::new();
    popover.set_child(Some(&menu_box));
    popover.set_has_arrow(false);
    popover.set_position(gtk::PositionType::Bottom);
    popover.set_parent(parent);
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let entries: [(&str, MenuAction); 5] = [
        ("Paste", MenuAction::Paste),
        ("Paste as plain text", MenuAction::PastePlain),
        (if pinned { "Unpin" } else { "Pin" }, MenuAction::SetPinned(!pinned)),
        ("Copy only (no paste)", MenuAction::CopyOnly),
        ("Delete", MenuAction::Delete),
    ];
    for (label, action) in entries {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        let (menu, w) = (popover.clone(), me.clone());
        button.connect_clicked(move |_| {
            menu.popdown();
            if let Some(p) = w.upgrade() {
                p.run_menu(id, action);
            }
        });
        menu_box.append(&button);
    }
    // Detach the popover once it has closed, after GTK is done with it.
    popover.connect_closed(|menu| {
        let menu = menu.clone();
        glib::idle_add_local_once(move || menu.unparent());
    });
    popover.popup();
}

fn tooltip_for_drag(widget: &gtk::Box) {
    widget.set_tooltip_text(Some("Drag to move"));
}

/// Lets the user move the whole window by dragging `widget` (like a title bar).
fn add_move_handle(widget: &impl IsA<gtk::Widget>, window: &gtk::Window, moving: &Rc<std::cell::Cell<bool>>) {
    widget.set_cursor_from_name(Some("grab"));
    // The move starts only after the pointer has really moved a few pixels, so a
    // plain click never turns into a window move.
    const THRESHOLD: f64 = 4.0;
    let drag = gtk::GestureDrag::new();
    drag.set_button(1);
    let started = Rc::new(std::cell::Cell::new(false));
    {
        let started = started.clone();
        drag.connect_drag_begin(move |_, _, _| started.set(false));
    }
    let (window, source, moving) = (window.clone(), widget.clone().upcast::<gtk::Widget>(), moving.clone());
    drag.connect_drag_update(move |gesture, dx, dy| {
        if started.get() || dx.hypot(dy) < THRESHOLD {
            return;
        }
        started.set(true);
        let Some((x, y)) = gesture.start_point() else { return };
        let Some(surface) = window.surface() else { return };
        let Ok(toplevel) = surface.downcast::<gtk::gdk::Toplevel>() else { return };
        // The window manager takes focus during the move; that is not "clicked away".
        moving.set(true);
        let flag = moving.clone();
        glib::timeout_add_local_once(std::time::Duration::from_secs(4), move || flag.set(false));
        let point = source
            .compute_point(&window, &gtk::graphene::Point::new(x as f32, y as f32))
            .map(|p| (f64::from(p.x()), f64::from(p.y())))
            .unwrap_or((x, y));
        let Some(device) = gesture.device() else { return };
        gesture.set_state(gtk::EventSequenceState::Claimed);
        toplevel.begin_move(&device, 1, point.0, point.1, gesture.current_event_time());
    });
    widget.add_controller(drag);
}

// --- Resizing ----------------------------------------------------------------------

const MIN_SIZE: (i32, i32) = (300, 260);
const MAX_SIZE: (i32, i32) = (900, 1000);

fn size_file() -> Option<std::path::PathBuf> {
    clipsy_core::config::data_dir().ok().map(|d| d.join("popup-size"))
}

/// The size the user last dragged the popup to, or the default 380x480.
fn load_saved_size() -> (i32, i32) {
    let parsed = size_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|text| {
            let (w, h) = text.trim().split_once('x')?;
            Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?))
        });
    match parsed {
        Some((w, h)) => (w.clamp(MIN_SIZE.0, MAX_SIZE.0), h.clamp(MIN_SIZE.1, MAX_SIZE.1)),
        None => (380, 480),
    }
}

fn save_size(width: i32, height: i32) {
    if width < MIN_SIZE.0 || height < MIN_SIZE.1 {
        return;
    }
    if let Some(file) = size_file() {
        let _ = std::fs::write(file, format!("{width}x{height}"));
    }
}

/// A thin invisible strip (or corner grip) that starts a window resize when dragged.
fn resize_handle(
    window: &gtk::Window,
    edge: gtk::gdk::SurfaceEdge,
    resizing: &Rc<std::cell::Cell<bool>>,
) -> gtk::Widget {
    use gtk::gdk::SurfaceEdge as E;
    let handle = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    handle.add_css_class("clipsy-resize");
    let (halign, valign, w, h, cursor) = match edge {
        E::East => (gtk::Align::End, gtk::Align::Fill, 5, -1, "e-resize"),
        E::South => (gtk::Align::Fill, gtk::Align::End, -1, 5, "s-resize"),
        _ => (gtk::Align::End, gtk::Align::End, 16, 16, "se-resize"),
    };
    handle.set_halign(halign);
    handle.set_valign(valign);
    handle.set_size_request(w, h);
    handle.set_cursor_from_name(Some(cursor));
    if edge == E::SouthEast {
        handle.add_css_class("clipsy-grip");
    }

    let click = gtk::GestureClick::new();
    click.set_button(1);
    let window = window.clone();
    let target = handle.clone();
    let resizing = resizing.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        // Ignore focus loss while the window manager runs the drag. If focus never
        // changes, a timer clears the flag so click-away still works afterwards.
        resizing.set(true);
        let flag = resizing.clone();
        glib::timeout_add_local_once(std::time::Duration::from_secs(4), move || flag.set(false));
        let Some(surface) = window.surface() else { return };
        let Ok(toplevel) = surface.downcast::<gtk::gdk::Toplevel>() else { return };
        // The resize needs the pointer position in window coordinates.
        let point = target
            .compute_point(&window, &gtk::graphene::Point::new(x as f32, y as f32))
            .map(|p| (f64::from(p.x()), f64::from(p.y())))
            .unwrap_or((x, y));
        toplevel.begin_resize(
            edge,
            gesture.device().as_ref(),
            1,
            point.0,
            point.1,
            gesture.current_event_time(),
        );
    });
    handle.add_controller(click);
    handle.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_age_uses_short_units() {
        assert_eq!(compact_age(0), "now");
        assert_eq!(compact_age(59), "now");
        assert_eq!(compact_age(60), "1m");
        assert_eq!(compact_age(150), "2m");
        assert_eq!(compact_age(3600), "1h");
        assert_eq!(compact_age(3 * 3600 + 5), "3h");
        assert_eq!(compact_age(86_400), "1d");
        assert_eq!(compact_age(5 * 86_400), "5d");
    }

    #[test]
    fn one_line_flattens_text() {
        assert_eq!(one_line("first\nsecond\tthird"), "first second third");
        assert_eq!(one_line("  padded  "), "padded");
        assert_eq!(one_line(""), "(empty)");
        assert_eq!(one_line(" \n "), "(empty)");
    }
}

