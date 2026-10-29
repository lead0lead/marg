//! Text search on a page's text (`/`).
//!
//! Smartcase: case is ignored unless the query has an uppercase letter.
//! Runs of whitespace match any whitespace, line and block breaks count as
//! a space, a word broken across lines with a hyphen is joined (as in
//! copied text, `text::join_part`), and ligatures match their letters.

use std::ops::Range;

use crate::text::{BBox, PageText};

/// One hit: a page and the boxes of its characters, one per line part.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub page: usize,
    pub rects: Vec<BBox>,
}

impl Hit {
    /// The top left of its first box.
    pub fn top(&self) -> (f32, f32) {
        self.rects.first().map_or((0.0, 0.0), |r| (r.x0, r.y0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    needle: Vec<char>,
    case_sensitive: bool,
}

impl Query {
    /// `None` for a query of only whitespace.
    pub fn new(query: &str) -> Option<Self> {
        let case_sensitive = query.chars().any(char::is_uppercase);
        let mut needle = Vec::new();
        for c in query.trim().chars() {
            push_folded(&mut needle, c, case_sensitive);
        }
        collapse_spaces(&mut needle);
        (!needle.is_empty()).then_some(Self {
            needle,
            case_sensitive,
        })
    }

    /// Character ranges of the non-overlapping matches on a page.
    pub fn find(&self, text: &PageText) -> Vec<Range<usize>> {
        // The searchable stream: folded characters, each with the index of
        // the page character it came from.
        let mut hay: Vec<(char, usize)> = Vec::with_capacity(text.chars.len() + text.lines.len());
        let mut folded = Vec::new();
        for line in &text.lines {
            let first = text.chars[line.chars.start].c;
            let hyphen = hay.len() >= 2
                && hay[hay.len() - 1].0 == '-'
                && hay[hay.len() - 2].0.is_alphabetic()
                && first.is_lowercase();
            if hyphen {
                hay.pop();
            } else if hay.last().is_some_and(|&(c, _)| c != ' ') {
                hay.push((' ', line.chars.start));
            }
            for i in line.chars.clone() {
                let c = text.chars[i].c;
                if c.is_whitespace() {
                    if hay.last().is_some_and(|&(c, _)| c != ' ') {
                        hay.push((' ', i));
                    }
                    continue;
                }
                folded.clear();
                push_folded(&mut folded, c, self.case_sensitive);
                hay.extend(folded.iter().map(|&f| (f, i)));
            }
        }
        let n = self.needle.len();
        let mut out = Vec::new();
        let mut at = 0;
        while at + n <= hay.len() {
            if hay[at..at + n]
                .iter()
                .zip(&self.needle)
                .all(|(&(h, _), &q)| h == q)
            {
                out.push(hay[at].1..hay[at + n - 1].1 + 1);
                at += n;
            } else {
                at += 1;
            }
        }
        out
    }

    /// The hits on a page, with their boxes.
    pub fn hits(&self, page: usize, text: &PageText) -> Vec<Hit> {
        self.find(text)
            .into_iter()
            .map(|r| Hit {
                page,
                rects: rects(text, r),
            })
            .collect()
    }
}

/// One box per line part of a character range (whitespace left out).
pub fn rects(text: &PageText, range: Range<usize>) -> Vec<BBox> {
    let mut out: Vec<(usize, BBox)> = Vec::new();
    for i in range {
        let ch = &text.chars[i];
        if ch.c.is_whitespace() {
            continue;
        }
        let line = text.line_of(i);
        match out.last_mut() {
            Some((l, b)) if *l == line => *b = b.union(ch.bbox),
            _ => out.push((line, ch.bbox)),
        }
    }
    out.into_iter().map(|(_, b)| b).collect()
}

fn push_folded(out: &mut Vec<char>, c: char, case_sensitive: bool) {
    let plain = match c {
        'ﬀ' => "ff",
        'ﬁ' => "fi",
        'ﬂ' => "fl",
        'ﬃ' => "ffi",
        'ﬄ' => "ffl",
        'ﬅ' | 'ﬆ' => "st",
        // Soft hyphen, zero-width space.
        '\u{ad}' | '\u{200b}' => "",
        _ => {
            if c.is_whitespace() {
                out.push(' ');
            } else if case_sensitive {
                out.push(c);
            } else {
                out.extend(c.to_lowercase());
            }
            return;
        }
    };
    out.extend(plain.chars());
}

fn collapse_spaces(v: &mut Vec<char>) {
    v.dedup_by(|a, b| *a == ' ' && *b == ' ');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::Char;

    /// A page from lines of text, one block per `|`-separated group;
    /// every character 10 pt wide, lines 20 pt apart.
    fn page(blocks: &[&[&str]]) -> PageText {
        let mut y = 0.0;
        let blocks = blocks
            .iter()
            .map(|lines| {
                lines
                    .iter()
                    .map(|l| {
                        y += 20.0;
                        l.chars()
                            .enumerate()
                            .map(|(i, c)| Char {
                                c,
                                bbox: BBox::new(
                                    i as f32 * 10.0,
                                    y,
                                    i as f32 * 10.0 + 10.0,
                                    y + 12.0,
                                ),
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        PageText::from_blocks(blocks)
    }

    fn found(text: &PageText, q: &str) -> Vec<String> {
        Query::new(q)
            .unwrap()
            .find(text)
            .into_iter()
            .map(|r| text.chars[r].iter().map(|c| c.c).collect())
            .collect()
    }

    #[test]
    fn smartcase() {
        let t = page(&[&["The theory, the THEME"]]);
        assert_eq!(found(&t, "the").len(), 4);
        assert_eq!(found(&t, "The"), ["The"]);
        assert_eq!(found(&t, "THE"), ["THE"]);
    }

    #[test]
    fn across_lines_and_hyphens() {
        let t = page(&[
            &["a reading or-", "der test with well-", "Known words"],
            &["next block"],
        ]);
        // The hit covers both parts, hyphen included.
        assert_eq!(found(&t, "order"), ["or-der"]);
        assert_eq!(found(&t, "test  with"), ["test with"]);
        // A capital after the hyphen: not a broken word.
        assert_eq!(found(&t, "well- known").len(), 1);
        assert_eq!(found(&t, "words next"), ["wordsnext"]);
    }

    #[test]
    fn ligatures_match_their_letters() {
        let t = page(&[&["e\u{fb03}cient \u{fb01}nd"]]);
        assert_eq!(found(&t, "efficient").len(), 1);
        assert_eq!(found(&t, "find").len(), 1);
    }

    #[test]
    fn matches_do_not_overlap_and_blank_queries_are_none() {
        let t = page(&[&["aaaa"]]);
        assert_eq!(found(&t, "aa").len(), 2);
        assert!(Query::new("   ").is_none());
    }

    #[test]
    fn hit_boxes_per_line() {
        let t = page(&[&["one two", "three"]]);
        let hits = Query::new("two three").unwrap().hits(4, &t);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page, 4);
        assert_eq!(
            hits[0].rects,
            [
                BBox::new(40.0, 20.0, 70.0, 32.0),
                BBox::new(0.0, 40.0, 50.0, 52.0)
            ]
        );
    }
}
