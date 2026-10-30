//! Per-OS locations of config, state, log and backups (concept §8).

use std::path::PathBuf;

use directories::BaseDirs;

pub struct Paths {
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub log_file: PathBuf,
    pub backup_dir: PathBuf,
}

impl Paths {
    /// Linux: `~/.config/marg` and `~/.local/state/marg` (respecting
    /// `XDG_*`), backups in `~/.local/share/marg/backups`. Windows:
    /// `%APPDATA%\marg` for config, `%LOCALAPPDATA%\marg` for the rest.
    /// macOS: everything in `~/Library/Application Support/marg` (no state
    /// dir there).
    pub fn resolve() -> Option<Self> {
        let base = BaseDirs::new()?;
        let config = base.config_dir().join("marg");
        let state = base
            .state_dir()
            .unwrap_or_else(|| base.data_local_dir())
            .join("marg");
        Some(Self {
            config_file: config.join("config.toml"),
            state_file: state.join("state.toml"),
            log_file: state.join("marg.log"),
            backup_dir: base.data_local_dir().join("marg").join("backups"),
        })
    }
}
