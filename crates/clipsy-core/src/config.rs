//! User configuration (`~/.config/clipsy/config.toml`) and XDG data directory helpers.
//! Defaults match docs/PLAN.md §7.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub appearance: Appearance,
    pub shortcuts: Shortcuts,
    pub privacy: Privacy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct General {
    pub autostart: bool,
    pub max_items: u32,
    pub max_total_mb: u32,
    pub max_item_mb: u32,
    pub save_images: bool,
    pub save_files: bool,
    pub save_rich_text: bool,
    /// Mouse-selection clipboard (PRIMARY).
    pub watch_primary: bool,
    pub paste_on_select: bool,
    /// Unpinned items not copied or used for this many days are deleted.
    /// 0 keeps them forever.
    pub keep_days: u32,
    /// The same for pinned items, which usually get a longer time. 0 keeps them forever.
    pub keep_pinned_days: u32,
    /// Extra apps that paste with Ctrl+Shift+V like a terminal, by window class
    /// (case does not matter), e.g. `["Code"]` for the VS Code terminal. Known
    /// terminals are detected without this.
    pub terminal_apps: Vec<String>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            autostart: true,
            max_items: 500,
            max_total_mb: 500,
            max_item_mb: 20,
            save_images: true,
            save_files: true,
            save_rich_text: true,
            watch_primary: false,
            paste_on_select: true,
            keep_days: 2,
            keep_pinned_days: 10,
            terminal_apps: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "cursor" | "center"
    pub popup_position: String,
    pub compact: bool,
    pub show_source_app: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            popup_position: "cursor".into(),
            compact: false,
            show_source_app: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    pub toggle_popup: String,
    /// Empty string means unassigned.
    pub clear_history: String,
    pub pause_recording: String,
    /// Used inside the popup.
    pub paste_plain: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            #[cfg(windows)]
            toggle_popup: "<Ctrl><Shift>v".into(),
            #[cfg(not(windows))]
            toggle_popup: "<Super><Shift>v".into(),
            clear_history: String::new(),
            pause_recording: String::new(),
            paste_plain: "<Shift>Return".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Privacy {
    pub paused: bool,
    pub skip_sensitive: bool,
    pub ignored_apps: Vec<String>,
    pub clear_on_exit: bool,
}

impl Default for Privacy {
    fn default() -> Self {
        Self {
            paused: false,
            skip_sensitive: true,
            ignored_apps: vec!["KeePassXC".into(), "1Password".into(), "Bitwarden".into()],
            clear_on_exit: false,
        }
    }
}

impl Config {
    /// Load the config from the default location. A missing or invalid file yields defaults.
    pub fn load() -> Config {
        match config_path() {
            Ok(path) => Config::load_from(&path),
            Err(e) => {
                warn!(error = %e, "no config location available, using defaults");
                Config::default()
            }
        }
    }

    /// Save the config to the default location.
    pub fn save(&self) -> Result<()> {
        self.save_to(&config_path()?)
    }

    /// Load from an explicit path. Missing file -> defaults silently; unreadable or
    /// unparsable file -> defaults with a warning; invalid enum values -> that field's default.
    pub fn load_from(path: &Path) -> Config {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == ErrorKind::NotFound => return Config::default(),
            Err(e) => {
                warn!(path = %path.display(), error = %e, "cannot read config, using defaults");
                return Config::default();
            }
        };
        match toml::from_str::<Config>(&text) {
            Ok(mut cfg) => {
                cfg.sanitize();
                cfg
            }
            Err(e) => {
                warn!(path = %path.display(), error = %e, "invalid config file, using defaults");
                Config::default()
            }
        }
    }

    /// Save to an explicit path (creating the parent directory with mode 0700).
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            ensure_private_dir(parent)?;
        }
        let text = toml::to_string_pretty(self).context("serializing config")?;
        let tmp = path.with_extension("toml.tmp");
        fs::write(&tmp, text).with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }

    /// Replace out-of-range enum-like strings with their defaults, logging a warning.
    fn sanitize(&mut self) {
        let defaults = Appearance::default();
        if !matches!(self.appearance.theme.as_str(), "system" | "light" | "dark") {
            warn!(value = %self.appearance.theme, "invalid appearance.theme, using default");
            self.appearance.theme = defaults.theme;
        }
        if !matches!(self.appearance.popup_position.as_str(), "cursor" | "center") {
            warn!(value = %self.appearance.popup_position, "invalid appearance.popup_position, using default");
            self.appearance.popup_position = defaults.popup_position;
        }
    }
}

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "clipsy").ok_or_else(|| anyhow!("could not determine the home directory"))
}

/// `~/.config/clipsy/config.toml`
pub fn config_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

/// `~/.local/share/clipsy` on Linux, `%LOCALAPPDATA%\clipsy\data` on Windows (local, not
/// roaming, so the history database does not sync with the profile). Created with mode 0700
/// on Unix if missing.
pub fn data_dir() -> Result<PathBuf> {
    let dir = project_dirs()?.data_local_dir().to_path_buf();
    ensure_private_dir(&dir)?;
    Ok(dir)
}

/// Create `path` (and parents) with mode 0700. Succeeds if it already exists.
#[cfg(unix)]
pub(crate) fn ensure_private_dir(path: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .with_context(|| format!("creating {}", path.display()))
}

/// Create `path` (and parents). Windows has no unix mode bits; the user profile is already private.
#[cfg(not(unix))]
pub(crate) fn ensure_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_plan() {
        let c = Config::default();
        assert_eq!(c.general.max_items, 500);
        assert_eq!(c.general.max_item_mb, 20);
        assert!(c.general.paste_on_select);
        assert_eq!(c.general.keep_days, 2);
        assert_eq!(c.general.keep_pinned_days, 10);
        assert_eq!(c.appearance.theme, "system");
        #[cfg(not(windows))]
        assert_eq!(c.shortcuts.toggle_popup, "<Super><Shift>v");
        assert_eq!(c.shortcuts.clear_history, "");
        assert!(c.privacy.skip_sensitive);
        assert_eq!(c.privacy.ignored_apps.len(), 3);
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_toggle_is_ctrl_shift_v() {
        assert_eq!(Config::default().shortcuts.toggle_popup, "<Ctrl><Shift>v");
    }

    #[test]
    fn roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("config.toml");

        let default = Config::default();
        default.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), default);

        let mut changed = Config::default();
        changed.general.max_items = 42;
        changed.appearance.theme = "dark".into();
        changed.privacy.ignored_apps.push("Foo".into());
        changed.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), changed);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Config::load_from(&dir.path().join("nope.toml")), Config::default());
    }

    #[test]
    fn invalid_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[general\nmax_items = = 3").unwrap();
        assert_eq!(Config::load_from(&path), Config::default());
    }

    #[test]
    fn wrong_types_fall_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[general]\nmax_items = \"lots\"\n").unwrap();
        assert_eq!(Config::load_from(&path), Config::default());
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[general]\nmax_items = 7\n[appearance]\ntheme = \"purple\"\n").unwrap();
        let cfg = Config::load_from(&path);
        assert_eq!(cfg.general.max_items, 7);
        assert_eq!(cfg.general.max_total_mb, 500);
        assert_eq!(cfg.appearance.theme, "system");
    }

    #[test]
    fn terminal_apps_default_empty_and_parse() {
        assert!(Config::default().general.terminal_apps.is_empty());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        // An older config without the field still loads.
        fs::write(&path, "[general]\nmax_items = 7\n").unwrap();
        assert!(Config::load_from(&path).general.terminal_apps.is_empty());
        fs::write(&path, "[general]\nterminal_apps = [\"Code\", \"my-term\"]\n").unwrap();
        assert_eq!(Config::load_from(&path).general.terminal_apps, ["Code", "my-term"]);
    }
}
