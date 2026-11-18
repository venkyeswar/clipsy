//! The "One more step for Wayland" box. On GNOME with Wayland the Clipsy GNOME
//! extension does the clipboard watching and the shortcuts, so the user has to
//! turn it on once. The welcome window and Settings both show this box until the
//! extension has connected.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use clipsy_backends::{SetupNeeded, GNOME_EXTENSION_UUID};
use gtk::glib;

use crate::app::State;

/// Where a package install puts the extension.
const SYSTEM_EXTENSION_DIR: &str = "/usr/share/gnome-shell/extensions";

/// True while the backend says the extension still needs to be set up.
fn still_needed(state: &State) -> bool {
    state.backend.borrow().setup_needed() == Some(SetupNeeded::GnomeExtension)
}

/// Turns the extension on. `gnome-extensions enable` only works for extensions
/// GNOME has already loaded, and a new one loads only at the next login, so this
/// adds the UUID to the enabled list in GSettings directly (once, keeping the rest).
fn enable_command(uuid: &str) -> String {
    // No `!` anywhere: in an interactive bash it triggers history expansion
    // ("event not found"), even inside double quotes.
    format!(
        "gsettings set org.gnome.shell disable-user-extensions false && \
L=\"$(gsettings get org.gnome.shell enabled-extensions)\" && \
case \"$L\" in *{uuid}*) ;; *) gsettings set org.gnome.shell enabled-extensions \
\"$(echo \"$L\" | sed -E \"s/^@as \\[\\]$/[]/; s/\\]$/, '{uuid}']/; s/^\\[, /[/\")\" ;; esac"
    )
}

/// The terminal command for this kind of install, plus an optional hint line.
fn command() -> (String, Option<&'static str>) {
    let uuid = GNOME_EXTENSION_UUID;
    let enable = enable_command(uuid);
    if std::env::var_os("SNAP").is_some() {
        let cmd = format!(
            "gnome-extensions install --force /snap/clipsy/current/share/clipsy/{uuid}.zip && {enable}"
        );
        (cmd, None)
    } else if Path::new(SYSTEM_EXTENSION_DIR).join(uuid).exists() {
        (enable, None)
    } else {
        let cmd = format!("gnome-extensions install --force {uuid}.zip && {enable}");
        (cmd, Some("Get the zip from github.com/venkyeswar/clipsy/releases"))
    }
}


fn dim_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("dim-label");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label
}

/// Builds the setup box, or returns None when no setup is needed.
pub fn setup_box(state: &Rc<State>) -> Option<gtk::Widget> {
    if !still_needed(state) {
        return None;
    }
    let (cmd, hint) = command();

    let title = gtk::Label::new(Some("One more step for Wayland"));
    title.add_css_class("heading");
    title.set_xalign(0.0);

    let intro = dim_label(
        "On GNOME with Wayland, Clipsy needs its small GNOME extension to see what you copy and to use the shortcut.",
    );

    // Step 1: the command and a Copy button.
    let step1 = gtk::Label::new(Some("1. Copy this command, paste it in a Terminal (Ctrl+Alt+T) and press Enter"));
    step1.set_xalign(0.0);
    step1.set_wrap(true);
    step1.add_css_class("caption-heading");

    let cmd_label = gtk::Label::new(Some(&cmd));
    cmd_label.add_css_class("clipsy-cmd");
    // Selectable, but not focusable, so it does not open with all text selected.
    cmd_label.set_selectable(true);
    cmd_label.set_focusable(false);
    cmd_label.set_wrap(true);
    cmd_label.set_wrap_mode(gtk::pango::WrapMode::Char);
    // Wrapping inside a word must not add hyphens: the command has to stay exact.
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrInt::new_insert_hyphens(false));
    cmd_label.set_attributes(Some(&attrs));
    cmd_label.set_xalign(0.0);
    cmd_label.set_hexpand(true);

    let copy = gtk::Button::with_label("Copy");
    copy.set_valign(gtk::Align::Center);
    {
        let cmd = cmd.clone();
        copy.connect_clicked(move |button| {
            button.display().clipboard().set_text(&cmd);
            button.set_label("Copied");
            // Back to "Copy" after a moment. A weak ref keeps a closed window from leaking.
            let weak = button.downgrade();
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                if let Some(button) = weak.upgrade() {
                    button.set_label("Copy");
                }
            });
        });
    }

    let cmd_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    cmd_row.append(&cmd_label);
    cmd_row.append(&copy);

    // Step 2: log out and in.
    let step2 = gtk::Label::new(Some("2. Log out, then log back in"));
    step2.set_xalign(0.0);
    step2.set_wrap(true);
    step2.add_css_class("caption-heading");
    let step2_note = dim_label("GNOME turns on new extensions only at login. Clipsy then connects by itself.");
    step2_note.add_css_class("caption");

    // Live status: waiting, then a green check.
    let spinner = gtk::Spinner::new();
    spinner.start();
    let waiting = dim_label("Waiting for the extension. This changes to Connected after you log back in.");
    waiting.add_css_class("caption");
    let waiting_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    waiting_row.append(&spinner);
    waiting_row.append(&waiting);

    let done_icon = gtk::Image::from_icon_name("object-select-symbolic");
    let done_text = gtk::Label::new(Some("Connected. You're all set."));
    let done_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    done_row.add_css_class("clipsy-success");
    done_row.append(&done_icon);
    done_row.append(&done_text);
    done_row.set_visible(false);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 6);
    root.append(&title);
    root.append(&intro);
    root.append(&step1);
    root.append(&cmd_row);
    if let Some(hint) = hint {
        let hint = dim_label(hint);
        hint.add_css_class("caption");
        root.append(&hint);
    }
    root.append(&step2);
    root.append(&step2_note);
    root.append(&waiting_row);
    root.append(&done_row);

    // The `card` class paints the background; the padding sits inside it.
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("card");
    outer.append(&root);

    // Poll every 2 s while the box is on screen. The timer is stopped on unmap.
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let (state, timer, weak) = (state.clone(), timer.clone(), outer.downgrade());
        let (waiting_row, done_row) = (waiting_row.clone(), done_row.clone());
        outer.connect_map(move |_| {
            if timer.borrow().is_some() {
                return;
            }
            let (state, timer_c, weak) = (state.clone(), timer.clone(), weak.clone());
            let (waiting_row, done_row) = (waiting_row.clone(), done_row.clone());
            let id = glib::timeout_add_local(Duration::from_secs(2), move || {
                if weak.upgrade().is_none() {
                    timer_c.borrow_mut().take();
                    return glib::ControlFlow::Break;
                }
                if still_needed(&state) {
                    return glib::ControlFlow::Continue;
                }
                waiting_row.set_visible(false);
                done_row.set_visible(true);
                // Returning Break removes the source, so just forget the id.
                timer_c.borrow_mut().take();
                glib::ControlFlow::Break
            });
            *timer.borrow_mut() = Some(id);
        });
    }
    outer.connect_unmap(move |_| {
        if let Some(id) = timer.borrow_mut().take() {
            id.remove();
        }
    });

    Some(outer.upcast())
}

#[cfg(test)]
mod tests {
    use super::enable_command;

    /// Runs the generated command against a fake `gsettings` and checks the result.
    #[test]
    fn enable_command_appends_once() {
        let uuid = "clipsy@venkyeswar.github.io";
        for (before, after) in [
            ("@as []", format!("['{uuid}']")),
            ("['a@b']", format!("['a@b', '{uuid}']")),
            (&*format!("['{uuid}', 'a@b']"), format!("['{uuid}', 'a@b']")),
        ] {
            let dir = std::env::temp_dir().join(format!("clipsy-gs-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let fake = dir.join("gsettings");
            let out = dir.join("out");
            // Starts as the old list: when Clipsy is already on, nothing is written.
            std::fs::write(&out, before).unwrap();
            std::fs::write(
                &fake,
                format!(
                    "#!/bin/sh\nif [ \"$1\" = get ]; then echo \"{before}\"; elif [ \"$3\" = enabled-extensions ]; then printf %s \"$4\" > '{}'; fi\n",
                    out.display()
                ),
            )
            .unwrap();
            std::process::Command::new("chmod").arg("+x").arg(&fake).status().unwrap();
            let path = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default());
            // Run it the way a user does: pasted into an interactive bash, where
            // history expansion is on.
            use std::io::Write;
            let mut child = std::process::Command::new("bash")
                .args(["--norc", "--noprofile", "-i"])
                .env("PATH", path)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let mut stdin = child.stdin.take().unwrap();
            writeln!(stdin, "{}", enable_command(uuid)).unwrap();
            writeln!(stdin, "exit").unwrap();
            drop(stdin);
            let output = child.wait_with_output().unwrap();
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!stderr.contains("event not found"), "{stderr}");
            assert_eq!(std::fs::read_to_string(&out).unwrap(), after);
            std::fs::remove_dir_all(&dir).ok();
        }
    }
}
