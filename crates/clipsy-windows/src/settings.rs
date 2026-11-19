//! Settings: hotkey validation (all platforms) and, on Windows, the settings window.

use crate::hotkey::parse_accel;

/// Checks that a shortcut string such as "<Ctrl><Shift>v" can be used as the
/// popup hotkey. Returns a human-readable message when it cannot.
pub fn validate_hotkey(s: &str) -> Result<(), String> {
    parse_accel(s).map(|_| ())
}

/// Splits a comma-separated settings field into items: trims each, drops empty
/// ones and removes repeats (ignoring case), keeping the first spelling and position.
pub fn parse_list(s: &str) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if !items.iter().any(|existing| existing.eq_ignore_ascii_case(part)) {
            items.push(part.to_string());
        }
    }
    items
}

/// Joins list items back into the comma-separated form shown in the settings window.
pub fn join_list(items: &[String]) -> String {
    items.join(", ")
}

/// Parses a number field, clamps it to `min..=max`, and returns `fallback` when the
/// text is not a number.
pub fn clamp_u32(s: &str, min: u32, max: u32, fallback: u32) -> u32 {
    match s.trim().parse::<u32>() {
        Ok(n) => n.clamp(min, max),
        Err(_) => fallback,
    }
}

/// Maps the theme field to an allowed value (`system`, `light` or `dark`). Anything else is `system`.
pub fn normalize_theme(s: &str) -> &'static str {
    match s.trim() {
        "light" => "light",
        "dark" => "dark",
        _ => "system",
    }
}

/// Maps the popup position field to an allowed value (`cursor` or `center`). Anything else is `cursor`.
pub fn normalize_position(s: &str) -> &'static str {
    match s.trim() {
        "center" => "center",
        _ => "cursor",
    }
}

#[cfg(windows)]
pub use window::SettingsWindow;

/// The Slint settings window and its glue (Windows only).
///
/// The generated `SettingsDialog` type is reached as `crate::ui::SettingsDialog`. The
/// `slint::include_modules!()` call that generates it lives in the crate's `ui` module,
/// which the integration task must make re-export every generated component.
#[cfg(windows)]
mod window {
    use std::cell::RefCell;
    use std::rc::Rc;

    use anyhow::{anyhow, Result};
    use clipsy_backends::Backend;
    use clipsy_core::config::Config;
    use slint::ComponentHandle;

    use super::{clamp_u32, join_list, normalize_position, normalize_theme, parse_list, validate_hotkey};
    use crate::autostart;
    use crate::backend::WindowsBackend;
    use crate::ui::SettingsDialog;

    /// Accepted range for "maximum items" (inclusive).
    const MAX_ITEMS_RANGE: (u32, u32) = (1, 100_000);
    /// Accepted range for the day limits (inclusive). 0 means keep forever.
    const KEEP_DAYS_RANGE: (u32, u32) = (0, 3650);

    /// The settings window. It edits the shared `Config`; Save writes it to disk.
    pub struct SettingsWindow {
        dialog: SettingsDialog,
        config: Rc<RefCell<Config>>,
    }

    impl SettingsWindow {
        /// Builds the window. `on_saved` runs after a successful Save, before the window hides.
        pub fn new(
            config: Rc<RefCell<Config>>,
            backend: Rc<WindowsBackend>,
            on_saved: impl Fn() + 'static,
        ) -> Result<Self> {
            let dialog = SettingsDialog::new()?;
            let on_saved: Rc<dyn Fn()> = Rc::new(on_saved);

            // Weak handles: the dialog owns these closures, so a strong handle would form a cycle.
            let weak = dialog.as_weak();
            dialog.on_shortcut_edited(move |text| {
                if let Some(dialog) = weak.upgrade() {
                    let reason = validate_hotkey(text.as_str()).err().unwrap_or_default();
                    dialog.set_hotkey_error(reason.into());
                }
            });

            let weak = dialog.as_weak();
            let shared = config.clone();
            dialog.on_save_requested(move || {
                let Some(dialog) = weak.upgrade() else {
                    return;
                };
                dialog.set_save_error("".into());
                if let Err(reason) = validate_hotkey(dialog.get_toggle_popup().as_str()) {
                    dialog.set_hotkey_error(reason.into());
                    return;
                }
                match save(&dialog, &shared, &backend) {
                    Ok(Saved::Done) => {
                        on_saved();
                        hide(&dialog);
                    }
                    Ok(Saved::Warning(message)) => {
                        // The settings were saved, so the app must reload them; the window stays open.
                        on_saved();
                        dialog.set_save_error(message.into());
                    }
                    Err(e) => dialog.set_save_error(format!("{e:#}").into()),
                }
            });

            let weak = dialog.as_weak();
            dialog.on_cancel_requested(move || {
                if let Some(dialog) = weak.upgrade() {
                    hide(&dialog);
                }
            });

            Ok(Self { dialog, config })
        }

        /// Fills the form from the current config and shows the window.
        pub fn show(&self) {
            load(&self.dialog, &self.config.borrow());
            if let Err(e) = self.dialog.show() {
                tracing::warn!(error = %e, "could not show the settings window");
            }
        }
    }

    fn hide(dialog: &SettingsDialog) {
        if let Err(e) = dialog.hide() {
            tracing::warn!(error = %e, "could not hide the settings window");
        }
    }

    /// Copies the config into the form fields.
    fn load(dialog: &SettingsDialog, cfg: &Config) {
        dialog.set_autostart(cfg.general.autostart);
        dialog.set_max_items(cfg.general.max_items.to_string().into());
        dialog.set_keep_days(cfg.general.keep_days.to_string().into());
        dialog.set_keep_pinned_days(cfg.general.keep_pinned_days.to_string().into());
        dialog.set_paste_on_select(cfg.general.paste_on_select);
        dialog.set_save_images(cfg.general.save_images);
        dialog.set_save_files(cfg.general.save_files);
        dialog.set_terminal_apps(join_list(&cfg.general.terminal_apps).into());

        dialog.set_theme(cfg.appearance.theme.as_str().into());
        dialog.set_popup_position(cfg.appearance.popup_position.as_str().into());
        dialog.set_compact(cfg.appearance.compact);
        dialog.set_show_source_app(cfg.appearance.show_source_app);

        dialog.set_toggle_popup(cfg.shortcuts.toggle_popup.as_str().into());
        let reason = validate_hotkey(&cfg.shortcuts.toggle_popup).err().unwrap_or_default();
        dialog.set_hotkey_error(reason.into());

        dialog.set_paused(cfg.privacy.paused);
        dialog.set_skip_sensitive(cfg.privacy.skip_sensitive);
        dialog.set_ignored_apps(join_list(&cfg.privacy.ignored_apps).into());
        dialog.set_clear_on_exit(cfg.privacy.clear_on_exit);

        dialog.set_save_error("".into());
    }

    /// How a save ended. `Done` closes the window; `Warning` keeps it open with a message
    /// because the settings were saved but one step (sign-in launch) failed.
    enum Saved {
        Done,
        Warning(String),
    }

    /// Reads the form, applies it and saves it. On `Err` the shared config and the
    /// hotkey binding are left as they were.
    fn save(
        dialog: &SettingsDialog,
        shared: &RefCell<Config>,
        backend: &WindowsBackend,
    ) -> Result<Saved> {
        let prev = shared.borrow().clone();
        let mut next = prev.clone();

        next.general.autostart = dialog.get_autostart();
        next.general.max_items = clamp_u32(
            dialog.get_max_items().as_str(),
            MAX_ITEMS_RANGE.0,
            MAX_ITEMS_RANGE.1,
            prev.general.max_items,
        );
        next.general.keep_days = clamp_u32(
            dialog.get_keep_days().as_str(),
            KEEP_DAYS_RANGE.0,
            KEEP_DAYS_RANGE.1,
            prev.general.keep_days,
        );
        next.general.keep_pinned_days = clamp_u32(
            dialog.get_keep_pinned_days().as_str(),
            KEEP_DAYS_RANGE.0,
            KEEP_DAYS_RANGE.1,
            prev.general.keep_pinned_days,
        );
        next.general.paste_on_select = dialog.get_paste_on_select();
        next.general.save_images = dialog.get_save_images();
        next.general.save_files = dialog.get_save_files();
        next.general.terminal_apps = parse_list(dialog.get_terminal_apps().as_str());

        next.appearance.theme = normalize_theme(dialog.get_theme().as_str()).to_string();
        next.appearance.popup_position = normalize_position(dialog.get_popup_position().as_str()).to_string();
        next.appearance.compact = dialog.get_compact();
        next.appearance.show_source_app = dialog.get_show_source_app();

        next.shortcuts.toggle_popup = dialog.get_toggle_popup().to_string();

        next.privacy.paused = dialog.get_paused();
        next.privacy.skip_sensitive = dialog.get_skip_sensitive();
        next.privacy.ignored_apps = parse_list(dialog.get_ignored_apps().as_str());
        next.privacy.clear_on_exit = dialog.get_clear_on_exit();

        // Rebind first: if the new shortcut is taken, nothing else is changed.
        let hotkey_changed = next.shortcuts.toggle_popup != prev.shortcuts.toggle_popup;
        if hotkey_changed {
            backend.rebind_toggle(&next.shortcuts.toggle_popup)?;
        }
        if let Err(e) = next.save() {
            let mut message = format!("{e:#}");
            if hotkey_changed {
                if let Err(restore) = backend.rebind_toggle(&prev.shortcuts.toggle_popup) {
                    message = format!(
                        "{message}. The previous shortcut {} could not be restored either: {restore:#}",
                        prev.shortcuts.toggle_popup
                    );
                }
            }
            return Err(anyhow!("{message}"));
        }
        *shared.borrow_mut() = next.clone();
        backend.set_terminal_apps(&next.general.terminal_apps);

        // The config is saved at this point. If the sign-in entry fails, keep everything
        // else and record what the registry really holds, so the form tells the truth.
        match autostart::set_enabled(&std::env::current_exe()?, next.general.autostart) {
            Ok(()) => Ok(Saved::Done),
            Err(autostart_error) => {
                next.general.autostart = autostart::is_enabled();
                dialog.set_autostart(next.general.autostart);
                let mut message = format!(
                    "Settings saved, but launching Clipsy at sign-in failed: {autostart_error:#}"
                );
                match next.save() {
                    Ok(()) => *shared.borrow_mut() = next,
                    Err(e) => {
                        *shared.borrow_mut() = next;
                        message = format!("{message}. The settings file could not be updated: {e:#}");
                    }
                }
                Ok(Saved::Warning(message))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_hotkey_accepted() {
        assert!(validate_hotkey("<Ctrl><Shift>v").is_ok());
    }

    #[test]
    fn hotkey_without_key_rejected_with_message() {
        let err = validate_hotkey("<Ctrl>").unwrap_err();
        assert!(err.contains("missing key"), "{err}");
    }

    #[test]
    fn parse_list_trims_drops_empty_and_dedupes() {
        assert_eq!(parse_list(" Code , ,my-term,Code,,  KeePassXC "), ["Code", "my-term", "KeePassXC"]);
        assert!(parse_list("").is_empty());
        assert!(parse_list(" , ,").is_empty());
    }

    #[test]
    fn parse_list_dedupes_case_insensitively() {
        assert_eq!(parse_list("Code, code, CODE, my-term"), ["Code", "my-term"]);
    }

    #[test]
    fn normalize_theme_accepts_known_and_defaults() {
        assert_eq!(normalize_theme("light"), "light");
        assert_eq!(normalize_theme(" dark "), "dark");
        assert_eq!(normalize_theme("system"), "system");
        assert_eq!(normalize_theme("purple"), "system");
        assert_eq!(normalize_theme(""), "system");
    }

    #[test]
    fn normalize_position_accepts_known_and_defaults() {
        assert_eq!(normalize_position("center"), "center");
        assert_eq!(normalize_position(" cursor"), "cursor");
        assert_eq!(normalize_position("top-left"), "cursor");
        assert_eq!(normalize_position(""), "cursor");
    }

    #[test]
    fn join_list_round_trips() {
        let items = parse_list("Code, my-term");
        assert_eq!(join_list(&items), "Code, my-term");
        assert_eq!(parse_list(&join_list(&items)), items);
        assert_eq!(join_list(&[]), "");
    }

    #[test]
    fn clamp_u32_parses_clamps_and_falls_back() {
        assert_eq!(clamp_u32("50", 1, 100, 5), 50);
        assert_eq!(clamp_u32(" 7 ", 1, 100, 5), 7);
        assert_eq!(clamp_u32("500", 1, 100, 5), 100);
        assert_eq!(clamp_u32("0", 1, 100, 5), 1);
        assert_eq!(clamp_u32("abc", 1, 100, 5), 5);
        assert_eq!(clamp_u32("", 1, 100, 5), 5);
        assert_eq!(clamp_u32("-3", 1, 100, 5), 5);
    }
}
