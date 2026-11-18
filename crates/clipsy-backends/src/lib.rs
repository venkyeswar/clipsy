//! Backend contract. X11 impl in x11.rs.
use clipsy_core::{ClipEvent, Format};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteMode { Normal, Terminal }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

pub mod x11;

/// Message shown when the session is Wayland (no backend exists for it yet).
pub const WAYLAND_UNSUPPORTED: &str = "Wayland not supported yet; use an Xorg session";

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

/// Pick a backend for the current session. Currently only X11 is implemented.
/// Wayland is rejected explicitly, even when XWayland provides a DISPLAY.
pub fn detect() -> anyhow::Result<Box<dyn Backend>> {
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland = session.eq_ignore_ascii_case("wayland")
        || (!session.eq_ignore_ascii_case("x11") && std::env::var_os("WAYLAND_DISPLAY").is_some());
    if wayland {
        anyhow::bail!(WAYLAND_UNSUPPORTED);
    }
    let session_x11 = session.eq_ignore_ascii_case("x11");
    let has_display = std::env::var_os("DISPLAY").is_some_and(|d| !d.is_empty());
    if session_x11 || has_display {
        return Ok(Box::new(x11::X11Backend::new()?));
    }
    anyhow::bail!("no supported session found (XDG_SESSION_TYPE is not x11 and DISPLAY is unset)")
}
