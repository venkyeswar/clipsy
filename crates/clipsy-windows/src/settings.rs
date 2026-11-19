//! Settings: hotkey validation (all platforms) and, on Windows, the settings window.

use crate::hotkey::parse_accel;

// Slint settings window: added in a later task

/// Checks that a shortcut string such as "<Ctrl><Shift>v" can be used as the
/// popup hotkey. Returns a human-readable message when it cannot.
pub fn validate_hotkey(s: &str) -> Result<(), String> {
    parse_accel(s).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_hotkey_accepted() {
        assert!(validate_hotkey("<Ctrl><Shift>v").is_ok());
    }

    #[test]
    fn hotkey_without_key_rejected_with_message() {
        let err = validate_hotkey("<Ctrl>").unwrap_err();
        assert!(err.contains("missing key"), "{err}");
    }
}
