//! Installs and turns on the Clipsy GNOME Shell extension, so users on GNOME
//! Wayland do not have to run a command.
//!
//! - The extension's files are built into Clipsy and copied to
//!   `~/.local/share/gnome-shell/extensions/<uuid>/`. In the snap this needs the
//!   `personal-files` permission for that one folder. Without it the copy fails
//!   and the setup card falls back to the Terminal command.
//! - Turning it on means adding the UUID to GNOME Shell's `enabled-extensions`
//!   setting. Outside the snap GNOME Shell's own schema is used. The snap does
//!   not include that schema, so it carries a copy of the two keys
//!   (`data/schemas/`), compiled into `$SNAP/share/clipsy/schemas`.
//! - Clipsy turns the extension on by itself only right after installing it
//!   for the first time. If the user later turns it off, Clipsy asks instead.

use std::path::{Path, PathBuf};

use anyhow::Context;
use clipsy_backends::GNOME_EXTENSION_UUID;
use gtk::gio;
use gtk::gio::prelude::*;

const EXTENSION_JS: &str =
    include_str!("../../../gnome-extension/clipsy@venkyeswar.github.io/extension.js");
const METADATA_JSON: &str =
    include_str!("../../../gnome-extension/clipsy@venkyeswar.github.io/metadata.json");

/// GNOME Shell's own schema (present outside the snap).
const SHELL_SCHEMA: &str = "org.gnome.shell";
/// Clipsy's copy of the two keys it needs (used in the snap).
const OWN_SCHEMA: &str = "io.github.clipsy.ShellKeys";
/// Where a package install (.deb) puts the extension for all users.
const SYSTEM_EXTENSIONS: &str = "/usr/share/gnome-shell/extensions";

/// Where the extension stands on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Not installed for this user or system-wide.
    Missing,
    /// Installed, but not in GNOME's list of enabled extensions.
    Disabled,
    /// Installed and enabled. GNOME loads it at the next login if it has not yet.
    Enabled,
}

/// What `auto_setup` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoSetup {
    /// Nothing to do: installed, up to date, and the user's on/off choice is kept.
    Unchanged,
    /// Installed for the first time and turned on. It loads at the next login.
    Installed,
    /// The files were updated to this version of Clipsy.
    Updated,
    /// Clipsy may not write the extension folder (the snap lacks the permission),
    /// or something else failed. The setup card shows the command instead.
    NotPossible(String),
}

/// The user's real home folder. In a snap, `$HOME` is a per-snap folder.
fn real_home() -> Option<PathBuf> {
    std::env::var_os("SNAP_REAL_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// `~/.local/share/gnome-shell/extensions/<uuid>`.
pub fn user_extension_dir() -> Option<PathBuf> {
    Some(real_home()?.join(".local/share/gnome-shell/extensions").join(GNOME_EXTENSION_UUID))
}

fn system_install_exists() -> bool {
    Path::new(SYSTEM_EXTENSIONS).join(GNOME_EXTENSION_UUID).join("extension.js").exists()
}

fn user_install_exists() -> bool {
    user_extension_dir().is_some_and(|d| d.join("extension.js").exists())
}

/// Installed (anywhere) and enabled?
pub fn status() -> Status {
    if !(user_install_exists() || system_install_exists()) {
        return Status::Missing;
    }
    match shell_settings() {
        Some(settings) if is_enabled(&settings) => Status::Enabled,
        // Without a readable setting we cannot tell; treat as off so the user
        // gets the "Turn on" button, which reports a clear error if it fails.
        _ => Status::Disabled,
    }
}

/// Runs at startup on GNOME Wayland. Installs or updates the extension files,
/// and turns the extension on when it was installed just now.
pub fn auto_setup() -> AutoSetup {
    // A package install already ships the files; do not add a second copy.
    if system_install_exists() {
        return AutoSetup::Unchanged;
    }
    let Some(dir) = user_extension_dir() else {
        return AutoSetup::NotPossible("no home folder".into());
    };
    let was_missing = !dir.join("extension.js").exists();
    match install_files(&dir) {
        Ok(false) => AutoSetup::Unchanged,
        Ok(true) if !was_missing => AutoSetup::Updated,
        Ok(true) => match enable() {
            Ok(()) => AutoSetup::Installed,
            Err(err) => AutoSetup::NotPossible(format!("{err:#}")),
        },
        Err(err) => AutoSetup::NotPossible(format!("{err:#}")),
    }
}

/// Writes the extension files into `dir` when they are missing or different.
/// Returns true when anything was written.
pub fn install_files(dir: &Path) -> anyhow::Result<bool> {
    let files = [("extension.js", EXTENSION_JS), ("metadata.json", METADATA_JSON)];
    let up_to_date = files
        .iter()
        .all(|(name, body)| std::fs::read_to_string(dir.join(name)).is_ok_and(|b| b == *body));
    if up_to_date {
        return Ok(false);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    for (name, body) in files {
        let path = dir.join(name);
        std::fs::write(&path, body).with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(true)
}

/// Adds the extension to GNOME's enabled list and makes sure user extensions
/// are allowed. If GNOME already knows the extension, it starts right away.
pub fn enable() -> anyhow::Result<()> {
    let settings = shell_settings().context("GNOME Shell settings are not available")?;
    let mut list: Vec<String> = settings.strv("enabled-extensions").iter().map(|s| s.to_string()).collect();
    if !list.iter().any(|u| u == GNOME_EXTENSION_UUID) {
        list.push(GNOME_EXTENSION_UUID.to_string());
        let refs: Vec<&str> = list.iter().map(String::as_str).collect();
        settings.set_strv("enabled-extensions", refs.as_slice())?;
    }
    if settings.boolean("disable-user-extensions") {
        settings.set_boolean("disable-user-extensions", false)?;
    }
    gio::Settings::sync();
    Ok(())
}

fn is_enabled(settings: &gio::Settings) -> bool {
    !settings.boolean("disable-user-extensions")
        && settings.strv("enabled-extensions").iter().any(|u| u.as_str() == GNOME_EXTENSION_UUID)
}

/// GNOME Shell's settings, through its own schema when installed, otherwise
/// through Clipsy's copy of the two keys.
fn shell_settings() -> Option<gio::Settings> {
    let default = gio::SettingsSchemaSource::default()?;
    // CLIPSY_SCHEMA_DIR forces Clipsy's own copy (used by the tests).
    let forced = std::env::var_os("CLIPSY_SCHEMA_DIR").map(PathBuf::from);
    if forced.is_none() && default.lookup(SHELL_SCHEMA, true).is_some() {
        return Some(gio::Settings::new(SHELL_SCHEMA));
    }
    let dir = forced.or_else(|| {
        std::env::var_os("SNAP").map(|snap| PathBuf::from(snap).join("share/clipsy/schemas"))
    })?;
    let source = gio::SettingsSchemaSource::from_directory(&dir, Some(&default), false).ok()?;
    let schema = source.lookup(OWN_SCHEMA, false)?;
    Some(gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_once_then_reports_up_to_date() {
        let dir = std::env::temp_dir().join(format!("clipsy-ext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(install_files(&dir).unwrap());
        assert_eq!(std::fs::read_to_string(dir.join("extension.js")).unwrap(), EXTENSION_JS);
        assert!(std::fs::read_to_string(dir.join("metadata.json")).unwrap().contains(GNOME_EXTENSION_UUID));
        assert!(!install_files(&dir).unwrap());
        // An older copy is replaced.
        std::fs::write(dir.join("extension.js"), "old").unwrap();
        assert!(install_files(&dir).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Uses Clipsy's own schema copy with an in-memory settings backend, so the
    /// real desktop settings are never touched.
    #[test]
    fn enable_adds_uuid_once_and_allows_user_extensions() {
        let schemas = std::env::temp_dir().join(format!("clipsy-schemas-{}", std::process::id()));
        std::fs::create_dir_all(&schemas).unwrap();
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/schemas/io.github.clipsy.ShellKeys.gschema.xml"),
            schemas.join("io.github.clipsy.ShellKeys.gschema.xml"),
        )
        .unwrap();
        let compiled = std::process::Command::new("glib-compile-schemas").arg(&schemas).status();
        if !compiled.is_ok_and(|s| s.success()) {
            eprintln!("glib-compile-schemas not available; skipping");
            return;
        }
        std::env::set_var("GSETTINGS_BACKEND", "memory");
        std::env::set_var("CLIPSY_SCHEMA_DIR", &schemas);

        let settings = shell_settings().expect("own schema loads");
        settings.set_strv("enabled-extensions", ["ubuntu-dock@ubuntu.com"].as_slice()).unwrap();
        settings.set_boolean("disable-user-extensions", true).unwrap();
        assert!(!is_enabled(&settings));

        enable().unwrap();
        enable().unwrap();
        let list: Vec<String> = settings.strv("enabled-extensions").iter().map(|s| s.to_string()).collect();
        assert_eq!(list, ["ubuntu-dock@ubuntu.com", GNOME_EXTENSION_UUID]);
        assert!(!settings.boolean("disable-user-extensions"));
        assert!(is_enabled(&settings));
        std::fs::remove_dir_all(&schemas).ok();
    }
}
