// Build scripts run on the host, so the target OS is read from the environment,
// not with cfg!(...).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=assets/clipsy.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        slint_build::compile("ui/app.slint").expect("failed to compile ui/app.slint");
        embed_exe_icon();
    }
}

/// Embeds the Clipsy icon in clipsy.exe, so Explorer and the taskbar show the logo.
/// `winresource` is only a build-dependency on Windows hosts, so this runs only there.
#[cfg(windows)]
fn embed_exe_icon() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/clipsy.ico");
    res.compile().expect("failed to embed the exe icon (assets/clipsy.ico)");
}

#[cfg(not(windows))]
fn embed_exe_icon() {}
