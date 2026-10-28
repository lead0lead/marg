//! The recent files list, shown when marg starts without a file.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, Event};
use marg_core::fuzzy;
use marg_core::list::{ListAction, ListNav};
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
    nav: ListNav,
    /// Indices into `items` matching the filter, best first.
    shown: Vec<usize>,
    /// Rows moved by `Ctrl-d`/`Ctrl-u`: half the rows that fit on screen.
    half_page: usize,
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
            nav: ListNav::default(),
            shown: Vec::new(),
            half_page: 1,
        };
        list.refilter();
        list
    }

    fn refilter(&mut self) {
        let paths: Vec<&str> = self.items.iter().map(|i| i.path.as_str()).collect();
        self.shown = fuzzy::filter_paths(self.nav.filter(), &paths);
        self.nav.set_len(self.shown.len());
    }

    /// Moving and filtering as in `marg_core::list`; Esc with no filter
    /// quits.
    pub fn event(&mut self, e: &Event) -> Action {
        let action = match e {
            Event::Text(t) if self.nav.filtering() => self.nav.text(t),
            _ => match input::key_press(e) {
                Some(k) => self.nav.key(k, self.half_page),
                None => return Action::None,
            },
        };
        match action {
            ListAction::None => {}
            ListAction::FilterChanged => self.refilter(),
            ListAction::Open => {
                if let Some(&i) = self.nav.selected().and_then(|r| self.shown.get(r)) {
                    return Action::Open(PathBuf::from(&self.items[i].path));
                }
            }
            ListAction::Close => return Action::Quit,
        }
        Action::None
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        egui::Frame::new().inner_margin(16).show(ui, |ui| {
            let filter = self.nav.filter();
            if self.nav.filtering() {
                let at = filter
                    .char_indices()
                    .nth(self.nav.cursor())
                    .map_or(filter.len(), |(i, _)| i);
                let (before, after) = filter.split_at(at);
                ui.label(text(&format!("/{before}▏{after}"), FG));
            } else if !filter.is_empty() {
                ui.label(text(&format!("/{filter}"), DIM));
            } else {
                ui.label(text("j/k move   / filter   Enter open   Esc quit", DIM));
            }
            ui.add_space(8.0);
            let row_h = ui.text_style_height(&egui::TextStyle::Monospace)
                + 4.0
                + ui.spacing().item_spacing.y;
            self.half_page = ((ui.available_height() / row_h) as usize / 2).max(1);
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
                    let selected = Some(row) == self.nav.selected();
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
