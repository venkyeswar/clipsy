//! Log setup shared by the Windows app. Ungated so the filter and target rules are tested on every OS.
//!
//! `CLIPSY_LOG` wins, then `RUST_LOG`, then `info`.
//!
//! Where logs go: a release build on Windows has no console, so it appends to
//! `%LOCALAPPDATA%\clipsy\logs\clipsy.log`. Debug builds, and any run where `CLIPSY_LOG` or
//! `RUST_LOG` is set, log to stderr as before.

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use clipsy_core::config::data_dir;

/// A log file bigger than this is emptied at startup instead of appended to.
const MAX_LOG_BYTES: u64 = 1_000_000;

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

/// True when logs go to the log file: a release build with neither filter variable set
/// (a variable that is set but blank counts as not set).
pub fn log_to_file(release_build: bool, clipsy_log: Option<&str>, rust_log: Option<&str>) -> bool {
    let set = |v: Option<&str>| v.is_some_and(|v| !v.trim().is_empty());
    release_build && !set(clipsy_log) && !set(rust_log)
}

/// True when a log file of `len` bytes is emptied before the new run appends to it.
pub fn needs_truncate(len: u64) -> bool {
    len > MAX_LOG_BYTES
}

/// `%LOCALAPPDATA%\clipsy\logs\clipsy.log` on Windows. The data folder is
/// `%LOCALAPPDATA%\clipsy\data`, so the logs folder is its sibling.
fn log_file_path() -> anyhow::Result<PathBuf> {
    let data = data_dir()?;
    let base = data.parent().context("the Clipsy data folder has no parent folder")?;
    Ok(base.join("logs").join("clipsy.log"))
}

/// Opens the log file for appending, creating its folder. Emptied first when too big.
fn open_log_file() -> anyhow::Result<File> {
    let path = log_file_path()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("could not create {}", dir.display()))?;
    }
    let too_big = std::fs::metadata(&path).is_ok_and(|m| needs_truncate(m.len()));
    let mut options = OpenOptions::new();
    options.create(true);
    if too_big {
        options.write(true).truncate(true);
    } else {
        options.append(true);
    }
    options
        .open(&path)
        .with_context(|| format!("could not open {}", path.display()))
}

/// Installs the logger: the log file for release builds on Windows, otherwise stderr.
/// Safe to call once; a second call is ignored.
pub fn init_logging() {
    let clipsy_log = std::env::var("CLIPSY_LOG").ok();
    let rust_log = std::env::var("RUST_LOG").ok();
    let filter = resolve_filter(clipsy_log.as_deref(), rust_log.as_deref());
    let env_filter = tracing_subscriber::EnvFilter::try_new(&filter)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let release_build = cfg!(all(windows, not(debug_assertions)));
    if log_to_file(release_build, clipsy_log.as_deref(), rust_log.as_deref()) {
        if let Ok(file) = open_log_file() {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(Mutex::new(file))
                .try_init();
            return;
        }
        // The file could not be opened. Fall through to stderr, which is then discarded
        // when there is no console, so nothing breaks.
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::{log_to_file, needs_truncate, resolve_filter};

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

    #[test]
    fn release_build_without_filter_logs_to_file() {
        assert!(log_to_file(true, None, None));
        assert!(log_to_file(true, Some("  "), Some("")));
    }

    #[test]
    fn filter_variable_or_debug_build_logs_to_stderr() {
        assert!(!log_to_file(true, Some("debug"), None));
        assert!(!log_to_file(true, None, Some("warn")));
        assert!(!log_to_file(false, None, None));
    }

    #[test]
    fn big_log_is_truncated() {
        assert!(!needs_truncate(0));
        assert!(!needs_truncate(1_000_000));
        assert!(needs_truncate(1_000_001));
    }
}
