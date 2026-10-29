//! The table of contents, links and their targets.

use crate::text::BBox;

/// Where an outline entry or an internal link points: a page, and the
/// point to show at the top left of the window if the PDF names one (in
/// points from the page's top-left corner).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dest {
    pub page: usize,
    pub x: Option<f32>,
    pub y: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutlineEntry {
    pub title: String,
    /// 0 for top-level entries.
    pub level: usize,
    /// `None`: the entry points nowhere in this document.
    pub dest: Option<Dest>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LinkTarget {
    /// A place in this document.
    Page(Dest),
    /// A URI for another program (a web page, mail).
    Uri(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Where it is on its page, in points.
    pub rect: BBox,
    pub target: LinkTarget,
}

/// The topmost link at a point of a page (the last one listed).
pub fn link_at(links: &[Link], x: f32, y: f32) -> Option<&Link> {
    links.iter().rev().find(|l| l.rect.contains(x, y))
}

/// The entry for page `page`: the last one, in document order, pointing
/// at that page or before it.
pub fn entry_for_page(entries: &[OutlineEntry], page: usize) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (i, e) in entries.iter().enumerate() {
        if let Some(d) = e.dest
            && d.page <= page
            && best.is_none_or(|(p, _)| d.page >= p)
        {
            best = Some((d.page, i));
        }
    }
    best.map(|(_, i)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, level: usize, page: Option<usize>) -> OutlineEntry {
        OutlineEntry {
            title: title.into(),
            level,
            dest: page.map(|page| Dest {
                page,
                x: None,
                y: None,
            }),
        }
    }

    #[test]
    fn finds_the_link_at_a_point() {
        let to = |page| {
            LinkTarget::Page(Dest {
                page,
                x: None,
                y: None,
            })
        };
        let links = [
            Link {
                rect: BBox::new(0.0, 0.0, 100.0, 20.0),
                target: to(1),
            },
            Link {
                rect: BBox::new(50.0, 0.0, 150.0, 20.0),
                target: to(2),
            },
        ];
        assert_eq!(link_at(&links, 10.0, 10.0).unwrap().target, to(1));
        assert_eq!(link_at(&links, 60.0, 10.0).unwrap().target, to(2));
        assert!(link_at(&links, 60.0, 30.0).is_none());
    }

    #[test]
    fn finds_the_entry_of_a_page() {
        let toc = [
            entry("Preface", 0, Some(2)),
            entry("1 Intro", 0, Some(5)),
            entry("1.1 Aims", 1, Some(5)),
            entry("Elsewhere", 1, None),
            entry("2 Method", 0, Some(9)),
        ];
        assert_eq!(entry_for_page(&toc, 0), None);
        assert_eq!(entry_for_page(&toc, 3), Some(0));
        // Several entries on one page: the last (deepest) one.
        assert_eq!(entry_for_page(&toc, 7), Some(2));
        assert_eq!(entry_for_page(&toc, 100), Some(4));
    }
}
