//! Logging and config loading at startup.

use std::fs;
use std::path::Path;

use log::LevelFilter;
use marg_core::config::Config;
use simplelog::{CombinedLogger, SharedLogger, SimpleLogger, WriteLogger};

use crate::complete;
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
    let config = match Config::from_toml(&text) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("config {}: {e}", path.display());
            return (Config::default(), Some(msg));
        }
    };
    if config.theme.trim().is_empty() {
        return (config, None);
    }
    // Relative to the config folder.
    let theme_path = path
        .parent()
        .unwrap_or(Path::new("."))
        .join(complete::expand_tilde(config.theme.trim()));
    let theme = match fs::read_to_string(&theme_path) {
        Ok(t) => t,
        Err(e) => return (config, Some(format!("theme {}: {e}", theme_path.display()))),
    };
    match Config::with_theme(&text, &theme) {
        Ok(c) => (c, None),
        Err((in_theme, e)) => {
            let file = if in_theme { &theme_path } else { path };
            (config, Some(format!("theme {}: {e}", file.display())))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &Path) -> Paths {
        Paths {
            config_file: dir.join("config.toml"),
            state_file: dir.join("state.toml"),
            log_file: dir.join("marg.log"),
            backup_dir: dir.join("backups"),
        }
    }

    #[test]
    fn theme_file_is_found_next_to_the_config() {
        let dir = std::env::temp_dir().join(format!("marg-theme-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("themes")).unwrap();
        fs::write(
            dir.join("config.toml"),
            "theme = \"themes/t.toml\"\n[colors]\ncaret = \"#010101\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("themes/t.toml"),
            "[colors]\ncaret = \"#020202\"\nbackground = \"#030303\"\n",
        )
        .unwrap();
        let (c, err) = load_config(Some(&paths(&dir)));
        assert_eq!(err, None);
        assert_eq!(c.colors.caret.to_string(), "#010101");
        assert_eq!(c.colors.background.to_string(), "#030303");

        // A missing theme: the config alone, and an error.
        fs::remove_file(dir.join("themes/t.toml")).unwrap();
        let (c, err) = load_config(Some(&paths(&dir)));
        assert!(err.is_some_and(|e| e.contains("t.toml")));
        assert_eq!(c.colors.caret.to_string(), "#010101");
        let _ = fs::remove_dir_all(&dir);
    }
}
