//! Backend contract. X11 impl in x11.rs.
use clipsy_core::{ClipEvent, Format};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode {
    Normal,
    Terminal,
    /// Non-text item such as an image: always send plain Ctrl+V, even to a terminal, so tools that read the clipboard themselves receive it.
    Binary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction { TogglePopup, ClearHistory, PauseRecording }

pub trait Backend: Send {
    fn name(&self) -> &'static str;
    /// Spawn watcher thread(s); send each new clipboard content on `clips`, and fired hotkeys on `hotkeys`.
    fn start(&mut self, clips: Sender<ClipEvent>, hotkeys: Sender<HotkeyAction>) -> anyhow::Result<()>;
    /// Become the clipboard owner offering all given formats.
    fn set_clipboard(&self, formats: &[Format]) -> anyhow::Result<()>;
    /// Remember the currently focused window (call right before showing the popup).
    fn remember_active_window(&self);
    /// Re-focus remembered window and send Ctrl+V (or Ctrl+Shift+V for Terminal mode / auto-detected terminals).
    fn paste(&self, mode: PasteMode) -> anyhow::Result<()>;
    fn supports_paste(&self) -> bool;
    /// accel uses GTK syntax e.g. "<Super>v". Err if grab fails (conflict).
    fn register_hotkey(&mut self, action: HotkeyAction, accel: &str) -> anyhow::Result<()>;
    fn unregister_hotkey(&mut self, action: HotkeyAction);
    /// Called right before the popup window is presented. `position` is
    /// `appearance.popup_position` ("cursor" or "center"). The GNOME backend asks
    /// the Shell extension to place and focus the popup; X11 does nothing here.
    fn popup_will_show(&self, _position: &str) {}
    /// Extra window classes (config `general.terminal_apps`) that `paste` treats as
    /// terminals, on top of the built-in list. Matched exactly, ignoring case.
    fn set_terminal_apps(&self, _apps: &[String]) {}
    /// Setup the user still has to do before this backend fully works, if any.
    fn setup_needed(&self) -> Option<SetupNeeded> {
        None
    }
}

/// Something the user must do before clipboard history works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupNeeded {
    /// GNOME on Wayland: the Clipsy GNOME Shell extension is not installed, not
    /// enabled, or has not connected yet (it loads at the next login after install).
    GnomeExtension,
}

/// UUID of the companion GNOME Shell extension (directory name under
/// `gnome-shell/extensions/`).
#[cfg(target_os = "linux")]
pub const GNOME_EXTENSION_UUID: &str = "clipsy@venkyeswar.github.io";

#[cfg(target_os = "linux")]
pub mod gnome;
#[cfg(target_os = "linux")]
pub mod x11;

/// Message shown when the session is Wayland (no backend exists for it yet).
#[cfg(target_os = "linux")]
pub const WAYLAND_UNSUPPORTED: &str = "This Wayland desktop is not supported yet. Clipsy works on GNOME (Ubuntu's default desktop) and on Xorg.";

/// Stand-in backend used when no real backend could start. Clipboard watching,
/// hotkeys and paste are all unavailable, but the app itself keeps running.
pub struct NoBackend;

impl Backend for NoBackend {
    fn name(&self) -> &'static str {
        "none"
    }
    fn start(&mut self, _clips: Sender<ClipEvent>, _hotkeys: Sender<HotkeyAction>) -> anyhow::Result<()> {
        Ok(())
    }
    fn set_clipboard(&self, _formats: &[Format]) -> anyhow::Result<()> {
        anyhow::bail!("no clipboard backend is running")
    }
    fn remember_active_window(&self) {}
    fn paste(&self, _mode: PasteMode) -> anyhow::Result<()> {
        anyhow::bail!("no clipboard backend is running")
    }
    fn supports_paste(&self) -> bool {
        false
    }
    fn register_hotkey(&mut self, _action: HotkeyAction, _accel: &str) -> anyhow::Result<()> {
        anyhow::bail!("no clipboard backend is running, so shortcuts cannot be used")
    }
    fn unregister_hotkey(&mut self, _action: HotkeyAction) {}
}

/// True when `XDG_CURRENT_DESKTOP` (colon-separated, e.g. "ubuntu:GNOME") names GNOME.
#[cfg(target_os = "linux")]
fn is_gnome_desktop() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|d| d.trim().eq_ignore_ascii_case("gnome"))
}

/// Pick a backend for the current session: GNOME on Wayland uses the Shell extension,
/// Xorg uses X11, other Wayland desktops are rejected (even when XWayland provides a
/// DISPLAY). `CLIPSY_BACKEND=x11|gnome` forces a backend, for testing.
#[cfg(target_os = "linux")]
pub fn detect() -> anyhow::Result<Box<dyn Backend>> {
    match std::env::var("CLIPSY_BACKEND").unwrap_or_default().to_ascii_lowercase().as_str() {
        "x11" => return Ok(Box::new(x11::X11Backend::new()?)),
        "gnome" => return Ok(Box::new(gnome::GnomeBackend::new()?)),
        _ => {}
    }
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland = session.eq_ignore_ascii_case("wayland")
        || (!session.eq_ignore_ascii_case("x11") && std::env::var_os("WAYLAND_DISPLAY").is_some());
    if wayland {
        if is_gnome_desktop() {
            return Ok(Box::new(gnome::GnomeBackend::new()?));
        }
        anyhow::bail!(WAYLAND_UNSUPPORTED);
    }
    let session_x11 = session.eq_ignore_ascii_case("x11");
    let has_display = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
    if session_x11 || has_display {
        return Ok(Box::new(x11::X11Backend::new()?));
    }
    anyhow::bail!("no supported session found (XDG_SESSION_TYPE is not x11 and DISPLAY is unset)")
}

/// No built-in backend exists off Linux yet. The Windows app supplies its own backend.
#[cfg(not(target_os = "linux"))]
pub fn detect() -> anyhow::Result<Box<dyn Backend>> {
    anyhow::bail!("no built-in backend on this platform")
}
