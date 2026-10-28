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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub view: ViewConfig,
    pub render: RenderConfig,
    pub ui: UiConfig,
    pub hints: HintsConfig,
    pub annotation: AnnotationConfig,
    /// Highlight categories; keys `1`–`5` pick the first five.
    #[serde(rename = "category")]
    pub categories: Vec<Category>,
    pub keys: KeysConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            view: ViewConfig::default(),
            render: RenderConfig::default(),
            ui: UiConfig::default(),
            hints: HintsConfig::default(),
            annotation: AnnotationConfig::default(),
            categories: default_categories(),
            keys: KeysConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Category {
    pub name: String,
    /// `#RRGGBB`.
    pub color: String,
}

fn default_categories() -> Vec<Category> {
    [
        ("Key point", "#FFD400"),
        ("Question", "#A28AE5"),
        ("Definition", "#2EA8E5"),
        ("Disagree", "#FF6666"),
        ("Evidence", "#5FB236"),
    ]
    .into_iter()
    .map(|(name, color)| Category {
        name: name.into(),
        color: color.into(),
    })
    .collect()
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct AnnotationConfig {
    /// Written as the annotation author; empty = the OS user name.
    pub author: String,
    pub highlight_opacity: f32,
    pub box_stroke_pt: f32,
    /// Save this long after the last annotation change.
    pub autosave_secs: f32,
    /// Undo steps kept per session.
    pub undo_steps: usize,
}

impl Default for AnnotationConfig {
    fn default() -> Self {
        Self {
            author: String::new(),
            highlight_opacity: 0.4,
            box_stroke_pt: 1.5,
            autosave_secs: 2.0,
            undo_steps: 100,
        }
    }
}

/// How text hints label targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HintMode {
    /// Lines first, then the words of the picked line.
    TwoStage,
    /// Every visible word at once.
    Words,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct HintsConfig {
    /// Label alphabet.
    pub chars: String,
    pub mode: HintMode,
    /// Added to `chars` for the word labels of a picked line, so long lines
    /// still get single-character labels.
    pub extra_chars: String,
}

impl Default for HintsConfig {
    fn default() -> Self {
        Self {
            chars: "asdfghjkl".into(),
            mode: HintMode::TwoStage,
            extra_chars: "qwertyuiop".into(),
        }
    }
}

/// Key bindings on top of the defaults: key sequence → command name
/// (`"nop"` removes a default binding).
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct KeysConfig {
    pub normal: BTreeMap<String, String>,
    /// Caret mode, on top of `normal`.
    pub caret: BTreeMap<String, String>,
    /// Visual mode, on top of `caret`.
    pub visual: BTreeMap<String, String>,
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
    /// Presses closer together than this count as a double or triple click.
    pub multi_click_ms: u64,
    /// How often the open file is checked for changes by other programs.
    pub file_check_secs: f32,
    /// Dragging a selection near or past the top or bottom edge scrolls by
    /// this many pixels per second for each pixel of distance into the
    /// edge zone.
    pub drag_scroll_rate: f32,
    /// Height of that edge zone, in logical pixels.
    pub drag_scroll_edge_px: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            status_bar_px: 22.0,
            message_secs: 4.0,
            sequence_timeout_ms: 1000,
            recent_files_max: 50,
            state_files_max: 1000,
            multi_click_ms: 400,
            file_check_secs: 1.0,
            drag_scroll_rate: 12.0,
            drag_scroll_edge_px: 16.0,
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
    fn categories_default_and_replace() {
        let c = Config::default();
        assert_eq!(c.categories.len(), 5);
        assert_eq!(c.categories[0].name, "Key point");
        let c =
            Config::from_toml("[[category]]\nname = \"Important\"\ncolor = \"#FF0000\"\n").unwrap();
        assert_eq!(c.categories.len(), 1);
        assert_eq!(c.categories[0].name, "Important");
    }

    #[test]
    fn hint_mode_parses() {
        let c = Config::from_toml("[hints]\nmode = \"words\"\n").unwrap();
        assert_eq!(c.hints.mode, HintMode::Words);
        assert_eq!(Config::default().hints.mode, HintMode::TwoStage);
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
