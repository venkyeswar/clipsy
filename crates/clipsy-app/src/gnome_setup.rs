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

/// The terminal command for this kind of install, plus an optional hint line.
fn command() -> (String, Option<&'static str>) {
    let uuid = GNOME_EXTENSION_UUID;
    if std::env::var_os("SNAP").is_some() {
        let cmd = format!(
            "gnome-extensions install --force /snap/clipsy/current/share/clipsy/{uuid}.zip && gnome-extensions enable {uuid}"
        );
        (cmd, None)
    } else if Path::new(SYSTEM_EXTENSION_DIR).join(uuid).exists() {
        (format!("gnome-extensions enable {uuid}"), None)
    } else {
        let cmd = format!(
            "gnome-extensions install --force {uuid}.zip && gnome-extensions enable {uuid}"
        );
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
    let step1 = gtk::Label::new(Some("1. Run this in a Terminal"));
    step1.set_xalign(0.0);
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
    let step2 = gtk::Label::new(Some("2. Log out and log back in"));
    step2.set_xalign(0.0);
    step2.add_css_class("caption-heading");
    let step2_note = dim_label("GNOME loads new extensions at login.");
    step2_note.add_css_class("caption");

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
