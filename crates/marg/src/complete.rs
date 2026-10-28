//! Path completion for `:e`.

use std::path::{MAIN_SEPARATOR, PathBuf};

use marg_core::lineedit::common_prefix;

/// `~` or `~/…` → the home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    let home = || directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf());
    if path == "~" {
        return home().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path
        .strip_prefix("~/")
        .or_else(|| path.strip_prefix(&format!("~{MAIN_SEPARATOR}")))
        && let Some(h) = home()
    {
        return h.join(rest);
    }
    PathBuf::from(path)
}

pub struct Completion {
    /// The completed argument, as the user typed it (with `~` kept).
    pub text: String,
    /// Matching names in the directory (directories end with a separator).
    pub candidates: Vec<String>,
}

fn is_separator(c: char) -> bool {
    c == '/' || c == MAIN_SEPARATOR
}

/// Complete `arg` against the directories and PDF files on disk.
pub fn path(arg: &str) -> Completion {
    let split = arg.rfind(is_separator).map_or(0, |i| i + 1);
    let (dir_part, prefix) = arg.split_at(split);
    let dir = if dir_part.is_empty() {
        PathBuf::from(".")
    } else {
        expand_tilde(dir_part)
    };
    let mut candidates: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.starts_with(prefix) || (name.starts_with('.') && !prefix.starts_with('.')) {
                return None;
            }
            // Follows symlinks, so linked directories complete as directories.
            let is_dir = entry.path().is_dir();
            if is_dir {
                Some(format!("{name}/"))
            } else if name.to_lowercase().ends_with(".pdf") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    candidates.sort();
    let common = common_prefix(&candidates);
    let completed = if common.len() > prefix.len() {
        common
    } else {
        prefix.to_string()
    };
    Completion {
        text: format!("{dir_part}{completed}"),
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_unique_and_common_prefixes() {
        let dir = std::env::temp_dir().join(format!("marg-complete-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("papers")).unwrap();
        for f in ["paper-one.pdf", "paper-two.pdf", "notes.txt"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let base = format!("{}/", dir.display());

        let c = path(&format!("{base}pap"));
        assert_eq!(c.text, format!("{base}paper"));
        assert_eq!(c.candidates, ["paper-one.pdf", "paper-two.pdf", "papers/"]);

        let c = path(&format!("{base}paper-t"));
        assert_eq!(c.text, format!("{base}paper-two.pdf"));

        // Non-PDF files are not offered.
        let c = path(&format!("{base}no"));
        assert!(c.candidates.is_empty());
        assert_eq!(c.text, format!("{base}no"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
