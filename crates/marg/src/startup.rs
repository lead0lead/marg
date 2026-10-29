//! Logging and config loading at startup.

use std::fs;
use std::path::{Path, PathBuf};

use log::LevelFilter;
use marg_core::config::{Config, unknown_settings};
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

/// Load the config file at startup. A missing file means defaults; an
/// unreadable or invalid one means defaults plus an error. Also returns
/// the other problems (`read_config`).
pub fn load_config(paths: Option<&Paths>) -> (Config, Vec<String>) {
    let Some(path) = paths.map(|p| &p.config_file) else {
        return (Config::default(), Vec::new());
    };
    match read_config(path) {
        Ok(c) => c,
        Err(e) => (Config::default(), vec![e]),
    }
}

/// Read the config file and its theme. `Err`: the config cannot be read or
/// parsed (with the line number). `Ok`: the config (defaults if the file
/// is missing) and its problems: a missing or broken theme (the config is
/// used without it), unknown settings.
pub fn read_config(path: &Path) -> Result<(Config, Vec<String>), String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Config::default(), Vec::new()));
        }
        Err(e) => return Err(format!("config {}: {e}", complete::tilde(path))),
    };
    let config = Config::from_toml(&text).map_err(|e| format!("{}: {e}", file_name(path)))?;
    let unknown = |file: &Path, text: &str| -> Vec<String> {
        unknown_settings(text)
            .into_iter()
            .map(|k| format!("{}: unknown setting {k}", file_name(file)))
            .collect()
    };
    let mut problems = unknown(path, &text);
    let Some(theme_path) = theme_file(path, &config) else {
        return Ok((config, problems));
    };
    let theme = match fs::read_to_string(&theme_path) {
        Ok(t) => t,
        Err(e) => {
            problems.insert(0, format!("theme {}: {e}", complete::tilde(&theme_path)));
            return Ok((config, problems));
        }
    };
    match Config::with_theme(&text, &theme) {
        Ok(c) => {
            problems.extend(unknown(&theme_path, &theme));
            Ok((c, problems))
        }
        Err((in_theme, e)) => {
            let file = if in_theme { &theme_path } else { path };
            problems.insert(0, format!("{}: {e}", file_name(file)));
            Ok((config, problems))
        }
    }
}

/// When each file was last changed, and its size (`None`: missing), to
/// notice changes.
pub type Stamp = Vec<Option<(std::time::SystemTime, u64)>>;

pub fn stamp(files: &[PathBuf]) -> Stamp {
    files
        .iter()
        .map(|f| {
            let m = fs::metadata(f).ok()?;
            Some((m.modified().ok()?, m.len()))
        })
        .collect()
}

/// The file name alone: messages about a file's contents lead with it, so
/// the problem fits in the status bar.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into(),
    )
}

/// The theme file named by `config`, relative to the config folder.
pub fn theme_file(config_path: &Path, config: &Config) -> Option<PathBuf> {
    let theme = config.theme.trim();
    if theme.is_empty() {
        return None;
    }
    Some(
        config_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(complete::expand_tilde(theme)),
    )
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
        assert!(err.is_empty(), "{err:?}");
        assert_eq!(c.colors.caret.to_string(), "#010101");
        assert_eq!(c.colors.background.to_string(), "#030303");

        // A missing theme: the config alone, and an error.
        fs::remove_file(dir.join("themes/t.toml")).unwrap();
        let (c, err) = load_config(Some(&paths(&dir)));
        assert!(err.len() == 1 && err[0].contains("t.toml"), "{err:?}");
        assert_eq!(c.colors.caret.to_string(), "#010101");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_config_is_an_error_and_unknown_settings_are_problems() {
        let dir = std::env::temp_dir().join(format!("marg-conf-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        assert_eq!(read_config(&path), Ok((Config::default(), Vec::new())));
        fs::write(&path, "[view]\nzom_step = 1.2\n").unwrap();
        let (c, problems) = read_config(&path).unwrap();
        assert_eq!(c, Config::default());
        assert!(
            problems.len() == 1 && problems[0].ends_with("unknown setting view.zom_step"),
            "{problems:?}"
        );
        fs::write(&path, "[view]\n\nzoom_step = \"x\"\n").unwrap();
        let e = read_config(&path).unwrap_err();
        assert!(e.contains("line 3"), "{e}");
        let (c, err) = load_config(Some(&paths(&dir)));
        assert_eq!((c, err.len()), (Config::default(), 1));
        let _ = fs::remove_dir_all(&dir);
    }
}
