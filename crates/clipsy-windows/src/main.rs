#[cfg(windows)]
fn main() {}

#[cfg(not(windows))]
fn main() {
    eprintln!("clipsy-windows only runs on Windows");
    std::process::exit(1);
}
