//! Continuous layout, scrolling, zoom and fit modes.
//!
//! Units: page sizes and document positions are in PDF points. The layout
//! is in *view units*: points × zoom, which the app treats as logical screen
//! pixels (zoom 1.0 = 100% = one point per logical pixel). Device pixels
//! never appear here.

use serde::{Deserialize, Serialize};

use crate::config::ViewConfig;

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
    max_w: f32,
    max_h: f32,
    /// Top of each page in layout space, at the current zoom.
    tops: Vec<f32>,
    total_h: f32,
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
    pub fn new(pages: Vec<(f32, f32)>, cfg: &ViewConfig, fit: Fit, viewport: (f32, f32)) -> Self {
        assert!(!pages.is_empty(), "a document has at least one page");
        let max_w = pages.iter().map(|p| p.0).fold(1.0, f32::max);
        let max_h = pages.iter().map(|p| p.1).fold(1.0, f32::max);
        let mut v = Self {
            tops: Vec::with_capacity(pages.len()),
            pages,
            max_w,
            max_h,
            total_h: 0.0,
            zoom: 1.0,
            fit,
            viewport,
            scroll: (0.0, 0.0),
            gap: cfg.page_gap_px,
            zoom_min: cfg.zoom_min,
            zoom_max: cfg.zoom_max,
        };
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

    fn content_w(&self) -> f32 {
        (self.max_w * self.zoom).max(self.viewport.0)
    }

    fn relayout(&mut self) {
        self.tops.clear();
        let mut y = 0.0;
        for &(_, h) in &self.pages {
            self.tops.push(y);
            y += h * self.zoom + self.gap;
        }
        self.total_h = y - self.gap;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max_x = (self.content_w() - self.viewport.0).max(0.0);
        let max_y = (self.total_h - self.viewport.1).max(0.0);
        self.scroll.0 = self.scroll.0.clamp(0.0, max_x);
        self.scroll.1 = self.scroll.1.clamp(0.0, max_y);
    }

    fn apply_fit(&mut self) {
        let (vw, vh) = self.viewport;
        let z = match self.fit {
            Fit::None => return,
            Fit::Width => vw / self.max_w,
            Fit::Page => (vw / self.max_w).min(vh / self.max_h),
        };
        self.zoom = z.clamp(self.zoom_min, self.zoom_max);
    }

    /// Page `i`'s rectangle in layout space.
    pub fn page_rect(&self, i: usize) -> Rect {
        let (w, h) = self.pages[i];
        let (w, h) = (w * self.zoom, h * self.zoom);
        Rect {
            x: (self.content_w() - w) / 2.0,
            y: self.tops[i],
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

    /// Index of the page whose vertical span contains `y` (layout space),
    /// or the nearest one.
    fn page_at_y(&self, y: f32) -> usize {
        // Last page whose top is at or above y.
        let i = self.tops.partition_point(|&t| t <= y).saturating_sub(1);
        // In the gap below page i: pick the nearer page.
        let bottom = self.page_rect(i).bottom();
        if y > bottom && i + 1 < self.pages.len() && y - bottom > self.tops[i + 1] - y {
            i + 1
        } else {
            i
        }
    }

    /// The page shown in the status bar and used by page motions: the one
    /// at the viewport's vertical centre.
    pub fn current_page(&self) -> usize {
        self.page_at_y(self.scroll.1 + self.viewport.1 / 2.0)
    }

    /// Pages that intersect the viewport, top to bottom.
    pub fn visible_pages(&self) -> std::ops::RangeInclusive<usize> {
        let first = self.page_at_y(self.scroll.1);
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
        let page = self.page_at_y(ly);
        let r = self.page_rect(page);
        let (w, h) = self.pages[page];
        DocPos {
            page,
            x: ((lx - r.x) / self.zoom).clamp(0.0, w),
            y: ((ly - r.y) / self.zoom).clamp(0.0, h),
        }
    }

    /// Scroll so that `pos` appears at viewport point `(sx, sy)`, as far as
    /// the scroll limits allow.
    pub fn put_doc_pos_at(&mut self, pos: DocPos, sx: f32, sy: f32) {
        let page = pos.page.min(self.pages.len() - 1);
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

    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.scroll.0 += dx;
        self.scroll.1 += dy;
        self.clamp_scroll();
    }

    /// Scroll by `fraction` of the viewport height (e.g. 0.5 for half a page).
    pub fn scroll_viewports(&mut self, fraction: f32) {
        self.scroll_by(0.0, self.viewport.1 * fraction);
    }

    /// Put the top of page `i` at the top of the viewport (clamped to the
    /// last page). The horizontal scroll is kept.
    pub fn goto_page(&mut self, i: usize) {
        let i = i.min(self.pages.len() - 1);
        self.scroll.1 = self.tops[i];
        self.clamp_scroll();
    }

    /// Move `n` pages forward (negative: back) from the current page.
    pub fn page_step(&mut self, n: isize) {
        let target = self.current_page().saturating_add_signed(n);
        self.goto_page(target);
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
        let v = View::new(letters(3), &cfg(), Fit::Width, (1224.0, 800.0));
        assert!(approx(v.zoom(), 2.0));
        let r = v.page_rect(1);
        assert!(approx(r.x, 0.0) && approx(r.w, 1224.0));
        assert!(approx(r.y, 792.0 * 2.0 + 10.0));
    }

    #[test]
    fn fit_page_fits_height_for_portrait_pages() {
        let v = View::new(letters(3), &cfg(), Fit::Page, (1000.0, 396.0));
        assert!(approx(v.zoom(), 0.5));
        // Narrow pages are centred.
        assert!(approx(v.page_rect(0).x, (1000.0 - 306.0) / 2.0));
    }

    #[test]
    fn scroll_is_clamped() {
        let mut v = View::new(letters(2), &cfg(), Fit::None, (612.0, 500.0));
        v.scroll_by(-50.0, -50.0);
        assert_eq!(v.scroll(), (0.0, 0.0));
        v.scroll_by(0.0, 1e6);
        assert!(approx(v.scroll().1, 792.0 * 2.0 + 10.0 - 500.0));
    }

    #[test]
    fn current_and_visible_pages() {
        let mut v = View::new(letters(5), &cfg(), Fit::None, (612.0, 500.0));
        assert_eq!(v.current_page(), 0);
        assert_eq!(v.visible_pages(), 0..=0);
        v.scroll_by(0.0, 700.0); // shows the end of page 0 and the start of page 1
        assert_eq!(v.visible_pages(), 0..=1);
        assert_eq!(v.current_page(), 1); // centre at 950 > 802
    }

    #[test]
    fn page_navigation() {
        let mut v = View::new(letters(5), &cfg(), Fit::None, (612.0, 500.0));
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
        let mut v = View::new(letters(3), &cfg(), Fit::Width, (612.0, 500.0));
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
        let mut v = View::new(letters(1), &cfg(), Fit::None, (612.0, 500.0));
        v.zoom_by(1000.0);
        assert_eq!(v.zoom(), cfg().zoom_max);
        v.zoom_by(1e-6);
        assert_eq!(v.zoom(), cfg().zoom_min);
    }

    #[test]
    fn resize_in_fit_width_keeps_position() {
        let mut v = View::new(letters(4), &cfg(), Fit::Width, (612.0, 500.0));
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
        let mut v = View::new(letters(5), &cfg(), Fit::Width, (612.0, 500.0));
        v.goto_page(2);
        v.scroll_by(0.0, 300.0); // middle of page 2
        v.set_fit(Fit::Page);
        let r = v.page_rect_on_screen(2);
        assert!(approx(r.y, 0.0) && r.bottom() <= 500.0 + 0.01);
    }

    #[test]
    fn position_round_trips() {
        let mut v = View::new(letters(10), &cfg(), Fit::Width, (800.0, 600.0));
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
        let v = View::new(pages, &cfg(), Fit::Width, (792.0, 600.0));
        assert!(approx(v.zoom(), 1.0));
        assert!(approx(v.page_rect(0).x, 90.0));
        assert!(approx(v.page_rect(1).x, 0.0));
    }

    #[test]
    fn doc_pos_in_gap_maps_to_nearest_page() {
        let v = View::new(letters(2), &cfg(), Fit::None, (612.0, 2000.0));
        // Gap runs from 792 to 802.
        assert_eq!(v.doc_pos_at(10.0, 794.0).page, 0);
        assert_eq!(v.doc_pos_at(10.0, 800.0).page, 1);
    }
}
