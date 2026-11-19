//! System tray icon with the Clipsy menu (Windows only).
//!
//! Menu clicks are reported as `Verb`s through the callback given to `Tray::new`.
//! The app loop must call `Tray::poll` regularly (or `Tray::handle_event` for each
//! event it reads itself), because muda delivers menu events through a global channel.

use std::cell::Cell;

use anyhow::Context;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::single_instance::Verb;

const ICON_SIZE: u32 = 32;
/// Corner radius of the generated icon, in pixels.
const ICON_RADIUS: f32 = 7.0;
/// Accent colour of the generated icon (RGB).
const ICON_COLOR: [u8; 3] = [53, 132, 228];

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
    /// menu item the user activates (only from `poll` or `handle_event`).
    pub fn new(on_verb: impl Fn(Verb) + 'static) -> anyhow::Result<Tray> {
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

        let icon = Icon::from_rgba(icon_rgba(), ICON_SIZE, ICON_SIZE)
            .map_err(|e| anyhow::anyhow!("invalid tray icon image: {e}"))?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Clipsy")
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

    /// Drains pending menu events and reports each one through the callback.
    pub fn poll(&self) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            self.handle_event(&event);
        }
    }

    /// Reports the verb for one menu event. Unknown ids are ignored.
    pub fn handle_event(&self, event: &MenuEvent) {
        let verb = if event.id == ID_OPEN {
            Verb::Toggle
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
        if let Err(e) = self.icon.set_tooltip(Some(text)) {
            tracing::warn!("could not set the tray tooltip: {e}");
        }
    }
}

/// Builds the RGBA pixels of a solid rounded square in the accent colour.
/// Pixels outside the rounded corners are fully transparent.
fn icon_rgba() -> Vec<u8> {
    let size = ICON_SIZE as f32;
    let mut rgba = Vec::with_capacity((ICON_SIZE * ICON_SIZE * 4) as usize);
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            // Distance from the pixel to the inner rectangle (inset by the radius).
            let nearest_x = px.clamp(ICON_RADIUS, size - ICON_RADIUS);
            let nearest_y = py.clamp(ICON_RADIUS, size - ICON_RADIUS);
            let inside = (px - nearest_x).hypot(py - nearest_y) <= ICON_RADIUS;
            let alpha = if inside { 255 } else { 0 };
            rgba.extend_from_slice(&[ICON_COLOR[0], ICON_COLOR[1], ICON_COLOR[2], alpha]);
        }
    }
    rgba
}
