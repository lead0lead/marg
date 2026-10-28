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

/// An annotation found in the PDF (by marg or another program).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotInfo {
    /// Object number; identifies the annotation for edits.
    pub xref: i32,
    pub page: usize,
    /// PDF subtype, e.g. "Highlight", "Underline", "Text", "Square".
    pub kind: String,
    /// Bounding box, in points from the page's top-left corner.
    pub rect: BBox,
    /// One box per QuadPoint, for text markup annotations.
    pub quads: Vec<BBox>,
    pub color: Option<[u8; 3]>,
    pub subject: Option<String>,
    pub contents: Option<String>,
}

impl AnnotInfo {
    /// The boxes it covers: its quads, or its rectangle.
    pub fn boxes(&self) -> &[BBox] {
        if self.quads.is_empty() {
            std::slice::from_ref(&self.rect)
        } else {
            &self.quads
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        self.boxes().iter().any(|b| b.contains(x, y))
    }

    /// Reading position: page, then top, then left.
    fn key(&self) -> (usize, f32, f32, i32) {
        (self.page, self.rect.y0, self.rect.x0, self.xref)
    }
}

/// Where `]a` / `[a` start from.
#[derive(Debug, Clone, Copy)]
pub enum AnnotFrom {
    /// The selected annotation.
    Selected { page: usize, xref: i32 },
    /// A document position (the top of the view), when none is selected.
    Pos { page: usize, y: f32 },
}

/// The next (or previous) annotation in reading order. `annots` holds all
/// known annotations, in any order.
pub fn step_annot(annots: &[AnnotInfo], from: AnnotFrom, forward: bool) -> Option<&AnnotInfo> {
    let cmp = |a: &AnnotInfo, b: &AnnotInfo| {
        a.key()
            .partial_cmp(&b.key())
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    let mut sorted: Vec<&AnnotInfo> = annots.iter().collect();
    sorted.sort_by(|a, b| cmp(a, b));
    match from {
        AnnotFrom::Selected { page, xref } => {
            let i = sorted
                .iter()
                .position(|a| a.page == page && a.xref == xref)?;
            if forward {
                sorted.get(i + 1).copied()
            } else {
                i.checked_sub(1).map(|j| sorted[j])
            }
        }
        AnnotFrom::Pos { page, y } => {
            let after = |a: &AnnotInfo| (a.page, a.rect.y0) >= (page, y);
            if forward {
                sorted.into_iter().find(|a| after(a))
            } else {
                sorted.into_iter().rev().find(|a| !after(a))
            }
        }
    }
}

/// The topmost annotation at a point on a page (later ones are drawn on
/// top).
pub fn annot_at(annots: &[AnnotInfo], page: usize, x: f32, y: f32) -> Option<&AnnotInfo> {
    annots
        .iter()
        .rev()
        .find(|a| a.page == page && a.contains(x, y))
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

    fn info(page: usize, xref: i32, y: f32) -> AnnotInfo {
        AnnotInfo {
            xref,
            page,
            kind: "Highlight".into(),
            rect: BBox::new(10.0, y, 100.0, y + 10.0),
            quads: vec![],
            color: None,
            subject: None,
            contents: None,
        }
    }

    #[test]
    fn steps_in_reading_order() {
        let a = vec![info(1, 5, 50.0), info(0, 7, 300.0), info(0, 3, 100.0)];
        let sel = |page, xref| AnnotFrom::Selected { page, xref };
        let next = |f, fw| step_annot(&a, f, fw).map(|a| a.xref);
        assert_eq!(next(sel(0, 3), true), Some(7));
        assert_eq!(next(sel(0, 7), true), Some(5));
        assert_eq!(next(sel(1, 5), true), None);
        assert_eq!(next(sel(0, 7), false), Some(3));
        assert_eq!(next(sel(0, 3), false), None);
        // Without a selection: from the top of the view.
        let pos = |page, y| AnnotFrom::Pos { page, y };
        assert_eq!(next(pos(0, 150.0), true), Some(7));
        assert_eq!(next(pos(0, 150.0), false), Some(3));
        assert_eq!(next(pos(0, 100.0), true), Some(3));
        assert_eq!(next(pos(2, 0.0), true), None);
    }

    #[test]
    fn hit_test_prefers_topmost_and_quads() {
        let mut low = info(0, 1, 0.0);
        low.rect = BBox::new(0.0, 0.0, 200.0, 200.0);
        let mut high = info(0, 2, 0.0);
        high.rect = BBox::new(0.0, 0.0, 200.0, 40.0);
        high.quads = vec![
            BBox::new(0.0, 0.0, 200.0, 10.0),
            BBox::new(0.0, 30.0, 50.0, 40.0),
        ];
        let a = vec![low, high];
        assert_eq!(annot_at(&a, 0, 5.0, 5.0).map(|a| a.xref), Some(2));
        // Inside the top annotation's rect but between its quads.
        assert_eq!(annot_at(&a, 0, 100.0, 20.0).map(|a| a.xref), Some(1));
        assert_eq!(annot_at(&a, 1, 5.0, 5.0), None);
    }

    #[test]
    fn colors_parse() {
        assert_eq!(parse_color("#FFD400"), Some([0xFF, 0xD4, 0x00]));
        assert_eq!(parse_color("a28ae5"), Some([0xA2, 0x8A, 0xE5]));
        assert_eq!(parse_color("#FFD40"), None);
        assert_eq!(parse_color("#GGGGGG"), None);
        assert_eq!(parse_color("#ÄÄÄ"), None);
    }
}
