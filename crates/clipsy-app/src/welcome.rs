//! First-run welcome: four pages (welcome, shortcut, privacy, settings), one at a time, with Back/Next. Closing the window also counts as seen.

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

/// Opens the welcome window: four short pages shown one at a time, with Back and Next.
pub fn show(app: &adw::Application, state: &Rc<State>) {
    install_css();

    let window = adw::Window::new();
    window.set_application(Some(app));
    window.set_title(Some("Welcome to Clipsy"));
    window.set_default_size(720, 640);
    window.set_resizable(true);
    window.set_size_request(520, 520);

    // A plain stack: exactly one page is visible and pages switch instantly, with no slide.
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::None);
    stack.set_vhomogeneous(false);
    stack.set_hhomogeneous(true);
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.add_named(&intro_page(), Some(page_name(0)));
    stack.add_named(&shortcut_page(&window, state), Some(page_name(1)));
    stack.add_named(&privacy_page(), Some(page_name(2)));
    stack.add_named(&settings_page(state), Some(page_name(3)));

    // Step dots: one small box per page. The current one gets the `current` class.
    let dots = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    dots.set_halign(gtk::Align::Center);
    for _ in 0..PAGE_NAMES.len() {
        let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        dot.add_css_class("step-dot");
        dot.set_valign(gtk::Align::Center);
        dots.append(&dot);
    }

    let back = gtk::Button::with_label("Back");
    {
        let s = stack.downgrade();
        back.connect_clicked(move |_| {
            let Some(stack) = s.upgrade() else {
                return;
            };
            let page = current_index(&stack);
            if page > 0 {
                stack.set_visible_child_name(page_name(page - 1));
            }
        });
    }

    let next = gtk::Button::with_label("Next");
    {
        let (s, w) = (stack.downgrade(), window.downgrade());
        next.connect_clicked(move |_| {
            let (Some(stack), Some(window)) = (s.upgrade(), w.upgrade()) else {
                return;
            };
            let page = current_index(&stack);
            if is_last_page(page, PAGE_NAMES.len() as u32) {
                finish_welcome(&window);
            } else {
                stack.set_visible_child_name(page_name(page + 1));
            }
        });
    }

    // Arrow keys move between pages. Left/Right in the capture phase, so they work
    // from any focused widget on the page.
    {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let s = stack.downgrade();
        keys.connect_key_pressed(move |_, keyval, _, _| {
            let Some(stack) = s.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let delta = match keyval {
                gdk::Key::Left => -1,
                gdk::Key::Right => 1,
                _ => return glib::Propagation::Proceed,
            };
            if step_page(&stack, delta) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(keys);
    }

    {
        let (b, n, d) = (back.clone(), next.clone(), dots.clone());
        stack.connect_visible_child_name_notify(move |s| sync_page(s, &b, &n, &d));
    }
    sync_page(&stack, &back, &next, &dots);

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
    toolbar.set_content(Some(&stack));
    toolbar.add_bottom_bar(&bottom);
    window.set_content(Some(&toolbar));
    window.present();
}

/// Page names, in order. Each page is added to the stack under its name.
const PAGE_NAMES: [&str; 4] = ["intro", "shortcut", "privacy", "settings"];

/// Stack name of the page at `index` (clamped to the first page if out of range).
fn page_name(index: u32) -> &'static str {
    PAGE_NAMES.get(index as usize).copied().unwrap_or(PAGE_NAMES[0])
}

/// Index of a page from its stack name.
fn page_index(name: &str) -> Option<u32> {
    PAGE_NAMES.iter().position(|page| *page == name).map(|i| i as u32)
}

/// Index of the page that is showing now.
fn current_index(stack: &gtk::Stack) -> u32 {
    stack
        .visible_child_name()
        .and_then(|name| page_index(&name))
        .unwrap_or(0)
}

/// Moves by `delta` pages (-1 or 1) without going past either end. True if the page changed.
fn step_page(stack: &gtk::Stack, delta: i32) -> bool {
    let target = current_index(stack) as i32 + delta;
    if target < 0 || target >= PAGE_NAMES.len() as i32 {
        return false;
    }
    stack.set_visible_child_name(page_name(target as u32));
    true
}

/// Updates the Back and Next buttons and the step dots for the visible page.
fn sync_page(stack: &gtk::Stack, back: &gtk::Button, next: &gtk::Button, dots: &gtk::Box) {
    let page = current_index(stack);
    let n_pages = PAGE_NAMES.len() as u32;
    sync_nav(page, n_pages, back, next);
    sync_dots(dots, page, n_pages);
}

/// Marks the dot for `page` as current and sets the "Step N of M" label on the dots.
fn sync_dots(dots: &gtk::Box, page: u32, n_pages: u32) {
    let mut index = 0;
    let mut child = dots.first_child();
    while let Some(dot) = child {
        if is_current_dot(index, page) {
            dot.add_css_class("current");
        } else {
            dot.remove_css_class("current");
        }
        child = dot.next_sibling();
        index += 1;
    }
    dots.update_property(&[gtk::accessible::Property::Label(&step_label(page, n_pages))]);
}

fn is_current_dot(dot: u32, page: u32) -> bool {
    dot == page
}

/// Accessible text for the step dots, for example "Step 2 of 4".
fn step_label(page: u32, n_pages: u32) -> String {
    format!("Step {} of {}", page + 1, n_pages)
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
    content.set_margin_start(32);
    content.set_margin_end(32);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content
}

/// Wraps a page's content so it sits in the middle, never gets wider than 520px,
/// and scrolls inside the page when the window is too short. Nothing is clipped.
fn page(content: &gtk::Box) -> gtk::ScrolledWindow {
    let clamp = adw::Clamp::new();
    clamp.set_maximum_size(520);
    clamp.set_tightening_threshold(420);
    clamp.set_child(Some(content));

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_vexpand(true);
    column.append(&spacer());
    column.append(&clamp);
    column.append(&spacer());

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_vscrollbar_policy(gtk::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    // Cap the natural height so a tall page (the setup card) scrolls instead of
    // making the window grow when the page changes.
    scroller.set_max_content_height(460);
    scroller.set_min_content_height(200);
    scroller.set_hexpand(true);
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
    use super::{
        accel_to_keycaps, back_visible, is_current_dot, is_last_page, next_label, page_index,
        page_name, step_label, PAGE_NAMES,
    };

    #[test]
    fn page_names_round_trip() {
        for (index, name) in PAGE_NAMES.iter().enumerate() {
            assert_eq!(page_name(index as u32), *name);
            assert_eq!(page_index(name), Some(index as u32));
        }
        assert_eq!(page_index("missing"), None);
        assert_eq!(page_name(99), "intro");
    }

    #[test]
    fn step_label_counts_from_one() {
        assert_eq!(step_label(0, 4), "Step 1 of 4");
        assert_eq!(step_label(1, 4), "Step 2 of 4");
        assert_eq!(step_label(3, 4), "Step 4 of 4");
    }

    #[test]
    fn only_the_current_dot_is_marked() {
        let marked: Vec<u32> = (0..4).filter(|dot| is_current_dot(*dot, 2)).collect();
        assert_eq!(marked, [2]);
    }

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
