//! System tray icon with the Clipsy menu (Windows only).
//!
//! Menu clicks and left clicks on the icon are reported as `Verb`s through the callback given
//! to `Tray::new`. A left click opens the popup (`Verb::Open`); the right click keeps showing the
//! menu. The app loop must call `Tray::poll` regularly (or the `handle_*` methods for each event
//! it reads itself), because muda and tray-icon deliver events through global channels.

use std::cell::Cell;

use anyhow::Context;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::logo::{self, TRAY_ICON_SIZE};
use crate::single_instance::Verb;

const ID_OPEN: &str = "clipsy-open";
const ID_PAUSE: &str = "clipsy-pause";
const ID_SETTINGS: &str = "clipsy-settings";
const ID_CLEAR: &str = "clipsy-clear";
const ID_QUIT: &str = "clipsy-quit";

const LABEL_PAUSE: &str = "Pause capture";
const LABEL_RESUME: &str = "Resume capture";

/// The tray icon and its menu. Keep it alive for as long as the tray should show.
pub struct Tray {
    icon: TrayIcon,
    pause_item: MenuItem,
    paused: Cell<bool>,
    on_verb: Box<dyn Fn(Verb)>,
}

impl Tray {
    /// Creates the tray icon and menu. `on_verb` is called with the verb for each
    /// menu item the user activates and for each left click (only from `poll` or the
    /// `handle_*` methods). `tooltip` is shown when the mouse hovers the icon.
    pub fn new(on_verb: impl Fn(Verb) + 'static, tooltip: &str) -> anyhow::Result<Tray> {
        let open = MenuItem::with_id(ID_OPEN, "Open", true, None);
        let pause_item = MenuItem::with_id(ID_PAUSE, LABEL_PAUSE, true, None);
        let settings = MenuItem::with_id(ID_SETTINGS, "Settings", true, None);
        let clear = MenuItem::with_id(ID_CLEAR, "Clear history", true, None);
        let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

        let menu = Menu::new();
        menu.append(&open).context("could not add the Open menu item")?;
        menu.append(&pause_item).context("could not add the Pause menu item")?;
        menu.append(&settings).context("could not add the Settings menu item")?;
        menu.append(&clear).context("could not add the Clear history menu item")?;
        menu.append(&PredefinedMenuItem::separator())
            .context("could not add the separator")?;
        menu.append(&quit).context("could not add the Quit menu item")?;

        let size = TRAY_ICON_SIZE;
        let icon = Icon::from_rgba(logo::tray_icon_rgba()?, size, size)
            .map_err(|e| anyhow::anyhow!("invalid tray icon image: {e}"))?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .build()
            .context("could not create the system tray icon")?;

        Ok(Tray {
            icon: tray_icon,
            pause_item,
            paused: Cell::new(false),
            on_verb: Box::new(on_verb),
        })
    }

    /// Drains pending menu and tray-icon events and reports each one through the callback.
    pub fn poll(&self) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            self.handle_event(&event);
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            self.handle_tray_event(&event);
        }
    }

    /// A left click (button released) on the icon opens the popup. Other clicks are ignored
    /// here; the right click is handled by the OS and shows the menu.
    pub fn handle_tray_event(&self, event: &TrayIconEvent) {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            (self.on_verb)(Verb::Open);
        }
    }

    /// Reports the verb for one menu event. Unknown ids are ignored.
    pub fn handle_event(&self, event: &MenuEvent) {
        let verb = if event.id == ID_OPEN {
            Verb::Open
        } else if event.id == ID_PAUSE {
            // The item toggles: whatever it shows now is the opposite of the state.
            let now_paused = !self.paused.get();
            self.set_paused(now_paused);
            if now_paused {
                Verb::Pause
            } else {
                Verb::Resume
            }
        } else if event.id == ID_SETTINGS {
            Verb::Settings
        } else if event.id == ID_CLEAR {
            Verb::Clear
        } else if event.id == ID_QUIT {
            Verb::Quit
        } else {
            return;
        };
        (self.on_verb)(verb);
    }

    /// Updates the Pause/Resume item to match the app's paused state.
    pub fn set_paused(&self, paused: bool) {
        self.paused.set(paused);
        self.pause_item.set_text(if paused { LABEL_RESUME } else { LABEL_PAUSE });
    }

    /// Shows a short message by setting the tray tooltip (hover the tray icon to read it),
    /// because tray-icon 0.19 has no balloon notification API. The message is also logged.
    /// The tooltip keeps only the first 127 UTF-16 units, and a later call replaces it.
    pub fn notify(&self, title: &str, body: &str) {
        let text = format!("{title}: {body}");
        tracing::info!("tray notification: {text}");
        self.set_tooltip(&text);
    }

    /// Replaces the tray tooltip text.
    pub fn set_tooltip(&self, text: &str) {
        if let Err(e) = self.icon.set_tooltip(Some(text.to_string())) {
            tracing::warn!("could not set the tray tooltip: {e}");
        }
    }
}
