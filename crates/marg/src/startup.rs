//! Logging and config loading at startup.

use std::fs;

use log::LevelFilter;
use marg_core::config::Config;
use simplelog::{CombinedLogger, SharedLogger, SimpleLogger, WriteLogger};

use crate::paths::Paths;

/// Log to stderr/stdout and, if possible, to the log file. `MARG_LOG` sets
/// the level of marg's own crates (`error`, `warn`, `info`, `debug`,
/// `trace`); default `info`. Dependencies log warnings and errors only.
pub fn init_logging(paths: Option<&Paths>) {
    let level = std::env::var("MARG_LOG")
        .ok()
        .and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);
    let ours = simplelog::ConfigBuilder::new()
        .add_filter_allow_str("marg")
        .build();
    let deps = simplelog::ConfigBuilder::new()
        .add_filter_ignore_str("marg")
        .build();
    let deps_level = LevelFilter::Warn;
    let mut loggers: Vec<Box<dyn SharedLogger>> = vec![
        SimpleLogger::new(level, ours.clone()),
        SimpleLogger::new(deps_level, deps.clone()),
    ];
    let mut file_error = None;
    if let Some(paths) = paths {
        let opened = paths
            .log_file
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| {
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&paths.log_file)
            });
        match opened {
            Ok(file) => {
                let file2 = file.try_clone();
                loggers.push(WriteLogger::new(level, ours, file));
                match file2 {
                    Ok(f) => loggers.push(WriteLogger::new(deps_level, deps, f)),
                    Err(e) => file_error = Some(format!("{}: {e}", paths.log_file.display())),
                }
            }
            Err(e) => file_error = Some(format!("{}: {e}", paths.log_file.display())),
        }
    }
    // Only fails if a logger is already set, which cannot happen here.
    let _ = CombinedLogger::init(loggers);
    if let Some(e) = file_error {
        log::warn!("cannot open log file {e}");
    }
}

/// Load the config file. A missing file means defaults; an unreadable or
/// invalid one means defaults plus an error message for the status bar.
pub fn load_config(paths: Option<&Paths>) -> (Config, Option<String>) {
    let Some(path) = paths.map(|p| &p.config_file) else {
        return (Config::default(), None);
    };
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Config::default(), None),
        Err(e) => return (Config::default(), Some(format!("config: {e}"))),
    };
    match Config::from_toml(&text) {
        Ok(c) => (c, None),
        Err(e) => {
            let msg = format!("config {}: {e}", path.display());
            (Config::default(), Some(msg))
        }
    }
}
