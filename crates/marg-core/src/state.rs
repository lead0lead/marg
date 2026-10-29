//! The state file: per-file reading position and view, and the recent
//! files list.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::Layout;
use crate::view::{DocPos, Fit};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileState {
    /// Last known path of the file.
    pub path: String,
    /// Seconds since the Unix epoch.
    pub last_opened: u64,
    /// Reading position at the top-left of the window.
    pub position: DocPos,
    pub zoom: f32,
    pub fit: Fit,
    pub layout: Layout,
    /// Marks `a`–`z` (`m{a-z}`, `'{a-z}`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub marks: BTreeMap<char, DocPos>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Keyed by [`file_key`].
    pub files: BTreeMap<String, FileState>,
}

/// The state key of a document: its permanent ID (first element of the
/// trailer `/ID`), which survives incremental saves; else its path.
pub fn file_key(id: Option<&[u8]>, canonical_path: &str) -> String {
    match id.filter(|id| !id.is_empty()) {
        Some(id) => {
            let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
            format!("id:{hex}")
        }
        None => format!("path:{canonical_path}"),
    }
}

impl State {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).expect("state serializes")
    }

    /// Insert or replace one file's state, then drop the least recently
    /// opened files beyond `max_files`.
    pub fn update(&mut self, key: String, file: FileState, max_files: usize) {
        self.files.insert(key, file);
        if self.files.len() > max_files {
            let mut by_age: Vec<(u64, String)> = self
                .files
                .iter()
                .map(|(k, f)| (f.last_opened, k.clone()))
                .collect();
            by_age.sort();
            let excess = self.files.len() - max_files;
            for (_, k) in by_age.into_iter().take(excess) {
                self.files.remove(&k);
            }
        }
    }

    /// The most recently opened files, newest first, one per path.
    pub fn recent(&self, max: usize) -> Vec<&FileState> {
        let mut files: Vec<&FileState> = self.files.values().collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.last_opened));
        let mut seen = std::collections::HashSet::new();
        files.retain(|f| seen.insert(f.path.as_str()));
        files.truncate(max);
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, last_opened: u64) -> FileState {
        FileState {
            path: path.into(),
            last_opened,
            position: DocPos {
                page: 3,
                x: 0.0,
                y: 120.5,
            },
            zoom: 1.25,
            fit: Fit::Width,
            layout: Layout::Continuous,
            marks: BTreeMap::new(),
        }
    }

    #[test]
    fn key_prefers_document_id() {
        assert_eq!(file_key(Some(b"\x01\xab"), "/a.pdf"), "id:01ab");
        assert_eq!(file_key(None, "/a.pdf"), "path:/a.pdf");
        assert_eq!(file_key(Some(b""), "/a.pdf"), "path:/a.pdf");
    }

    #[test]
    fn round_trips_through_toml() {
        let mut s = State::default();
        s.update("id:01".into(), file("/papers/a.pdf", 10), 100);
        s.update("path:/b c.pdf".into(), file("/b c.pdf", 20), 100);
        let back = State::from_toml(&s.to_toml()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn recent_is_newest_first_and_capped() {
        let mut s = State::default();
        for i in 0..5 {
            s.update(format!("id:{i}"), file(&format!("/{i}.pdf"), i), 100);
        }
        let paths: Vec<_> = s.recent(3).iter().map(|f| f.path.clone()).collect();
        assert_eq!(paths, ["/4.pdf", "/3.pdf", "/2.pdf"]);
    }

    #[test]
    fn recent_lists_a_path_once() {
        let mut s = State::default();
        // A file whose ID changed (rewritten by another program).
        s.update("id:old".into(), file("/a.pdf", 1), 100);
        s.update("id:new".into(), file("/a.pdf", 2), 100);
        assert_eq!(s.recent(10).len(), 1);
    }

    #[test]
    fn update_drops_oldest_beyond_limit() {
        let mut s = State::default();
        for i in 0..5 {
            s.update(format!("id:{i}"), file("/x.pdf", i), 3);
        }
        let keys: Vec<_> = s.files.keys().cloned().collect();
        assert_eq!(keys, ["id:2", "id:3", "id:4"]);
    }

    #[test]
    fn unreadable_state_is_an_error() {
        assert!(State::from_toml("files = 3").is_err());
        assert_eq!(State::from_toml("").unwrap(), State::default());
    }
}
