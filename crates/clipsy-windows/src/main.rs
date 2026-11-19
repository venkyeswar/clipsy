// The Windows build keeps the console subsystem on purpose: CLI verbs run from a terminal
// must print their errors. Adding `windows_subsystem = "windows"` (no console window at login)
// would also need a message box for fatal startup errors, which does not exist yet.

#[cfg(windows)]
fn main() {
    clipsy_windows::logging::init_logging();
    if let Err(e) = clipsy_windows::app::run(std::env::args().skip(1).collect()) {
        eprintln!("clipsy-windows: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("clipsy-windows only runs on Windows");
    std::process::exit(1);
}
