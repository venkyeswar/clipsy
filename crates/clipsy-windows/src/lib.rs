//! Windows port of Clipsy. Shared logic lives in clipsy-core and clipsy-backends.

pub mod formats;
pub mod hotkey;
pub mod popup_state;
pub mod settings;
pub mod terminals;
pub mod util;

#[cfg(windows)]
pub mod app;
pub mod autostart;
#[cfg(windows)]
pub mod backend;
#[cfg(windows)]
pub mod clipboard;
#[cfg(windows)]
pub mod popup;
pub mod paste;
pub mod single_instance;
#[cfg(windows)]
pub mod tray;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_clipsy_windows() {
        assert_eq!(env!("CARGO_PKG_NAME"), "clipsy-windows");
    }
}
