//! marg: keyboard-driven PDF reader and annotator.

use std::path::PathBuf;

use eframe::egui;

mod app;
mod complete;
mod input;
mod pages;
mod paths;
mod recent;
mod startup;
mod statefile;

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
                    version: format!("marg {} ({GIT_HASH})", marg_core::VERSION),
                    path: args.path,
                    start_page: args.page.map(|p| p - 1),
                    state_path: paths.as_ref().map(|p| p.state_file.clone()),
                    errors: config_error.into_iter().collect(),
                },
            )))
        }),
    )
}
