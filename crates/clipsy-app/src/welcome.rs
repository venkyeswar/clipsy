//! First-run welcome: one small window with the app name, the current shortcut
//! drawn as keycaps (with a Change button), two switches and a button that
//! starts Clipsy. Closing the window also counts as "seen".

use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use clipsy_backends::HotkeyAction;
use gtk::{gdk, glib};

use crate::app::{data_dir, set_hotkey, update_config, State};
use crate::{autostart, gnome_setup, APP_ID};

/// Keycap style. Only libadwaita named colors, so light and dark both work.
const CSS: &str = "
.clipsy-keycap {
    background-color: @card_bg_color;
    color: @window_fg_color;
    border-radius: 6px;
    border-bottom: 1px solid alpha(@window_fg_color, 0.25);
    padding: 2px 8px;
    min-width: 16px;
    font-weight: bold;
}
";

/// File that exists once the welcome has been shown.
fn marker_path() -> Option<PathBuf> {
    data_dir().ok().map(|dir| dir.join("welcome-done"))
}

/// True until the welcome has been shown once.
pub fn is_needed() -> bool {
    marker_path().is_some_and(|path| !path.exists())
}

fn mark_done() {
    let Some(path) = marker_path() else {
        return;
    };
    if let Err(err) = fs::write(&path, b"1") {
        tracing::warn!("could not remember that the welcome was shown: {err}");
    }
}

/// Splits a GTK accelerator such as `<Super><Shift>v` into keycap labels:
/// `["Super", "Shift", "V"]`. An empty accelerator gives no keycaps.
pub fn accel_to_keycaps(accel: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = accel.trim();
    while let Some(stripped) = rest.strip_prefix('<') {
        let Some(end) = stripped.find('>') else {
            return Vec::new();
        };
        keys.push(modifier_label(&stripped[..end]));
        rest = &stripped[end + 1..];
    }
    let key = rest.trim();
    if !key.is_empty() {
        keys.push(key_label(key));
    }
    keys
}

fn modifier_label(name: &str) -> String {
    match name {
        "Control" | "Primary" | "Ctrl" => "Ctrl".to_string(),
        other => other.to_string(),
    }
}

fn key_label(key: &str) -> String {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c.to_uppercase().collect(),
        _ => key.to_string(),
    }
}

/// Opens the welcome window: a carousel of four short pages with Back and Next.
pub fn show(app: &adw::Application, state: &Rc<State>) {
    install_css();

    let window = adw::Window::new();
    window.set_application(Some(app));
    window.set_title(Some("Welcome to Clipsy"));
    window.set_default_size(480, 600);
    window.set_resizable(true);
    window.set_size_request(420, 520);

    let carousel = adw::Carousel::new();
    carousel.set_vexpand(true);
    carousel.append(&intro_page());
    carousel.append(&shortcut_page(&window, state));
    carousel.append(&privacy_page());
    carousel.append(&settings_page(state));

    let dots = adw::CarouselIndicatorDots::new();
    dots.set_carousel(Some(&carousel));
    dots.set_halign(gtk::Align::Center);

    let back = gtk::Button::with_label("Back");
    {
        let c = carousel.downgrade();
        back.connect_clicked(move |_| {
            let Some(carousel) = c.upgrade() else {
                return;
            };
            let page = current_page(&carousel);
            if page > 0 {
                carousel.scroll_to(&carousel.nth_page(page - 1), true);
            }
        });
    }

    let next = gtk::Button::with_label("Next");
    {
        let (c, w) = (carousel.downgrade(), window.downgrade());
        next.connect_clicked(move |_| {
            let (Some(carousel), Some(window)) = (c.upgrade(), w.upgrade()) else {
                return;
            };
            let page = current_page(&carousel);
            if is_last_page(page, carousel.n_pages()) {
                finish_welcome(&window);
            } else {
                carousel.scroll_to(&carousel.nth_page(page + 1), true);
            }
        });
    }

    {
        let (b, n) = (back.clone(), next.clone());
        carousel.connect_page_changed(move |c, page| sync_nav(page, c.n_pages(), &b, &n));
    }
    sync_nav(0, carousel.n_pages(), &back, &next);

    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    let nav = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    nav.set_margin_start(24);
    nav.set_margin_end(24);
    nav.append(&back);
    nav.append(&spacer);
    nav.append(&next);

    let bottom = gtk::Box::new(gtk::Orientation::Vertical, 12);
    bottom.set_margin_top(12);
    bottom.set_margin_bottom(16);
    bottom.append(&dots);
    bottom.append(&nav);

    // Closing the window with the title bar also counts as "seen".
    window.connect_close_request(move |_| {
        mark_done();
        glib::Propagation::Proceed
    });

    // A slim, flat title bar: gives the window a close button and a place to
    // drag it from (Wayland windows have no frame of their own).
    let header = adw::HeaderBar::new();
    header.set_show_title(false);
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&carousel));
    toolbar.add_bottom_bar(&bottom);
    window.set_content(Some(&toolbar));
    window.present();
}

/// Index of the page that is showing now.
fn current_page(carousel: &adw::Carousel) -> u32 {
    carousel.position().round() as u32
}

/// Sets the Back and Next buttons for a page. The last page turns Next into the pill.
fn sync_nav(page: u32, n_pages: u32, back: &gtk::Button, next: &gtk::Button) {
    back.set_visible(back_visible(page));
    next.set_label(next_label(page, n_pages));
    if is_last_page(page, n_pages) {
        next.add_css_class("pill");
        next.add_css_class("suggested-action");
    } else {
        next.remove_css_class("pill");
        next.remove_css_class("suggested-action");
    }
}

fn is_last_page(page: u32, n_pages: u32) -> bool {
    page + 1 >= n_pages
}

fn back_visible(page: u32) -> bool {
    page > 0
}

fn next_label(page: u32, n_pages: u32) -> &'static str {
    if is_last_page(page, n_pages) {
        "Start using Clipsy"
    } else {
        "Next"
    }
}

/// Empty column for a page. Two of these around the content center it vertically.
fn spacer() -> gtk::Box {
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    spacer
}

/// Box for a page's content, with the page margins and spacing.
fn content_box() -> gtk::Box {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content
}

/// Wraps a page's content so it sits in the middle and scrolls when the window is short.
fn page(content: &gtk::Box) -> gtk::ScrolledWindow {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_vexpand(true);
    column.append(&spacer());
    column.append(content);
    column.append(&spacer());

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&column));
    scroller
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("title-2");
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label
}

fn body_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label
}

/// Page 1: the app icon, the name and the tagline.
fn intro_page() -> gtk::ScrolledWindow {
    let icon = gtk::Image::from_icon_name(APP_ID);
    icon.set_pixel_size(96);

    let title = gtk::Label::new(Some("Welcome to Clipsy"));
    title.add_css_class("title-1");
    title.set_wrap(true);
    title.set_justify(gtk::Justification::Center);

    let content = content_box();
    content.append(&icon);
    content.append(&title);
    content.append(&body_label("Everything you copy, one shortcut away."));
    page(&content)
}

/// Page 2: the popup shortcut, with Change, and the GNOME setup box when it is needed.
fn shortcut_page(window: &adw::Window, state: &Rc<State>) -> gtk::ScrolledWindow {
    let keycaps = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    fill_keycaps(&keycaps, &state.config.borrow().shortcuts.toggle_popup);

    let status = gtk::Label::new(None);
    status.add_css_class("dim-label");
    status.set_wrap(true);
    status.set_justify(gtk::Justification::Center);
    status.set_visible(false);

    let change = gtk::Button::with_label("Change");
    {
        let w = window.downgrade();
        let (s, k, st) = (state.clone(), keycaps.clone(), status.clone());
        change.connect_clicked(move |_| {
            if let Some(w) = w.upgrade() {
                record_shortcut(&w, &s, &k, &st);
            }
        });
    }

    let shortcut_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    shortcut_row.set_halign(gtk::Align::Center);
    shortcut_row.append(&keycaps);
    shortcut_row.append(&change);

    let content = content_box();
    content.append(&heading("Open it anywhere"));
    content.append(&body_label("Press this shortcut in any app to open Clipsy."));
    content.append(&shortcut_row);
    content.append(&status);
    if let Some(setup) = gnome_setup::setup_box(state) {
        content.append(&setup);
    }
    page(&content)
}

/// Page 3: what stays private and how to clear a tab.
fn privacy_page() -> gtk::ScrolledWindow {
    let content = content_box();
    content.append(&heading("Private by default"));
    content.append(&body_label("Your history stays on this computer."));
    content.append(&body_label(
        "Copies that password managers mark as secret are skipped by default.",
    ));
    content.append(&body_label(
        "Use the 🗑 button next to the tabs to clear a tab's history.",
    ));
    page(&content)
}

/// Page 4: the two switches.
fn settings_page(state: &Rc<State>) -> gtk::ScrolledWindow {
    let autostart_row = adw::SwitchRow::new();
    autostart_row.set_title("Start at login");
    autostart_row.set_active(state.config.borrow().general.autostart);
    {
        let st = state.clone();
        autostart_row.connect_active_notify(move |row| {
            let enabled = row.is_active();
            if let Err(err) = autostart::set_enabled(enabled) {
                tracing::warn!("could not set autostart: {err:#}");
            }
            update_config(&st, |c| c.general.autostart = enabled);
        });
    }

    let paste_row = adw::SwitchRow::new();
    paste_row.set_title("Paste after selecting");
    paste_row.set_active(state.config.borrow().general.paste_on_select);
    {
        let st = state.clone();
        paste_row.connect_active_notify(move |row| {
            let enabled = row.is_active();
            update_config(&st, |c| c.general.paste_on_select = enabled);
        });
    }

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.append(&autostart_row);
    list.append(&paste_row);

    let content = content_box();
    content.append(&heading("Make it yours"));
    content.append(&list);
    page(&content)
}

fn install_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Redraws the keycap labels for an accelerator.
fn fill_keycaps(keycaps: &gtk::Box, accel: &str) {
    while let Some(child) = keycaps.first_child() {
        keycaps.remove(&child);
    }
    let keys = accel_to_keycaps(accel);
    if keys.is_empty() {
        keycaps.append(&gtk::Label::new(Some("Not set")));
        return;
    }
    for key in keys {
        let label = gtk::Label::new(Some(&key));
        label.add_css_class("clipsy-keycap");
        keycaps.append(&label);
    }
}

/// Opens the "press a new key" dialog for the popup shortcut.
fn record_shortcut(window: &adw::Window, state: &Rc<State>, keycaps: &gtk::Box, status: &gtk::Label) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some("Set shortcut: Open Clipsy"),
        Some("Press the new key combination.\nEsc cancels."),
    );
    dialog.add_response("cancel", "Cancel");

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (dialog_c, state_c, keycaps_c, status_c) =
        (dialog.clone(), state.clone(), keycaps.clone(), status.clone());
    keys.connect_key_pressed(move |_, keyval, _code, mods| {
        if keyval == gdk::Key::Escape {
            dialog_c.close();
            return glib::Propagation::Stop;
        }
        // Wait for the real key. Pressing Shift or Ctrl alone does nothing yet.
        if is_modifier(keyval) {
            return glib::Propagation::Stop;
        }
        let mods = mods & gtk::accelerator_get_default_mod_mask();
        // A global shortcut without Ctrl/Alt/Super would hijack a normal key in every app.
        let strong = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK;
        let is_fkey = keyval.name().is_some_and(|n| {
            n.len() > 1 && n.starts_with('F') && n[1..].chars().all(|c| c.is_ascii_digit())
        });
        if !mods.intersects(strong) && !is_fkey {
            return glib::Propagation::Stop;
        }
        let accel = gtk::accelerator_name(keyval, mods).to_string();
        dialog_c.close();
        apply_shortcut(&state_c, &accel, &keycaps_c, &status_c);
        glib::Propagation::Stop
    });
    dialog.add_controller(keys);
    dialog.present();
}

fn is_modifier(key: gdk::Key) -> bool {
    const MODIFIER_PREFIXES: [&str; 9] = [
        "Shift", "Control", "Alt", "Super", "Meta", "Hyper", "ISO_Level3", "Caps", "Num",
    ];
    key.name()
        .is_some_and(|name| MODIFIER_PREFIXES.iter().any(|prefix| name.starts_with(*prefix)))
}

/// Registers the new shortcut. If another app already uses it, the old one comes back.
fn apply_shortcut(state: &Rc<State>, accel: &str, keycaps: &gtk::Box, status: &gtk::Label) {
    let previous = state.config.borrow().shortcuts.toggle_popup.clone();
    match set_hotkey(state, HotkeyAction::TogglePopup, accel) {
        Ok(()) => {
            update_config(state, |c| c.shortcuts.toggle_popup = accel.to_string());
            fill_keycaps(keycaps, accel);
            status.set_visible(false);
        }
        Err(err) => {
            tracing::warn!("shortcut {accel} was rejected: {err:#}");
            if let Err(err) = set_hotkey(state, HotkeyAction::TogglePopup, &previous) {
                tracing::warn!("could not restore the old shortcut: {err:#}");
            }
            status.set_label("That shortcut is already used by another app.");
            status.set_visible(true);
        }
    }
}

fn finish_welcome(window: &adw::Window) {
    mark_done();
    window.close();
}

#[cfg(test)]
mod tests {
    use super::{accel_to_keycaps, back_visible, is_last_page, next_label};

    #[test]
    fn first_page_has_no_back_button() {
        assert!(!back_visible(0));
        assert!(back_visible(1));
        assert!(back_visible(3));
    }

    #[test]
    fn last_page_is_the_start_button() {
        assert!(!is_last_page(0, 4));
        assert!(!is_last_page(2, 4));
        assert!(is_last_page(3, 4));
        assert_eq!(next_label(0, 4), "Next");
        assert_eq!(next_label(2, 4), "Next");
        assert_eq!(next_label(3, 4), "Start using Clipsy");
    }

    #[test]
    fn keycaps_for_default_shortcut() {
        assert_eq!(accel_to_keycaps("<Super><Shift>v"), ["Super", "Shift", "V"]);
    }

    #[test]
    fn keycaps_for_ctrl_alt() {
        assert_eq!(accel_to_keycaps("<Ctrl><Alt>c"), ["Ctrl", "Alt", "C"]);
    }

    #[test]
    fn keycaps_map_control_to_ctrl() {
        assert_eq!(accel_to_keycaps("<Control>x"), ["Ctrl", "X"]);
    }

    #[test]
    fn keycaps_keep_named_keys() {
        assert_eq!(accel_to_keycaps("<Shift>Return"), ["Shift", "Return"]);
        assert_eq!(accel_to_keycaps("<Super>F5"), ["Super", "F5"]);
    }

    #[test]
    fn keycaps_for_empty_or_malformed() {
        assert!(accel_to_keycaps("").is_empty());
        assert!(accel_to_keycaps("   ").is_empty());
        assert!(accel_to_keycaps("<Super").is_empty());
    }
}
