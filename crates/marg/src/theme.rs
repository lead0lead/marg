//! The look: `[colors]` as egui colors, converted once when the config
//! loads, and `[fonts]`.

use std::sync::Arc;

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use marg_core::theme::{ColorsConfig, FontsConfig, Rgba};

use crate::complete;

fn c(x: Rgba) -> Color32 {
    let [r, g, b, a] = x.0;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The colors marg draws with; see `ColorsConfig` for what each is for.
#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Color32,
    pub page_edge: Color32,
    pub statusbar_bg: Color32,
    pub statusbar_fg: Color32,
    pub statusbar_warn: Color32,
    pub statusbar_error: Color32,
    pub panel_bg: Color32,
    pub panel_fg: Color32,
    pub panel_dim: Color32,
    pub panel_border: Color32,
    pub panel_selected_bg: Color32,
    pub editor_selection: Color32,
    pub hint_bg: Color32,
    pub hint_fg: Color32,
    pub hint_typed: Color32,
    pub hint_border: Color32,
    pub selection: Color32,
    pub caret: Color32,
    pub caret_fill: Color32,
    pub search_hit: Color32,
    pub search_current: Color32,
    pub grid_line: Color32,
    pub grid_cell: Color32,
    /// Hint and grid labels.
    pub hint_font: FontId,
}

impl Theme {
    pub fn new(k: &ColorsConfig, fonts: &FontsConfig) -> Self {
        Self {
            hint_font: FontId::monospace(fonts.hint_size.clamp(4.0, 64.0)),
            background: c(k.background),
            page_edge: c(k.page_edge),
            statusbar_bg: c(k.statusbar_bg),
            statusbar_fg: c(k.statusbar_fg),
            statusbar_warn: c(k.statusbar_warn),
            statusbar_error: c(k.statusbar_error),
            panel_bg: c(k.panel_bg),
            panel_fg: c(k.panel_fg),
            panel_dim: c(k.panel_dim),
            panel_border: c(k.panel_border),
            panel_selected_bg: c(k.panel_selected_bg),
            editor_selection: c(k.editor_selection),
            hint_bg: c(k.hint_bg),
            hint_fg: c(k.hint_fg),
            hint_typed: c(k.hint_typed),
            hint_border: c(k.hint_border),
            selection: c(k.selection),
            caret: c(k.caret),
            caret_fill: c(k.caret_fill),
            search_hit: c(k.search_hit),
            search_current: c(k.search_current),
            grid_line: c(k.grid_line),
            grid_cell: c(k.grid_cell),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(&ColorsConfig::default(), &FontsConfig::default())
    }
}

/// Use the `[fonts]` font and size for all text. Returns an error message
/// if the font file cannot be used; the built-in font stays then.
pub fn install_fonts(ctx: &egui::Context, cfg: &FontsConfig) -> Option<String> {
    let size = cfg.size.clamp(4.0, 64.0);
    ctx.all_styles_mut(|s| {
        for style in [TextStyle::Monospace, TextStyle::Body, TextStyle::Button] {
            s.text_styles.insert(style, FontId::monospace(size));
        }
    });
    let file = cfg.file.trim();
    if file.is_empty() {
        return None;
    }
    let path = complete::expand_tilde(file);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return Some(format!("font {}: {e}", path.display())),
    };
    // egui panics on a font it cannot parse, so check it the same way first.
    if let Err(e) = skrifa::FontRef::from_index(&bytes, 0) {
        return Some(format!("font {}: not a usable font ({e})", path.display()));
    }
    let mut defs = FontDefinitions::default();
    defs.font_data
        .insert("user".into(), Arc::new(FontData::from_owned(bytes)));
    // First choice in both families; the built-in fonts fill in missing
    // characters.
    for family in [FontFamily::Monospace, FontFamily::Proportional] {
        defs.families
            .entry(family)
            .or_default()
            .insert(0, "user".into());
    }
    ctx.set_fonts(defs);
    None
}
