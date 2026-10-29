use std::process::Command;

fn main() {
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=MARG_GIT_HASH={hash}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    #[cfg(windows)]
    windows_resources();
}

/// The .exe icon and version details (Explorer, taskbar, Properties).
#[cfg(windows)]
fn windows_resources() {
    println!("cargo:rerun-if-changed=assets/marg.ico");
    let mut res = winresource::WindowsResource::new();
    // Version and the name "marg" come from Cargo.toml.
    res.set_icon("assets/marg.ico");
    res.compile().expect("embedding the Windows resources");
}
