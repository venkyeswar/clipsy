//! Global hotkey handling: parses GTK-style accelerators such as "<Ctrl><Shift>v".

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
}
