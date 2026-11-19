// Build scripts run on the host, so the target OS is read from the environment,
// not with cfg!(...).
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=ui/app.slint");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        slint_build::compile("ui/app.slint").expect("failed to compile ui/app.slint");
    }
}
