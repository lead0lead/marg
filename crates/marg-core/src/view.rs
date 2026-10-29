//! Page layouts (continuous, single, spread), scrolling, zoom and fit
//! modes.
//!
//! Units: page sizes and document positions are in PDF points. The layout
//! is in *view units*: points × zoom, which the app treats as logical screen
//! pixels (zoom 1.0 = 100% = one point per logical pixel). Device pixels
//! never appear here.

use serde::{Deserialize, Serialize};

use crate::config::{Layout, ViewConfig};

/// How the zoom follows the viewport size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Manual zoom.
    None,
    /// The widest page fills the viewport width.
    Width,
    /// The largest page fits entirely in the viewport.
    Page,
}

/// A position in the document: a point on a page, in PDF points from the
/// page's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct DocPos {
    pub page: usize,
    pub x: f32,
    pub y: f32,
}

/// A rectangle in view units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }
}

#[derive(Debug, Clone)]
pub struct View {
    /// Page sizes in points, `(width, height)`.
    pages: Vec<(f32, f32)>,
    layout: Layout,
    /// Spread layout: the first page (a cover) has a row of its own.
    first_alone: bool,
    /// Rows of the layout: the first page and the page count (1 or 2) of
    /// each. One page per row, except in the spread layout.
    rows: Vec<(usize, usize)>,
    /// Row of each page.
    row_of: Vec<usize>,
    /// Top of each row in layout space, at the current zoom.
    tops: Vec<f32>,
    total_h: f32,
    /// The row shown in the single and spread layouts.
    row: usize,
    zoom: f32,
    fit: Fit,
    viewport: (f32, f32),
    /// Top-left corner of the viewport in layout space.
    scroll: (f32, f32),
    gap: f32,
    zoom_min: f32,
    zoom_max: f32,
}

impl View {
    /// `pages` must not be empty.
    pub fn new(
        pages: Vec<(f32, f32)>,
        cfg: &ViewConfig,
        layout: Layout,
        fit: Fit,
        viewport: (f32, f32),
    ) -> Self {
        assert!(!pages.is_empty(), "a document has at least one page");
        let mut v = Self {
            tops: Vec::new(),
            rows: Vec::new(),
            row_of: Vec::new(),
            pages,
            layout,
            first_alone: cfg.spread_first_page_alone,
            total_h: 0.0,
            row: 0,
            zoom: 1.0,
            fit,
            viewport,
            scroll: (0.0, 0.0),
            gap: cfg.page_gap_px,
            zoom_min: cfg.zoom_min,
            zoom_max: cfg.zoom_max,
        };
        v.build_rows();
        v.apply_fit();
        v.relayout();
        v
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    pub fn fit(&self) -> Fit {
        self.fit
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn first_page_alone(&self) -> bool {
        self.first_alone
    }

    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    pub fn scroll(&self) -> (f32, f32) {
        self.scroll
    }

    /// Size of page `i` in points.
    pub fn page_size(&self, i: usize) -> (f32, f32) {
        self.pages[i]
    }

    /// Single and spread: one row at a time, no scrolling past its edges.
    fn paged(&self) -> bool {
        self.layout != Layout::Continuous
    }

    fn build_rows(&mut self) {
        let n = self.pages.len();
        self.rows.clear();
        let mut i = 0;
        while i < n {
            let pair = self.layout == Layout::Spread && !(i == 0 && self.first_alone);
            let count = if pair { 2.min(n - i) } else { 1 };
            self.rows.push((i, count));
            i += count;
        }
        self.row_of = vec![0; n];
        for (r, &(first, count)) in self.rows.iter().enumerate() {
            for p in first..first + count {
                self.row_of[p] = r;
            }
        }
    }

    fn row_pages(&self, r: usize) -> std::ops::Range<usize> {
        let (first, count) = self.rows[r];
        first..first + count
    }

    /// Width and height of row `r` in points, without the gaps.
    fn row_size_pt(&self, r: usize) -> (f32, f32) {
        self.row_pages(r)
            .map(|p| self.pages[p])
            .fold((0.0, 0.0), |(w, h), (pw, ph)| (w + pw, h.max(ph)))
    }

    /// Gaps between the pages of row `r`, in view units.
    fn row_gaps(&self, r: usize) -> f32 {
        (self.rows[r].1 - 1) as f32 * self.gap
    }

    fn row_size(&self, r: usize) -> (f32, f32) {
        let (w, h) = self.row_size_pt(r);
        (w * self.zoom + self.row_gaps(r), h * self.zoom)
    }

    /// Width of the layout: the widest row (only the shown row when
    /// paged), at least the viewport.
    fn content_w(&self) -> f32 {
        let w = if self.paged() {
            self.row_size(self.row).0
        } else {
            (0..self.rows.len())
                .map(|r| self.row_size(r).0)
                .fold(0.0, f32::max)
        };
        w.max(self.viewport.0)
    }

    fn relayout(&mut self) {
        self.tops.clear();
        let mut y = 0.0;
        for r in 0..self.rows.len() {
            self.tops.push(y);
            y += self.row_size(r).1 + self.gap;
        }
        self.total_h = y - self.gap;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max_x = (self.content_w() - self.viewport.0).max(0.0);
        self.scroll.0 = self.scroll.0.clamp(0.0, max_x);
        let vh = self.viewport.1;
        if self.paged() {
            let (top, h) = (self.tops[self.row], self.row_size(self.row).1);
            self.scroll.1 = if h <= vh {
                // Centred vertically.
                top - (vh - h) / 2.0
            } else {
                self.scroll.1.clamp(top, top + h - vh)
            };
        } else {
            let max_y = (self.total_h - vh).max(0.0);
            self.scroll.1 = self.scroll.1.clamp(0.0, max_y);
        }
    }

    fn apply_fit(&mut self) {
        let (vw, vh) = self.viewport;
        let z = (0..self.rows.len())
            .map(|r| {
                let (w, h) = self.row_size_pt(r);
                let zw = (vw - self.row_gaps(r)).max(1.0) / w.max(1.0);
                match self.fit {
                    Fit::None => f32::INFINITY,
                    Fit::Width => zw,
                    Fit::Page => zw.min(vh / h.max(1.0)),
                }
            })
            .fold(f32::INFINITY, f32::min);
        if self.fit != Fit::None {
            self.zoom = z.clamp(self.zoom_min, self.zoom_max);
        }
    }

    /// Page `i`'s rectangle in layout space.
    pub fn page_rect(&self, i: usize) -> Rect {
        let r = self.row_of[i];
        let (row_w, row_h) = self.row_size(r);
        let content_w = if self.paged() {
            row_w.max(self.viewport.0)
        } else {
            self.content_w()
        };
        let mut x = (content_w - row_w) / 2.0;
        for p in self.rows[r].0..i {
            x += self.pages[p].0 * self.zoom + self.gap;
        }
        let (w, h) = self.pages[i];
        let (w, h) = (w * self.zoom, h * self.zoom);
        Rect {
            x,
            y: self.tops[r] + (row_h - h) / 2.0,
            w,
            h,
        }
    }

    /// Page `i`'s rectangle relative to the viewport's top-left corner.
    pub fn page_rect_on_screen(&self, i: usize) -> Rect {
        let r = self.page_rect(i);
        Rect {
            x: r.x - self.scroll.0,
            y: r.y - self.scroll.1,
            ..r
        }
    }

    /// A viewport position in points on page `page` (outside the page if
    /// the position is).
    pub fn point_on_page(&self, page: usize, sx: f32, sy: f32) -> (f32, f32) {
        let r = self.page_rect_on_screen(page);
        ((sx - r.x) / self.zoom, (sy - r.y) / self.zoom)
    }

    /// Index of the row whose vertical span contains `y` (layout space),
    /// or the nearest one. Paged: the shown row.
    fn row_at_y(&self, y: f32) -> usize {
        if self.paged() {
            return self.row;
        }
        // Last row whose top is at or above y.
        let i = self.tops.partition_point(|&t| t <= y).saturating_sub(1);
        // In the gap below row i: pick the nearer row.
        let bottom = self.tops[i] + self.row_size(i).1;
        if y > bottom && i + 1 < self.rows.len() && y - bottom > self.tops[i + 1] - y {
            i + 1
        } else {
            i
        }
    }

    /// The page of row `r` nearest to layout position `x`.
    fn page_in_row_at_x(&self, r: usize, x: f32) -> usize {
        let pages = self.row_pages(r);
        let last = pages.end - 1;
        pages
            .take_while(|&p| p < last)
            .find(|&p| x < self.page_rect(p).right() + self.gap / 2.0)
            .unwrap_or(last)
    }

    /// The page shown in the status bar and used by page motions: the one
    /// at the viewport's vertical centre; paged: the first page shown.
    pub fn current_page(&self) -> usize {
        if self.paged() {
            return self.rows[self.row].0;
        }
        self.row_at_y(self.scroll.1 + self.viewport.1 / 2.0)
    }

    /// The pages of the current row (two in a spread).
    pub fn current_pages(&self) -> std::ops::Range<usize> {
        self.row_pages(self.row_of[self.current_page()])
    }

    /// Pages that intersect the viewport, top to bottom; paged: the pages
    /// shown.
    pub fn visible_pages(&self) -> std::ops::RangeInclusive<usize> {
        if self.paged() {
            let pages = self.row_pages(self.row);
            return pages.start..=pages.end - 1;
        }
        let first = self.row_at_y(self.scroll.1);
        let first = if self.page_rect(first).bottom() < self.scroll.1 {
            (first + 1).min(self.pages.len() - 1)
        } else {
            first
        };
        let bottom = self.scroll.1 + self.viewport.1;
        let last = self.tops.partition_point(|&t| t < bottom).saturating_sub(1);
        first..=last.max(first)
    }

    /// Document position at a point of the viewport (view units from its
    /// top-left corner). Points in a gap or margin map onto the nearest page.
    pub fn doc_pos_at(&self, sx: f32, sy: f32) -> DocPos {
        let (lx, ly) = (sx + self.scroll.0, sy + self.scroll.1);
        let page = self.page_in_row_at_x(self.row_at_y(ly), lx);
        let r = self.page_rect(page);
        let (w, h) = self.pages[page];
        DocPos {
            page,
            x: ((lx - r.x) / self.zoom).clamp(0.0, w),
            y: ((ly - r.y) / self.zoom).clamp(0.0, h),
        }
    }

    /// Scroll so that `pos` appears at viewport point `(sx, sy)`, as far as
    /// the scroll limits allow. Paged: shows `pos`'s row.
    pub fn put_doc_pos_at(&mut self, pos: DocPos, sx: f32, sy: f32) {
        let page = pos.page.min(self.pages.len() - 1);
        self.row = self.row_of[page];
        let r = self.page_rect(page);
        self.scroll = (r.x + pos.x * self.zoom - sx, r.y + pos.y * self.zoom - sy);
        self.clamp_scroll();
    }

    /// The reading position to save: the document point at the viewport's
    /// top-left corner.
    pub fn position(&self) -> DocPos {
        self.doc_pos_at(0.0, 0.0)
    }

    pub fn set_position(&mut self, pos: DocPos) {
        self.put_doc_pos_at(pos, 0.0, 0.0);
    }

    /// Scroll by view units. Paged: stops at the edges of the shown row.
    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.scroll.0 += dx;
        self.scroll.1 += dy;
        self.clamp_scroll();
    }

    /// Scroll by `fraction` of the viewport height (e.g. 0.5 for half a page).
    pub fn scroll_viewports(&mut self, fraction: f32) {
        self.scroll_by(0.0, self.viewport.1 * fraction);
    }

    /// Space / Shift-Space: one screen down (`forward`) or up. Paged: at
    /// the bottom of the shown row, show the next row from its top; at the
    /// top, the previous row from its bottom. Continuous: the next or
    /// previous page.
    pub fn screen_step(&mut self, forward: bool) {
        if !self.paged() {
            self.page_step(if forward { 1 } else { -1 });
            return;
        }
        let before = self.scroll.1;
        self.scroll_viewports(if forward { 1.0 } else { -1.0 });
        if (self.scroll.1 - before).abs() > 0.5 {
            return;
        }
        if forward && self.row + 1 < self.rows.len() {
            self.row += 1;
            self.scroll.1 = self.tops[self.row];
        } else if !forward && self.row > 0 {
            self.row -= 1;
            self.scroll.1 = f32::INFINITY;
        }
        self.clamp_scroll();
    }

    /// Put the top of page `i` at the top of the viewport (clamped to the
    /// last page). The horizontal scroll is kept.
    pub fn goto_page(&mut self, i: usize) {
        let i = i.min(self.pages.len() - 1);
        self.row = self.row_of[i];
        self.scroll.1 = self.tops[self.row];
        self.clamp_scroll();
    }

    /// Move `n` pages forward (negative: back) from the current page;
    /// spread: `n` spreads.
    pub fn page_step(&mut self, n: isize) {
        if self.paged() {
            let row = self.row.saturating_add_signed(n).min(self.rows.len() - 1);
            self.goto_page(self.rows[row].0);
        } else {
            let target = self.current_page().saturating_add_signed(n);
            self.goto_page(target);
        }
    }

    /// Switch the layout, keeping the reading position.
    pub fn set_layout(&mut self, layout: Layout) {
        self.relayout_keeping(|v| v.layout = layout);
    }

    /// Spread layout: give the first page a row of its own, or not.
    pub fn set_first_page_alone(&mut self, alone: bool) {
        self.relayout_keeping(|v| v.first_alone = alone);
    }

    /// New gap and zoom limits (`:set`, config reload), keeping the
    /// reading position.
    pub fn set_spacing(&mut self, cfg: &ViewConfig) {
        self.relayout_keeping(|v| {
            v.gap = cfg.page_gap_px;
            v.zoom_min = cfg.zoom_min;
            v.zoom_max = cfg.zoom_max;
            v.zoom = v.zoom.clamp(v.zoom_min, v.zoom_max);
        });
    }

    fn relayout_keeping(&mut self, change: impl FnOnce(&mut Self)) {
        let pos = self.position();
        change(self);
        self.build_rows();
        self.row = self.row_of[pos.page];
        self.apply_fit();
        self.relayout();
        self.set_position(pos);
    }

    pub fn set_viewport(&mut self, w: f32, h: f32) {
        if (w, h) == self.viewport {
            return;
        }
        let pos = self.position();
        self.viewport = (w.max(1.0), h.max(1.0));
        self.apply_fit();
        self.relayout();
        self.set_position(pos);
    }

    /// Set the zoom, keeping the document point under viewport point
    /// `anchor` in place. Leaves fit mode.
    pub fn set_zoom_at(&mut self, zoom: f32, anchor: (f32, f32)) {
        let pos = self.doc_pos_at(anchor.0, anchor.1);
        self.fit = Fit::None;
        self.zoom = zoom.clamp(self.zoom_min, self.zoom_max);
        self.relayout();
        self.put_doc_pos_at(pos, anchor.0, anchor.1);
    }

    /// Multiply the zoom by `factor` around the viewport centre.
    pub fn zoom_by(&mut self, factor: f32) {
        let c = (self.viewport.0 / 2.0, self.viewport.1 / 2.0);
        self.set_zoom_at(self.zoom * factor, c);
    }

    /// Enter a fit mode (or leave it with `Fit::None`), keeping the reading
    /// position at the top of the viewport. Fit page shows the current page
    /// whole, from its top.
    pub fn set_fit(&mut self, fit: Fit) {
        let current = self.current_page();
        let pos = self.position();
        self.fit = fit;
        self.apply_fit();
        self.relayout();
        if fit == Fit::Page {
            self.scroll.0 = 0.0;
            self.goto_page(current);
        } else {
            self.set_position(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LETTER: (f32, f32) = (612.0, 792.0);

    fn cfg() -> ViewConfig {
        ViewConfig {
            page_gap_px: 10.0,
            ..ViewConfig::default()
        }
    }

    fn letters(n: usize) -> Vec<(f32, f32)> {
        vec![LETTER; n]
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn fit_width_scales_widest_page_to_viewport() {
        let v = View::new(
            letters(3),
            &cfg(),
            Layout::Continuous,
            Fit::Width,
            (1224.0, 800.0),
        );
        assert!(approx(v.zoom(), 2.0));
        let r = v.page_rect(1);
        assert!(approx(r.x, 0.0) && approx(r.w, 1224.0));
        assert!(approx(r.y, 792.0 * 2.0 + 10.0));
    }

    #[test]
    fn fit_page_fits_height_for_portrait_pages() {
        let v = View::new(
            letters(3),
            &cfg(),
            Layout::Continuous,
            Fit::Page,
            (1000.0, 396.0),
        );
        assert!(approx(v.zoom(), 0.5));
        // Narrow pages are centred.
        assert!(approx(v.page_rect(0).x, (1000.0 - 306.0) / 2.0));
    }

    #[test]
    fn scroll_is_clamped() {
        let mut v = View::new(
            letters(2),
            &cfg(),
            Layout::Continuous,
            Fit::None,
            (612.0, 500.0),
        );
        v.scroll_by(-50.0, -50.0);
        assert_eq!(v.scroll(), (0.0, 0.0));
        v.scroll_by(0.0, 1e6);
        assert!(approx(v.scroll().1, 792.0 * 2.0 + 10.0 - 500.0));
    }

    #[test]
    fn current_and_visible_pages() {
        let mut v = View::new(
            letters(5),
            &cfg(),
            Layout::Continuous,
            Fit::None,
            (612.0, 500.0),
        );
        assert_eq!(v.current_page(), 0);
        assert_eq!(v.visible_pages(), 0..=0);
        v.scroll_by(0.0, 700.0); // shows the end of page 0 and the start of page 1
        assert_eq!(v.visible_pages(), 0..=1);
        assert_eq!(v.current_page(), 1); // centre at 950 > 802
    }

    #[test]
    fn page_navigation() {
        let mut v = View::new(
            letters(5),
            &cfg(),
            Layout::Continuous,
            Fit::None,
            (612.0, 500.0),
        );
        v.page_step(2);
        assert_eq!(v.current_page(), 2);
        assert!(approx(v.scroll().1, 2.0 * 802.0));
        v.page_step(-5);
        assert_eq!(v.scroll().1, 0.0);
        v.goto_page(99);
        assert_eq!(v.current_page(), 4);
        // Clamped: the last page cannot scroll to the very top when the
        // viewport is taller than what remains.
        assert!(approx(v.scroll().1, 4.0 * 802.0));
    }

    #[test]
    fn zoom_keeps_anchor_point() {
        let mut v = View::new(
            letters(3),
            &cfg(),
            Layout::Continuous,
            Fit::Width,
            (612.0, 500.0),
        );
        v.scroll_by(0.0, 1000.0);
        let anchor = (300.0, 250.0);
        let before = v.doc_pos_at(anchor.0, anchor.1);
        v.set_zoom_at(2.0, anchor);
        assert_eq!(v.fit(), Fit::None);
        let after = v.doc_pos_at(anchor.0, anchor.1);
        assert_eq!(before.page, after.page);
        assert!(approx(before.x, after.x) && approx(before.y, after.y));
    }

    #[test]
    fn zoom_is_clamped_to_limits() {
        let mut v = View::new(
            letters(1),
            &cfg(),
            Layout::Continuous,
            Fit::None,
            (612.0, 500.0),
        );
        v.zoom_by(1000.0);
        assert_eq!(v.zoom(), cfg().zoom_max);
        v.zoom_by(1e-6);
        assert_eq!(v.zoom(), cfg().zoom_min);
    }

    #[test]
    fn resize_in_fit_width_keeps_position() {
        let mut v = View::new(
            letters(4),
            &cfg(),
            Layout::Continuous,
            Fit::Width,
            (612.0, 500.0),
        );
        v.goto_page(2);
        v.scroll_by(0.0, 100.0);
        let before = v.position();
        v.set_viewport(1224.0, 500.0);
        assert!(approx(v.zoom(), 2.0));
        let after = v.position();
        assert_eq!(after.page, 2);
        assert!(approx(before.y, after.y));
    }

    #[test]
    fn fit_page_shows_current_page_whole() {
        let mut v = View::new(
            letters(5),
            &cfg(),
            Layout::Continuous,
            Fit::Width,
            (612.0, 500.0),
        );
        v.goto_page(2);
        v.scroll_by(0.0, 300.0); // middle of page 2
        v.set_fit(Fit::Page);
        let r = v.page_rect_on_screen(2);
        assert!(approx(r.y, 0.0) && r.bottom() <= 500.0 + 0.01);
    }

    #[test]
    fn position_round_trips() {
        let mut v = View::new(
            letters(10),
            &cfg(),
            Layout::Continuous,
            Fit::Width,
            (800.0, 600.0),
        );
        let pos = DocPos {
            page: 6,
            x: 0.0,
            y: 123.0,
        };
        v.set_position(pos);
        let got = v.position();
        assert_eq!(got.page, 6);
        assert!(approx(got.y, 123.0));
    }

    #[test]
    fn mixed_page_sizes_are_centred() {
        let pages = vec![(612.0, 792.0), (792.0, 612.0)];
        let v = View::new(
            pages,
            &cfg(),
            Layout::Continuous,
            Fit::Width,
            (792.0, 600.0),
        );
        assert!(approx(v.zoom(), 1.0));
        assert!(approx(v.page_rect(0).x, 90.0));
        assert!(approx(v.page_rect(1).x, 0.0));
    }

    #[test]
    fn doc_pos_in_gap_maps_to_nearest_page() {
        let v = View::new(
            letters(2),
            &cfg(),
            Layout::Continuous,
            Fit::None,
            (612.0, 2000.0),
        );
        // Gap runs from 792 to 802.
        assert_eq!(v.doc_pos_at(10.0, 794.0).page, 0);
        assert_eq!(v.doc_pos_at(10.0, 800.0).page, 1);
    }

    fn paged(layout: Layout, n: usize, fit: Fit, vp: (f32, f32)) -> View {
        View::new(letters(n), &cfg(), layout, fit, vp)
    }

    #[test]
    fn single_shows_one_page_centred() {
        let v = paged(Layout::Single, 3, Fit::Page, (1000.0, 396.0));
        assert_eq!(v.visible_pages(), 0..=0);
        let r = v.page_rect_on_screen(0);
        assert!(approx(r.y, 0.0) && approx(r.x, (1000.0 - 306.0) / 2.0));
        // Smaller than the viewport: centred vertically too.
        let v = paged(Layout::Single, 3, Fit::None, (1000.0, 1000.0));
        assert!(approx(v.page_rect_on_screen(0).y, (1000.0 - 792.0) / 2.0));
    }

    #[test]
    fn single_scrolls_inside_the_page_only() {
        let mut v = paged(Layout::Single, 3, Fit::Width, (612.0, 500.0));
        v.scroll_by(0.0, 1e6);
        assert_eq!(v.current_page(), 0);
        assert!(approx(v.scroll().1, 792.0 - 500.0));
        v.page_step(1);
        assert_eq!(v.visible_pages(), 1..=1);
        assert!(approx(v.page_rect_on_screen(1).y, 0.0));
        v.scroll_by(0.0, -1e6);
        assert_eq!(v.current_page(), 1);
        v.page_step(10);
        assert_eq!(v.current_page(), 2);
    }

    #[test]
    fn screen_step_turns_the_page_at_its_edge() {
        let mut v = paged(Layout::Single, 2, Fit::Width, (612.0, 500.0));
        v.screen_step(true); // 0 → 292 (bottom)
        assert_eq!(v.current_page(), 0);
        assert!(approx(v.scroll().1, 292.0));
        v.screen_step(true); // at the bottom: next page, top
        assert_eq!(v.current_page(), 1);
        assert!(approx(v.page_rect_on_screen(1).y, 0.0));
        v.screen_step(false); // at the top: previous page, bottom
        assert_eq!(v.current_page(), 0);
        assert!(approx(v.page_rect_on_screen(0).bottom(), 500.0));
        // Continuous: a page step.
        let mut c = paged(Layout::Continuous, 3, Fit::Width, (612.0, 500.0));
        c.screen_step(true);
        assert_eq!(c.current_page(), 1);
    }

    #[test]
    fn spread_pairs_pages() {
        let mut v = paged(Layout::Spread, 5, Fit::Width, (1234.0, 500.0));
        // Two pages and a gap fill the width.
        assert!(approx(v.zoom(), (1234.0 - 10.0) / (2.0 * 612.0)));
        assert_eq!(v.visible_pages(), 0..=1);
        let (a, b) = (v.page_rect_on_screen(0), v.page_rect_on_screen(1));
        assert!(approx(a.x, 0.0) && approx(b.x, a.right() + 10.0) && approx(b.right(), 1234.0));
        assert_eq!(v.doc_pos_at(b.x + 5.0, 10.0).page, 1);
        assert_eq!(v.doc_pos_at(a.right() + 2.0, 10.0).page, 0);
        v.page_step(2);
        assert_eq!(v.visible_pages(), 4..=4); // odd page count: last alone
        assert_eq!(v.current_pages(), 4..5);
        v.set_first_page_alone(true);
        assert_eq!(v.visible_pages(), 3..=4);
        v.goto_page(0);
        assert_eq!(v.visible_pages(), 0..=0);
        v.page_step(1);
        assert_eq!(v.visible_pages(), 1..=2);
    }

    #[test]
    fn layout_switch_keeps_the_position() {
        let mut v = paged(Layout::Continuous, 6, Fit::Width, (612.0, 500.0));
        v.goto_page(3);
        v.scroll_by(0.0, 100.0);
        v.set_layout(Layout::Single);
        assert_eq!(v.current_page(), 3);
        assert!(approx(v.position().y, 100.0));
        v.set_layout(Layout::Spread);
        assert_eq!(v.visible_pages(), 2..=3);
        // The position is the left page of the spread.
        v.set_layout(Layout::Continuous);
        assert_eq!(v.position().page, 2);
    }
}
