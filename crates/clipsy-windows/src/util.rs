//! Small shared helpers. Ungated so the logic is tested on every OS.

use std::time::Duration;

/// Call `f` up to `attempts` times, sleeping `delay_ms` between tries. Returns the first `Some`,
/// or `None` once every attempt has failed. `attempts == 0` never calls `f`.
pub fn retry<T>(attempts: u32, delay_ms: u64, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    for attempt in 0..attempts {
        if let Some(value) = f() {
            return Some(value);
        }
        if attempt + 1 < attempts && delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
    }
    None
}

/// True when `bytes` holds at least four bytes and the first four read as a little-endian
/// DWORD equal to zero. Shorter buffers are not zero (they are malformed, not "disallowed").
pub fn dword_is_zero(bytes: &[u8]) -> bool {
    matches!(bytes.get(..4), Some([0, 0, 0, 0]))
}

/// The part of `units` before its first NUL. Clipboard memory is usually larger than the string
/// it holds, so bytes after the terminator are slack and must not be kept.
pub fn cut_at_nul(units: &[u16]) -> &[u16] {
    match units.iter().position(|&u| u == 0) {
        Some(end) => &units[..end],
        None => units,
    }
}

/// Decode `%XX` escapes as UTF-8 bytes. Only two ASCII hex digits count as an escape, so `%+1`
/// stays as written. Malformed escapes are kept unchanged.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let (hi, lo) = (bytes[i + 1], bytes[i + 2]);
            if hi.is_ascii_hexdigit() && lo.is_ascii_hexdigit() {
                out.push(hex_value(hi) * 16 + hex_value(lo));
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Value of an ASCII hex digit. Callers check `is_ascii_hexdigit` first.
fn hex_value(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

/// Takes at most `max` messages that are ready on `rx` right now, without waiting. Stops early
/// when the channel is empty or disconnected.
pub fn drain_up_to<T>(rx: &std::sync::mpsc::Receiver<T>, max: usize) -> Vec<T> {
    let mut items = Vec::new();
    while items.len() < max {
        match rx.try_recv() {
            Ok(item) => items.push(item),
            Err(_) => break,
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_up_to_stops_at_max() {
        let (tx, rx) = std::sync::mpsc::channel();
        for i in 0..10 {
            tx.send(i).unwrap();
        }
        assert_eq!(drain_up_to(&rx, 4), vec![0, 1, 2, 3]);
        assert_eq!(drain_up_to(&rx, 100), vec![4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn drain_up_to_returns_all_when_fewer() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send("a").unwrap();
        tx.send("b").unwrap();
        assert_eq!(drain_up_to(&rx, 16), vec!["a", "b"]);
        assert!(drain_up_to(&rx, 16).is_empty());
    }

    #[test]
    fn drain_up_to_handles_disconnected_channel() {
        let (tx, rx) = std::sync::mpsc::channel::<u8>();
        tx.send(7).unwrap();
        drop(tx);
        assert_eq!(drain_up_to(&rx, 16), vec![7]);
        assert!(drain_up_to(&rx, 16).is_empty());
    }

    #[test]
    fn dword_zero_is_true() {
        assert!(dword_is_zero(&[0, 0, 0, 0]));
        assert!(dword_is_zero(&[0, 0, 0, 0, 9, 9]));
    }

    #[test]
    fn dword_nonzero_is_false() {
        assert!(!dword_is_zero(&[1, 0, 0, 0]));
        assert!(!dword_is_zero(&[0, 0, 0, 1]));
    }

    #[test]
    fn short_buffer_is_false() {
        assert!(!dword_is_zero(&[]));
        assert!(!dword_is_zero(&[0, 0, 0]));
    }

    #[test]
    fn cut_at_nul_stops_at_first_nul() {
        assert_eq!(cut_at_nul(&[104, 105, 0, 0, 7]), &[104, 105]);
    }

    #[test]
    fn cut_at_nul_without_nul_returns_all() {
        assert_eq!(cut_at_nul(&[104, 105]), &[104, 105]);
    }

    #[test]
    fn cut_at_nul_leading_nul_is_empty() {
        assert!(cut_at_nul(&[0, 104]).is_empty());
    }

    #[test]
    fn percent_decode_decodes_hex() {
        assert_eq!(percent_decode("a%20b%C3%A9"), "a b\u{e9}");
    }

    #[test]
    fn percent_decode_rejects_plus() {
        assert_eq!(percent_decode("%+1"), "%+1");
    }

    #[test]
    fn retry_stops_on_first_success() {
        let mut calls = 0;
        let got = retry(10, 0, || {
            calls += 1;
            if calls == 3 { Some("ok") } else { None }
        });
        assert_eq!(got, Some("ok"));
        assert_eq!(calls, 3);
    }

    #[test]
    fn retry_gives_up_after_attempts() {
        let mut calls = 0;
        let got: Option<u8> = retry(4, 0, || {
            calls += 1;
            None
        });
        assert_eq!(got, None);
        assert_eq!(calls, 4);
    }
}
