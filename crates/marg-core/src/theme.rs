//! The look: every color marg draws (`[colors]`). Defaults are marg's
//! own look; a theme file or the config overrides them (§6 "Theming").

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A color with alpha, written `#RRGGBB` or `#RRGGBBAA` (not
/// premultiplied).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 0xFF])
    }

    pub const fn with_alpha(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    /// Parse `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
    pub fn parse(s: &str) -> Option<Self> {
        let hex = s.trim();
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if !hex.is_ascii() || !(hex.len() == 6 || hex.len() == 8) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        let a = if hex.len() == 8 { byte(6)? } else { 0xFF };
        Some(Self([byte(0)?, byte(2)?, byte(4)?, a]))
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b, a] = self.0;
        if a == 0xFF {
            write!(f, "#{r:02X}{g:02X}{b:02X}")
        } else {
            write!(f, "#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgba::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!("\"{s}\" is not a color (#RRGGBB or #RRGGBBAA)"))
        })
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

/// `[fonts]`: the font of everything marg writes (all monospace).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct FontsConfig {
    /// A `.ttf`/`.otf`/`.ttc` file; empty: egui's built-in font.
    pub file: String,
    /// Status bar, lists, editor, tooltips.
    pub size: f32,
    /// Hint and grid labels.
    pub hint_size: f32,
}

impl Default for FontsConfig {
    fn default() -> Self {
        Self {
            file: String::new(),
            size: 12.0,
            hint_size: 12.0,
        }
    }
}

/// `[colors]`: every color marg draws.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct ColorsConfig {
    /// Around the pages.
    pub background: Rgba,
    /// The line at page edges.
    pub page_edge: Rgba,
    pub statusbar_bg: Rgba,
    pub statusbar_fg: Rgba,
    pub statusbar_warn: Rgba,
    pub statusbar_error: Rgba,
    /// Lists, the comment editor, tooltips, the debug overlay.
    pub panel_bg: Rgba,
    pub panel_fg: Rgba,
    pub panel_dim: Rgba,
    pub panel_border: Rgba,
    pub panel_selected_bg: Rgba,
    /// Selected text in the comment editor.
    pub editor_selection: Rgba,
    /// Hint and grid labels.
    pub hint_bg: Rgba,
    pub hint_fg: Rgba,
    pub hint_typed: Rgba,
    pub hint_border: Rgba,
    /// Text selection on the page.
    pub selection: Rgba,
    pub caret: Rgba,
    pub caret_fill: Rgba,
    /// Box mode grid, and the picked cell.
    pub grid_line: Rgba,
    pub grid_cell: Rgba,
    /// What black and white become on recolored pages.
    pub recolor_fg: Rgba,
    pub recolor_bg: Rgba,
}

impl Default for ColorsConfig {
    fn default() -> Self {
        Self {
            background: Rgba::rgb(0x2B, 0x2B, 0x2B),
            page_edge: Rgba::with_alpha(0, 0, 0, 0x5A),
            statusbar_bg: Rgba::rgb(0x1E, 0x1E, 0x1E),
            statusbar_fg: Rgba::rgb(0xD0, 0xD0, 0xD0),
            statusbar_warn: Rgba::rgb(0xFF, 0xC0, 0x60),
            statusbar_error: Rgba::rgb(0xFF, 0x70, 0x70),
            panel_bg: Rgba::rgb(0x1E, 0x1E, 0x1E),
            panel_fg: Rgba::rgb(0xD0, 0xD0, 0xD0),
            panel_dim: Rgba::rgb(0x80, 0x80, 0x80),
            panel_border: Rgba::rgb(0x55, 0x55, 0x55),
            panel_selected_bg: Rgba::rgb(0x3A, 0x3A, 0x3A),
            editor_selection: Rgba::rgb(0x2D, 0x4E, 0x8F),
            hint_bg: Rgba::rgb(0xFF, 0xD8, 0x4D),
            hint_fg: Rgba::rgb(0, 0, 0),
            hint_typed: Rgba::rgb(0x9A, 0x80, 0x20),
            hint_border: Rgba::rgb(0x80, 0x68, 0x10),
            selection: Rgba::with_alpha(0x3D, 0x7E, 0xFF, 0x59),
            caret: Rgba::rgb(0xE0, 0x20, 0x20),
            caret_fill: Rgba::with_alpha(0xE0, 0x20, 0x20, 0x28),
            grid_line: Rgba::with_alpha(0x3D, 0x7E, 0xFF, 0x5A),
            grid_cell: Rgba::with_alpha(0x3D, 0x7E, 0xFF, 0x3C),
            recolor_fg: Rgba::rgb(0xD0, 0xD0, 0xD0),
            recolor_bg: Rgba::rgb(0x1E, 0x1E, 0x1E),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse_with_and_without_alpha() {
        assert_eq!(Rgba::parse("#1e1E1e"), Some(Rgba::rgb(0x1E, 0x1E, 0x1E)));
        assert_eq!(
            Rgba::parse("3D7EFF59"),
            Some(Rgba::with_alpha(0x3D, 0x7E, 0xFF, 0x59))
        );
        for bad in ["", "#12345", "#1234567", "#GG0000", "#ääää"] {
            assert_eq!(Rgba::parse(bad), None, "{bad}");
        }
        assert_eq!(Rgba::with_alpha(1, 2, 3, 4).to_string(), "#01020304");
        assert_eq!(Rgba::rgb(1, 2, 3).to_string(), "#010203");
    }
}
