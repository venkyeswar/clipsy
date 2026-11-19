//! Wiring: builds the shared state, starts the clipboard watcher and the global
//! hotkeys, and connects the application actions used by the CLI and the windows.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use adw::prelude::*;
use anyhow::Context;
use gtk::glib::prelude::IsA;
use clipsy_backends::{Backend, HotkeyAction, NoBackend, SetupNeeded};
use clipsy_core::{ClipEvent, Config, Store};
use gtk::{gio, glib};

use crate::popup::Popup;
use crate::{autostart, gnome_install, settings, welcome};

/// Everything the windows share. Only touched from the GTK main thread.
///
/// `RefCell` lets us change the value through a shared `Rc`. The rule is that a
/// borrow is never held while another piece of code borrows the same value.
pub struct State {
    pub config: RefCell<Config>,
    pub store: RefCell<Store>,
    pub backend: RefCell<Box<dyn Backend>>,
}

/// Folder for the database and small marker files (for example the welcome flag).
pub fn data_dir() -> anyhow::Result<PathBuf> {
    clipsy_core::config::data_dir()
}

/// Adds padding around a widget. `horizontal` is the left and right margin,
/// `vertical` is the top and bottom margin, both in pixels.
pub fn pad(widget: &impl IsA<gtk::Widget>, horizontal: i32, vertical: i32) {
    widget.set_margin_start(horizontal);
    widget.set_margin_end(horizontal);
    widget.set_margin_top(vertical);
    widget.set_margin_bottom(vertical);
}

/// Connects the startup handler. GApplication calls it once in the first instance,
/// before any command (`toggle`, `settings`, ...) is handled.
pub fn connect(app: &adw::Application) {
    app.connect_startup(|app| {
        // Keep the process alive when no window is open. The popup is hidden,
        // not closed, so this is what keeps Clipsy running in the background.
        // `hold` returns a guard that releases the hold when dropped. We want the hold
        // for the whole life of the process, so the guard is intentionally leaked.
        std::mem::forget(app.hold());
        if let Err(err) = start(app) {
            tracing::error!("Clipsy could not start: {err:#}");
            app.quit();
        }
    });
}

fn start(app: &adw::Application) -> anyhow::Result<()> {
    load_css();
    register_app_icons();

    // Missing or broken values fall back to the defaults (the core logs a warning).
    let config = Config::load();

    let store = Store::open(&data_dir()?).context("could not open the history database")?;

    // Backends report through plain std channels. Helper threads (see `pump`) move
    // those messages onto the GTK main thread.
    let (clip_tx, clip_rx) = mpsc::channel::<ClipEvent>();
    let (hotkey_tx, hotkey_rx) = mpsc::channel::<HotkeyAction>();

    // If no backend works (for example on an unsupported desktop), Clipsy still starts. It shows
    // one notification and runs with `NoBackend`: no recording, no shortcuts.
    let started = clipsy_backends::detect().and_then(|mut backend| {
        backend.start(clip_tx, hotkey_tx)?;
        Ok(backend)
    });
    let backend_usable = started.is_ok();
    let backend: Box<dyn Backend> = match started {
        Ok(backend) => backend,
        Err(err) => {
            backend_unavailable(app, &err);
            Box::new(NoBackend)
        }
    };

    let state = Rc::new(State {
        config: RefCell::new(config),
        store: RefCell::new(store),
        backend: RefCell::new(backend),
    });

    apply_theme(&state.config.borrow().appearance.theme);

    let start_at_login = state.config.borrow().general.autostart;
    if let Err(err) = autostart::set_enabled(start_at_login) {
        tracing::warn!("could not update the autostart entry: {err:#}");
    }

    if backend_usable {
        for accel in register_all_hotkeys(&state) {
            notify(
                app,
                "Shortcut already in use",
                &format!("{accel} is used by another app. Choose another one in Settings."),
            );
        }
    }

    // The backend can start fine and still need the user to turn on the GNOME
    // extension. Give it a few seconds to connect, then remind the user once.
    // On GNOME Wayland, install or update the extension files and turn the
    // extension on after a first install, so the user does not need a Terminal.
    let auto_setup = if state.backend.borrow().name() == "gnome" {
        let outcome = gnome_install::auto_setup();
        tracing::info!("GNOME extension setup: {outcome:?}");
        Some(outcome)
    } else {
        None
    };
    if state.backend.borrow().setup_needed() == Some(SetupNeeded::GnomeExtension) {
        let (app, state) = (app.clone(), state.clone());
        glib::timeout_add_local_once(std::time::Duration::from_secs(6), move || {
            if state.backend.borrow().setup_needed() != Some(SetupNeeded::GnomeExtension) {
                return;
            }
            let body = match (auto_setup, gnome_install::status()) {
                (_, gnome_install::Status::Enabled) => {
                    "Log out and log back in once to finish setting up Clipsy on Wayland."
                }
                (_, gnome_install::Status::Disabled) => {
                    "The Clipsy GNOME extension is turned off. Open Clipsy to turn it on."
                }
                _ => "Turn on the Clipsy GNOME extension to record your clipboard on Wayland. Open Clipsy for the steps.",
            };
            notify(&app, "Finish setting up Clipsy", body);
        });
    }

    let popup = Popup::new(app, state.clone());

    // Everything the backend reports ends up in this handler, on the GTK main thread.
    {
        let (state, popup, app) = (state.clone(), popup.clone(), app.clone());
        let handler: Box<dyn FnMut(Message)> = Box::new(move |message| match message {
            Message::Clip(event) => on_clip_changed(&state, &popup, event),
            Message::Hotkey(action) => on_hotkey(&app, &state, &popup, action),
        });
        HANDLER.with(|slot| *slot.borrow_mut() = Some(handler));
    }
    pump(clip_rx, Message::Clip);
    pump(hotkey_rx, Message::Hotkey);

    connect_actions(app, &state, &popup);

    // Delete expired items now, then once an hour, so old items go away even
    // when nothing new is copied.
    prune_history(&state, &popup);
    {
        let (state, popup) = (state.clone(), popup.clone());
        glib::timeout_add_seconds_local(60 * 60, move || {
            prune_history(&state, &popup);
            glib::ControlFlow::Continue
        });
    }

    // Clear history on exit if the user asked for it.
    {
        let state = state.clone();
        app.connect_shutdown(move |_| {
            let clear = state.config.borrow().privacy.clear_on_exit;
            if clear {
                if let Err(err) = state.store.borrow().clear(false) {
                    tracing::warn!("could not clear history on exit: {err:#}");
                }
            }
        });
    }

    Ok(())
}

/// Tells the user, once, that clipboard history is off because no backend works.
fn backend_unavailable(app: &adw::Application, err: &anyhow::Error) {
    tracing::error!("clipboard history is not available: {err:#}");
    notify(app, "Clipboard history is off", &format!("{err:#}"));
}

/// Defines the actions (`app.toggle`, `app.settings`, ...) and what happens on
/// `activate` (plain `clipsy` with no arguments).
fn connect_actions(app: &adw::Application, state: &Rc<State>, popup: &Rc<Popup>) {
    let p = popup.clone();
    add_action(app, "toggle", move || p.toggle());

    let (a, s) = (app.clone(), state.clone());
    add_action(app, "settings", move || settings::show(&a, &s));

    let (a, s, p) = (app.clone(), state.clone(), popup.clone());
    add_action(app, "pause", move || set_paused(&a, &s, &p, true));

    let (a, s, p) = (app.clone(), state.clone(), popup.clone());
    add_action(app, "resume", move || set_paused(&a, &s, &p, false));

    let (s, p) = (state.clone(), popup.clone());
    add_action(app, "clear", move || clear_history(&s, &p, true));

    let a = app.clone();
    add_action(app, "quit", move || a.quit());

    // `clipsy` with no arguments: first run shows the welcome, later runs show Settings.
    let (s, a) = (state.clone(), app.clone());
    app.connect_activate(move |_| {
        if welcome::is_needed() {
            welcome::show(&a, &s);
        } else {
            settings::show(&a, &s);
        }
    });
}

/// Helper that creates a GAction with no parameter and adds it to the application.
fn add_action(app: &adw::Application, name: &str, handler: impl Fn() + 'static) {
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| handler());
    app.add_action(&action);
}

/// What the backend reports from its own threads.
enum Message {
    Clip(ClipEvent),
    Hotkey(HotkeyAction),
}

/// The function type that handles messages on the GTK main thread.
type MessageHandler = Box<dyn FnMut(Message)>;

thread_local! {
    /// The function that handles messages. It lives on the GTK main thread only,
    /// because it uses `Rc` values, which cannot be sent to other threads.
    static HANDLER: RefCell<Option<MessageHandler>> = RefCell::new(None);
}

/// Starts a helper thread that waits on `rx`. Each message it receives is passed to
/// the GTK main loop. The thread sleeps while nothing happens, so idle CPU stays near zero.
fn pump<T: Send + 'static>(rx: mpsc::Receiver<T>, wrap: fn(T) -> Message) {
    thread::spawn(move || {
        while let Ok(item) = rx.recv() {
            // Only `message` (which is Send) is moved to the main thread.
            let message = wrap(item);
            glib::idle_add_once(move || deliver(message));
        }
    });
}

/// Runs on the GTK main thread: hands one message to the handler.
fn deliver(message: Message) {
    HANDLER.with(|slot| {
        if let Some(handler) = slot.borrow_mut().as_mut() {
            handler(message);
        }
    });
}

/// A new clipboard item arrived from the backend.
fn on_clip_changed(state: &State, popup: &Popup, event: ClipEvent) {
    if state.config.borrow().privacy.paused {
        return;
    }
    let stored = {
        let cfg = state.config.borrow();
        let mut store = state.store.borrow_mut();
        match store.insert_event(&event, &cfg) {
            Ok(Some(_)) => {
                if let Err(err) = store.prune(&cfg) {
                    tracing::warn!("could not prune history: {err:#}");
                }
                true
            }
            // Ignored on purpose (duplicate, whitespace only, sensitive, ...).
            Ok(None) => false,
            Err(err) => {
                tracing::warn!("could not save a clipboard item: {err:#}");
                false
            }
        }
    };
    if stored {
        popup.refresh();
    }
}

/// Applies the age, count and size limits to the history.
fn prune_history(state: &State, popup: &Popup) {
    let cfg = state.config.borrow().clone();
    match state.store.borrow_mut().prune(&cfg) {
        Ok(0) => {}
        Ok(_) => popup.refresh(),
        Err(err) => tracing::warn!("could not clean up history: {err:#}"),
    }
}

/// A global shortcut was pressed.
fn on_hotkey(app: &adw::Application, state: &Rc<State>, popup: &Popup, action: HotkeyAction) {
    match action {
        HotkeyAction::TogglePopup => popup.toggle(),
        HotkeyAction::ClearHistory => clear_history(state, popup, true),
        HotkeyAction::PauseRecording => {
            let paused = state.config.borrow().privacy.paused;
            set_paused(app, state, popup, !paused);
        }
    }
}

/// Pauses or resumes recording, saves the choice and tells the user.
pub fn set_paused(app: &adw::Application, state: &State, popup: &Popup, paused: bool) {
    update_config(state, |cfg| cfg.privacy.paused = paused);
    popup.refresh();
    let text = if paused {
        "Clipboard recording is paused."
    } else {
        "Clipboard recording is on again."
    };
    notify(app, "Clipsy", text);
}

/// Deletes history. Pinned items survive when `keep_pinned` is true.
pub fn clear_history(state: &State, popup: &Popup, keep_pinned: bool) {
    if let Err(err) = state.store.borrow().clear(keep_pinned) {
        tracing::warn!("could not clear history: {err:#}");
    }
    popup.refresh();
}

/// Changes the settings in memory and writes config.toml right away.
pub fn update_config(state: &State, change: impl FnOnce(&mut Config)) {
    let mut cfg = state.config.borrow_mut();
    change(&mut cfg);
    if let Err(err) = cfg.save() {
        tracing::warn!("could not save settings: {err:#}");
    }
}

/// Registers (or, with an empty `accel`, removes) one global shortcut.
/// Returns an error when another app already owns the key combination.
pub fn set_hotkey(state: &State, action: HotkeyAction, accel: &str) -> anyhow::Result<()> {
    let mut backend = state.backend.borrow_mut();
    backend.unregister_hotkey(action);
    if accel.is_empty() {
        return Ok(());
    }
    backend.register_hotkey(action, accel)
}

/// Registers the shortcuts from the config. Returns the ones that failed.
fn register_all_hotkeys(state: &State) -> Vec<String> {
    let bindings = {
        let cfg = state.config.borrow();
        [
            (HotkeyAction::TogglePopup, cfg.shortcuts.toggle_popup.clone()),
            (HotkeyAction::ClearHistory, cfg.shortcuts.clear_history.clone()),
            (HotkeyAction::PauseRecording, cfg.shortcuts.pause_recording.clone()),
        ]
    };
    let mut failed = Vec::new();
    for (action, accel) in bindings {
        if let Err(err) = set_hotkey(state, action, &accel) {
            tracing::warn!("could not register shortcut {accel}: {err:#}");
            failed.push(accel);
        }
    }
    failed
}

/// Applies the theme setting. libadwaita then recolors every window.
pub fn apply_theme(theme: &str) {
    let scheme = match theme {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

/// Shows a desktop notification.
pub fn notify(app: &adw::Application, title: &str, body: &str) {
    let notification = gio::Notification::new(title);
    notification.set_body(Some(body));
    app.send_notification(Some("clipsy-status"), &notification);
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../resources/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}


/// The app's own icons. The SVG files are compiled into the binary, so the icons
/// exist even when Clipsy runs from a build folder instead of an installed package.
const APP_ICONS: &[(&str, &[u8])] = &[
    (
        "io.github.clipsy.Clipsy",
        include_bytes!("../../../data/icons/io.github.clipsy.Clipsy.svg"),
    ),
    (
        "io.github.clipsy.Clipsy-symbolic",
        include_bytes!("../../../data/icons/io.github.clipsy.Clipsy-symbolic.svg"),
    ),
];

/// Minimal theme index. GTK only treats a folder as an icon theme when it has one.
const ICON_INDEX_THEME: &str = "[Icon Theme]\n\
Name=hicolor\n\
Comment=Clipsy app icons\n\
Directories=scalable/apps\n\
Scalable=Directories\n\
\n\
[scalable/apps]\n\
Size=64\n\
Type=Scalable\n\
MinSize=8\n\
MaxSize=512\n";

/// Folder added to the icon theme search path. It is laid out like an icon theme:
/// `<root>/hicolor/index.theme` and `<root>/hicolor/scalable/apps/<name>.svg`.
fn app_icon_root() -> PathBuf {
    glib::user_cache_dir().join("clipsy").join("icons")
}

/// Folder that holds the app icon files inside `root`.
fn app_icon_dir(root: &std::path::Path) -> PathBuf {
    root.join("hicolor").join("scalable").join("apps")
}

/// Path of one app icon file inside `root`.
fn app_icon_path(root: &std::path::Path, icon_name: &str) -> PathBuf {
    app_icon_dir(root).join(format!("{icon_name}.svg"))
}

/// Writes the icon files and the theme index under `root`. A file is only written
/// when it is missing or its size differs from the embedded copy.
fn write_app_icons(root: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(app_icon_dir(root))?;
    for (name, svg) in APP_ICONS {
        let path = app_icon_path(root, name);
        let up_to_date = std::fs::metadata(&path).is_ok_and(|m| m.len() == svg.len() as u64);
        if !up_to_date {
            std::fs::write(&path, svg)?;
        }
    }
    let index = root.join("hicolor").join("index.theme");
    let up_to_date = std::fs::metadata(&index).is_ok_and(|m| m.len() == ICON_INDEX_THEME.len() as u64);
    if !up_to_date {
        std::fs::write(&index, ICON_INDEX_THEME)?;
    }
    Ok(())
}

/// Makes the app icons available by name (`APP_ID`, the symbolic one). Does nothing
/// when the icon theme already has them, which is the case for installed packages.
fn register_app_icons() {
    let Some(display) = gtk::gdk::Display::default() else { return };
    let theme = gtk::IconTheme::for_display(&display);
    if APP_ICONS.iter().all(|(name, _)| theme.has_icon(name)) {
        return;
    }
    let root = app_icon_root();
    match write_app_icons(&root) {
        // Adding the path makes GTK scan it for icons.
        Ok(()) => theme.add_search_path(&root),
        Err(err) => tracing::debug!("could not write app icons to {}: {err}", root.display()),
    }
}

#[cfg(test)]
mod icon_tests {
    use super::*;

    #[test]
    fn icon_path_follows_theme_layout() {
        let root = std::path::Path::new("/cache/clipsy/icons");
        assert_eq!(
            app_icon_path(root, "io.github.clipsy.Clipsy"),
            std::path::Path::new("/cache/clipsy/icons/hicolor/scalable/apps/io.github.clipsy.Clipsy.svg")
        );
    }

    #[test]
    fn writes_icons_and_repairs_damaged_files() {
        let dir = tempfile::tempdir().unwrap();
        write_app_icons(dir.path()).unwrap();
        for (name, svg) in APP_ICONS {
            let path = app_icon_path(dir.path(), name);
            assert_eq!(std::fs::read(&path).unwrap().as_slice(), *svg);
        }
        // A damaged (wrong size) file is replaced on the next run.
        let path = app_icon_path(dir.path(), APP_ICONS[0].0);
        std::fs::write(&path, b"x").unwrap();
        write_app_icons(dir.path()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap().as_slice(), APP_ICONS[0].1);
    }
}
