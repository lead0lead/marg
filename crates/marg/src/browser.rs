//! Opening external links with the system's default program.

use std::process::{Command, Stdio};

/// Start the default program for `url` (a browser, a mail client)
/// without waiting for it.
pub fn open(url: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    let mut cmd = {
        // No shell in between, so `&` and `^` in URLs stay as they are.
        let mut c = Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Reap it when it exits.
    std::thread::spawn(move || child.wait());
    Ok(())
}
