//! Pure logic for marg: no MuPDF, no egui.

pub mod annot;
pub mod annotlist;
pub mod boxsel;
pub mod caret;
pub mod command;
pub mod config;
pub mod fuzzy;
pub mod hints;
pub mod jumps;
pub mod keymap;
pub mod keys;
pub mod lineedit;
pub mod list;
pub mod lru;
pub mod state;
pub mod stats;
pub mod text;
pub mod theme;
pub mod undo;
pub mod view;

/// Crate version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_set() {
        assert!(!VERSION.is_empty());
    }
}
