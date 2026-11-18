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

/// Opens the welcome window.
pub fn show(app: &adw::Application, state: &Rc<State>) {
    install_css();

    let window = adw::Window::new();
    window.set_application(Some(app));
    window.set_title(Some("Welcome to Clipsy"));
    window.set_default_size(380, -1);
    window.set_resizable(false);

    let icon = gtk::Image::from_icon_name(APP_ID);
    icon.set_pixel_size(64);

    let title = gtk::Label::new(Some("Clipsy"));
    title.add_css_class("title-1");

    let tagline = gtk::Label::new(Some("Everything you copy, one shortcut away."));
    tagline.add_css_class("dim-label");
    tagline.set_wrap(true);
    tagline.set_justify(gtk::Justification::Center);

    let keycaps = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    fill_keycaps(&keycaps, &state.config.borrow().shortcuts.toggle_popup);

    let status = gtk::Label::new(None);
    status.add_css_class("dim-label");
    status.set_wrap(true);
    status.set_justify(gtk::Justification::Center);
    status.set_visible(false);

    let change = gtk::Button::with_label("Change");
    {
        let (w, s, k, st) = (window.clone(), state.clone(), keycaps.clone(), status.clone());
        change.connect_clicked(move |_| record_shortcut(&w, &s, &k, &st));
    }

    let shortcut_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    shortcut_row.set_halign(gtk::Align::Center);
    shortcut_row.append(&keycaps);
    shortcut_row.append(&change);

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

    let start = gtk::Button::with_label("Start using Clipsy");
    start.add_css_class("pill");
    start.add_css_class("suggested-action");
    start.set_halign(gtk::Align::Center);
    {
        let w = window.clone();
        start.connect_clicked(move |_| finish_welcome(&w));
    }

    // Closing the window with the title bar also counts as "seen".
    window.connect_close_request(move |_| {
        mark_done();
        glib::Propagation::Proceed
    });

    let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
    content.set_margin_top(24);
    content.set_margin_bottom(24);
    content.set_margin_start(24);
    content.set_margin_end(24);
    content.append(&icon);
    content.append(&title);
    content.append(&tagline);
    content.append(&shortcut_row);
    content.append(&status);
    if let Some(setup) = gnome_setup::setup_box(state) {
        content.append(&setup);
    }
    content.append(&list);
    content.append(&start);
    // The setup box makes the window tall, so the content scrolls on small screens.
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(640);
    scroller.set_child(Some(&content));
    window.set_content(Some(&scroller));
    window.present();
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
    use super::accel_to_keycaps;

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
