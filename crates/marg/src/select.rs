//! Text targeting in the page view: hints, the caret and the selection,
//! and drawing them.

use std::ops::Range;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, text::LayoutJob};
use marg_core::annot::{AnnotInfo, AnnotKind, AnnotSpec};
use marg_core::caret::{Selection, TextPos, TextSource};
use marg_core::config::{HintMode, HintsConfig};
use marg_core::hints::{HintInput, labels};
use marg_core::text::{BBox, PageText};
use marg_core::view::View;

const HINT_BG: Color32 = Color32::from_rgb(0xFF, 0xD8, 0x4D);
const HINT_FG: Color32 = Color32::BLACK;
const HINT_TYPED: Color32 = Color32::from_rgb(0x9A, 0x80, 0x20);
const HINT_EDGE: Color32 = Color32::from_rgb(0x80, 0x68, 0x10);
/// `#3D7EFF` at 35%.
const SELECTION: Color32 = Color32::from_rgba_unmultiplied_const(0x3D, 0x7E, 0xFF, 89);
const CARET: Color32 = Color32::from_rgb(0xE0, 0x20, 0x20);
const CARET_FILL: Color32 = Color32::from_rgba_unmultiplied_const(0xE0, 0x20, 0x20, 40);

/// The caret: a character position, and the horizontal position that
/// vertical motions keep.
#[derive(Debug, Clone, Copy)]
pub struct Caret {
    pub pos: TextPos,
    pub want_x: Option<f32>,
}

/// What the picked word is for.
#[derive(Debug, Clone)]
pub enum Purpose {
    /// Place the caret at its start.
    Caret,
    /// First word of a range selection.
    SelectStart,
    /// Second word; the first was (start, end).
    SelectEnd(TextPos, TextPos),
}

#[derive(Debug, Clone)]
enum Targets {
    /// (page, line index).
    Lines(Vec<(usize, usize)>),
    /// (page, character range).
    Words(Vec<(usize, Range<usize>)>),
}

/// An active hint session.
pub struct Hinting {
    pub purpose: Purpose,
    targets: Targets,
    input: HintInput,
}

/// Rectangle of `b` (points on `page`) on screen.
pub fn screen_rect(view: &View, area: Rect, page: usize, b: BBox) -> Rect {
    let r = view.page_rect_on_screen(page);
    let z = view.zoom();
    Rect::from_min_max(
        Pos2::new(area.min.x + r.x + b.x0 * z, area.min.y + r.y + b.y0 * z),
        Pos2::new(area.min.x + r.x + b.x1 * z, area.min.y + r.y + b.y1 * z),
    )
}

fn on_screen(view: &View, area: Rect, page: usize, b: BBox) -> bool {
    area.intersects(screen_rect(view, area, page, b))
}

fn visible_lines(view: &View, area: Rect, texts: &[Option<PageText>]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for page in view.visible_pages() {
        let Some(t) = texts.text(page) else { continue };
        for (i, l) in t.lines.iter().enumerate() {
            if on_screen(view, area, page, l.bbox) {
                out.push((page, i));
            }
        }
    }
    out
}

fn word_bbox(t: &PageText, r: &Range<usize>) -> BBox {
    t.chars[r.clone()]
        .iter()
        .map(|c| c.bbox)
        .reduce(BBox::union)
        .unwrap_or_default()
}

fn alphabet(s: &str) -> Vec<char> {
    let mut v: Vec<char> = Vec::new();
    for c in s.chars() {
        if !v.contains(&c) {
            v.push(c);
        }
    }
    v
}

impl Hinting {
    /// Start hints for `purpose` on the visible text. `Err` is a message
    /// for the status bar.
    pub fn start(
        purpose: Purpose,
        view: &View,
        area: Rect,
        texts: &[Option<PageText>],
        cfg: &HintsConfig,
    ) -> Result<Self, &'static str> {
        let targets = match cfg.mode {
            HintMode::TwoStage => Targets::Lines(visible_lines(view, area, texts)),
            HintMode::Words => Targets::Words(
                visible_lines(view, area, texts)
                    .into_iter()
                    .flat_map(|(page, l)| {
                        let t = texts.text(page).expect("visible lines have text");
                        t.words(l)
                            .into_iter()
                            .filter(move |r| on_screen(view, area, page, word_bbox(t, r)))
                            .map(move |r| (page, r))
                    })
                    .collect(),
            ),
        };
        let n = match &targets {
            Targets::Lines(v) => v.len(),
            Targets::Words(v) => v.len(),
        };
        if n == 0 {
            let loading = view.visible_pages().any(|p| texts.text(p).is_none());
            return Err(if loading {
                "Text is still loading"
            } else {
                "No text on this page — use a box (b)"
            });
        }
        Ok(Self {
            purpose,
            input: HintInput::new(labels(n, &alphabet(&cfg.chars))),
            targets,
        })
    }

    pub fn typed(&self) -> &str {
        self.input.typed()
    }

    /// Type a hint character. Returns the picked word (page, characters)
    /// once one is picked.
    pub fn push(
        &mut self,
        c: char,
        texts: &[Option<PageText>],
        cfg: &HintsConfig,
    ) -> Option<(usize, Range<usize>)> {
        let i = self.input.push(c)?;
        match &self.targets {
            Targets::Words(w) => Some(w[i].clone()),
            Targets::Lines(lines) => {
                let (page, l) = lines[i];
                let words = texts.text(page)?.words(l);
                if words.len() == 1 {
                    return Some((page, words[0].clone()));
                }
                let chars = alphabet(&format!("{}{}", cfg.chars, cfg.extra_chars));
                self.input = HintInput::new(labels(words.len(), &chars));
                self.targets = Targets::Words(words.into_iter().map(|r| (page, r)).collect());
                None
            }
        }
    }

    /// Backspace. Returns false if nothing was typed in this stage.
    pub fn pop(&mut self) -> bool {
        self.input.pop()
    }

    /// Draw the labels that still match.
    pub fn draw(
        &self,
        painter: &egui::Painter,
        view: &View,
        area: Rect,
        texts: &[Option<PageText>],
    ) {
        let font = FontId::monospace(12.0);
        for (i, label) in self.input.labels().iter().enumerate() {
            if !self.input.matches(i) {
                continue;
            }
            let (page, bbox, left_of) = match &self.targets {
                Targets::Lines(v) => {
                    let (page, l) = v[i];
                    let Some(t) = texts.text(page) else { continue };
                    (page, t.lines[l].bbox, true)
                }
                Targets::Words(v) => {
                    let (page, r) = &v[i];
                    let Some(t) = texts.text(*page) else { continue };
                    (*page, word_bbox(t, r), false)
                }
            };
            let target = screen_rect(view, area, page, bbox);
            let typed = self.input.typed().len();
            let mut job = LayoutJob::default();
            let fmt = |color| egui::TextFormat::simple(font.clone(), color);
            job.append(&label[..typed], 0.0, fmt(HINT_TYPED));
            job.append(&label[typed..], 0.0, fmt(HINT_FG));
            let galley = painter.layout_job(job);
            let size = galley.size() + egui::vec2(6.0, 2.0);
            let min = if left_of {
                // In the margin left of the line, so labels never cover text.
                Pos2::new(
                    target.min.x - size.x - 3.0,
                    target.center().y - size.y / 2.0,
                )
            } else {
                Pos2::new(target.min.x - 1.0, target.center().y - size.y / 2.0)
            };
            let rect = Rect::from_min_size(min, size);
            painter.rect(
                rect,
                3.0,
                HINT_BG,
                egui::Stroke::new(1.0, HINT_EDGE),
                egui::StrokeKind::Inside,
            );
            painter.galley(rect.min + egui::vec2(3.0, 1.0), galley, HINT_FG);
        }
    }
}

/// Draw the selection: one translucent box per selected line.
pub fn draw_selection(
    painter: &egui::Painter,
    view: &View,
    area: Rect,
    sel: &Selection,
    texts: &[Option<PageText>],
) {
    let visible = view.visible_pages();
    for (page, b) in sel.rects(texts) {
        if visible.contains(&page) {
            painter.rect_filled(screen_rect(view, area, page, b), 0.0, SELECTION);
        }
    }
}

/// Draw the caret: a block around its character, like vim's Normal-mode
/// cursor (the caret is on the character, not between two).
pub fn draw_caret(
    painter: &egui::Painter,
    view: &View,
    area: Rect,
    caret: &Caret,
    texts: &[Option<PageText>],
) {
    let Some(t) = texts.text(caret.pos.page) else {
        return;
    };
    let r = screen_rect(view, area, caret.pos.page, t.chars[caret.pos.idx].bbox);
    // At least a few pixels wide, so narrow characters (i, l, .) show.
    let r = r.expand2(egui::vec2(((4.0 - r.width()) / 2.0).max(0.0), 0.0));
    painter.rect(
        r,
        1.0,
        CARET_FILL,
        egui::Stroke::new(1.5, CARET),
        egui::StrokeKind::Outside,
    );
}

/// Scroll so that the caret's character is inside the viewport, with a
/// margin.
pub fn keep_visible(view: &mut View, caret: &Caret, texts: &[Option<PageText>]) {
    let Some(t) = texts.text(caret.pos.page) else {
        return;
    };
    let (vw, vh) = view.viewport();
    let r = screen_rect(
        view,
        Rect::from_min_size(Pos2::ZERO, egui::vec2(vw, vh)),
        caret.pos.page,
        t.chars[caret.pos.idx].bbox,
    );
    let margin = (vh * 0.1).min(48.0);
    let dy = if r.min.y < margin {
        r.min.y - margin
    } else if r.max.y > vh - margin {
        r.max.y - (vh - margin)
    } else {
        0.0
    };
    let dx = if r.min.x < 0.0 {
        r.min.x - 8.0
    } else if r.max.x > vw {
        r.max.x - vw + 8.0
    } else {
        0.0
    };
    if dx != 0.0 || dy != 0.0 {
        view.scroll_by(dx, dy);
    }
}

/// Draw an annotation the document thread has not rendered yet.
pub fn draw_provisional(painter: &egui::Painter, view: &View, area: Rect, spec: &AnnotSpec) {
    if !view.visible_pages().contains(&spec.page) {
        return;
    }
    let [r, g, b] = spec.color;
    for &bbox in &spec.rects {
        let rect = screen_rect(view, area, spec.page, bbox);
        match spec.kind {
            AnnotKind::Highlight => {
                let a = (spec.opacity.clamp(0.0, 1.0) * 255.0) as u8;
                painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(r, g, b, a));
            }
            AnnotKind::Underline => {
                let t = (rect.height() * 0.07).max(1.0);
                let line = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - t), rect.max);
                painter.rect_filled(line, 0.0, Color32::from_rgb(r, g, b));
            }
        }
    }
}

/// Draw the selected annotation: a dashed outline in its color around each
/// of its boxes.
pub fn draw_selected_annot(painter: &egui::Painter, view: &View, area: Rect, a: &AnnotInfo) {
    if !view.visible_pages().contains(&a.page) {
        return;
    }
    let color = a.color.map_or(Color32::from_gray(90), |[r, g, b]| {
        Color32::from_rgb(r, g, b)
    });
    // A dark line under the colored dashes, so light colors show on white.
    let stroke = egui::Stroke::new(1.5, color);
    let under = egui::Stroke::new(2.5, Color32::from_black_alpha(110));
    for &b in a.boxes() {
        let r = screen_rect(view, area, a.page, b).expand(2.0);
        let pts = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        painter.add(egui::Shape::dashed_line(&pts, under, 5.0, 3.0));
        painter.add(egui::Shape::dashed_line(&pts, stroke, 5.0, 3.0));
    }
}
