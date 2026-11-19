//! Start-at-login support. The command-line value is pure and unit-tested on
//! every platform; the registry calls are Windows only.

use std::path::Path;

/// The value stored under the Run key: the exe path quoted, then `--background`.
pub fn run_value(exe: &Path) -> String {
    format!("\"{}\" --background", exe.display())
}

#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE_NAME: &str = "Clipsy";

/// Adds (or removes) Clipsy from the per-user Run key so it starts at login.
#[cfg(windows)]
pub fn set_enabled(exe: &Path, enabled: bool) -> anyhow::Result<()> {
    use anyhow::Context;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    let run = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        .context("open the Run registry key")?;
    if enabled {
        run.set_value(VALUE_NAME, &run_value(exe))
            .context("write the Clipsy Run value")?;
    } else {
        match run.delete_value(VALUE_NAME) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).context("delete the Clipsy Run value"),
        }
    }
    Ok(())
}

/// True when the Run key has a Clipsy value.
#[cfg(windows)]
pub fn is_enabled() -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .and_then(|run| run.get_value::<String, _>(VALUE_NAME))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_value_quotes_path_with_spaces() {
        let exe = Path::new(r"C:\Program Files\Clipsy\clipsy-windows.exe");
        assert_eq!(
            run_value(exe),
            r#""C:\Program Files\Clipsy\clipsy-windows.exe" --background"#
        );
    }

    #[test]
    fn run_value_adds_background_flag() {
        let exe = Path::new(r"C:\Clipsy\clipsy-windows.exe");
        assert!(run_value(exe).ends_with(" --background"));
    }
}
