//! The Wayland setup box. On GNOME with Wayland the Clipsy GNOME extension does
//! the clipboard watching and the shortcuts. Clipsy installs and turns it on by
//! itself where it can (see `gnome_install`); this box then only asks the user to
//! log out and back in once. It shows a "Turn on" button when the extension is
//! installed but off, and the Terminal command only when Clipsy could not
//! install it. The welcome window and Settings show it until the extension connects.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use clipsy_backends::{SetupNeeded, GNOME_EXTENSION_UUID};
use gtk::glib;

use crate::app::State;
use crate::gnome_install::{self, Status};

/// Where a package install puts the extension.
const SYSTEM_EXTENSION_DIR: &str = "/usr/share/gnome-shell/extensions";

// Developer preview: CLIPSY_PREVIEW_SETUP=missing forces the setup card on any session.
fn preview_missing() -> bool {
    std::env::var("CLIPSY_PREVIEW_SETUP").as_deref() == Ok("missing")
}

/// True while the backend says the extension still needs to be set up.
fn still_needed(state: &State) -> bool {
    preview_missing() || state.backend.borrow().setup_needed() == Some(SetupNeeded::GnomeExtension)
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

/// Icon and label of the copy button: "Copy command", then "Copied" for a moment.
fn copy_button_face(copied: bool) -> (&'static str, &'static str) {
    if copied {
        ("object-select-symbolic", "Copied")
    } else {
        ("edit-copy-symbolic", "Copy command")
    }
}

/// Sets the icon and label of the copy button, and keeps its accessible name in step.
fn set_copy_face(button: &gtk::Button, copied: bool) {
    let (icon, label) = copy_button_face(copied);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.set_halign(gtk::Align::Center);
    content.append(&gtk::Image::from_icon_name(icon));
    content.append(&gtk::Label::new(Some(label)));
    button.set_child(Some(&content));
    button.update_property(&[gtk::accessible::Property::Label(label)]);
    if copied {
        button.add_css_class("copied");
    } else {
        button.remove_css_class("copied");
    }
}

/// The copy button, the log-out step and the collapsed command, for when Clipsy
/// could not install the extension itself. The command text is only ever shown
/// inside the "Show the command" expander, which starts collapsed.
fn command_steps(root: &gtk::Box) {
    let (cmd, hint) = command();

    let sentence = dim_label("Copy the command below, open a terminal, paste it and press Enter.");
    root.append(&sentence);

    // The one primary action: copy the command, then confirm for about two seconds.
    let copy = gtk::Button::new();
    copy.add_css_class("suggested-action");
    copy.add_css_class("pill");
    copy.add_css_class("setup-copy");
    copy.set_halign(gtk::Align::Start);
    copy.set_tooltip_text(Some("Copy the command to the clipboard"));
    set_copy_face(&copy, false);
    {
        let cmd = cmd.clone();
        // The pending "back to Copy" timer, so a second click restarts it.
        let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        copy.connect_clicked(move |button| {
            button.display().clipboard().set_text(&cmd);
            set_copy_face(button, true);
            if let Some(id) = timer.borrow_mut().take() {
                id.remove();
            }
            // A weak ref keeps a closed window from leaking.
            let weak = button.downgrade();
            let timer_c = timer.clone();
            let id = glib::timeout_add_local_once(Duration::from_secs(2), move || {
                // The source is gone once it has run, so forget its id first.
                timer_c.borrow_mut().take();
                if let Some(button) = weak.upgrade() {
                    set_copy_face(&button, false);
                }
            });
            *timer.borrow_mut() = Some(id);
        });
    }
    root.append(&copy);

    if let Some(hint) = hint {
        let hint = dim_label(hint);
        hint.add_css_class("caption");
        root.append(&hint);
    }

    // The command itself: collapsed by default, selectable and wrapped so it stays readable.
    let title = gtk::Label::new(Some("Show the command"));
    title.add_css_class("setup-link");
    title.set_xalign(0.0);

    let cmd_label = gtk::Label::new(Some(&cmd));
    cmd_label.add_css_class("command-box");
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

    let expander = gtk::Expander::new(None);
    expander.set_label_widget(Some(&title));
    expander.set_expanded(false);
    expander.set_child(Some(&cmd_label));
    expander.add_css_class("setup-expander");
    root.append(&expander);

    let why = dim_label("On GNOME with Wayland, Clipsy needs its small GNOME extension to see what you copy and to use the shortcut.");
    why.add_css_class("caption");
    root.append(&why);

    // Log out and in, last.
    let step2 = gtk::Label::new(Some("Then log out and log back in once."));
    step2.set_xalign(0.0);
    step2.set_wrap(true);
    step2.add_css_class("caption-heading");
    let step2_note = dim_label("GNOME turns on new extensions only at login. Clipsy then connects by itself.");
    step2_note.add_css_class("caption");
    root.append(&step2);
    root.append(&step2_note);
}

/// Builds the setup box, or returns None when no setup is needed.
pub fn setup_box(state: &Rc<State>) -> Option<gtk::Widget> {
    if !still_needed(state) {
        return None;
    }
    // One rounded card (CSS `setup-card`): the background and padding live in style.css.
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.add_css_class("setup-card");
    let heading = |text: &str| {
        let title = gtk::Label::new(Some(text));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        title.set_wrap(true);
        title
    };
    let status = if preview_missing() { Status::Missing } else { gnome_install::status() };
    match status {
        // Installed and on: GNOME starts it at the next login.
        Status::Enabled => {
            root.append(&heading("Almost ready"));
            root.append(&dim_label(
                "Clipsy has set up its small GNOME extension. Log out and log back in once to finish. GNOME turns on new extensions only at login.",
            ));
        }
        // Installed but turned off (for example by the user): one click turns it on.
        Status::Disabled => {
            root.append(&heading("Turn on the Clipsy extension"));
            root.append(&dim_label(
                "Clipsy's small GNOME extension is installed but turned off. Clipsy needs it on Wayland to see what you copy and to use the shortcut.",
            ));
            let button = gtk::Button::with_label("Turn on");
            button.add_css_class("suggested-action");
            button.add_css_class("pill");
            button.set_halign(gtk::Align::Start);
            let result = dim_label("");
            result.add_css_class("caption");
            result.set_visible(false);
            {
                let result = result.clone();
                button.connect_clicked(move |button| {
                    match gnome_install::enable() {
                        Ok(()) => {
                            button.set_visible(false);
                            result.set_text("Turned on. Clipsy connects in a few seconds. If it does not, log out and log back in.");
                        }
                        Err(err) => result.set_text(&format!("Could not turn it on: {err:#}")),
                    }
                    result.set_visible(true);
                });
            }
            root.append(&button);
            root.append(&result);
        }
        // Clipsy could not install it: show the copy button and the collapsed command.
        Status::Missing => {
            root.append(&heading("One quick step to finish setup"));
            command_steps(&root);
        }
    }

    // Live status: waiting, then a green check.
    let spinner = gtk::Spinner::new();
    spinner.start();
    let waiting = dim_label("Waiting for the extension to connect...");
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

    root.append(&waiting_row);
    root.append(&done_row);

    // Poll every 2 s while the box is on screen. The timer is stopped on unmap.
    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let (state, timer, weak) = (state.clone(), timer.clone(), root.downgrade());
        let (waiting_row, done_row) = (waiting_row.clone(), done_row.clone());
        root.connect_map(move |_| {
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
    root.connect_unmap(move |_| {
        if let Some(id) = timer.borrow_mut().take() {
            id.remove();
        }
    });

    Some(root.upcast())
}

#[cfg(test)]
mod tests {
    use super::{copy_button_face, enable_command};

    /// The copy button shows its icon and label, and the label is the accessible name.
    #[test]
    fn copy_button_says_copy_then_copied() {
        assert_eq!(copy_button_face(false), ("edit-copy-symbolic", "Copy command"));
        assert_eq!(copy_button_face(true), ("object-select-symbolic", "Copied"));
    }

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
