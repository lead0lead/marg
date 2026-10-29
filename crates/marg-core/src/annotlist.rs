//! The annotation list (§2.10): one row per annotation, in reading order,
//! filtered by text and category.

use crate::annot::{AnnotInfo, category_of};
use crate::fuzzy;
use crate::text::PageText;

/// One row of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub page: usize,
    pub xref: i32,
    /// Index into the categories; `None`: Uncategorised.
    pub category: Option<usize>,
    pub color: Option<[u8; 3]>,
    /// `text` is marked text (a highlight or similar), not a placeholder.
    pub marked: bool,
    /// The marked text (empty while the page's text is loading), or a
    /// placeholder such as "[Box]" for annotations without text.
    pub text: String,
    pub comment: String,
}

/// The rows for `annots` (any order) in reading order: page, top, left.
/// `categories`: name and color of each.
pub fn entries(
    annots: &[AnnotInfo],
    texts: &[Option<PageText>],
    categories: &[(&str, Option<[u8; 3]>)],
) -> Vec<Entry> {
    let mut sorted: Vec<&AnnotInfo> = annots.iter().collect();
    sorted.sort_by(|a, b| {
        (a.page, a.rect.y0, a.rect.x0)
            .partial_cmp(&(b.page, b.rect.y0, b.rect.x0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted
        .into_iter()
        .map(|a| {
            let marked = matches!(
                a.kind.as_str(),
                "Highlight" | "Underline" | "Squiggly" | "StrikeOut"
            );
            let text = match a.kind.as_str() {
                _ if marked => texts
                    .get(a.page)
                    .and_then(|t| t.as_ref())
                    .map(|t| t.covered(a.boxes()))
                    .unwrap_or_default(),
                "Square" => "[Box]".into(),
                "Text" => "[Note]".into(),
                k => format!("[{k}]"),
            };
            Entry {
                page: a.page,
                xref: a.xref,
                category: category_of(a, categories),
                color: a.color,
                marked,
                text,
                comment: a.contents.clone().unwrap_or_default(),
            }
        })
        .collect()
}

/// Category filter of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryFilter {
    All,
    /// Index into the categories.
    Only(usize),
}

/// Indices of the rows that match `query` (fuzzy, over text and comment;
/// best first, reading order when empty) and the category filter.
pub fn filter(entries: &[Entry], query: &str, category: CategoryFilter) -> Vec<usize> {
    let keyed: Vec<(usize, String)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| match category {
            CategoryFilter::All => true,
            CategoryFilter::Only(c) => e.category == Some(c),
        })
        .map(|(i, e)| (i, format!("{} {}", e.text, e.comment)))
        .collect();
    let haystack: Vec<&str> = keyed.iter().map(|(_, s)| s.as_str()).collect();
    fuzzy::filter(query, &haystack)
        .into_iter()
        .map(|k| keyed[k].0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::BBox;
    use crate::text::tests::page;

    fn annot(page: usize, xref: i32, kind: &str, rect: BBox) -> AnnotInfo {
        AnnotInfo {
            xref,
            page,
            kind: kind.into(),
            rect,
            quads: vec![],
            color: Some([0xFF, 0xD4, 0x00]),
            opacity: 1.0,
            subject: None,
            author: None,
            contents: None,
        }
    }

    const CATS: [(&str, Option<[u8; 3]>); 2] = [
        ("Key point", Some([0xFF, 0xD4, 0x00])),
        ("Question", Some([0xA2, 0x8A, 0xE5])),
    ];

    #[test]
    fn rows_in_reading_order_with_marked_text() {
        // Lines are 12 pt apart, characters 6 pt wide.
        let t = page("one two\nthree four");
        let mut hl = annot(0, 1, "Highlight", BBox::new(24.0, 0.0, 42.0, 10.0));
        hl.quads = vec![
            BBox::new(24.0, 0.0, 42.0, 10.0),
            BBox::new(0.0, 12.0, 30.0, 22.0),
        ];
        let mut boxed = annot(0, 2, "Square", BBox::new(0.0, 0.0, 10.0, 10.0));
        boxed.contents = Some("see this".into());
        boxed.subject = Some("Question".into());
        let note = annot(1, 3, "Text", BBox::new(0.0, 0.0, 20.0, 20.0));
        let unloaded = annot(1, 4, "Underline", BBox::new(0.0, 30.0, 20.0, 40.0));
        let rows = entries(&[note, unloaded, hl, boxed], &[Some(t), None], &CATS);
        let summary: Vec<_> = rows
            .iter()
            .map(|r| (r.xref, r.text.as_str(), r.category))
            .collect();
        assert_eq!(
            summary,
            [
                (2, "[Box]", Some(1)),
                (1, "two three", Some(0)),
                (3, "[Note]", Some(0)),
                (4, "", Some(0)),
            ]
        );
        assert_eq!(rows[0].comment, "see this");
    }

    #[test]
    fn filter_by_text_comment_and_category() {
        let row = |text: &str, comment: &str, category| Entry {
            page: 0,
            xref: 0,
            category,
            color: None,
            marked: true,
            text: text.into(),
            comment: comment.into(),
        };
        let rows = [
            row("graph theory", "", Some(0)),
            row("[Box]", "a graph", Some(1)),
            row("other", "", None),
        ];
        assert_eq!(filter(&rows, "", CategoryFilter::All), [0, 1, 2]);
        assert_eq!(filter(&rows, "graph", CategoryFilter::All), [0, 1]);
        assert_eq!(filter(&rows, "graph", CategoryFilter::Only(1)), [1]);
        assert_eq!(filter(&rows, "", CategoryFilter::Only(0)), [0]);
    }
}
