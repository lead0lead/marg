//! One-line prompts (command line, search, filters) with a text cursor.

use eframe::egui::{self, Color32, text::CCursor};

/// `prefix` and `text` as one monospace label, with a thin cursor before
/// the `cursor`-th character of `text`, drawn between the characters like
/// a text field's (a cursor character would take a whole cell).
pub fn show(ui: &mut egui::Ui, prefix: &str, text: &str, cursor: usize, color: Color32) {
    let galley = egui::WidgetText::from(
        egui::RichText::new(format!("{prefix}{text}"))
            .monospace()
            .color(color),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Monospace,
    );
    let at = galley.pos_from_cursor(CCursor::new(prefix.chars().count() + cursor));
    let rect = ui.label(galley).rect;
    let width = ui.visuals().text_cursor.stroke.width;
    ui.painter().vline(
        rect.left() + at.left(),
        (rect.top() + at.top())..=(rect.top() + at.bottom()),
        egui::Stroke::new(width, color),
    );
}
