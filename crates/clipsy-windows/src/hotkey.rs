//! Global hotkey handling: parses GTK-style accelerators such as "<Ctrl><Shift>v".

/// Default popup shortcut on Windows: Win+Shift+V.
pub const WINDOWS_DEFAULT: &str = "<Super><Shift>v";
/// Used when [`WINDOWS_DEFAULT`] cannot be registered (Windows already owns Win+Shift+V).
pub const FALLBACK: &str = "<Ctrl><Shift>v";
/// The default that older builds wrote to config.toml on Windows.
const LEGACY_DEFAULT: &str = "<Ctrl><Shift>v";

/// Moves the exact legacy default to [`WINDOWS_DEFAULT`]. Any other value, including a custom
/// shortcut, is returned unchanged.
pub fn migrate_toggle_default(current: &str) -> String {
    if current == LEGACY_DEFAULT {
        WINDOWS_DEFAULT.to_string()
    } else {
        current.to_string()
    }
}

/// A parsed hotkey: modifier flags plus a Windows virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accel {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub win: bool,
    pub vk: u32,
}

/// Parse an accelerator like "<Ctrl><Shift>v", "<Alt>F4" or "<Super>space".
///
/// Modifiers are `<Ctrl>`/`<Control>`/`<Primary>`, `<Shift>`, `<Alt>`, `<Super>`/`<Win>`
/// (case-insensitive). The key is one letter or digit, `F1`..`F24`, `Return`, `space`,
/// `Tab` or `Escape`. An uppercase letter (e.g. "V") implies Shift.
pub fn parse_accel(s: &str) -> Result<Accel, String> {
    let trimmed = s.trim();
    let mut acc = Accel { ctrl: false, shift: false, alt: false, win: false, vk: 0 };
    let mut rest = trimmed;
    while let Some(stripped) = rest.trim_start().strip_prefix('<') {
        let end = stripped
            .find('>')
            .ok_or_else(|| format!("unclosed modifier in accelerator {trimmed:?}"))?;
        match stripped[..end].to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "primary" => acc.ctrl = true,
            "shift" => acc.shift = true,
            "alt" => acc.alt = true,
            "super" | "win" => acc.win = true,
            other => return Err(format!("unknown modifier <{other}> in accelerator {trimmed:?}")),
        }
        rest = &stripped[end + 1..];
    }
    let key = rest.trim();
    if key.is_empty() {
        return Err(format!("missing key in accelerator {trimmed:?}"));
    }
    let (vk, implies_shift) = vk_from_key(key)
        .ok_or_else(|| format!("unknown key {key:?} in accelerator {trimmed:?}"))?;
    acc.vk = vk;
    acc.shift |= implies_shift;
    Ok(acc)
}

/// Human-readable label for an accelerator, for example "<Ctrl><Shift>v" -> "Ctrl+Shift+V".
/// Returns `None` when the text is not a valid accelerator, so callers can fall back to a
/// plain name instead of showing a broken label.
pub fn shortcut_label(accel: &str) -> Option<String> {
    parse_accel(accel).ok()?;
    let mut parts = Vec::new();
    let mut rest = accel.trim();
    while let Some(stripped) = rest.trim_start().strip_prefix('<') {
        let end = stripped.find('>')?;
        let name = match stripped[..end].to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "primary" => "Ctrl",
            "shift" => "Shift",
            "alt" => "Alt",
            "super" | "win" => "Win",
            _ => return None,
        };
        parts.push(name.to_string());
        rest = &stripped[end + 1..];
    }
    let key = rest.trim();
    // A single letter is shown in capitals ("v" -> "V"); names such as "space" or "F5" get an
    // initial capital.
    let key_label = if key.len() == 1 {
        key.to_ascii_uppercase()
    } else {
        let mut chars = key.chars();
        chars.next().map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())?
    };
    parts.push(key_label);
    Some(parts.join("+"))
}

/// Tray tooltip text. Names the open shortcut when it is known, otherwise plain "Clipsy".
pub fn tray_tooltip(shortcut: Option<&str>) -> String {
    match shortcut.and_then(shortcut_label) {
        Some(label) => format!("Clipsy - {label} opens your history"),
        None => "Clipsy".to_string(),
    }
}

/// Map a key name to (virtual-key code, whether the spelling implies Shift).
fn vk_from_key(key: &str) -> Option<(u32, bool)> {
    if let [c] = key.as_bytes() {
        // VK codes for letters are the uppercase ASCII values, digits are '0'..'9'.
        return match *c {
            b'a'..=b'z' => Some((u32::from(c.to_ascii_uppercase()), false)),
            b'A'..=b'Z' => Some((u32::from(*c), true)),
            b'0'..=b'9' => Some((u32::from(*c), false)),
            _ => None,
        };
    }
    let lower = key.to_ascii_lowercase();
    let vk = match lower.as_str() {
        "return" => 0x0D,
        "space" => 0x20,
        "tab" => 0x09,
        "escape" => 0x1B,
        _ => {
            // F1..F24 are VK_F1 (0x70) onwards. Require plain digits: u32 parsing would accept "+1".
            let digits = lower.strip_prefix('f')?;
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: u32 = digits.parse().ok()?;
            if !(1..=24).contains(&n) {
                return None;
            }
            0x70 + n - 1
        }
    };
    Some((vk, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_shift_v() {
        let a = parse_accel("<Ctrl><Shift>v").unwrap();
        assert!(a.ctrl);
        assert!(a.shift);
        assert!(!a.alt);
        assert!(!a.win);
        assert_eq!(a.vk, 0x56);
    }

    #[test]
    fn parses_aliases_case_insensitively() {
        let control = parse_accel("<Control>v").unwrap();
        let primary = parse_accel("<PRIMARY>v").unwrap();
        assert!(control.ctrl && primary.ctrl);
        let win = parse_accel("<win>v").unwrap();
        let super_ = parse_accel("<Super>v").unwrap();
        assert!(win.win && super_.win);
        let alt = parse_accel("<ALT>v").unwrap();
        assert!(alt.alt);
    }

    #[test]
    fn parses_function_key() {
        let f1 = parse_accel("<Alt>F1").unwrap();
        assert!(f1.alt);
        assert_eq!(f1.vk, 0x70);
        assert_eq!(parse_accel("F24").unwrap().vk, 0x87);
        assert_eq!(parse_accel("f12").unwrap().vk, 0x7B);
    }

    #[test]
    fn uppercase_letter_means_shift() {
        let a = parse_accel("<Ctrl>V").unwrap();
        assert!(a.ctrl);
        assert!(a.shift);
        assert_eq!(a.vk, 0x56);
    }

    #[test]
    fn parses_named_keys_and_digits() {
        assert_eq!(parse_accel("Return").unwrap().vk, 0x0D);
        assert_eq!(parse_accel("space").unwrap().vk, 0x20);
        assert_eq!(parse_accel("Tab").unwrap().vk, 0x09);
        assert_eq!(parse_accel("Escape").unwrap().vk, 0x1B);
        assert_eq!(parse_accel("<Ctrl>5").unwrap().vk, 0x35);
    }

    #[test]
    fn rejects_unknown_modifier() {
        let err = parse_accel("<Hyper>v").unwrap_err();
        assert!(err.contains("unknown modifier"), "{err}");
        assert!(err.contains("<hyper>"), "{err}");
    }

    #[test]
    fn rejects_unclosed_modifier() {
        let err = parse_accel("<Ctrlv").unwrap_err();
        assert!(err.contains("unclosed modifier"), "{err}");
    }

    #[test]
    fn rejects_missing_key() {
        let err = parse_accel("<Ctrl>").unwrap_err();
        assert!(err.contains("missing key"), "{err}");
        let err = parse_accel("<Ctrl>  ").unwrap_err();
        assert!(err.contains("missing key"), "{err}");
    }

    #[test]
    fn rejects_empty_and_unknown_keys() {
        let err = parse_accel("").unwrap_err();
        assert!(err.contains("missing key"), "{err}");
        let err = parse_accel("<Ctrl>F25").unwrap_err();
        assert!(err.contains("unknown key"), "{err}");
        let err = parse_accel("<Ctrl>Pause").unwrap_err();
        assert!(err.contains("unknown key"), "{err}");
        assert!(parse_accel("F0").is_err());
    }

    #[test]
    fn ignores_surrounding_spaces() {
        let a = parse_accel("  <Ctrl> <Shift>v  ").unwrap();
        assert!(a.ctrl && a.shift);
        assert_eq!(a.vk, 0x56);
        assert_eq!(parse_accel("  F5 ").unwrap().vk, 0x74);
    }

    #[test]
    fn shortcut_label_names_modifiers_and_key() {
        assert_eq!(shortcut_label("<Ctrl><Shift>v").as_deref(), Some("Ctrl+Shift+V"));
        assert_eq!(shortcut_label("<Super><Shift>v").as_deref(), Some("Win+Shift+V"));
        assert_eq!(shortcut_label("<Alt>F4").as_deref(), Some("Alt+F4"));
        assert_eq!(shortcut_label("<Ctrl>space").as_deref(), Some("Ctrl+Space"));
        assert_eq!(shortcut_label("F5").as_deref(), Some("F5"));
    }

    #[test]
    fn shortcut_label_rejects_invalid_text() {
        assert_eq!(shortcut_label(""), None);
        assert_eq!(shortcut_label("<Hyper>v"), None);
        assert_eq!(shortcut_label("<Ctrl>Pause"), None);
    }

    #[test]
    fn migrate_legacy_default_to_super_shift_v() {
        assert_eq!(migrate_toggle_default("<Ctrl><Shift>v"), "<Super><Shift>v");
    }

    #[test]
    fn migrate_keeps_custom_shortcut() {
        assert_eq!(migrate_toggle_default("<Ctrl><Alt>k"), "<Ctrl><Alt>k");
        assert_eq!(migrate_toggle_default("<Super><Shift>v"), "<Super><Shift>v");
    }

    #[test]
    fn super_shift_v_parses() {
        let a = parse_accel("<Super><Shift>v").unwrap();
        assert!(a.win);
        assert!(a.shift);
        assert!(!a.ctrl);
        assert_eq!(a.vk, 0x56);
    }

    #[test]
    fn tray_tooltip_uses_shortcut_or_plain_name() {
        assert_eq!(
            tray_tooltip(Some("<Ctrl><Shift>v")),
            "Clipsy - Ctrl+Shift+V opens your history"
        );
        assert_eq!(tray_tooltip(Some("bogus<")), "Clipsy");
        assert_eq!(tray_tooltip(None), "Clipsy");
    }
}
