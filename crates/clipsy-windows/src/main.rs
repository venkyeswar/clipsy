// Release builds use the Windows GUI subsystem: no console window appears at login or on a
// double-click. CLI output still reaches a terminal because `main` attaches to the parent
// console when arguments are given (see `has_console`). Debug builds keep the console.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
use windows::core::{PCWSTR, w};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::System::Console::{AttachConsole, GetConsoleWindow, ATTACH_PARENT_PROCESS};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

/// True when this process has a console to print to. With arguments, first try to attach to
/// the parent terminal (the one that started a release build, which has no console of its own).
/// Without arguments there is nothing to print, so no attach is tried.
#[cfg(windows)]
fn has_console(args_given: bool) -> bool {
    if !args_given {
        return false;
    }
    // SAFETY: GetConsoleWindow takes no arguments and only reads the handle of the console
    // attached to this process (null when there is none).
    if !unsafe { GetConsoleWindow() }.0.is_null() {
        return true;
    }
    // SAFETY: AttachConsole takes only a process id; ATTACH_PARENT_PROCESS asks for the parent's
    // console and fails when there is none. No pointers are passed.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) }.is_ok()
}

/// Shows a fatal startup error in a message box. Used when no console can show it.
#[cfg(windows)]
fn show_error_box(text: &str) {
    let body: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `body` is NUL-terminated and alive for the call; the title is a static wide string.
    // A null owner window makes the box a top-level one. The box is modal and returns when closed.
    unsafe {
        MessageBoxW(HWND::default(), PCWSTR(body.as_ptr()), w!("Clipsy"), MB_OK | MB_ICONERROR);
    }
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Decided before anything is printed, because the console must be attached first.
    let console = has_console(!args.is_empty());
    clipsy_windows::logging::init_logging();
    if let Err(e) = clipsy_windows::app::run(args) {
        let text = format!("{e:#}");
        eprintln!("clipsy-windows: {text}");
        // With no console the eprintln above goes nowhere, so the user would see nothing.
        if !console {
            show_error_box(&format!("Clipsy could not start.\n\n{text}"));
        }
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("clipsy-windows only runs on Windows");
    std::process::exit(1);
}
