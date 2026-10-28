//! The comment editor: a popup next to an annotation (Insert mode).

use eframe::egui::{self, Color32, Key, Modifiers, Rect};
use marg_core::text::BBox;

/// Which annotation the comment belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// A listed annotation, by object number.
    Annot { page: usize, xref: i32 },
    /// An annotation just created by the edit with this tag (its object
    /// number may not be known yet). A `note` is removed again when the
    /// editor is left without text.
    Created { page: usize, tag: u64, note: bool },
}

impl Target {
    pub fn page(&self) -> usize {
        match *self {
            Target::Annot { page, .. } | Target::Created { page, .. } => page,
        }
    }
}

/// What the user did this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Editing,
    /// `Ctrl-Enter`: keep this text.
    Save(String),
    /// Esc (twice if the text changed).
    Cancel,
}

pub struct CommentEditor {
    pub target: Target,
    /// The annotation's box (points on the target's page): the popup sits
    /// next to it.
    pub anchor: BBox,
    text: String,
    original: String,
    /// Esc was pressed once on changed text.
    armed: bool,
}

impl CommentEditor {
    pub fn new(target: Target, anchor: BBox, text: String) -> Self {
        Self {
            target,
            anchor,
            original: text.clone(),
            text,
            armed: false,
        }
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn changed(&self) -> bool {
        self.text != self.original
    }

    /// Take `Ctrl-Enter` and Esc out of this frame's input, before the text
    /// field sees them.
    pub fn keys(&mut self, ctx: &egui::Context) -> Outcome {
        let (save, esc) = ctx.input_mut(|i| {
            let save = i.consume_key(Modifiers::CTRL, Key::Enter)
                || i.consume_key(Modifiers::COMMAND, Key::Enter);
            (save, i.consume_key(Modifiers::NONE, Key::Escape))
        });
        if save {
            return Outcome::Save(self.text.trim_end().to_string());
        }
        if esc {
            if !self.changed() || self.armed {
                return Outcome::Cancel;
            }
            self.armed = true;
        }
        Outcome::Editing
    }

    /// The status bar text while editing.
    pub fn status(&self) -> &'static str {
        if self.armed {
            "Esc again to discard the changes"
        } else {
            "Ctrl-Enter saves, Esc cancels"
        }
    }

    /// Draw the popup next to `anchor` (on screen), inside `area`.
    pub fn show(&mut self, ctx: &egui::Context, anchor: Rect, area: Rect, width: f32, max_h: f32) {
        let [x, y] = popup_pos(anchor, area, width);
        egui::Area::new(egui::Id::unique("marg-comment"))
            .fixed_pos([x, y])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(egui::Stroke::new(1.0, PANEL_EDGE))
                    .inner_margin(6)
                    .show(ui, |ui| {
                        ui.set_width(width - 12.0);
                        let v = ui.visuals_mut();
                        v.text_cursor.stroke.color = TEXT;
                        v.selection.bg_fill = SELECTION;
                        egui::ScrollArea::vertical()
                            .max_height(max_h.min(area.bottom() - y - 12.0).max(40.0))
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                let r = ui.add(
                                    egui::TextEdit::multiline(&mut self.text)
                                        .frame(egui::Frame::NONE)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(3)
                                        .lock_focus(true)
                                        .text_color(TEXT)
                                        .font(egui::TextStyle::Monospace),
                                );
                                // Keys always go to the comment, also after
                                // a click elsewhere.
                                r.request_focus();
                            });
                    });
            });
    }
}

/// Where a popup `width` wide goes next to `anchor`, inside `area`: right
/// of it if there is room, else left of it, else over it.
fn popup_pos(anchor: Rect, area: Rect, width: f32) -> [f32; 2] {
    const GAP: f32 = 8.0;
    let x = if anchor.right() + GAP + width <= area.right() {
        anchor.right() + GAP
    } else if anchor.left() - GAP - width >= area.left() {
        anchor.left() - GAP - width
    } else {
        (area.right() - width).max(area.left())
    };
    let y = anchor
        .top()
        .clamp(area.top(), (area.bottom() - 60.0).max(area.top()));
    [x, y]
}

/// Show a comment next to its annotation (`anchor`, on screen), at most
/// `width` wide and `max_h` high (longer text is cut off).
pub fn tooltip(ctx: &egui::Context, text: &str, anchor: Rect, area: Rect, width: f32, max_h: f32) {
    let [x, y] = popup_pos(anchor, area, width);
    egui::Area::new(egui::Id::unique("marg-tooltip"))
        .fixed_pos([x, y])
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(PANEL_BG)
                .stroke(egui::Stroke::new(1.0, PANEL_EDGE))
                .inner_margin(6)
                .show(ui, |ui| {
                    ui.set_max_width(width - 12.0);
                    ui.set_max_height(max_h.min(area.bottom() - y - 12.0).max(40.0));
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).monospace().color(TEXT)).wrap(),
                    );
                });
        });
}

const PANEL_BG: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const PANEL_EDGE: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
const TEXT: Color32 = Color32::from_rgb(0xDD, 0xDD, 0xDD);
const SELECTION: Color32 = Color32::from_rgb(0x2D, 0x4E, 0x8F);

#[cfg(test)]
mod tests {
    use super::*;

    /// Run one frame with a key press and return the editor's outcome.
    fn press(ed: &mut CommentEditor, key: Key, modifiers: Modifiers) -> Outcome {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let mut out = Outcome::Editing;
        let mut output = ctx.run_ui(input, |ui| out = ed.keys(ui.ctx()));
        output.textures_delta.clear();
        out
    }

    fn editor(text: &str) -> CommentEditor {
        let target = Target::Annot { page: 0, xref: 7 };
        CommentEditor::new(target, BBox::new(0.0, 0.0, 1.0, 1.0), text.into())
    }

    #[test]
    fn ctrl_enter_saves_trimmed_text() {
        let mut ed = editor("note");
        ed.text.push_str("s\n\n");
        assert_eq!(
            press(&mut ed, Key::Enter, Modifiers::CTRL),
            Outcome::Save("notes".into())
        );
        // Plain Enter is the text field's (a newline).
        assert_eq!(
            press(&mut ed, Key::Enter, Modifiers::NONE),
            Outcome::Editing
        );
    }

    #[test]
    fn esc_cancels_at_once_unless_changed() {
        let mut ed = editor("same");
        assert_eq!(
            press(&mut ed, Key::Escape, Modifiers::NONE),
            Outcome::Cancel
        );
        let mut ed = editor("old");
        ed.text = "new".into();
        assert_eq!(
            press(&mut ed, Key::Escape, Modifiers::NONE),
            Outcome::Editing
        );
        assert_eq!(ed.status(), "Esc again to discard the changes");
        assert_eq!(
            press(&mut ed, Key::Escape, Modifiers::NONE),
            Outcome::Cancel
        );
    }
}
