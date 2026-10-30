//! Configuration types and their defaults.
//!
//! Every tuning value lives here, so the defaults are defined in one place.
//! A missing config file or a missing key means the default below.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::theme::{ColorsConfig, FontsConfig, StatusbarConfig};

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
    /// A theme file with `[colors]` (and `[fonts]`); entries here override
    /// it. Relative to the config folder; empty: none.
    pub theme: String,
    pub view: ViewConfig,
    pub render: RenderConfig,
    pub ui: UiConfig,
    pub hints: HintsConfig,
    pub search: SearchConfig,
    pub annotation: AnnotationConfig,
    /// Highlight categories; keys `1`–`5` pick the first five.
    #[serde(rename = "category")]
    pub categories: Vec<Category>,
    pub keys: KeysConfig,
    pub colors: ColorsConfig,
    pub fonts: FontsConfig,
    pub statusbar: StatusbarConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: String::new(),
            view: ViewConfig::default(),
            render: RenderConfig::default(),
            ui: UiConfig::default(),
            hints: HintsConfig::default(),
            search: SearchConfig::default(),
            annotation: AnnotationConfig::default(),
            categories: default_categories(),
            keys: KeysConfig::default(),
            colors: ColorsConfig::default(),
            fonts: FontsConfig::default(),
            statusbar: StatusbarConfig::default(),
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
    /// Size of a note's icon.
    pub note_size_pt: f32,
    /// Box mode: the label grid over the view.
    pub box_grid_cols: usize,
    pub box_grid_rows: usize,
    /// Box mode: how far `hjkl` / `HJKL` move a corner, and with Ctrl.
    pub box_step_pt: f32,
    pub box_fine_step_pt: f32,
    /// Save this long after the last annotation change.
    pub autosave_secs: f32,
    /// Undo steps kept per session.
    pub undo_steps: usize,
    /// Annotate files whose permissions forbid it (otherwise they open
    /// read-only; `:unlock` allows it for one file).
    pub ignore_permissions: bool,
}

impl Default for AnnotationConfig {
    fn default() -> Self {
        Self {
            author: String::new(),
            highlight_opacity: 0.4,
            box_stroke_pt: 1.5,
            note_size_pt: 20.0,
            box_grid_cols: 9,
            box_grid_rows: 12,
            box_step_pt: 4.0,
            box_fine_step_pt: 1.0,
            autosave_secs: 2.0,
            undo_steps: 100,
            ignore_permissions: false,
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

/// The search engine `selection.search` uses without a name.
pub const DEFAULT_ENGINE: &str = "https://duckduckgo.com/?q=%s";

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct SearchConfig {
    /// Longer selections are cut at a word boundary (characters).
    pub max_chars: usize,
    /// Name → URL with `%s` for the text. Added to the built-in
    /// `default`, which an entry of that name replaces.
    #[serde(deserialize_with = "engines_over_default")]
    pub engines: BTreeMap<String, String>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_chars: 500,
            engines: default_engines(),
        }
    }
}

fn default_engines() -> BTreeMap<String, String> {
    BTreeMap::from([(
        crate::lookup::DEFAULT.to_string(),
        DEFAULT_ENGINE.to_string(),
    )])
}

fn engines_over_default<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    let mut engines = default_engines();
    engines.extend(BTreeMap::<String, String>::deserialize(d)?);
    Ok(engines)
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
    /// Normal mode while an annotation is selected, on top of `normal`.
    pub annot: BTreeMap<String, String>,
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
    /// Positions kept in the jump history (`Ctrl-o` / `Ctrl-i`).
    pub jump_history: usize,
    /// Dark pages: page content in `recolor_fg` on `recolor_bg`
    /// (`[colors]`); `view.recolor` toggles it.
    pub recolor: bool,
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
            jump_history: 100,
            recolor: false,
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
    /// The reading position is saved this often while it changes (and on
    /// opening, switching files and exit).
    pub state_save_secs: f32,
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
    /// Width of the comment editor, in logical pixels.
    pub comment_width_px: f32,
    /// The comment editor grows up to this share of the window height,
    /// then scrolls.
    pub comment_max_height: f32,
    /// Width of the annotation list, as a share of the window width.
    pub annot_list_width: f32,
    /// Width of the outline, as a share of the window width.
    pub outline_width: f32,
    /// Dropping more PDFs than this at once opens none of them (each one
    /// after the first starts a window).
    pub drop_max_files: usize,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            status_bar_px: 22.0,
            message_secs: 4.0,
            sequence_timeout_ms: 1000,
            recent_files_max: 50,
            state_files_max: 1000,
            state_save_secs: 5.0,
            multi_click_ms: 400,
            file_check_secs: 1.0,
            drag_scroll_rate: 12.0,
            drag_scroll_edge_px: 16.0,
            comment_width_px: 360.0,
            comment_max_height: 0.4,
            annot_list_width: 0.35,
            outline_width: 0.4,
            drop_max_files: 10,
        }
    }
}

impl Config {
    /// Parse a config file. Missing keys take their defaults. The error
    /// message includes the line number.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| one_line(&e))
    }

    /// Parse a config file on top of a theme file: the theme's values are
    /// used where the config has none. Errors name the file they are in
    /// (`Err((in_theme, message))`), with the line number.
    pub fn with_theme(text: &str, theme: &str) -> Result<Self, (bool, String)> {
        let own = Self::from_toml(text).map_err(|e| (false, e))?;
        toml::from_str::<ThemeFile>(theme).map_err(|e| (true, one_line(&e)))?;
        let mut merged: toml::Table = toml::from_str(text).map_err(|e| (false, one_line(&e)))?;
        let theme: toml::Table = toml::from_str(theme).map_err(|e| (true, one_line(&e)))?;
        for (section, values) in theme {
            let (Some(entry), toml::Value::Table(values)) = (
                merged
                    .entry(section)
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()))
                    .as_table_mut(),
                values,
            ) else {
                continue;
            };
            for (k, v) in values {
                entry.entry(k).or_insert(v);
            }
        }
        let merged = Self::deserialize(merged).map_err(|e| (true, one_line(&e)))?;
        Ok(Self {
            theme: own.theme,
            ..merged
        })
    }
}

/// Settings `:set` does not change: tables and the theme file.
const NOT_SET: [&str; 4] = ["keys", "category", "theme", "search.engines"];

impl Config {
    /// `:set name=value`: this config with one setting changed. `name` is
    /// dotted (`view.zoom_step`) or, when unique, the last part alone
    /// (`zoom_step`). `value` is TOML (`1.2`, `true`, `"x"`, `["a"]`);
    /// anything else is taken as a string (`words`, `#000000`). Returns
    /// the full name too.
    pub fn set(&self, name: &str, value: &str) -> Result<(Config, String), String> {
        let path = self.setting_name(name)?;
        let value = match toml::from_str::<toml::Table>(&format!("v = {value}")) {
            Ok(mut t) => t
                .remove("v")
                .unwrap_or_else(|| toml::Value::String(value.into())),
            Err(_) => toml::Value::String(value.trim().into()),
        };
        let mut table = toml::Table::try_from(self).map_err(|e| e.to_string())?;
        let (section, key) = match path.split_once('.') {
            Some((s, k)) => (
                table
                    .get_mut(s)
                    .and_then(|v| v.as_table_mut())
                    .ok_or("no such setting")?,
                k,
            ),
            None => (&mut table, path.as_str()),
        };
        section.insert(key.to_string(), value);
        let config = Config::deserialize(table).map_err(|e| format!("{path}: {}", one_line(&e)))?;
        Ok((config, path))
    }

    /// `:set name`: the full name and the current value, as TOML.
    pub fn get(&self, name: &str) -> Result<(String, String), String> {
        let path = self.setting_name(name)?;
        let root = toml::Value::try_from(self).map_err(|e| e.to_string())?;
        let value = path
            .split('.')
            .try_fold(&root, |v, k| v.get(k))
            .ok_or("no such setting")?;
        // Settings are f32: show 1.1, not 1.100000023841858.
        let shown = match value {
            toml::Value::Float(f) => format!("{}", *f as f32),
            v => v.to_string(),
        };
        Ok((path, shown))
    }

    /// The full dotted name of a setting `:set` may change.
    fn setting_name(&self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if NOT_SET
            .iter()
            .any(|n| name == *n || name.starts_with(&format!("{n}.")))
        {
            return Err(format!("{name}: change it in the config file"));
        }
        let mut all = BTreeSet::new();
        if let Ok(t) = toml::Table::try_from(Config::default()) {
            setting_paths(&t, "", &mut all);
        }
        let all: Vec<String> = all
            .into_iter()
            .filter(|p| {
                !NOT_SET
                    .iter()
                    .any(|n| p == n || p.starts_with(&format!("{n}.")))
            })
            .collect();
        if all.iter().any(|p| p == name) {
            return Ok(name.to_string());
        }
        let matches: Vec<&String> = all
            .iter()
            .filter(|p| p.rsplit('.').next() == Some(name))
            .collect();
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(format!("unknown setting {name}")),
            many => Err(format!(
                "{name} is ambiguous: {}",
                many.iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
}

/// A TOML error on one line, for the status bar: its position, then the
/// reason, without the quoted source lines.
fn one_line(e: &impl std::fmt::Display) -> String {
    let text = e.to_string();
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next().unwrap_or_default();
    let first = first.strip_prefix("TOML parse error at ").unwrap_or(first);
    // The quoted source: "|", "4 | [view", "|      ^".
    let rest: Vec<&str> = lines
        .filter(|l| {
            !l.starts_with('|')
                && !l
                    .split_once(" |")
                    .is_some_and(|(n, _)| n.trim().parse::<u32>().is_ok())
        })
        .collect();
    if rest.is_empty() {
        first.to_string()
    } else {
        format!("{first}: {}", rest.join(": "))
    }
}

/// Settings in a config or theme file that marg does not know (dotted,
/// such as `view.zom_step`); serde would ignore them silently. Empty if
/// the text does not parse (that error is reported by the parse).
pub fn unknown_settings(text: &str) -> Vec<String> {
    let Ok(table) = toml::from_str::<toml::Table>(text) else {
        return Vec::new();
    };
    let mut known = BTreeSet::new();
    if let Ok(t) = toml::Table::try_from(Config::default()) {
        setting_paths(&t, "", &mut known);
    }
    let mut written = BTreeSet::new();
    setting_paths(&table, "", &mut written);
    written.difference(&known).cloned().collect()
}

/// Every setting in `t`, dotted. The `keys.*` tables and
/// `search.engines` count as one setting each (their keys are key
/// sequences or names); arrays of tables (such as
/// `category`) by their tables' keys.
fn setting_paths(t: &toml::Table, prefix: &str, out: &mut BTreeSet<String>) {
    for (k, v) in t {
        let p = format!("{prefix}{k}");
        match v {
            toml::Value::Table(_) if prefix == "keys." || p == "search.engines" => {
                out.insert(p);
            }
            toml::Value::Table(t) => setting_paths(t, &format!("{p}."), out),
            toml::Value::Array(a) if !a.is_empty() && a.iter().all(|v| v.is_table()) => {
                for t in a.iter().filter_map(|v| v.as_table()) {
                    setting_paths(t, &format!("{p}."), out);
                }
            }
            _ => {
                out.insert(p);
            }
        }
    }
}

/// What a theme file may hold (parsed only to check it).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    #[serde(default, rename = "colors")]
    _colors: ColorsConfig,
    #[serde(default, rename = "fonts")]
    _fonts: FontsConfig,
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
    fn colors_override_one_by_one_and_report_bad_values() {
        let c = Config::from_toml("[colors]\nbackground = \"#000000\"\n").unwrap();
        assert_eq!(c.colors.background.to_string(), "#000000");
        assert_eq!(c.colors.hint_bg, ColorsConfig::default().hint_bg);
        let e = Config::from_toml("[colors]\n\nselection = \"blue\"\n").unwrap_err();
        assert!(e.contains("line 3") && e.contains("not a color"), "{e}");
    }

    #[test]
    fn theme_fills_in_what_the_config_leaves_out() {
        let theme =
            "[colors]\nbackground = \"#111111\"\nhint_bg = \"#222222\"\n[fonts]\nsize = 14.0\n";
        let text =
            "theme = \"dark.toml\"\n[colors]\nhint_bg = \"#333333\"\n[view]\nzoom_step = 1.2\n";
        let c = Config::with_theme(text, theme).unwrap();
        assert_eq!(c.theme, "dark.toml");
        assert_eq!(c.colors.background.to_string(), "#111111");
        assert_eq!(c.colors.hint_bg.to_string(), "#333333", "the config wins");
        assert_eq!(c.colors.caret, ColorsConfig::default().caret);
        assert_eq!(c.view.zoom_step, 1.2);
        assert_eq!(c.fonts.size, 14.0);
        assert_eq!(c.fonts.hint_size, 12.0);
        // Errors say which file and line.
        let bad = "[colors]\n\ncaret = \"red\"\n";
        let (in_theme, e) = Config::with_theme(text, bad).unwrap_err();
        assert!(in_theme && e.contains("line 3"), "{e}");
        let (in_theme, e) = Config::with_theme("[view]\nzoom_step = \"x\"\n", theme).unwrap_err();
        assert!(!in_theme && e.contains("line 2"), "{e}");
        // A theme holds only looks.
        let (in_theme, e) = Config::with_theme(text, "[view]\nzoom_step = 2.0\n").unwrap_err();
        assert!(in_theme && e.contains("view"), "{e}");
    }

    #[test]
    fn example_themes_load_and_set_every_color() {
        let themes = [
            include_str!("../../../docs/themes/gruvbox-dark.toml"),
            include_str!("../../../docs/themes/solarized-light.toml"),
        ];
        // Every [colors] key, as the default config writes them.
        let keys = toml::Table::try_from(ColorsConfig::default()).unwrap();
        for theme in themes {
            let c = Config::with_theme("", theme).unwrap();
            assert_ne!(c.colors, ColorsConfig::default());
            let t: toml::Table = toml::from_str(theme).unwrap();
            let set = t["colors"].as_table().unwrap();
            let missing: Vec<_> = keys.keys().filter(|k| !set.contains_key(*k)).collect();
            assert!(missing.is_empty(), "theme does not set {missing:?}");
        }
    }

    #[test]
    fn set_changes_one_setting() {
        let c = Config::default();
        let (c2, name) = c.set("view.zoom_step", "1.2").unwrap();
        assert_eq!((c2.view.zoom_step, name.as_str()), (1.2, "view.zoom_step"));
        // Short names, integers for floats, bare strings, enums.
        let (c2, name) = c.set("scroll_step_px", "80").unwrap();
        assert_eq!(
            (c2.view.scroll_step_px, name.as_str()),
            (80.0, "view.scroll_step_px")
        );
        assert_eq!(
            c.set("mode", "words").unwrap().0.hints.mode,
            HintMode::Words
        );
        assert!(c.set("recolor", "true").unwrap().0.view.recolor);
        let (c2, _) = c.set("colors.background", "#102030").unwrap();
        assert_eq!(c2.colors.background.to_string(), "#102030");
        let (c2, _) = c.set("statusbar.right", "[\"keys\"]").unwrap();
        assert_eq!(c2.statusbar.right, ["keys"]);
        // Everything else stays.
        assert_eq!(c.set("zoom_step", "1.1").unwrap().0, c);
        // Errors.
        assert!(
            c.set("zoom_step", "fast")
                .unwrap_err()
                .contains("view.zoom_step")
        );
        assert!(c.set("mode", "fuzzy").is_err());
        assert!(
            c.set("background", "blue")
                .unwrap_err()
                .contains("not a color")
        );
        assert!(
            c.set("nonsense", "1")
                .unwrap_err()
                .contains("unknown setting")
        );
        assert!(
            c.set("keys.normal", "1")
                .unwrap_err()
                .contains("config file")
        );
        assert!(
            c.set("theme", "x.toml")
                .unwrap_err()
                .contains("config file")
        );
        assert!(c.set("category.name", "x").is_err());
    }

    #[test]
    fn get_shows_values_and_finds_ambiguous_names() {
        let c = Config::default();
        assert_eq!(
            c.get("zoom_step"),
            Ok(("view.zoom_step".into(), "1.1".into()))
        );
        assert_eq!(
            c.get("hints.mode"),
            Ok(("hints.mode".into(), "\"two-stage\"".into()))
        );
        // `size` is only in [fonts]; `chars` in [hints].
        assert_eq!(c.get("chars").unwrap().0, "hints.chars");
        let names: Vec<String> = {
            let mut s = BTreeSet::new();
            setting_paths(
                &toml::Table::try_from(Config::default()).unwrap(),
                "",
                &mut s,
            );
            s.into_iter()
                .filter(|p| !NOT_SET.iter().any(|n| p.starts_with(n)))
                .collect()
        };
        let last = |p: &String| p.rsplit('.').next().unwrap().to_string();
        if let Some(dup) = names
            .iter()
            .find(|p| names.iter().filter(|q| last(q) == last(p)).count() > 1)
        {
            assert!(c.get(&last(dup)).unwrap_err().contains("ambiguous"));
        }
    }

    #[test]
    fn unknown_settings_are_found() {
        let text = "zoom = 2\n[view]\nzom_step = 1.2\nzoom_step = 1.2\n\
                    [keys.normal]\n\"j\" = \"scroll.up\"\n[keys.insert]\n\"x\" = \"undo\"\n\
                    [[category]]\nname = \"A\"\ncolour = \"#FF0000\"\n\
                    [colors]\nbackgrund = \"#000000\"\n[fonts]\nsize = 14.0\n";
        assert_eq!(
            unknown_settings(text),
            [
                "category.colour",
                "colors.backgrund",
                "keys.insert",
                "view.zom_step",
                "zoom"
            ]
        );
        assert!(unknown_settings("").is_empty());
        assert!(unknown_settings("[view\n").is_empty(), "a parse error");
    }

    #[test]
    fn error_mentions_line() {
        let e = Config::from_toml("[view]\n\nzoom_step = \"x\"\n").unwrap_err();
        assert!(e.contains("line 3"), "{e}");
        let e = Config::from_toml("[colors]\n\n[view\n").unwrap_err();
        assert!(!e.contains('\n') && !e.contains('|'), "one line: {e}");
        assert!(e.contains("line 3") && e.contains("unclosed table"), "{e}");
    }
}
