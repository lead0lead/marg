//! Box mode: pick two grid cells for the corners (or drag with the
//! mouse), then adjust the box and confirm it.

use eframe::egui::{self, Color32, FontId, Pos2, Rect};
use marg_core::boxsel::{BoxSel, grid_cell};
use marg_core::hints::{HintInput, labels};
use marg_core::view::View;

use crate::select::screen_rect;

const GRID_LINE: Color32 = Color32::from_rgba_unmultiplied_const(0x3D, 0x7E, 0xFF, 90);
const FIRST_CELL: Color32 = Color32::from_rgba_unmultiplied_const(0x3D, 0x7E, 0xFF, 60);
const LABEL_BG: Color32 = Color32::from_rgb(0xFF, 0xD8, 0x4D);
const LABEL_EDGE: Color32 = Color32::from_rgb(0x80, 0x68, 0x10);
const LABEL_TYPED: Color32 = Color32::from_rgb(0x9A, 0x80, 0x20);

#[derive(Debug, Clone, Copy)]
enum Stage {
    /// Picking the cell of the top-left corner.
    First,
    /// Picking the cell of the bottom-right corner; the first cell is
    /// given in viewport coordinates.
    Second(Rect),
    /// The box exists; keys adjust it.
    Adjust,
}

pub struct BoxMode {
    stage: Stage,
    input: HintInput,
    cols: usize,
    rows: usize,
    pub sel: Option<BoxSel>,
    /// Category, 1-based.
    pub category: u8,
    /// Where a mouse drag started: (page, point).
    drag: Option<(usize, (f32, f32))>,
}

impl BoxMode {
    pub fn new(cols: usize, rows: usize, chars: &[char], category: u8) -> Self {
        Self {
            stage: Stage::First,
            input: HintInput::new(labels(cols * rows, chars)),
            cols,
            rows,
            sel: None,
            category,
            drag: None,
        }
    }

    pub fn adjusting(&self) -> bool {
        matches!(self.stage, Stage::Adjust)
    }

    /// The typed part of a grid label.
    pub fn typed(&self) -> &str {
        if self.adjusting() {
            ""
        } else {
            self.input.typed()
        }
    }

    /// Cell `i` in viewport coordinates.
    fn cell(&self, view: &View, i: usize) -> Rect {
        let (w, h) = view.viewport();
        let (x0, y0, x1, y1) = grid_cell(w, h, self.cols, self.rows, i);
        Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1))
    }

    /// Type a grid label character. After the second cell the box exists.
    pub fn push(&mut self, c: char, view: &View) {
        let Some(i) = self.input.push(c) else { return };
        let cell = self.cell(view, i);
        match self.stage {
            Stage::First => {
                self.stage = Stage::Second(cell);
                self.input = HintInput::new(self.input.labels().to_vec());
            }
            Stage::Second(first) => {
                // The box spans both cells, in whichever order they were
                // picked, on the first corner's page.
                let span = first.union(cell);
                let page = view.doc_pos_at(first.min.x, first.min.y).page;
                let a = view.point_on_page(page, span.min.x, span.min.y);
                let b = view.point_on_page(page, span.max.x, span.max.y);
                self.sel = Some(BoxSel::from_corners(page, a, b, view.page_size(page)));
                self.stage = Stage::Adjust;
            }
            Stage::Adjust => {}
        }
    }

    /// Backspace: one typed character, or back to the first cell.
    pub fn pop(&mut self) {
        if !self.input.pop()
            && let Stage::Second(_) = self.stage
        {
            self.stage = Stage::First;
        }
    }

    /// Mouse press at a viewport position: a drag starts there.
    pub fn drag_start(&mut self, view: &View, x: f32, y: f32) {
        let page = view.doc_pos_at(x, y).page;
        self.drag = Some((page, view.point_on_page(page, x, y)));
    }

    /// Mouse moved while dragging: the box spans the drag.
    pub fn drag_to(&mut self, view: &View, x: f32, y: f32) {
        let Some((page, start)) = self.drag else {
            return;
        };
        let end = view.point_on_page(page, x, y);
        if (end.0 - start.0).abs() < 1.0 && (end.1 - start.1).abs() < 1.0 {
            return;
        }
        self.sel = Some(BoxSel::from_corners(page, start, end, view.page_size(page)));
        self.stage = Stage::Adjust;
    }

    pub fn drag_end(&mut self) {
        self.drag = None;
    }

    /// Move a corner by (dx, dy) points.
    pub fn nudge(&mut self, end: bool, dx: f32, dy: f32, view: &View) {
        if let Some(s) = &mut self.sel {
            s.move_corner(end, dx, dy, view.page_size(s.page));
        }
    }

    /// Draw the grid (while picking cells) or the box.
    pub fn draw(&self, painter: &egui::Painter, view: &View, area: Rect, color: Color32) {
        if let Some(s) = &self.sel {
            let r = screen_rect(view, area, s.page, s.rect);
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(2.0, color),
                egui::StrokeKind::Middle,
            );
            for corner in [r.left_top(), r.right_bottom()] {
                painter.rect_filled(
                    Rect::from_center_size(corner, egui::vec2(6.0, 6.0)),
                    0.0,
                    color,
                );
            }
        }
        if self.adjusting() {
            return;
        }
        let shift = area.min.to_vec2();
        if let Stage::Second(first) = self.stage {
            painter.rect_filled(first.translate(shift), 0.0, FIRST_CELL);
        }
        let stroke = egui::Stroke::new(1.0, GRID_LINE);
        for c in 1..self.cols {
            let x = self.cell(view, c).min.x + shift.x;
            painter.line_segment(
                [Pos2::new(x, area.top()), Pos2::new(x, area.bottom())],
                stroke,
            );
        }
        for r in 1..self.rows {
            let y = self.cell(view, r * self.cols).min.y + shift.y;
            painter.line_segment(
                [Pos2::new(area.left(), y), Pos2::new(area.right(), y)],
                stroke,
            );
        }
        let font = FontId::monospace(12.0);
        let typed = self.input.typed().len();
        for (i, label) in self.input.labels().iter().enumerate() {
            if !self.input.matches(i) {
                continue;
            }
            let center = self.cell(view, i).translate(shift).center();
            let mut job = egui::text::LayoutJob::default();
            let fmt = |c| egui::TextFormat::simple(font.clone(), c);
            job.append(&label[..typed], 0.0, fmt(LABEL_TYPED));
            job.append(&label[typed..], 0.0, fmt(Color32::BLACK));
            let galley = painter.layout_job(job);
            let size = galley.size() + egui::vec2(6.0, 2.0);
            let rect = Rect::from_center_size(center, size);
            painter.rect(
                rect,
                3.0,
                LABEL_BG,
                egui::Stroke::new(1.0, LABEL_EDGE),
                egui::StrokeKind::Inside,
            );
            painter.galley(rect.min + egui::vec2(3.0, 1.0), galley, Color32::BLACK);
        }
    }
}
