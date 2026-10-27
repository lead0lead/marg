//! Pure logic for marg: no MuPDF, no egui.

pub mod command;
pub mod config;
pub mod fuzzy;
pub mod keymap;
pub mod keys;
pub mod lineedit;
pub mod lru;
pub mod state;
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
