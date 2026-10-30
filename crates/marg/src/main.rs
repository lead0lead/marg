//! marg: keyboard-driven PDF reader and annotator.

// No console window next to marg on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use eframe::egui;

mod annotlist;
mod app;
mod boxmode;
mod browser;
mod comment;
mod complete;
mod drop;
mod help;
mod input;
mod outline;
mod pages;
mod paths;
mod prompt;
mod recent;
mod select;
mod startup;
mod statefile;
mod theme;

const GIT_HASH: &str = env!("MARG_GIT_HASH");

/// Window icon (X11 and Windows; Wayland takes it from `marg.desktop`
/// through the app id). Rendered from `assets/marg.svg`.
const ICON_PNG: &[u8] = include_bytes!("../assets/marg-256.png");

const USAGE: &str = "usage: marg [<file.pdf> [--page N]]";

struct Args {
    /// No file: show the recent files list.
    path: Option<PathBuf>,
    /// 1-based page from `--page`.
    page: Option<usize>,
}

fn parse_args() -> Result<Args, String> {
    let mut path = None;
    let mut page = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(a) = args.next() {
        match a.to_str() {
            Some("--page") => {
                let n = args.next().ok_or("--page needs a number")?;
                let n = n
                    .to_str()
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|&n| n > 0)
                    .ok_or("--page needs a number from 1")?;
                page = Some(n);
            }
            Some("--version") => {
                println!("marg {} ({GIT_HASH})", marg_core::VERSION);
                std::process::exit(0);
            }
            _ if path.is_none() => path = Some(PathBuf::from(a)),
            _ => return Err(format!("unexpected argument {}", a.to_string_lossy())),
        }
    }
    if page.is_some() && path.is_none() {
        return Err("--page needs a file".into());
    }
    Ok(Args { path, page })
}

/// Started from a terminal on Windows: print `--version`, usage errors
/// and the log there. A windows-subsystem program has no console of its
/// own; without a parent console this does nothing.
#[cfg(windows)]
fn attach_console() {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    // SAFETY: takes a plain process id and fails harmlessly.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn main() -> eframe::Result {
    #[cfg(windows)]
    attach_console();
    let start = std::time::Instant::now();
    let timing = std::env::var_os("MARG_TIMING").is_some_and(|v| v != "0");
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("marg: {e}\n{USAGE}");
            std::process::exit(2);
        }
    };
    let paths = paths::Paths::resolve();
    startup::init_logging(paths.as_ref());
    let (config, config_errors) = startup::load_config(paths.as_ref());
    // Open the file while the window is being created.
    let repaint: Arc<OnceLock<egui::Context>> = Arc::default();
    let wake = repaint.clone();
    let doc = marg_pdf::DocThread::spawn(move || {
        if let Some(ctx) = wake.get() {
            ctx.request_repaint();
        }
    });
    if let Some(p) = &paths {
        doc.send(marg_pdf::Request::BackupDir(p.backup_dir.clone()));
    }
    if let Some(path) = &args.path {
        doc.send(marg_pdf::Request::Open {
            path: path.clone(),
            password: None,
        });
    }
    let title = args
        .path
        .as_deref()
        .and_then(|p| p.file_name())
        .map_or("marg".into(), |n| format!("{} — marg", n.to_string_lossy()));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(title)
        .with_app_id("marg");
    match eframe::icon_data::from_png_bytes(ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => log::warn!("window icon: {e}"),
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "marg",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::MargApp::new(
                cc,
                app::Startup {
                    config,
                    doc,
                    repaint,
                    version: format!("marg {} ({GIT_HASH})", marg_core::VERSION),
                    path: args.path,
                    start_page: args.page.map(|p| p - 1),
                    state_path: paths.as_ref().map(|p| p.state_file.clone()),
                    config_path: paths.as_ref().map(|p| p.config_file.clone()),
                    errors: config_errors,
                    timing_start: timing.then_some(start),
                },
            )))
        }),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_decodes() {
        let icon = eframe::icon_data::from_png_bytes(super::ICON_PNG).unwrap();
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
