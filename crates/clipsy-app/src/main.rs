//! Clipsy: clipboard history for Linux.
//!
//! How the binary works:
//! - Started with no arguments (or from the app menu), it becomes the running
//!   "primary" instance and stays alive in the background.
//! - Any later run (`clipsy toggle`, `clipsy settings`, ...) is forwarded to the
//!   running instance over GApplication's D-Bus connection. The running instance
//!   then performs that command as an app action.
//! - Inside a strictly confined snap, owning a D-Bus name needs a store review, so
//!   there the app runs "non-unique" and uses a private Unix socket instead
//!   (see `snap_single_instance`). Behaviour is the same for the user.

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

    let in_snap = std::env::var_os("SNAP").is_some();
    let socket = if in_snap {
        let verb = args.first().cloned().unwrap_or_default();
        // Another copy is already running: hand it the command and stop here.
        if snap_single_instance::forward(&verb) {
            return;
        }
        Some(snap_single_instance::socket_path())
    } else {
        None
    };

    // Started by hand in a terminal: hand off to a detached copy and return, so that
    // closing the terminal does not stop Clipsy.
    if detach_from_terminal(&args) {
        return;
    }

    let mut flags = gio::ApplicationFlags::HANDLES_COMMAND_LINE;
    if in_snap {
        flags |= gio::ApplicationFlags::NON_UNIQUE;
    }
    let app = adw::Application::builder()
        .application_id(APP_ID)
        // We want to see the command line even when this is a second run, so
        // `clipsy toggle` can be forwarded to the running instance.
        .flags(flags)
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
        dispatch(app, &verb);
        // Exit status for the calling process.
        0
    });

    // Keep the listener alive for as long as the app runs.
    let _listener = socket.and_then(|path| snap_single_instance::listen(&app, &path));

    app.run();
}

/// When `clipsy` (or `clipsy --background`) is run from a terminal, start the
/// long-lived copy in a new session with no terminal attached. A closed
/// terminal then has nothing to hang up. Returns true when the detached copy started.
fn detach_from_terminal(args: &[String]) -> bool {
    use std::io::IsTerminal;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    const DETACHED: &str = "CLIPSY_DETACHED";
    if std::env::var_os(DETACHED).is_some() {
        return false;
    }
    // Only the commands that start the app; `clipsy toggle` and friends just
    // talk to the running copy and exit.
    let verb = args.first().map(String::as_str).unwrap_or("");
    if !(verb.is_empty() || verb == BACKGROUND_FLAG) {
        return false;
    }
    let from_terminal = std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::io::stderr().is_terminal();
    if !from_terminal {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut command = Command::new(exe);
    command
        .args(args)
        .env(DETACHED, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: setsid() is async-signal-safe and takes no pointers. Running it in
    // the child before exec gives the copy a new session with no controlling
    // terminal, so a closed terminal cannot send it SIGHUP.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().is_ok()
}

/// Runs one command in this (the running) instance.
fn dispatch(app: &adw::Application, verb: &str) {
    match verb {
        // No command: open the normal window (Settings, or the welcome on first run).
        "" => app.activate(),
        // Autostart at login: just keep running in the background.
        BACKGROUND_FLAG => {}
        v if COMMANDS.contains(&v) => app.activate_action(v, None),
        other => eprintln!("clipsy: unknown command '{other}'"),
    }
}

/// Single-instance support for the snap, through a Unix socket in the snap's
/// private runtime folder instead of a D-Bus name.
mod snap_single_instance {
    use super::dispatch;
    use adw::prelude::*;
    use gtk::gio;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};

    pub fn socket_path() -> PathBuf {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        dir.join("clipsy.sock")
    }

    /// Sends `verb` to a running copy. Returns false when no copy is running.
    pub fn forward(verb: &str) -> bool {
        match UnixStream::connect(socket_path()) {
            Ok(mut stream) => stream.write_all(format!("{verb}\n").as_bytes()).is_ok(),
            Err(_) => false,
        }
    }

    /// Listens for commands from later runs and performs them on the main thread.
    pub fn listen(app: &adw::Application, path: &Path) -> Option<gio::SocketService> {
        // A socket file left over from a crash would block binding; nobody answers on it.
        let _ = std::fs::remove_file(path);
        let service = gio::SocketService::new();
        let address = gio::UnixSocketAddress::new(path);
        if let Err(err) = service.add_address(
            &address,
            gio::SocketType::Stream,
            gio::SocketProtocol::Default,
            None::<&gtk::glib::Object>,
        ) {
            tracing::warn!("could not open the single-instance socket: {err}");
            return None;
        }
        let app = app.clone();
        service.connect_incoming(move |_, connection, _| {
            let input = gio::DataInputStream::new(&connection.input_stream());
            if let Ok(Some(line)) = input.read_line_utf8(None::<&gio::Cancellable>) {
                dispatch(&app, line.trim());
            }
            false
        });
        service.start();
        Some(service)
    }
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
