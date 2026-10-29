//! The outline overlay: the PDF's table of contents (`o`).

use eframe::egui::{self, Color32, Event};
use marg_core::fuzzy;
use marg_core::list::{ListAction, ListNav};
use marg_core::outline::{Dest, OutlineEntry, entry_for_page};

use crate::input;
use crate::prompt;
use crate::theme::Theme;

pub enum Action {
    None,
    Jump(Dest),
    Close,
}

pub struct OutlineList {
    entries: Vec<OutlineEntry>,
    nav: ListNav,
    /// Indices into `entries` matching the filter, best first.
    shown: Vec<usize>,
    half_page: usize,
}

impl OutlineList {
    /// The entry of `page` starts selected.
    pub fn new(entries: Vec<OutlineEntry>, page: usize) -> Self {
        let mut list = Self {
            entries,
            nav: ListNav::default(),
            shown: Vec::new(),
            half_page: 1,
        };
        list.refilter();
        if let Some(i) = entry_for_page(&list.entries, page) {
            list.nav.select(i);
        }
        list
    }

    fn refilter(&mut self) {
        let titles: Vec<&str> = self.entries.iter().map(|e| e.title.as_str()).collect();
        self.shown = fuzzy::filter(self.nav.filter(), &titles);
        self.nav.set_len(self.shown.len());
    }

    /// Moving and filtering as in `marg_core::list`.
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
                let entry = self.nav.selected().and_then(|r| self.shown.get(r));
                if let Some(d) = entry.and_then(|&i| self.entries[i].dest) {
                    return Action::Jump(d);
                }
            }
            ListAction::Close => return Action::Close,
        }
        Action::None
    }

    /// Draw the rows; `labels`: page labels.
    pub fn draw(&mut self, ui: &mut egui::Ui, th: &Theme, labels: Option<&[String]>) {
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        egui::Frame::new()
            .fill(th.panel_bg)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                let filter = self.nav.filter();
                if self.nav.filtering() {
                    prompt::show(ui, "/", filter, self.nav.cursor(), th.panel_fg);
                } else if !filter.is_empty() {
                    ui.label(text(&format!("/{filter}"), th.panel_dim));
                } else {
                    ui.label(text("j/k move  / filter  ⏎ go  Esc close", th.panel_dim));
                }
                ui.add_space(6.0);
                let row_h = ui.text_style_height(&egui::TextStyle::Monospace)
                    + 4.0
                    + ui.spacing().item_spacing.y;
                self.half_page = ((ui.available_height() / row_h) as usize / 2).max(1);
                if self.shown.is_empty() {
                    ui.label(text("No matches", th.panel_dim));
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (row, &i) in self.shown.iter().enumerate() {
                        let e = &self.entries[i];
                        let selected = Some(row) == self.nav.selected();
                        let frame = egui::Frame::new()
                            .fill(if selected {
                                th.panel_selected_bg
                            } else {
                                Color32::TRANSPARENT
                            })
                            .inner_margin(egui::Margin::symmetric(6, 2));
                        let r = frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.add_space(e.level as f32 * 16.0);
                                let fg = if e.dest.is_some() {
                                    th.panel_fg
                                } else {
                                    th.panel_dim
                                };
                                let page = e.dest.map_or(String::new(), |d| {
                                    labels
                                        .and_then(|l| l.get(d.page))
                                        .cloned()
                                        .unwrap_or_else(|| (d.page + 1).to_string())
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(text(&page, th.panel_dim));
                                        ui.with_layout(
                                            egui::Layout::left_to_right(egui::Align::Center),
                                            |ui| {
                                                ui.add(
                                                    egui::Label::new(text(&e.title, fg)).truncate(),
                                                )
                                            },
                                        );
                                    },
                                );
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
