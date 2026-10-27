//! Configuration types and their defaults.
//!
//! Every tuning value lives here, so the defaults are defined in one place.
//! A missing config file or a missing key means the default below.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Page layout of a document view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    Continuous,
    Single,
    Spread,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub view: ViewConfig,
    pub render: RenderConfig,
    pub ui: UiConfig,
    pub keys: KeysConfig,
}

/// Key bindings on top of the defaults: key sequence → command name
/// (`"nop"` removes a default binding).
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct KeysConfig {
    pub normal: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct ViewConfig {
    pub default_layout_portrait: Layout,
    pub default_layout_landscape: Layout,
    /// Gap between pages in continuous layout, in logical pixels.
    pub page_gap_px: f32,
    /// Distance of one `j`/`k`/`h`/`l` scroll step, in logical pixels.
    pub scroll_step_px: f32,
    /// Zoom factor per `+`/`-` step.
    pub zoom_step: f32,
    /// Zoom limits; 1.0 = 100% (one PDF point per logical pixel).
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub scroll_smooth: bool,
    pub spread_first_page_alone: bool,
}

impl Default for ViewConfig {
    fn default() -> Self {
        Self {
            default_layout_portrait: Layout::Continuous,
            default_layout_landscape: Layout::Single,
            page_gap_px: 8.0,
            scroll_step_px: 60.0,
            zoom_step: 1.1,
            zoom_min: 0.1,
            zoom_max: 8.0,
            scroll_smooth: false,
            spread_first_page_alone: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct RenderConfig {
    /// Pages pre-rendered before and after the visible ones.
    pub prerender_pages: usize,
    /// Texture cache budget.
    pub cache_mb: usize,
    /// Above this zoom, pages render as tiles (only visible tiles).
    pub tile_above_zoom: f32,
    /// Tile edge length in device pixels.
    pub tile_px: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            prerender_pages: 2,
            cache_mb: 256,
            tile_above_zoom: 2.0,
            tile_px: 512,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct UiConfig {
    pub status_bar_px: f32,
    /// How long a status message replaces the right part of the status bar.
    pub message_secs: f32,
    /// Timeout for a pending multi-key sequence such as `gg`.
    pub sequence_timeout_ms: u64,
    /// Length of the recent files list.
    pub recent_files_max: usize,
    /// Files remembered in the state file; the least recently opened are
    /// dropped beyond this.
    pub state_files_max: usize,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            status_bar_px: 22.0,
            message_secs: 4.0,
            sequence_timeout_ms: 1000,
            recent_files_max: 50,
            state_files_max: 1000,
        }
    }
}

impl Config {
    /// Parse a config file. Missing keys take their defaults. The error
    /// message includes the line number.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::from_toml("").unwrap(), Config::default());
    }

    #[test]
    fn partial_section_keeps_other_defaults() {
        let c = Config::from_toml("[view]\nscroll_step_px = 100\n").unwrap();
        assert_eq!(c.view.scroll_step_px, 100.0);
        assert_eq!(c.view.zoom_step, ViewConfig::default().zoom_step);
        assert_eq!(c.render, RenderConfig::default());
    }

    #[test]
    fn layout_names_parse() {
        let c = Config::from_toml("[view]\ndefault_layout_portrait = \"spread\"\n").unwrap();
        assert_eq!(c.view.default_layout_portrait, Layout::Spread);
    }

    #[test]
    fn key_tables_parse() {
        let c = Config::from_toml("[keys.normal]\n\"Ctrl-o\" = \"page.prev\"\n").unwrap();
        assert_eq!(c.keys.normal["Ctrl-o"], "page.prev");
    }

    #[test]
    fn error_mentions_line() {
        let e = Config::from_toml("[view]\n\nzoom_step = \"x\"\n").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
    }
}
