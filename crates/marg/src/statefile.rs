//! Reading and writing the state file.
//!
//! Several marg windows may share the file, so each save re-reads it and
//! changes only its own document's entry.

use std::fs;
use std::io::Write;
use std::path::Path;

use marg_core::state::{FileState, State};

pub fn load(path: &Path) -> Result<State, String> {
    match fs::read_to_string(path) {
        Ok(text) => State::from_toml(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Update one file's entry on disk (atomically: temp file + rename).
pub fn save_entry(path: &Path, key: &str, file: FileState, max_files: usize) -> Result<(), String> {
    // An unreadable state file is replaced rather than blocking all saves;
    // load() has already reported it at startup.
    let mut state = load(path).unwrap_or_default();
    state.update(key.to_string(), file, max_files);
    write_atomic(path, state.to_toml().as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("toml.{}.marg-tmp", std::process::id()));
    let mut f = fs::File::create(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use marg_core::config::Layout;
    use marg_core::view::{DocPos, Fit};

    #[test]
    fn save_keeps_other_entries() {
        let dir = std::env::temp_dir().join(format!("marg-state-{}", std::process::id()));
        let path = dir.join("state.toml");
        let entry = |p: &str| FileState {
            path: p.into(),
            last_opened: 1,
            position: DocPos {
                page: 0,
                x: 0.0,
                y: 0.0,
            },
            zoom: 1.0,
            fit: Fit::Width,
            layout: Layout::Continuous,
        };
        save_entry(&path, "id:aa", entry("/a.pdf"), 10).unwrap();
        save_entry(&path, "id:bb", entry("/b.pdf"), 10).unwrap();
        let state = load(&path).unwrap();
        assert_eq!(state.files.len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }
}
