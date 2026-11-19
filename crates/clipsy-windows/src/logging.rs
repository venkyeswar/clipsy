//! Log setup shared by the Windows app. Ungated so the filter rule is tested on every OS.
//!
//! `CLIPSY_LOG` wins, then `RUST_LOG`, then `info`. Logs go to stderr.

/// Picks the log filter from the two environment values (already read).
/// The first non-empty value wins; with neither set the filter is `info`.
pub fn resolve_filter(clipsy_log: Option<&str>, rust_log: Option<&str>) -> String {
    [clipsy_log, rust_log]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|v| !v.is_empty())
        .unwrap_or("info")
        .to_string()
}

/// Installs the stderr logger. Safe to call once; a second call is ignored.
pub fn init_logging() {
    let filter = resolve_filter(
        std::env::var("CLIPSY_LOG").ok().as_deref(),
        std::env::var("RUST_LOG").ok().as_deref(),
    );
    let env_filter = tracing_subscriber::EnvFilter::try_new(&filter)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::resolve_filter;

    #[test]
    fn clipsy_log_wins() {
        assert_eq!(resolve_filter(Some("debug"), Some("warn")), "debug");
    }

    #[test]
    fn rust_log_is_fallback() {
        assert_eq!(resolve_filter(None, Some("warn")), "warn");
        assert_eq!(resolve_filter(Some("  "), Some("warn")), "warn");
    }

    #[test]
    fn defaults_to_info() {
        assert_eq!(resolve_filter(None, None), "info");
    }
}
