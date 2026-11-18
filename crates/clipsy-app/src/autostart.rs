//! Start-at-login support. Writes or removes a .desktop file in
//! ~/.config/autostart, which the desktop session reads at login.

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use gtk::glib;

use crate::APP_ID;

/// Full path of the autostart entry.
fn entry_path() -> PathBuf {
    glib::user_config_dir()
        .join("autostart")
        .join(format!("{APP_ID}.desktop"))
}

/// Creates the autostart entry when `enabled` is true, and removes it when false.
pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    let path = entry_path();
    if !enabled {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err).context("could not remove the autostart entry"),
        }
        return Ok(());
    }

    // Point at the binary we are running right now, so the entry survives
    // reinstalls to other paths when the setting is saved again.
    // Inside a snap the real path changes with every update, so use the stable
    // `clipsy` command that snapd provides instead.
    let exe = if std::env::var_os("SNAP").is_some() {
        PathBuf::from("clipsy")
    } else {
        std::env::current_exe().context("could not find the Clipsy program path")?
    };
    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Clipsy\n\
         Comment=Clipboard history\n\
         Exec=\"{}\" --background\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        exe.display()
    );
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).context("could not create the autostart folder")?;
    }
    fs::write(&path, contents).context("could not write the autostart entry")?;
    Ok(())
}
