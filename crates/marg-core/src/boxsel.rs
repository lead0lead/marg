//! Box mode geometry: the label grid, the box's corners and adjusting
//! them. Boxes are in points on one page; the grid is in view units.

use crate::text::BBox;

/// Smallest box side, in points.
const MIN_SIDE: f32 = 2.0;

/// Cell `i` of a `cols` × `rows` grid over a `w` × `h` area, row by row:
/// (x0, y0, x1, y1).
pub fn grid_cell(w: f32, h: f32, cols: usize, rows: usize, i: usize) -> (f32, f32, f32, f32) {
    let (cols, rows) = (cols.max(1), rows.max(1));
    let (c, r) = (i % cols, i / cols);
    let (cw, ch) = (w / cols as f32, h / rows as f32);
    (
        c as f32 * cw,
        r as f32 * ch,
        (c + 1) as f32 * cw,
        (r + 1) as f32 * ch,
    )
}

/// The image a box overlaps most, if it overlaps any.
pub fn snap_target(rect: BBox, images: &[BBox]) -> Option<BBox> {
    let overlap = |b: &BBox| {
        let w = rect.x1.min(b.x1) - rect.x0.max(b.x0);
        let h = rect.y1.min(b.y1) - rect.y0.max(b.y0);
        if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
    };
    images
        .iter()
        .map(|b| (overlap(b), *b))
        .filter(|(a, _)| *a > 0.0)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, b)| b)
}

/// A box being made, on one page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoxSel {
    pub page: usize,
    pub rect: BBox,
}

impl BoxSel {
    /// The box spanned by two corners (in either order), clamped to a page
    /// of size `page_size` and at least `MIN_SIDE` wide and high.
    pub fn from_corners(page: usize, a: (f32, f32), b: (f32, f32), page_size: (f32, f32)) -> Self {
        let mut s = Self {
            page,
            rect: BBox::new(a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)),
        };
        s.fit(page_size);
        s
    }

    /// Move the bottom-right corner (`end`) or the top-left one by
    /// (dx, dy) points. The corner stops at the page edge and at the
    /// minimum size.
    pub fn move_corner(&mut self, end: bool, dx: f32, dy: f32, page_size: (f32, f32)) {
        let r = &mut self.rect;
        if end {
            r.x1 = (r.x1 + dx).clamp(r.x0 + MIN_SIDE, page_size.0);
            r.y1 = (r.y1 + dy).clamp(r.y0 + MIN_SIDE, page_size.1);
        } else {
            r.x0 = (r.x0 + dx).clamp(0.0, r.x1 - MIN_SIDE);
            r.y0 = (r.y0 + dy).clamp(0.0, r.y1 - MIN_SIDE);
        }
    }

    /// Replace the box (e.g. snapped to an image), kept on the page.
    pub fn set(&mut self, rect: BBox, page_size: (f32, f32)) {
        self.rect = rect;
        self.fit(page_size);
    }

    /// Clamp into the page, at least `MIN_SIDE` wide and high.
    fn fit(&mut self, (w, h): (f32, f32)) {
        let r = &mut self.rect;
        r.x0 = r.x0.clamp(0.0, (w - MIN_SIDE).max(0.0));
        r.y0 = r.y0.clamp(0.0, (h - MIN_SIDE).max(0.0));
        r.x1 = r.x1.clamp(r.x0 + MIN_SIDE, w.max(r.x0 + MIN_SIDE));
        r.y1 = r.y1.clamp(r.y0 + MIN_SIDE, h.max(r.y0 + MIN_SIDE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: (f32, f32) = (600.0, 800.0);

    #[test]
    fn snaps_to_the_image_overlapped_most() {
        let images = [
            BBox::new(0.0, 0.0, 100.0, 100.0),
            BBox::new(90.0, 0.0, 300.0, 100.0),
        ];
        // 10×50 over the first, 40×50 over the second.
        let b = BBox::new(80.0, 20.0, 130.0, 70.0);
        assert_eq!(snap_target(b, &images), Some(images[1]));
        assert_eq!(
            snap_target(BBox::new(0.0, 200.0, 50.0, 250.0), &images),
            None
        );
    }

    #[test]
    fn grid_cells_row_by_row() {
        assert_eq!(grid_cell(90.0, 120.0, 9, 12, 0), (0.0, 0.0, 10.0, 10.0));
        assert_eq!(grid_cell(90.0, 120.0, 9, 12, 10), (10.0, 10.0, 20.0, 20.0));
        assert_eq!(
            grid_cell(90.0, 120.0, 9, 12, 107),
            (80.0, 110.0, 90.0, 120.0)
        );
    }

    #[test]
    fn corners_in_any_order_are_clamped_to_the_page() {
        let b = BoxSel::from_corners(3, (500.0, 900.0), (100.0, -20.0), PAGE);
        assert_eq!(b.page, 3);
        assert_eq!(b.rect, BBox::new(100.0, 0.0, 500.0, 800.0));
        // Both corners past the right edge: still a minimal box inside.
        let b = BoxSel::from_corners(0, (700.0, 10.0), (650.0, 10.0), PAGE);
        assert_eq!(b.rect, BBox::new(598.0, 10.0, 600.0, 12.0));
    }

    #[test]
    fn corners_move_within_page_and_minimum_size() {
        let mut b = BoxSel::from_corners(0, (100.0, 100.0), (200.0, 200.0), PAGE);
        b.move_corner(true, 4.0, -4.0, PAGE);
        assert_eq!(b.rect, BBox::new(100.0, 100.0, 204.0, 196.0));
        b.move_corner(false, -1.0, 1.0, PAGE);
        assert_eq!(b.rect, BBox::new(99.0, 101.0, 204.0, 196.0));
        b.move_corner(true, -500.0, 0.0, PAGE);
        assert_eq!(b.rect.x1, 101.0);
        b.move_corner(false, -500.0, -500.0, PAGE);
        assert_eq!((b.rect.x0, b.rect.y0), (0.0, 0.0));
        b.move_corner(true, 1000.0, 1000.0, PAGE);
        assert_eq!((b.rect.x1, b.rect.y1), PAGE);
    }
}
