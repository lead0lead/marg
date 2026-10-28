//! Annotations as marg creates them, independent of the PDF engine.

use crate::text::BBox;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotKind {
    Highlight,
    Underline,
}

/// Everything needed to create an annotation (or recreate it on redo).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotSpec {
    pub page: usize,
    pub kind: AnnotKind,
    /// One box per line, in points from the page's top-left corner.
    pub rects: Vec<BBox>,
    /// RGB, 0–255.
    pub color: [u8; 3],
    pub opacity: f32,
    /// Category name, written to `/Subj`.
    pub subject: String,
    /// Written to `/T`.
    pub author: String,
}

/// Parse `#RRGGBB` (the `#` is optional).
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#FFD400"), Some([0xFF, 0xD4, 0x00]));
        assert_eq!(parse_color("a28ae5"), Some([0xA2, 0x8A, 0xE5]));
        assert_eq!(parse_color("#FFD40"), None);
        assert_eq!(parse_color("#GGGGGG"), None);
        assert_eq!(parse_color("#ÄÄÄ"), None);
    }
}
