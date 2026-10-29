//! marg: keyboard-driven PDF reader and annotator.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use eframe::egui;

mod annotlist;
mod app;
mod boxmode;
mod browser;
mod comment;
mod complete;
mod help;
mod input;
mod outline;
mod pages;
mod paths;
mod recent;
mod select;
mod startup;
mod statefile;
mod theme;

const GIT_HASH: &str = env!("MARG_GIT_HASH");

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

fn main() -> eframe::Result {
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
    let (config, config_error) = startup::load_config(paths.as_ref());
    if let Some(e) = &config_error {
        log::error!("{e}");
    }
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
        doc.send(marg_pdf::Request::Open(path.clone()));
    }
    let title = args
        .path
        .as_deref()
        .and_then(|p| p.file_name())
        .map_or("marg".into(), |n| format!("{} — marg", n.to_string_lossy()));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_app_id("marg"),
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
                    errors: config_error.into_iter().collect(),
                    timing_start: timing.then_some(start),
                },
            )))
        }),
    )
}
