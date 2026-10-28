//! Caret motions and selections over the text of a document.
//!
//! A caret sits on a character. Motions follow the reading order of the
//! text model across lines, blocks and pages. Words are runs of
//! non-whitespace characters within a line (vim's `W`), so a word includes
//! its punctuation. Pages whose text is not loaded yet end the document for
//! motions; pages without text are skipped.

use std::ops::Range;

use crate::text::{BBox, PageText};

/// A character position: page index and character index in that page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextPos {
    pub page: usize,
    pub idx: usize,
}

impl TextPos {
    pub fn new(page: usize, idx: usize) -> Self {
        Self { page, idx }
    }
}

/// Page texts by page index; `None` = not loaded.
pub trait TextSource {
    fn text(&self, page: usize) -> Option<&PageText>;
    fn page_count(&self) -> usize;
}

impl TextSource for [Option<PageText>] {
    fn text(&self, page: usize) -> Option<&PageText> {
        self.get(page).and_then(|t| t.as_ref())
    }

    fn page_count(&self) -> usize {
        self.len()
    }
}

impl TextSource for Vec<Option<PageText>> {
    fn text(&self, page: usize) -> Option<&PageText> {
        self.as_slice().text(page)
    }

    fn page_count(&self) -> usize {
        self.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    CharNext,
    CharPrev,
    WordNext,
    WordPrev,
    WordEnd,
    WordEndPrev,
    LineStart,
    LineEnd,
    LineDown,
    LineUp,
    SentencePrev,
    SentenceNext,
    BlockPrev,
    BlockNext,
}

/// Characters after a sentence end that still belong to it.
const CLOSERS: &[char] = &['"', '\'', ')', ']', '”', '’', '»'];

fn ch(src: &(impl TextSource + ?Sized), p: TextPos) -> char {
    src.text(p.page).map_or(' ', |t| t.chars[p.idx].c)
}

fn is_space(src: &(impl TextSource + ?Sized), p: TextPos) -> bool {
    ch(src, p).is_whitespace()
}

/// The first page after `page` that has text, if every page in between is
/// loaded.
fn next_page(src: &(impl TextSource + ?Sized), page: usize) -> Option<usize> {
    (page + 1..src.page_count())
        .map(|p| (p, src.text(p)))
        .take_while(|(_, t)| t.is_some())
        .find(|(_, t)| t.is_some_and(|t| !t.is_empty()))
        .map(|(p, _)| p)
}

fn prev_page(src: &(impl TextSource + ?Sized), page: usize) -> Option<usize> {
    (0..page)
        .rev()
        .map(|p| (p, src.text(p)))
        .take_while(|(_, t)| t.is_some())
        .find(|(_, t)| t.is_some_and(|t| !t.is_empty()))
        .map(|(p, _)| p)
}

/// The next character in reading order.
pub fn next(src: &(impl TextSource + ?Sized), p: TextPos) -> Option<TextPos> {
    let t = src.text(p.page)?;
    if p.idx + 1 < t.chars.len() {
        return Some(TextPos::new(p.page, p.idx + 1));
    }
    next_page(src, p.page).map(|n| TextPos::new(n, 0))
}

/// The previous character in reading order.
pub fn prev(src: &(impl TextSource + ?Sized), p: TextPos) -> Option<TextPos> {
    if p.idx > 0 {
        return Some(TextPos::new(p.page, p.idx - 1));
    }
    let n = prev_page(src, p.page)?;
    Some(TextPos::new(n, src.text(n)?.chars.len() - 1))
}

fn line(src: &(impl TextSource + ?Sized), p: TextPos) -> Range<usize> {
    src.text(p.page).map_or(p.idx..p.idx + 1, |t| {
        t.lines[t.line_of(p.idx)].chars.clone()
    })
}

fn block_of(src: &(impl TextSource + ?Sized), p: TextPos) -> usize {
    src.text(p.page)
        .map_or(0, |t| t.lines[t.line_of(p.idx)].block)
}

pub fn is_word_start(src: &(impl TextSource + ?Sized), p: TextPos) -> bool {
    !is_space(src, p)
        && (p.idx == line(src, p).start || is_space(src, TextPos::new(p.page, p.idx - 1)))
}

pub fn is_word_end(src: &(impl TextSource + ?Sized), p: TextPos) -> bool {
    !is_space(src, p)
        && (p.idx + 1 == line(src, p).end || is_space(src, TextPos::new(p.page, p.idx + 1)))
}

/// A word start that begins a sentence: the first word of a block, or one
/// whose previous non-space character in the block ends a sentence.
fn is_sentence_start(src: &(impl TextSource + ?Sized), p: TextPos) -> bool {
    if !is_word_start(src, p) {
        return false;
    }
    let block = block_of(src, p);
    let mut q = p;
    let mut closers = 0;
    loop {
        match prev(src, q) {
            Some(r) if r.page == p.page && block_of(src, r) == block => q = r,
            _ => return closers == 0,
        }
        let c = ch(src, q);
        if c.is_whitespace() {
            continue;
        }
        if CLOSERS.contains(&c) && closers < 2 {
            closers += 1;
            continue;
        }
        return matches!(c, '.' | '?' | '!');
    }
}

fn find_next(
    src: &(impl TextSource + ?Sized),
    p: TextPos,
    pred: impl Fn(TextPos) -> bool,
) -> Option<TextPos> {
    let mut q = next(src, p)?;
    loop {
        if pred(q) {
            return Some(q);
        }
        q = next(src, q)?;
    }
}

fn find_prev(
    src: &(impl TextSource + ?Sized),
    p: TextPos,
    pred: impl Fn(TextPos) -> bool,
) -> Option<TextPos> {
    let mut q = prev(src, p)?;
    loop {
        if pred(q) {
            return Some(q);
        }
        q = prev(src, q)?;
    }
}

/// The line after (or before) the one containing `p`, as (page, line).
fn adjacent_line(
    src: &(impl TextSource + ?Sized),
    p: TextPos,
    down: bool,
) -> Option<(usize, usize)> {
    let t = src.text(p.page)?;
    let l = t.line_of(p.idx);
    if down {
        if l + 1 < t.lines.len() {
            return Some((p.page, l + 1));
        }
        next_page(src, p.page).map(|n| (n, 0))
    } else {
        if l > 0 {
            return Some((p.page, l - 1));
        }
        let n = prev_page(src, p.page)?;
        Some((n, src.text(n)?.lines.len() - 1))
    }
}

/// The character of a line closest to `x`, preferring non-space ones.
fn char_near_x(t: &PageText, line: usize, x: f32) -> usize {
    t.lines[line]
        .chars
        .clone()
        .min_by(|&a, &b| {
            let key = |i: usize| {
                let c = &t.chars[i];
                (c.c.is_whitespace(), (c.bbox.center_x() - x).abs())
            };
            let (sa, da) = key(a);
            let (sb, db) = key(b);
            sa.cmp(&sb).then(da.total_cmp(&db))
        })
        .expect("lines are not empty")
}

fn block_start(src: &(impl TextSource + ?Sized), page: usize, block: usize) -> TextPos {
    let t = src.text(page).expect("loaded");
    TextPos::new(page, t.lines[t.blocks[block].start].chars.start)
}

/// The caret after a motion. `want_x` is the horizontal position (points)
/// that `j`/`k` keep; other motions reset it. Returns the new position and
/// `want_x`. A motion that cannot move leaves the caret where it is.
pub fn apply(
    src: &(impl TextSource + ?Sized),
    p: TextPos,
    motion: Motion,
    want_x: Option<f32>,
) -> (TextPos, Option<f32>) {
    let Some(t) = src.text(p.page) else {
        return (p, want_x);
    };
    let moved = match motion {
        Motion::CharNext => next(src, p),
        Motion::CharPrev => prev(src, p),
        Motion::WordNext => find_next(src, p, |q| is_word_start(src, q)),
        Motion::WordPrev => find_prev(src, p, |q| is_word_start(src, q)),
        Motion::WordEnd => find_next(src, p, |q| is_word_end(src, q)),
        Motion::WordEndPrev => find_prev(src, p, |q| is_word_end(src, q)),
        Motion::LineStart => Some(TextPos::new(p.page, line(src, p).start)),
        Motion::LineEnd => {
            let r = line(src, p);
            let last = r.clone().rev().find(|&i| !t.chars[i].c.is_whitespace());
            Some(TextPos::new(p.page, last.unwrap_or(r.end - 1)))
        }
        Motion::LineDown | Motion::LineUp => {
            let x = want_x.unwrap_or_else(|| t.chars[p.idx].bbox.center_x());
            return match adjacent_line(src, p, motion == Motion::LineDown) {
                Some((page, l)) => {
                    let t = src.text(page).expect("loaded");
                    (TextPos::new(page, char_near_x(t, l, x)), Some(x))
                }
                None => (p, Some(x)),
            };
        }
        Motion::SentenceNext => find_next(src, p, |q| is_sentence_start(src, q)),
        Motion::SentencePrev => find_prev(src, p, |q| is_sentence_start(src, q)),
        Motion::BlockNext => {
            let b = block_of(src, p);
            if b + 1 < t.blocks.len() {
                Some(block_start(src, p.page, b + 1))
            } else {
                next_page(src, p.page).map(|n| block_start(src, n, 0))
            }
        }
        Motion::BlockPrev => {
            let b = block_of(src, p);
            let start = block_start(src, p.page, b);
            if p != start {
                Some(start)
            } else if b > 0 {
                Some(block_start(src, p.page, b - 1))
            } else {
                prev_page(src, p.page).map(|n| {
                    let blocks = src.text(n).expect("loaded").blocks.len();
                    block_start(src, n, blocks - 1)
                })
            }
        }
    };
    (moved.unwrap_or(p), None)
}

/// Apply a motion `count` times.
pub fn apply_n(
    src: &(impl TextSource + ?Sized),
    mut p: TextPos,
    motion: Motion,
    mut want_x: Option<f32>,
    count: u32,
) -> (TextPos, Option<f32>) {
    for _ in 0..count.max(1) {
        (p, want_x) = apply(src, p, motion, want_x);
    }
    (p, want_x)
}

/// A selection between two caret positions, both included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The fixed end.
    pub anchor: TextPos,
    /// The end that motions move.
    pub head: TextPos,
    /// Whole lines (`V`).
    pub linewise: bool,
}

impl Selection {
    pub fn new(anchor: TextPos, head: TextPos) -> Self {
        Self {
            anchor,
            head,
            linewise: false,
        }
    }

    /// Swap the moving end (`o`).
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.anchor, &mut self.head);
    }

    /// First and last selected character, in reading order.
    pub fn range(&self, src: &(impl TextSource + ?Sized)) -> (TextPos, TextPos) {
        let (a, b) = if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        if !self.linewise {
            return (a, b);
        }
        (
            TextPos::new(a.page, line(src, a).start),
            TextPos::new(b.page, line(src, b).end - 1),
        )
    }

    /// The selected part of each line, in reading order, without leading
    /// and trailing whitespace: (page, line index, character range).
    pub fn line_parts(
        &self,
        src: &(impl TextSource + ?Sized),
    ) -> Vec<(usize, usize, Range<usize>)> {
        let (a, b) = self.range(src);
        let mut out = Vec::new();
        for page in a.page..=b.page {
            let Some(t) = src.text(page) else { continue };
            for (li, l) in t.lines.iter().enumerate() {
                let start = if page == a.page {
                    l.chars.start.max(a.idx)
                } else {
                    l.chars.start
                };
                let end = if page == b.page {
                    l.chars.end.min(b.idx + 1)
                } else {
                    l.chars.end
                };
                let mut r = start..end;
                while r.start < r.end && t.chars[r.start].c.is_whitespace() {
                    r.start += 1;
                }
                while r.end > r.start && t.chars[r.end - 1].c.is_whitespace() {
                    r.end -= 1;
                }
                if !r.is_empty() {
                    out.push((page, li, r));
                }
            }
        }
        out
    }

    /// One box per selected line part: (page, box in points).
    pub fn rects(&self, src: &(impl TextSource + ?Sized)) -> Vec<(usize, BBox)> {
        self.line_parts(src)
            .into_iter()
            .filter_map(|(page, _, r)| {
                let t = src.text(page)?;
                t.chars[r]
                    .iter()
                    .map(|c| c.bbox)
                    .reduce(BBox::union)
                    .map(|b| (page, b))
            })
            .collect()
    }

    /// The selected text: lines joined with spaces; a hyphen at a line end
    /// followed by a lowercase letter is removed and the word joined.
    pub fn text(&self, src: &(impl TextSource + ?Sized)) -> String {
        let mut out = String::new();
        for (page, _, r) in self.line_parts(src) {
            let Some(t) = src.text(page) else { continue };
            let part = t.string(r);
            let joins = out.ends_with('-')
                && part.chars().next().is_some_and(|c| c.is_lowercase())
                && out.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic());
            if joins {
                out.pop();
            } else if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&part);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::tests::page;

    fn doc(pages: &[&str]) -> Vec<Option<PageText>> {
        pages.iter().map(|p| Some(page(p))).collect()
    }

    /// Position of the `n`th occurrence of `needle` on page `p`.
    fn at(d: &[Option<PageText>], p: usize, needle: &str, n: usize) -> TextPos {
        let t = d[p].as_ref().unwrap();
        let s: String = t.chars.iter().map(|c| c.c).collect();
        let byte = s.match_indices(needle).nth(n).unwrap().0;
        TextPos::new(p, s[..byte].chars().count())
    }

    fn word_at(d: &[Option<PageText>], p: TextPos) -> String {
        let t = d[p.page].as_ref().unwrap();
        let r = line(d, p);
        let mut e = p.idx;
        while e < r.end && !t.chars[e].c.is_whitespace() {
            e += 1;
        }
        t.string(p.idx..e)
    }

    fn mv(d: &[Option<PageText>], p: TextPos, m: Motion) -> TextPos {
        apply(d, p, m, None).0
    }

    #[test]
    fn word_motions_cross_lines_and_pages() {
        let d = doc(&["one two,\nthree", "four"]);
        let mut p = TextPos::new(0, 0);
        let mut seen = vec![];
        for _ in 0..4 {
            p = mv(&d, p, Motion::WordNext);
            seen.push(word_at(&d, p));
        }
        assert_eq!(seen, ["two,", "three", "four", "four"]);
        p = mv(&d, p, Motion::WordPrev);
        assert_eq!(word_at(&d, p), "three");
        let end = mv(&d, TextPos::new(0, 0), Motion::WordEnd);
        assert_eq!(end, at(&d, 0, "e", 0));
        let end = mv(&d, end, Motion::WordEnd);
        assert_eq!(end, at(&d, 0, ",", 0));
        assert_eq!(
            mv(&d, TextPos::new(1, 0), Motion::WordEndPrev),
            at(&d, 0, "e", 2)
        );
    }

    #[test]
    fn char_motions() {
        let d = doc(&["ab", "c"]);
        assert_eq!(
            mv(&d, TextPos::new(0, 1), Motion::CharNext),
            TextPos::new(1, 0)
        );
        assert_eq!(
            mv(&d, TextPos::new(1, 0), Motion::CharPrev),
            TextPos::new(0, 1)
        );
        assert_eq!(
            mv(&d, TextPos::new(0, 0), Motion::CharPrev),
            TextPos::new(0, 0)
        );
    }

    #[test]
    fn counts_repeat() {
        let d = doc(&["a b c d e"]);
        let (p, _) = apply_n(&d, TextPos::new(0, 0), Motion::WordNext, None, 3);
        assert_eq!(word_at(&d, p), "d");
    }

    #[test]
    fn line_start_end_and_vertical_keeps_x() {
        let d = doc(&["abcdefgh\nab\nabcdefgh"]);
        let p = at(&d, 0, "f", 0);
        assert_eq!(mv(&d, p, Motion::LineStart), TextPos::new(0, 0));
        assert_eq!(mv(&d, p, Motion::LineEnd), TextPos::new(0, 7));
        let (p1, x) = apply(&d, p, Motion::LineDown, None);
        assert_eq!(p1, at(&d, 0, "b", 1));
        let (p2, _) = apply(&d, p1, Motion::LineDown, x);
        assert_eq!(p2, at(&d, 0, "f", 1));
    }

    #[test]
    fn missing_page_text_stops_motions() {
        let mut d = doc(&["one", "two"]);
        d[1] = None;
        assert_eq!(
            mv(&d, TextPos::new(0, 0), Motion::WordNext),
            TextPos::new(0, 0)
        );
        // An empty page is skipped.
        let d = vec![
            Some(page("one")),
            Some(PageText::default()),
            Some(page("three")),
        ];
        assert_eq!(
            mv(&d, TextPos::new(0, 0), Motion::WordNext),
            TextPos::new(2, 0)
        );
    }

    #[test]
    fn sentences() {
        let d = doc(&["Hi there. Next one\nruns on? \"Quoted.\" After\n\nNew block"]);
        let mut p = TextPos::new(0, 0);
        let mut seen = vec![];
        for _ in 0..4 {
            p = mv(&d, p, Motion::SentenceNext);
            seen.push(word_at(&d, p));
        }
        assert_eq!(seen, ["Next", "\"Quoted.\"", "After", "New"]);
        let mid = at(&d, 0, "one", 0);
        assert_eq!(word_at(&d, mv(&d, mid, Motion::SentencePrev)), "Next");
    }

    #[test]
    fn blocks() {
        let d = doc(&["a b\nc\n\nd e\n\nf", "g"]);
        let p = mv(&d, TextPos::new(0, 0), Motion::BlockNext);
        assert_eq!(word_at(&d, p), "d");
        let p2 = mv(&d, p, Motion::BlockNext);
        assert_eq!(word_at(&d, mv(&d, p2, Motion::BlockNext)), "g");
        let inside = at(&d, 0, "e", 0);
        assert_eq!(mv(&d, inside, Motion::BlockPrev), p);
        assert_eq!(mv(&d, p, Motion::BlockPrev), TextPos::new(0, 0));
    }

    #[test]
    fn selection_text_dehyphenates_and_joins() {
        let d = doc(&["the annota-\ntions are well-\nKnown here"]);
        let s = Selection::new(TextPos::new(0, 0), at(&d, 0, "Known", 0));
        // A capitalised continuation keeps its hyphen.
        assert_eq!(s.text(&d), "the annotations are well- K");
        let mut back = s;
        back.swap();
        assert_eq!(back.text(&d), s.text(&d));
    }

    #[test]
    fn linewise_selection_and_rects() {
        let d = doc(&["  abc def\nghi\njkl"]);
        let mut s = Selection::new(at(&d, 0, "def", 0), at(&d, 0, "h", 0));
        assert_eq!(s.text(&d), "def gh");
        s.linewise = true;
        assert_eq!(s.text(&d), "abc def ghi");
        let r = s.rects(&d);
        assert_eq!(r.len(), 2);
        // Leading spaces are not part of the box.
        assert_eq!(r[0].1.x0, 12.0);
    }

    #[test]
    fn selection_across_pages() {
        let d = doc(&["one two", "three four"]);
        let s = Selection::new(at(&d, 0, "two", 0), at(&d, 1, "three", 0));
        assert_eq!(s.text(&d), "two t");
        assert_eq!(s.rects(&d).len(), 2);
    }
}
