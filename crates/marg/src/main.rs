//! marg: keyboard-driven PDF reader and annotator.

use std::path::PathBuf;

use eframe::egui;

mod app;
mod pages;
mod paths;
mod startup;

const GIT_HASH: &str = env!("MARG_GIT_HASH");

const USAGE: &str = "usage: marg <file.pdf> [--page N]";

struct Args {
    path: PathBuf,
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
    Ok(Args {
        path: path.ok_or("no file given")?,
        page,
    })
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
        .file_name()
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
                config,
                args.path,
                args.page.map(|p| p - 1),
                config_error,
            )))
        }),
    )
}
