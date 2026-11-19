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

#[cfg(test)]
mod tests {
    use super::*;

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
