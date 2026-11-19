//! Windows application wiring (Windows only).
//!
//! `run` decides whether this process is the running instance or a CLI verb for one. As the
//! running instance it starts the clipboard watcher, hotkeys, popup, tray and settings window,
//! then runs the Slint event loop. A timer on that thread moves messages from the backend,
//! the hotkeys, the pipe server and the tray into the store and the windows.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use anyhow::{Context, Result};
use clipsy_backends::{Backend, HotkeyAction};
use clipsy_core::config::{config_path, data_dir};
use clipsy_core::{ClipEvent, Config, Store};
use slint::{Timer, TimerMode};

use crate::autostart;
use crate::backend::WindowsBackend;
use crate::popup::Popup;
use crate::settings::SettingsWindow;
use crate::single_instance::{self, Role, Verb};
use crate::tray::Tray;

/// How often the UI thread drains the message channels.
const PUMP_EVERY: Duration = Duration::from_millis(50);
/// How often expired items are deleted while the app runs (also done once at start).
const PRUNE_EVERY: Duration = Duration::from_secs(60 * 60);

const USAGE: &str = "Clipsy for Windows - clipboard history

Usage:
  clipsy-windows                 start Clipsy (does nothing if it is already running)
  clipsy-windows --background    the same, used by start at login
  clipsy-windows toggle          show or hide the clipboard popup
  clipsy-windows settings        open the Settings window
  clipsy-windows pause|resume    pause or resume recording
  clipsy-windows clear           clear history (pinned items are kept)
  clipsy-windows quit            stop Clipsy
  clipsy-windows --version       print the version";

/// Runs Clipsy. `args` are the command-line arguments without the program name.
///
/// Returns when the app quits, or straight away for `--version`, `--help` and CLI verbs that
/// were forwarded to a running instance. Bad arguments print usage and exit with status 2.
pub fn run(args: Vec<String>) -> Result<()> {
    let mut verb: Option<Verb> = None;
    let mut background = false;
    for arg in &args {
        match arg.as_str() {
            "-V" | "--version" => {
                println!("clipsy-windows {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "--background" => background = true,
            other => match (single_instance::parse_verb(other), verb) {
                (Some(parsed), None) => verb = Some(parsed),
                _ => {
                    eprintln!("unknown or repeated argument: {other}\n\n{USAGE}");
                    std::process::exit(2);
                }
            },
        }
    }

    let guard = match single_instance::acquire()? {
        Role::Secondary => {
            // A login start (`--background`) must not pop the popup open in a running instance.
            if verb.is_none() && background {
                return Ok(());
            }
            return single_instance::forward(verb.unwrap_or(Verb::Toggle));
        }
        Role::Primary(guard) => guard,
    };
    // Held until this function returns; dropping it releases the single-instance mutex.
    let _instance = guard;

    let (verb_tx, verb_rx) = mpsc::channel::<Verb>();
    single_instance::serve(verb_tx.clone());
    if let Some(verb) = verb {
        // A verb given to a process that is the first instance is handled like one from the pipe.
        let _ = verb_tx.send(verb);
    }

    // Read before `Config::load`, which does not create the file.
    let first_run = config_path().is_ok_and(|path| !path.exists());
    let config = Rc::new(RefCell::new(Config::load()));
    if first_run {
        // Writing the file marks this as no longer the first run.
        if let Err(e) = config.borrow().save() {
            tracing::warn!("could not write the first-run config: {e:#}");
        }
    }
    let hotkey_name = config.borrow().shortcuts.toggle_popup.clone();
    if first_run {
        tracing::info!("Clipsy is running. Press {hotkey_name} to open it.");
    }
    apply_autostart(&config.borrow());

    let store = Rc::new(RefCell::new(
        Store::open(&data_dir()?).context("could not open the history database")?,
    ));

    let (clip_tx, clip_rx) = mpsc::channel::<ClipEvent>();
    let (hotkey_tx, hotkey_rx) = mpsc::channel::<HotkeyAction>();
    let mut backend = WindowsBackend::new();
    backend
        .start(clip_tx, hotkey_tx)
        .context("could not start clipboard watching")?;

    // Shortcuts must be registered before the backend is shared. A shortcut that is taken
    // or invalid is reported and skipped; the app keeps running without it.
    let bindings = {
        let cfg = config.borrow();
        [
            (HotkeyAction::TogglePopup, cfg.shortcuts.toggle_popup.clone()),
            (HotkeyAction::ClearHistory, cfg.shortcuts.clear_history.clone()),
            (HotkeyAction::PauseRecording, cfg.shortcuts.pause_recording.clone()),
        ]
    };
    let mut shortcut_problems = Vec::new();
    for (action, accel) in bindings {
        if accel.is_empty() {
            continue;
        }
        if let Err(e) = backend.register_hotkey(action, &accel) {
            tracing::warn!("shortcut {accel} for {action:?} is not available: {e:#}");
            shortcut_problems.push(format!("Shortcut {accel} is not available. Choose another one in Settings."));
        }
    }
    backend.set_terminal_apps(&config.borrow().general.terminal_apps);
    let backend = Rc::new(backend);

    // The popup creates the Slint window, which also starts Slint's backend on this thread.
    // The tray is created after that, so its menu messages are pumped by the Slint event loop.
    let popup = Rc::new(Popup::new(store.clone(), config.clone(), backend.clone()));
    let tray_verbs = verb_tx.clone();
    let tray = Rc::new(Tray::new(move |verb| {
        let _ = tray_verbs.send(verb);
    })?);
    tray.set_paused(config.borrow().privacy.paused);
    for problem in &shortcut_problems {
        tray.notify("Clipsy", problem);
    }

    let app = Rc::new(App {
        config,
        store,
        backend,
        popup,
        tray,
        settings: RefCell::new(None),
        inbox: Inbox {
            clips: clip_rx,
            hotkeys: hotkey_rx,
            verbs: verb_rx,
        },
    });

    // Expired items go now and then once an hour, so old items leave even when nothing is copied.
    app.prune();
    let message_timer = Timer::default();
    {
        let app = app.clone();
        message_timer.start(TimerMode::Repeated, PUMP_EVERY, move || app.tick());
    }
    let prune_timer = Timer::default();
    {
        let app = app.clone();
        prune_timer.start(TimerMode::Repeated, PRUNE_EVERY, move || app.prune());
    }

    // No window is shown at start, so this loop runs until a Quit verb asks it to stop.
    slint::run_event_loop_until_quit().context("the event loop stopped with an error")?;
    Ok(())
}

/// Turns on start at login when the config asks for it and the Run key is missing (for
/// example after the exe moved). Failures are logged; they do not stop the app.
fn apply_autostart(config: &Config) {
    if !config.general.autostart || autostart::is_enabled() {
        return;
    }
    let result = std::env::current_exe()
        .context("could not find the Clipsy executable")
        .and_then(|exe| autostart::set_enabled(&exe, true));
    if let Err(e) = result {
        tracing::warn!("could not turn on start at login: {e:#}");
    }
}

/// Channels from the backend, the hotkeys, the pipe server and the tray menu. The UI thread
/// drains them on every tick.
struct Inbox {
    clips: Receiver<ClipEvent>,
    hotkeys: Receiver<HotkeyAction>,
    verbs: Receiver<Verb>,
}

/// Everything the UI thread needs. Lives in an `Rc` shared with the timers.
struct App {
    config: Rc<RefCell<Config>>,
    store: Rc<RefCell<Store>>,
    backend: Rc<WindowsBackend>,
    popup: Rc<Popup>,
    tray: Rc<Tray>,
    /// Created the first time Settings is opened, then reused.
    settings: RefCell<Option<SettingsWindow>>,
    inbox: Inbox,
}

impl App {
    /// One pass of the message pump. Runs on the UI thread every `PUMP_EVERY`.
    fn tick(&self) {
        // Menu clicks reach `verbs` from here, so the tray must be polled before draining.
        self.tray.poll();
        while let Ok(event) = self.inbox.clips.try_recv() {
            self.on_clip(&event);
        }
        while let Ok(action) = self.inbox.hotkeys.try_recv() {
            self.on_hotkey(action);
        }
        while let Ok(verb) = self.inbox.verbs.try_recv() {
            self.on_verb(verb);
        }
    }

    /// A new clipboard item arrived from the backend. Dropped while paused.
    fn on_clip(&self, event: &ClipEvent) {
        if self.config.borrow().privacy.paused {
            return;
        }
        let stored = {
            let cfg = self.config.borrow();
            let mut store = self.store.borrow_mut();
            match store.insert_event(event, &cfg) {
                Ok(Some(_)) => {
                    if let Err(e) = store.prune(&cfg) {
                        tracing::warn!("could not prune history: {e:#}");
                    }
                    true
                }
                // Ignored on purpose (duplicate, whitespace only, sensitive, ...).
                Ok(None) => false,
                Err(e) => {
                    tracing::warn!("could not save a clipboard item: {e:#}");
                    false
                }
            }
        };
        if stored {
            self.popup.refresh();
        }
    }

    /// Deletes expired items and refreshes the popup if anything went.
    fn prune(&self) {
        // The result is taken out of the store borrow first: `refresh` borrows the store too.
        let pruned = {
            let cfg = self.config.borrow();
            self.store.borrow_mut().prune(&cfg)
        };
        match pruned {
            Ok(0) => {}
            Ok(_) => self.popup.refresh(),
            Err(e) => tracing::warn!("could not clean up history: {e:#}"),
        }
    }

    fn on_hotkey(&self, action: HotkeyAction) {
        match action {
            HotkeyAction::TogglePopup => self.popup.toggle(),
            HotkeyAction::ClearHistory => self.clear_history(),
            HotkeyAction::PauseRecording => {
                let paused = self.config.borrow().privacy.paused;
                self.set_paused(!paused);
            }
        }
    }

    fn on_verb(&self, verb: Verb) {
        match verb {
            Verb::Toggle => self.popup.toggle(),
            Verb::Settings => self.show_settings(),
            Verb::Pause => self.set_paused(true),
            Verb::Resume => self.set_paused(false),
            Verb::Clear => self.clear_history(),
            Verb::Quit => self.quit(),
        }
    }

    /// Pauses or resumes recording, saves the choice and updates the tray menu.
    fn set_paused(&self, paused: bool) {
        {
            let mut cfg = self.config.borrow_mut();
            cfg.privacy.paused = paused;
            if let Err(e) = cfg.save() {
                tracing::warn!("could not save settings: {e:#}");
            }
        }
        self.tray.set_paused(paused);
        tracing::info!("clipboard recording {}", if paused { "paused" } else { "resumed" });
    }

    /// Deletes history. Pinned items are kept.
    fn clear_history(&self) {
        let cleared = self.store.borrow().clear(true);
        if let Err(e) = cleared {
            tracing::warn!("could not clear history: {e:#}");
        }
        self.popup.refresh();
    }

    fn show_settings(&self) {
        let mut slot = self.settings.borrow_mut();
        if slot.is_none() {
            let config = self.config.clone();
            let popup = self.popup.clone();
            let tray = self.tray.clone();
            // The window has already written the config and applied the terminal list and the
            // shortcut. Here the tray and the popup pick up the new values.
            let on_saved = move || {
                let paused = config.borrow().privacy.paused;
                tray.set_paused(paused);
                popup.refresh();
            };
            match SettingsWindow::new(self.config.clone(), self.backend.clone(), on_saved) {
                Ok(window) => *slot = Some(window),
                Err(e) => {
                    tracing::warn!("could not create the settings window: {e:#}");
                    return;
                }
            }
        }
        if let Some(window) = slot.as_ref() {
            window.show();
        }
    }

    /// Quit: clear history if the config says so, then stop the event loop.
    fn quit(&self) {
        let clear_on_exit = self.config.borrow().privacy.clear_on_exit;
        if clear_on_exit {
            // Same as the Linux app: "clear on exit" removes pinned items too.
            if let Err(e) = self.store.borrow().clear(false) {
                tracing::warn!("could not clear history on exit: {e:#}");
            }
        }
        if let Err(e) = slint::quit_event_loop() {
            tracing::warn!("could not stop the event loop: {e}");
        }
    }
}
