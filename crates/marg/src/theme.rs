//! `[colors]` as egui colors, converted once when the config loads.

use eframe::egui::Color32;
use marg_core::theme::{ColorsConfig, Rgba};

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
    pub grid_line: Color32,
    pub grid_cell: Color32,
}

impl Theme {
    pub fn new(k: &ColorsConfig) -> Self {
        Self {
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
            grid_line: c(k.grid_line),
            grid_cell: c(k.grid_cell),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(&ColorsConfig::default())
    }
}
