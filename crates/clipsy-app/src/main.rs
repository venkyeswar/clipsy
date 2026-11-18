//! Clipsy: clipboard history for Linux.
//!
//! How the binary works:
//! - Started with no arguments (or from the app menu), it becomes the running
//!   "primary" instance and stays alive in the background.
//! - Any later run (`clipsy toggle`, `clipsy settings`, ...) is forwarded to the
//!   running instance over GApplication's D-Bus connection. The running instance
//!   then performs that command as an app action.

mod app;
mod autostart;
mod placement;
mod popup;
mod settings;
mod welcome;

use adw::prelude::*;
use gtk::gio;

/// Application ID. Also used for the desktop file, icon and D-Bus name.
pub const APP_ID: &str = "io.github.clipsy.Clipsy";

/// Commands that are forwarded to the running instance as actions of the same name.
const COMMANDS: &[&str] = &["toggle", "settings", "pause", "resume", "clear", "quit"];

/// Used by the autostart entry so that login does not open the Settings window.
const BACKGROUND_FLAG: &str = "--background";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // The popup is a simple list, so the software renderer is plenty. It uses about a
    // quarter less memory than the GPU one. Users can still override it.
    if std::env::var_os("GSK_RENDERER").is_none() {
        std::env::set_var("GSK_RENDERER", "cairo");
    }

    // Some answers do not need GTK at all, so handle them before starting anything.
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("clipsy {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Some("--help") | Some("-h") => {
            print_help();
            return;
        }
        Some(cmd) if !COMMANDS.contains(&cmd) && cmd != BACKGROUND_FLAG => {
            eprintln!("clipsy: unknown command '{cmd}'\n");
            print_help();
            std::process::exit(2);
        }
        _ => {}
    }

    init_logging();

    let app = adw::Application::builder()
        .application_id(APP_ID)
        // We want to see the command line even when this is a second run, so
        // `clipsy toggle` can be forwarded to the running instance.
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    // Sets up the backend, the popup and the actions. Runs once in the first instance.
    app::connect(&app);

    app.connect_command_line(|app, command_line| {
        // Element 0 is the program name, element 1 is the command (if any).
        let verb = command_line
            .arguments()
            .get(1)
            .map(|arg| arg.to_string_lossy().into_owned())
            .unwrap_or_default();

        match verb.as_str() {
            // No command: open the normal window (Settings, or the welcome on first run).
            "" => app.activate(),
            // Autostart at login: just keep running in the background.
            BACKGROUND_FLAG => {}
            v if COMMANDS.contains(&v) => app.activate_action(v, None),
            other => eprintln!("clipsy: unknown command '{other}'"),
        }
        // Exit status for the calling process.
        0
    });

    app.run();
}

fn init_logging() {
    // CLIPSY_LOG=debug|info|warn|error (RUST_LOG is read too). Default is warn: a
    // background app should stay quiet.
    let level = std::env::var("CLIPSY_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .ok()
        .and_then(|v| v.rsplit('=').next().and_then(|l| l.trim().parse::<tracing::Level>().ok()))
        .unwrap_or(tracing::Level::WARN);
    tracing_subscriber::fmt().with_max_level(level).init();
}

fn print_help() {
    println!(
        "Clipsy - clipboard history for Linux\n\n\
         Usage:\n\
         \x20 clipsy                 start Clipsy in the background (or open Settings)\n\
         \x20 clipsy toggle          show or hide the clipboard popup\n\
         \x20 clipsy settings        open the Settings window\n\
         \x20 clipsy pause|resume    pause or resume recording\n\
         \x20 clipsy clear           clear history (pinned items are kept)\n\
         \x20 clipsy quit            stop Clipsy\n\
         \x20 clipsy --version       print the version"
    );
}
