//! Annotations as marg creates them, independent of the PDF engine.

use std::collections::HashSet;

use crate::text::{BBox, PageText};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotKind {
    Highlight,
    Underline,
    /// A sticky note (PDF `Text`); its one rect is the icon.
    Note,
    /// A rectangle (PDF `Square`), stroked, not filled; one rect.
    Box,
}

/// Everything needed to create an annotation (or recreate it on redo).
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotSpec {
    pub page: usize,
    pub kind: AnnotKind,
    /// One box per line, in points from the page's top-left corner (for a
    /// note: the icon).
    pub rects: Vec<BBox>,
    /// RGB, 0–255.
    pub color: [u8; 3],
    pub opacity: f32,
    /// Line width of a box, in points.
    pub stroke_pt: f32,
    /// Category name, written to `/Subj`.
    pub subject: String,
    /// Written to `/T`.
    pub author: String,
    /// The comment (`/Contents`).
    pub contents: Option<String>,
}

/// A change to an existing annotation; `None` fields stay as they are.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotChange {
    pub color: Option<[u8; 3]>,
    pub opacity: Option<f32>,
    /// Category name for `/Subj`; empty removes it.
    pub subject: Option<String>,
    /// The comment; empty removes it.
    pub contents: Option<String>,
    /// Switch between highlight and underline.
    pub kind: Option<AnnotKind>,
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
    pub opacity: f32,
    pub subject: Option<String>,
    pub author: Option<String>,
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

/// Erase the `selected` characters of page text `t` from a highlight or
/// underline. `None`: it covers none of them, so it stays as it is.
/// Otherwise the groups of boxes that remain, in reading order; each group
/// becomes one annotation, with one box per line part (empty: nothing
/// remains). A character belongs to the annotation if its centre lies in
/// one of its boxes.
pub fn erase(a: &AnnotInfo, t: &PageText, selected: &HashSet<usize>) -> Option<Vec<Vec<BBox>>> {
    let covered = |i: usize| {
        let b = t.chars[i].bbox;
        a.boxes()
            .iter()
            .any(|q| q.contains(b.center_x(), b.center_y()))
    };
    if !selected.iter().any(|&i| i < t.chars.len() && covered(i)) {
        return None;
    }
    let mut groups = Vec::new();
    // The group being built: (line, box) per line part.
    let mut current: Vec<(usize, BBox)> = Vec::new();
    // A selected character of the annotation since the last kept one.
    let mut cut = false;
    for (li, line) in t.lines.iter().enumerate() {
        // Text outside the annotation since the last kept character.
        let mut outside = false;
        for i in line.chars.clone() {
            let c = &t.chars[i];
            if !covered(i) {
                outside |= !c.c.is_whitespace();
                continue;
            }
            if selected.contains(&i) {
                cut = true;
                continue;
            }
            if c.c.is_whitespace() {
                continue;
            }
            if cut && !current.is_empty() {
                groups.push(current.drain(..).map(|(_, b)| b).collect());
            }
            cut = false;
            match current.last_mut() {
                Some((l, b)) if *l == li && !outside => *b = b.union(c.bbox),
                _ => current.push((li, c.bbox)),
            }
            outside = false;
        }
    }
    if !current.is_empty() {
        groups.push(current.into_iter().map(|(_, b)| b).collect());
    }
    Some(groups)
}

/// The topmost annotation at a point on a page (later ones are drawn on
/// top).
pub fn annot_at(annots: &[AnnotInfo], page: usize, x: f32, y: f32) -> Option<&AnnotInfo> {
    annots
        .iter()
        .rev()
        .find(|a| a.page == page && a.contains(x, y))
}

/// The category of an annotation, as an index into `categories` (name and
/// color each): the one named by its `/Subj` (ignoring case), else the one
/// with the nearest color. `None`: Uncategorised (no match and no color).
pub fn category_of(a: &AnnotInfo, categories: &[(&str, Option<[u8; 3]>)]) -> Option<usize> {
    if let Some(subj) = &a.subject {
        let subj = subj.trim();
        if let Some(i) = categories
            .iter()
            .position(|(name, _)| name.eq_ignore_ascii_case(subj))
        {
            return Some(i);
        }
    }
    let color = a.color?;
    let dist = |c: [u8; 3]| {
        (0..3)
            .map(|k| (i32::from(c[k]) - i32::from(color[k])).pow(2))
            .sum::<i32>()
    };
    categories
        .iter()
        .enumerate()
        .filter_map(|(i, (_, c))| c.map(|c| (dist(c), i)))
        .min()
        .map(|(_, i)| i)
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
    use crate::text::tests::page;

    fn info(page: usize, xref: i32, y: f32) -> AnnotInfo {
        AnnotInfo {
            xref,
            page,
            kind: "Highlight".into(),
            rect: BBox::new(10.0, y, 100.0, y + 10.0),
            quads: vec![],
            color: None,
            opacity: 1.0,
            subject: None,
            author: None,
            contents: None,
        }
    }

    /// A highlight over the given lines of `t`, from character `from` to
    /// `to` (inclusive), like marg creates them.
    fn highlight(t: &PageText, from: usize, to: usize) -> AnnotInfo {
        let mut a = info(0, 1, 0.0);
        a.quads = t
            .lines
            .iter()
            .filter_map(|l| {
                let s = l.chars.start.max(from);
                let e = l.chars.end.min(to + 1);
                (s < e).then(|| {
                    t.chars[s..e]
                        .iter()
                        .map(|c| c.bbox)
                        .reduce(BBox::union)
                        .unwrap()
                })
            })
            .collect();
        a
    }

    fn texts(t: &PageText, groups: &[Vec<BBox>]) -> Vec<Vec<String>> {
        groups
            .iter()
            .map(|g| {
                g.iter()
                    .map(|b| {
                        t.chars
                            .iter()
                            .filter(|c| b.contains(c.bbox.center_x(), c.bbox.center_y()))
                            .map(|c| c.c)
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    fn sel(r: std::ops::Range<usize>) -> HashSet<usize> {
        r.collect()
    }

    #[test]
    fn erase_middle_splits_in_two() {
        let t = page("one two three\nfour five");
        // "two three four"
        let a = highlight(&t, 4, 17);
        // Erase "three".
        let g = erase(&a, &t, &sel(8..13)).unwrap();
        assert_eq!(texts(&t, &g), [vec!["two"], vec!["four"]]);
    }

    #[test]
    fn erase_ends_and_whole() {
        let t = page("one two three");
        let a = highlight(&t, 0, 12);
        let g = erase(&a, &t, &sel(0..4)).unwrap();
        assert_eq!(texts(&t, &g), [vec!["two three"]]);
        let g = erase(&a, &t, &sel(8..13)).unwrap();
        assert_eq!(texts(&t, &g), [vec!["one two"]]);
        assert_eq!(erase(&a, &t, &sel(0..13)), Some(vec![]));
    }

    #[test]
    fn erase_outside_leaves_it_alone() {
        let t = page("one two three\nfour");
        let a = highlight(&t, 0, 2);
        assert_eq!(erase(&a, &t, &sel(4..17)), None);
    }

    #[test]
    fn erase_keeps_multi_line_rest_together() {
        let t = page("aa bb\ncc dd\nee ff");
        // All three lines; erase "aa" only.
        let a = highlight(&t, 0, 16);
        let g = erase(&a, &t, &sel(0..2)).unwrap();
        assert_eq!(texts(&t, &g), [vec!["bb", "cc dd", "ee ff"]]);
    }

    #[test]
    fn erase_respects_gaps_in_foreign_highlights() {
        // Two quads on one line, with unhighlighted text between them.
        let t = page("aaa bbb ccc ddd");
        let mut a = highlight(&t, 0, 2);
        a.quads.extend(highlight(&t, 8, 14).quads);
        // Erase "ddd": "aaa" and "ccc" stay as separate boxes.
        let g = erase(&a, &t, &sel(12..15)).unwrap();
        assert_eq!(texts(&t, &g), [vec!["aaa", "ccc"]]);
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

    #[test]
    fn category_from_subject_then_nearest_color() {
        let cats = [
            ("Key point", Some([0xFF, 0xD4, 0x00])),
            ("Question", Some([0xA2, 0x8A, 0xE5])),
            ("Broken", None),
        ];
        let mut a = info(0, 1, 0.0);
        assert_eq!(category_of(&a, &cats), None);
        a.color = Some([0x90, 0x80, 0xF0]);
        assert_eq!(category_of(&a, &cats), Some(1));
        a.color = Some([0xFF, 0xFF, 0x00]);
        assert_eq!(category_of(&a, &cats), Some(0));
        a.subject = Some(" question".into());
        assert_eq!(category_of(&a, &cats), Some(1));
        a.subject = Some("broken".into());
        assert_eq!(category_of(&a, &cats), Some(2));
        a.subject = Some("Unknown".into());
        assert_eq!(category_of(&a, &cats), Some(0));
    }
}
