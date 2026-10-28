//! The text of a page: blocks → lines → characters with boxes, in reading
//! order as the PDF engine reports it. Coordinates are PDF points from the
//! page's top-left corner, y down.

use std::ops::Range;

/// An axis-aligned box in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl BBox {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn union(self, o: BBox) -> BBox {
        BBox::new(
            self.x0.min(o.x0),
            self.y0.min(o.y0),
            self.x1.max(o.x1),
            self.y1.max(o.y1),
        )
    }

    pub fn center_x(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }

    pub fn center_y(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Char {
    pub c: char,
    pub bbox: BBox,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Its characters: `PageText::chars[chars]`. Never empty.
    pub chars: Range<usize>,
    /// Index into `PageText::blocks`.
    pub block: usize,
    pub bbox: BBox,
}

/// Append a line's text to `out`: after a space, or, when `out` ends in a
/// letter and `-` and `part` starts in lowercase, without the hyphen (a
/// word broken across lines). Empty parts are skipped.
pub fn join_part(out: &mut String, part: &str) {
    if part.is_empty() {
        return;
    }
    let joins = out.ends_with('-')
        && part.chars().next().is_some_and(|c| c.is_lowercase())
        && out.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic());
    if joins {
        out.pop();
    } else if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(part);
}

/// One page's text, flattened: characters in reading order, with line and
/// block boundaries as ranges.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageText {
    pub chars: Vec<Char>,
    pub lines: Vec<Line>,
    /// Each block's lines: `lines[blocks[b]]`. Never empty.
    pub blocks: Vec<Range<usize>>,
}

impl PageText {
    /// Build from blocks of lines of characters. Lines that contain only
    /// whitespace are dropped (real PDFs have them), and so are blocks left
    /// empty.
    pub fn from_blocks(blocks: Vec<Vec<Vec<Char>>>) -> Self {
        let mut t = PageText::default();
        for block in blocks {
            let first_line = t.lines.len();
            for line in block {
                if line.iter().all(|c| c.c.is_whitespace()) {
                    continue;
                }
                let start = t.chars.len();
                let bbox = line
                    .iter()
                    .filter(|c| !c.c.is_whitespace())
                    .map(|c| c.bbox)
                    .reduce(BBox::union)
                    .unwrap_or_default();
                t.chars.extend(line);
                t.lines.push(Line {
                    chars: start..t.chars.len(),
                    block: t.blocks.len(),
                    bbox,
                });
            }
            if t.lines.len() > first_line {
                t.blocks.push(first_line..t.lines.len());
            }
        }
        t
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// The line containing character `i`.
    pub fn line_of(&self, i: usize) -> usize {
        self.lines
            .partition_point(|l| l.chars.end <= i)
            .min(self.lines.len().saturating_sub(1))
    }

    /// The words of line `l`: maximal runs of non-whitespace characters, as
    /// character ranges.
    pub fn words(&self, l: usize) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut start = None;
        let r = self.lines[l].chars.clone();
        for i in r.clone() {
            match (self.chars[i].c.is_whitespace(), start) {
                (false, None) => start = Some(i),
                (true, Some(s)) => {
                    out.push(s..i);
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            out.push(s..r.end);
        }
        out
    }

    /// The characters of `range` as a string.
    pub fn string(&self, range: Range<usize>) -> String {
        self.chars[range].iter().map(|c| c.c).collect()
    }

    /// The text of the characters whose centre lies in one of `boxes`
    /// (what a highlight covers), with leading and trailing spaces left
    /// out, its line parts joined as `join_part` does.
    pub fn covered(&self, boxes: &[BBox]) -> String {
        let mut out = String::new();
        for line in &self.lines {
            let part: String = self.chars[line.chars.clone()]
                .iter()
                .filter(|c| {
                    boxes
                        .iter()
                        .any(|b| b.contains(c.bbox.center_x(), c.bbox.center_y()))
                })
                .map(|c| c.c)
                .collect();
            join_part(&mut out, part.trim());
        }
        out
    }

    /// The character nearest to point `(x, y)`: on the line closest
    /// vertically (preferring lines that contain `y`), the character closest
    /// horizontally. `None` if the page has no text.
    pub fn char_at(&self, x: f32, y: f32) -> Option<usize> {
        let dist_y = |b: &BBox| {
            if y < b.y0 {
                b.y0 - y
            } else if y > b.y1 {
                y - b.y1
            } else {
                0.0
            }
        };
        let dist_x = |b: &BBox| {
            if x < b.x0 {
                b.x0 - x
            } else if x > b.x1 {
                x - b.x1
            } else {
                0.0
            }
        };
        let line = self.lines.iter().min_by(|a, b| {
            (dist_y(&a.bbox), dist_x(&a.bbox))
                .partial_cmp(&(dist_y(&b.bbox), dist_x(&b.bbox)))
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
        line.chars
            .clone()
            .min_by(|&a, &b| dist_x(&self.chars[a].bbox).total_cmp(&dist_x(&self.chars[b].bbox)))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A page from lines of text, one block per `\n\n`-separated group;
    /// characters are 6 pt wide, lines 12 pt apart.
    pub fn page(text: &str) -> PageText {
        let mut y = 0.0;
        let blocks = text
            .split("\n\n")
            .map(|block| {
                block
                    .lines()
                    .map(|line| {
                        let chars = line
                            .chars()
                            .enumerate()
                            .map(|(i, c)| Char {
                                c,
                                bbox: BBox::new(i as f32 * 6.0, y, i as f32 * 6.0 + 6.0, y + 10.0),
                            })
                            .collect();
                        y += 12.0;
                        chars
                    })
                    .collect()
            })
            .collect();
        PageText::from_blocks(blocks)
    }

    #[test]
    fn whitespace_lines_and_empty_blocks_are_dropped() {
        let t = page("one two\n  \nthree\n\n \n\nfour");
        assert_eq!(t.lines.len(), 3);
        assert_eq!(t.blocks, vec![0..2, 2..3]);
        assert_eq!(t.string(t.lines[1].chars.clone()), "three");
        assert_eq!(t.lines[2].block, 1);
    }

    #[test]
    fn words_split_on_whitespace() {
        let t = page(" one  two three ");
        let w: Vec<String> = t.words(0).into_iter().map(|r| t.string(r)).collect();
        assert_eq!(w, ["one", "two", "three"]);
    }

    #[test]
    fn line_of_and_char_at() {
        let t = page("abc\ndef");
        assert_eq!(t.line_of(0), 0);
        assert_eq!(t.line_of(2), 0);
        assert_eq!(t.line_of(3), 1);
        // Inside 'e' on the second line.
        assert_eq!(t.char_at(8.0, 15.0), Some(4));
        // Far right of the first line: its last character.
        assert_eq!(t.char_at(100.0, 2.0), Some(2));
        // Below everything: the last line.
        assert_eq!(t.char_at(0.0, 500.0), Some(3));
        assert_eq!(PageText::default().char_at(0.0, 0.0), None);
    }
}
