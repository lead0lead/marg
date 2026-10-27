//! The recent files list, shown when marg starts without a file.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Event};
use marg_core::fuzzy;
use marg_core::keys::{KeyCode, Named};
use marg_core::lineedit::LineEdit;
use marg_core::state::State;

use crate::input;

const FG: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);
const DIM: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
const MISSING: Color32 = Color32::from_rgb(0x60, 0x60, 0x60);
const SELECTED_BG: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);

struct Item {
    path: String,
    exists: bool,
}

pub struct RecentList {
    items: Vec<Item>,
    filter: LineEdit,
    /// Indices into `items` matching the filter, best first.
    shown: Vec<usize>,
    selected: usize,
}

pub enum Action {
    None,
    Open(PathBuf),
    Quit,
}

impl RecentList {
    pub fn new(state: &State, max: usize) -> Self {
        let items = state
            .recent(max)
            .into_iter()
            .map(|f| Item {
                exists: Path::new(&f.path).is_file(),
                path: f.path.clone(),
            })
            .collect();
        let mut list = Self {
            items,
            filter: LineEdit::default(),
            shown: Vec::new(),
            selected: 0,
        };
        list.refilter();
        list
    }

    fn refilter(&mut self) {
        let paths: Vec<&str> = self.items.iter().map(|i| i.path.as_str()).collect();
        self.shown = fuzzy::filter(self.filter.text(), &paths);
        self.selected = 0;
    }

    fn step(&mut self, d: isize) {
        if !self.shown.is_empty() {
            let n = self.shown.len() as isize;
            self.selected = (self.selected as isize + d).rem_euclid(n) as usize;
        }
    }

    /// Typing filters; Ctrl-n/Ctrl-p or the arrow keys move; Enter opens;
    /// Esc clears the filter, or quits when it is empty.
    pub fn event(&mut self, e: &Event) -> Action {
        if let Event::Text(t) = e {
            self.filter.insert(t);
            self.refilter();
            return Action::None;
        }
        let Some(k) = input::key_press(e) else {
            return Action::None;
        };
        match (k.code, k.ctrl) {
            (KeyCode::Named(Named::Esc), _) if self.filter.is_empty() => return Action::Quit,
            (KeyCode::Named(Named::Esc), _) => {
                self.filter.clear();
                self.refilter();
            }
            (KeyCode::Named(Named::Enter), _) => {
                if let Some(&i) = self.shown.get(self.selected) {
                    return Action::Open(PathBuf::from(&self.items[i].path));
                }
            }
            (KeyCode::Named(Named::Backspace), _) => {
                self.filter.backspace();
                self.refilter();
            }
            (KeyCode::Named(Named::Space), false) => {
                self.filter.insert(" ");
                self.refilter();
            }
            (KeyCode::Named(Named::Down), _) | (KeyCode::Char('n'), true) => self.step(1),
            (KeyCode::Named(Named::Up), _) | (KeyCode::Char('p'), true) => self.step(-1),
            _ => {}
        }
        Action::None
    }

    pub fn draw(&self, ui: &mut egui::Ui) {
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            ui.label(text(&format!("> {}▏", self.filter.text()), FG));
            ui.add_space(8.0);
            if self.items.is_empty() {
                ui.label(text("No recent files. Run marg <file.pdf>.", DIM));
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (row, &i) in self.shown.iter().enumerate() {
                    let item = &self.items[i];
                    let path = Path::new(&item.path);
                    let name = path
                        .file_name()
                        .map_or(item.path.clone(), |n| n.to_string_lossy().into_owned());
                    let dir = path
                        .parent()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default();
                    let selected = row == self.selected;
                    let frame = egui::Frame::new()
                        .fill(if selected {
                            SELECTED_BG
                        } else {
                            Color32::TRANSPARENT
                        })
                        .inner_margin(egui::Margin::symmetric(6, 2));
                    let r = frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let fg = if item.exists { FG } else { MISSING };
                            ui.label(text(&name, fg).strong());
                            ui.label(text(&dir, if item.exists { DIM } else { MISSING }));
                            if !item.exists {
                                ui.label(text("(missing)", MISSING));
                            }
                        });
                    });
                    if selected {
                        r.response.scroll_to_me(None);
                    }
                }
            });
        });
    }
}
