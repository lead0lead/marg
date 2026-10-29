//! The table of contents and link targets.

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
