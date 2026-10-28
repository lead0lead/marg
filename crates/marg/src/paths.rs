//! Per-OS locations of config, state and log (concept §8). Backups come
//! with saving in Phase 2.

use std::path::PathBuf;

use directories::BaseDirs;

pub struct Paths {
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub log_file: PathBuf,
}

impl Paths {
    /// Linux: `~/.config/marg` and `~/.local/state/marg` (respecting
    /// `XDG_*`). Windows: `%APPDATA%\marg` for config, `%LOCALAPPDATA%\marg`
    /// for the rest.
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
        })
    }
}
