//! The Settings window (an `adw::PreferencesWindow`).
//!
//! Pages: General, Appearance, Shortcuts, Privacy and About. Every change is
//! saved to config.toml right away, so there is no "Save" button.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use clipsy_backends::HotkeyAction;
use clipsy_core::Config;
use gtk::{gdk, glib};

use crate::app::{apply_theme, set_hotkey, update_config, State};
use crate::{autostart, gnome_setup, APP_ID};

/// Name used to find an open Settings window again.
const WINDOW_NAME: &str = "clipsy-settings";

/// Opens the Settings window, or brings the open one to the front.
pub fn show(app: &adw::Application, state: &Rc<State>) {
    for window in app.windows() {
        if window.widget_name().as_str() == WINDOW_NAME {
            window.present();
            return;
        }
    }
    let window = build(app, state);
    window.present();
}

fn build(app: &adw::Application, state: &Rc<State>) -> adw::PreferencesWindow {
    let window = adw::PreferencesWindow::new();
    window.set_application(Some(app));
    window.set_widget_name(WINDOW_NAME);
    window.set_title(Some("Clipsy Settings"));
    window.set_default_size(640, 720);
    window.set_search_enabled(false);

    window.add(&general_page(state));
    window.add(&appearance_page(state));
    window.add(&shortcuts_page(&window, state));
    window.add(&privacy_page(app, &window, state));
    window.add(&about_page(&window));
    window
}

// ----------------------------------------------------------------------
// Pages
// ----------------------------------------------------------------------

fn general_page(state: &Rc<State>) -> adw::PreferencesPage {
    let page = new_page("preferences-system-symbolic", "General");

    if let Some(setup) = gnome_setup::setup_box(state) {
        let group = adw::PreferencesGroup::new();
        group.add(&setup);
        page.add(&group);
    }

    let startup = new_group("Startup");
    startup.add(&autostart_row(state));
    startup.add(&switch_row(
        "Paste after selecting",
        "Put the chosen item into the window you were using.",
        state,
        |c| c.general.paste_on_select,
        |c, v| c.general.paste_on_select = v,
    ));
    page.add(&startup);

    let history = new_group("History");
    history.add(&keep_days_row(state));
    history.add(&spin_row(
        "Items to keep",
        "The oldest unpinned items are removed first.",
        state,
        (10.0, 10_000.0, 10.0),
        |c| c.general.max_items as f64,
        |c, v| c.general.max_items = v as _,
    ));
    history.add(&spin_row(
        "Maximum storage (MB)",
        "Limit for all saved items together.",
        state,
        (50.0, 50_000.0, 50.0),
        |c| c.general.max_total_mb as f64,
        |c, v| c.general.max_total_mb = v as _,
    ));
    history.add(&spin_row(
        "Largest item (MB)",
        "Bigger copies are skipped.",
        state,
        (1.0, 500.0, 1.0),
        |c| c.general.max_item_mb as f64,
        |c, v| c.general.max_item_mb = v as _,
    ));
    page.add(&history);

    let kinds = new_group("What to remember");
    kinds.add(&switch_row(
        "Images",
        "Screenshots and copied pictures.",
        state,
        |c| c.general.save_images,
        |c, v| c.general.save_images = v,
    ));
    kinds.add(&switch_row(
        "Files",
        "Files copied in the file manager.",
        state,
        |c| c.general.save_files,
        |c, v| c.general.save_files = v,
    ));
    kinds.add(&switch_row(
        "Rich text",
        "Formatting from web pages and word processors.",
        state,
        |c| c.general.save_rich_text,
        |c, v| c.general.save_rich_text = v,
    ));
    kinds.add(&switch_row(
        "Mouse selection",
        "Also remember text that is selected with the mouse.",
        state,
        |c| c.general.watch_primary,
        |c, v| c.general.watch_primary = v,
    ));
    page.add(&kinds);
    page
}

fn appearance_page(state: &Rc<State>) -> adw::PreferencesPage {
    let page = new_page("applications-graphics-symbolic", "Appearance");

    let theme = new_group("Theme");
    theme.add(&choice_row(
        "Color scheme",
        "Follow the system, or always use light or dark.",
        &[("system", "System"), ("light", "Light"), ("dark", "Dark")],
        state,
        |c| c.appearance.theme.clone(),
        |c, v| c.appearance.theme = v,
        apply_theme,
    ));
    page.add(&theme);

    let popup = new_group("Popup");
    popup.add(&choice_row(
        "Position",
        "Where the popup opens. Near the cursor works on X11 only.",
        &[("cursor", "Near the cursor"), ("center", "Center of the screen")],
        state,
        |c| c.appearance.popup_position.clone(),
        |c, v| c.appearance.popup_position = v,
        |_| {},
    ));
    popup.add(&switch_row(
        "Compact rows",
        "Show more items at once.",
        state,
        |c| c.appearance.compact,
        |c, v| c.appearance.compact = v,
    ));
    popup.add(&switch_row(
        "Show source app",
        "Show which app each copy came from.",
        state,
        |c| c.appearance.show_source_app,
        |c, v| c.appearance.show_source_app = v,
    ));
    page.add(&popup);
    page
}

fn shortcuts_page(window: &adw::PreferencesWindow, state: &Rc<State>) -> adw::PreferencesPage {
    let page = new_page("input-keyboard-symbolic", "Shortcuts");

    let group = new_group("Global shortcuts");
    group.set_description(Some("Click a row, then press the new key combination."));
    group.add(&shortcut_row(
        window,
        state,
        Slot {
            action: HotkeyAction::TogglePopup,
            title: "Open clipboard",
            get: |c| c.shortcuts.toggle_popup.clone(),
            set: |c, v| c.shortcuts.toggle_popup = v,
        },
    ));
    group.add(&shortcut_row(
        window,
        state,
        Slot {
            action: HotkeyAction::ClearHistory,
            title: "Clear history",
            get: |c| c.shortcuts.clear_history.clone(),
            set: |c, v| c.shortcuts.clear_history = v,
        },
    ));
    group.add(&shortcut_row(
        window,
        state,
        Slot {
            action: HotkeyAction::PauseRecording,
            title: "Pause recording",
            get: |c| c.shortcuts.pause_recording.clone(),
            set: |c, v| c.shortcuts.pause_recording = v,
        },
    ));
    page.add(&group);
    page
}

fn privacy_page(
    app: &adw::Application,
    window: &adw::PreferencesWindow,
    state: &Rc<State>,
) -> adw::PreferencesPage {
    let page = new_page("security-high-symbolic", "Privacy");

    let recording = new_group("Recording");
    recording.add(&pause_row(app, state));
    recording.add(&switch_row(
        "Skip passwords",
        "Do not save copies that password managers mark as secret.",
        state,
        |c| c.privacy.skip_sensitive,
        |c, v| c.privacy.skip_sensitive = v,
    ));
    recording.add(&switch_row(
        "Clear history on exit",
        "Delete unpinned items when Clipsy quits.",
        state,
        |c| c.privacy.clear_on_exit,
        |c, v| c.privacy.clear_on_exit = v,
    ));
    page.add(&recording);

    let ignored = adw::PreferencesGroup::new();
    ignored.set_title("Ignored apps");
    ignored.set_description(Some("Copies from these apps are never saved."));
    let list = Rc::new(IgnoredList {
        group: ignored.clone(),
        rows: RefCell::new(Vec::new()),
        state: state.clone(),
        window: window.clone(),
    });
    fill_ignored(&list);
    page.add(&ignored);

    let add_group = adw::PreferencesGroup::new();
    let add_row = adw::ActionRow::new();
    add_row.set_title("Add app…");
    add_row.set_activatable(true);
    add_row.add_suffix(&gtk::Image::from_icon_name("list-add-symbolic"));
    let l = list.clone();
    add_row.connect_activated(move |_| add_ignored_app(&l));
    add_group.add(&add_row);
    page.add(&add_group);

    let data = new_group("Data");
    let clear_row = adw::ActionRow::new();
    clear_row.set_title("Clear all history");
    clear_row.set_subtitle("Removes every item, including pinned ones.");
    let clear_button = gtk::Button::with_label("Clear…");
    clear_button.add_css_class("destructive-action");
    clear_button.set_valign(gtk::Align::Center);
    let (w, s) = (window.clone(), state.clone());
    clear_button.connect_clicked(move |_| confirm_clear(&w, &s));
    clear_row.add_suffix(&clear_button);
    data.add(&clear_row);
    page.add(&data);

    page
}

fn about_page(window: &adw::PreferencesWindow) -> adw::PreferencesPage {
    let page = new_page("help-about-symbolic", "About");
    let group = new_group("Clipsy");

    let row = adw::ActionRow::new();
    row.set_title("About Clipsy");
    row.set_subtitle(&format!("Version {}", env!("CARGO_PKG_VERSION")));
    row.set_activatable(true);
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let w = window.clone();
    row.connect_activated(move |_| show_about(&w));

    group.add(&row);
    page.add(&group);
    page
}

fn show_about(window: &adw::PreferencesWindow) {
    let about = adw::AboutWindow::new();
    about.set_application_name("Clipsy");
    about.set_application_icon(APP_ID);
    about.set_version(env!("CARGO_PKG_VERSION"));
    about.set_comments("Clipboard history for Linux.");
    about.set_license_type(gtk::License::Gpl30);
    about.set_transient_for(Some(window));
    about.present();
}

// ----------------------------------------------------------------------
// Row builders. Each one reads a value from the config and writes it back.
// `get` and `set` are plain functions that name the setting, for example
// `|c| c.general.max_items` and `|c, v| c.general.max_items = v`.
// ----------------------------------------------------------------------

fn new_page(icon: &str, title: &str) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::new();
    page.set_title(title);
    page.set_icon_name(Some(icon));
    page
}

fn new_group(title: &str) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(title);
    group
}

/// An on/off row.
fn switch_row(
    title: &str,
    subtitle: &str,
    state: &Rc<State>,
    get: fn(&Config) -> bool,
    set: fn(&mut Config, bool),
) -> adw::SwitchRow {
    let row = adw::SwitchRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_active(get(&state.config.borrow()));
    let st = state.clone();
    row.connect_active_notify(move |row| {
        let value = row.is_active();
        update_config(&st, |c| set(c, value));
    });
    row
}

/// Choices for "Delete items older than": (days, label). 0 keeps items forever.
const KEEP_DAYS: &[(u32, &str)] = &[
    (1, "1 day"),
    (2, "2 days"),
    (7, "1 week"),
    (30, "30 days"),
    (90, "90 days"),
    (0, "Never"),
];

/// "Delete items older than" drop-down. Applies the new limit right away.
fn keep_days_row(state: &Rc<State>) -> adw::ComboRow {
    let row = adw::ComboRow::new();
    row.set_title("Delete items older than");
    row.set_subtitle("Counted from when you last copied or used an item. Pinned items are kept until you remove them.");
    let labels: Vec<&str> = KEEP_DAYS.iter().map(|(_, label)| *label).collect();
    row.set_model(Some(&gtk::StringList::new(&labels)));
    let current = state.config.borrow().general.keep_days;
    // A value set by hand in config.toml that is not in the list shows as the closest longer choice.
    let index = KEEP_DAYS
        .iter()
        .position(|(days, _)| *days == current)
        .or_else(|| KEEP_DAYS.iter().position(|(days, _)| *days >= current && *days != 0))
        .unwrap_or(KEEP_DAYS.len() - 1);
    row.set_selected(index as u32);
    let st = state.clone();
    row.connect_selected_notify(move |row| {
        let Some((days, _)) = KEEP_DAYS.get(row.selected() as usize) else {
            return;
        };
        update_config(&st, |c| c.general.keep_days = *days);
        let cfg = st.config.borrow().clone();
        if let Err(err) = st.store.borrow_mut().prune(&cfg) {
            tracing::warn!("could not delete old items: {err:#}");
        }
    });
    row
}

/// A number row with plus and minus buttons. `range` is (min, max, step).
fn spin_row(
    title: &str,
    subtitle: &str,
    state: &Rc<State>,
    range: (f64, f64, f64),
    get: fn(&Config) -> f64,
    set: fn(&mut Config, f64),
) -> adw::SpinRow {
    let row = adw::SpinRow::with_range(range.0, range.1, range.2);
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_value(get(&state.config.borrow()));
    let st = state.clone();
    row.connect_value_notify(move |row| {
        let value = row.value();
        update_config(&st, |c| set(c, value));
    });
    row
}

/// A drop-down row. `choices` is a list of (saved value, label shown to the user).
fn choice_row(
    title: &str,
    subtitle: &str,
    choices: &'static [(&'static str, &'static str)],
    state: &Rc<State>,
    get: fn(&Config) -> String,
    set: fn(&mut Config, String),
    on_change: fn(&str),
) -> adw::ComboRow {
    let labels: Vec<&str> = choices.iter().map(|(_, label)| *label).collect();
    let row = adw::ComboRow::new();
    row.set_title(title);
    row.set_subtitle(subtitle);
    row.set_model(Some(&gtk::StringList::new(&labels)));

    let current = get(&state.config.borrow());
    let index = choices
        .iter()
        .position(|(value, _)| *value == current)
        .unwrap_or(0);
    row.set_selected(index as u32);

    let st = state.clone();
    row.connect_selected_notify(move |row| {
        let Some((value, _)) = choices.get(row.selected() as usize) else {
            return;
        };
        update_config(&st, |c| set(c, value.to_string()));
        on_change(value);
    });
    row
}

/// The "start when I log in" row. It also writes or removes the autostart file.
fn autostart_row(state: &Rc<State>) -> adw::SwitchRow {
    let row = adw::SwitchRow::new();
    row.set_title("Start when I log in");
    row.set_subtitle("Clipsy starts in the background.");
    row.set_active(state.config.borrow().general.autostart);
    let st = state.clone();
    row.connect_active_notify(move |row| {
        let enabled = row.is_active();
        if let Err(err) = autostart::set_enabled(enabled) {
            tracing::warn!("could not change autostart: {err:#}");
        }
        update_config(&st, |c| c.general.autostart = enabled);
    });
    row
}

/// The "pause recording" row. It uses the same actions as the shortcut and the popup.
fn pause_row(app: &adw::Application, state: &Rc<State>) -> adw::SwitchRow {
    let row = adw::SwitchRow::new();
    row.set_title("Pause recording");
    row.set_subtitle("While paused, nothing you copy is saved.");
    row.set_active(state.config.borrow().privacy.paused);
    let a = app.clone();
    row.connect_active_notify(move |row| {
        let action = if row.is_active() { "pause" } else { "resume" };
        a.activate_action(action, None);
    });
    row
}

// ----------------------------------------------------------------------
// Shortcuts
// ----------------------------------------------------------------------

/// Describes one shortcut setting: its title, how to read it and how to write it.
#[derive(Clone, Copy)]
struct Slot {
    action: HotkeyAction,
    title: &'static str,
    get: fn(&Config) -> String,
    set: fn(&mut Config, String),
}

fn shortcut_row(window: &adw::PreferencesWindow, state: &Rc<State>, slot: Slot) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(slot.title);
    let current = (slot.get)(&state.config.borrow());
    row.set_subtitle(&accel_label(&current));
    row.set_activatable(true);
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));

    let (window, state, target) = (window.clone(), state.clone(), row.clone());
    row.connect_activated(move |_| record_shortcut(&window, &state, slot, &target));
    row
}

/// Turns a GTK accelerator such as `<Super>v` into text such as `Super+V`.
pub fn accel_label(accel: &str) -> String {
    if accel.is_empty() {
        return "Not set".to_string();
    }
    match gtk::accelerator_parse(accel) {
        Some((key, mods)) => gtk::accelerator_get_label(key, mods).to_string(),
        None => accel.to_string(),
    }
}

/// Opens the "press a new key" dialog for one shortcut.
fn record_shortcut(window: &adw::PreferencesWindow, state: &Rc<State>, slot: Slot, row: &adw::ActionRow) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some(&format!("Set shortcut: {}", slot.title)),
        Some("Press the new key combination.\nEsc cancels. Backspace clears the shortcut."),
    );
    dialog.add_response("cancel", "Cancel");

    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (dialog_c, state_c, row_c, window_c) =
        (dialog.clone(), state.clone(), row.clone(), window.clone());
    keys.connect_key_pressed(move |_, keyval, _code, mods| {
        if keyval == gdk::Key::Escape {
            dialog_c.close();
            return glib::Propagation::Stop;
        }
        // Wait for the real key. Pressing Shift or Ctrl alone does nothing yet.
        if is_modifier(keyval) {
            return glib::Propagation::Stop;
        }
        let accel = if keyval == gdk::Key::BackSpace {
            String::new()
        } else {
            let mods = mods & gtk::accelerator_get_default_mod_mask();
            // A global shortcut without Ctrl/Alt/Super would hijack a normal key in every app.
            let strong = gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK;
            let is_fkey = keyval.name().is_some_and(|n| {
                n.len() > 1 && n.starts_with('F') && n[1..].chars().all(|c| c.is_ascii_digit())
            });
            if !mods.intersects(strong) && !is_fkey {
                // Ignore the press; keep the dialog open so the user can try again.
                return glib::Propagation::Stop;
            }
            gtk::accelerator_name(keyval, mods).to_string()
        };
        dialog_c.close();
        apply_shortcut(&window_c, &state_c, slot, &row_c, &accel);
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
fn apply_shortcut(
    window: &adw::PreferencesWindow,
    state: &Rc<State>,
    slot: Slot,
    row: &adw::ActionRow,
    accel: &str,
) {
    let previous = (slot.get)(&state.config.borrow());
    match set_hotkey(state, slot.action, accel) {
        Ok(()) => {
            update_config(state, |c| (slot.set)(c, accel.to_string()));
            row.set_subtitle(&accel_label(accel));
        }
        Err(err) => {
            tracing::warn!("shortcut {accel} was rejected: {err:#}");
            if let Err(err) = set_hotkey(state, slot.action, &previous) {
                tracing::warn!("could not restore the old shortcut: {err:#}");
            }
            window.add_toast(adw::Toast::new(
                "That shortcut is already used by another app.",
            ));
        }
    }
}

// ----------------------------------------------------------------------
// Ignored apps
// ----------------------------------------------------------------------

/// The list of ignored apps, plus what it needs to redraw itself.
struct IgnoredList {
    group: adw::PreferencesGroup,
    rows: RefCell<Vec<adw::ActionRow>>,
    state: Rc<State>,
    window: adw::PreferencesWindow,
}

/// Redraws the rows from the config. Old rows are removed first.
fn fill_ignored(list: &Rc<IgnoredList>) {
    let old_rows: Vec<adw::ActionRow> = list.rows.borrow_mut().drain(..).collect();
    for row in &old_rows {
        list.group.remove(row);
    }

    let apps = list.state.config.borrow().privacy.ignored_apps.clone();
    for name in apps {
        let row = adw::ActionRow::new();
        row.set_title(&name);

        let remove = gtk::Button::from_icon_name("user-trash-symbolic");
        remove.add_css_class("flat");
        remove.set_valign(gtk::Align::Center);
        remove.set_tooltip_text(Some("Stop ignoring this app"));
        row.add_suffix(&remove);

        let l = list.clone();
        remove.connect_clicked(move |_| {
            update_config(&l.state, |c| c.privacy.ignored_apps.retain(|x| x != &name));
            fill_ignored(&l);
        });

        list.group.add(&row);
        list.rows.borrow_mut().push(row);
    }
}

/// Asks for an app name and adds it to the ignored list.
fn add_ignored_app(list: &Rc<IgnoredList>) {
    let dialog = adw::MessageDialog::new(
        Some(&list.window),
        Some("Ignore an app"),
        Some("Type the app's name, for example KeePassXC. Copies from it will not be saved."),
    );
    let entry = gtk::Entry::new();
    entry.set_placeholder_text(Some("App name"));
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("add", "Add");
    dialog.set_response_appearance("add", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("add"));
    dialog.set_close_response("cancel");

    let (l, e) = (list.clone(), entry.clone());
    dialog.connect_response(None, move |_, response| {
        if response != "add" {
            return;
        }
        let name = e.text().trim().to_string();
        if name.is_empty() {
            return;
        }
        update_config(&l.state, |c| {
            if !c.privacy.ignored_apps.contains(&name) {
                c.privacy.ignored_apps.push(name.clone());
            }
        });
        fill_ignored(&l);
    });
    dialog.present();
}

// ----------------------------------------------------------------------
// Clear history
// ----------------------------------------------------------------------

/// Asks before deleting everything, including pinned items.
fn confirm_clear(window: &adw::PreferencesWindow, state: &Rc<State>) {
    let dialog = adw::MessageDialog::new(
        Some(window),
        Some("Clear all history?"),
        Some("Every item will be removed, including pinned ones. This cannot be undone."),
    );
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("clear", "Clear all");
    dialog.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let (st, w) = (state.clone(), window.clone());
    dialog.connect_response(None, move |_, response| {
        if response != "clear" {
            return;
        }
        if let Err(err) = st.store.borrow().clear(false) {
            tracing::warn!("could not clear history: {err:#}");
            return;
        }
        w.add_toast(adw::Toast::new("History cleared"));
    });
    dialog.present();
}
