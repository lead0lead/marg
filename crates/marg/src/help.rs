//! The help overlay (`?`, the current mode's keys) and the help page
//! (`:help`, everything). Both are lists: moving and filtering as in
//! `marg_core::list`.

use eframe::egui::{self, Color32, Event};
use marg_core::help::{HelpLine, filter};
use marg_core::keys::KeyCode;
use marg_core::list::{ListAction, ListNav};

use crate::input;
use crate::theme::Theme;

/// Width of the keys column, in characters.
const KEYS_COLS: usize = 26;

pub struct HelpView {
    title: String,
    lines: Vec<HelpLine>,
    nav: ListNav,
    /// Indices into `lines` that pass the filter.
    shown: Vec<usize>,
    half_page: usize,
}

impl HelpView {
    pub fn new(title: String, lines: Vec<HelpLine>) -> Self {
        let mut v = Self {
            title,
            lines,
            nav: ListNav::default(),
            shown: Vec::new(),
            half_page: 1,
        };
        v.refilter();
        v
    }

    fn refilter(&mut self) {
        self.shown = filter(&self.lines, self.nav.filter());
        self.nav.set_len(self.shown.len());
    }

    /// True when the help should close (Esc, or `?` outside the filter).
    pub fn event(&mut self, e: &Event) -> bool {
        let action = match e {
            Event::Text(t) if self.nav.filtering() => self.nav.text(t),
            _ => match input::key_press(e) {
                Some(k) if k.code == KeyCode::Char('?') && !self.nav.filtering() => return true,
                Some(k) => self.nav.key(k, self.half_page),
                None => return false,
            },
        };
        match action {
            ListAction::FilterChanged => self.refilter(),
            ListAction::Close => return true,
            ListAction::None | ListAction::Open => {}
        }
        false
    }

    pub fn draw(&mut self, ui: &mut egui::Ui, th: &Theme) {
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        egui::Frame::new()
            .fill(th.panel_bg)
            .stroke(egui::Stroke::new(1.0, th.panel_border))
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.horizontal(|ui| {
                    ui.label(text(&self.title, th.panel_fg).strong());
                    let filter = self.nav.filter();
                    if self.nav.filtering() {
                        let at = filter
                            .char_indices()
                            .nth(self.nav.cursor())
                            .map_or(filter.len(), |(i, _)| i);
                        let (before, after) = filter.split_at(at);
                        ui.label(text(&format!("/{before}▏{after}"), th.panel_fg));
                    } else if !filter.is_empty() {
                        ui.label(text(&format!("/{filter}"), th.panel_dim));
                    } else {
                        ui.label(text("j/k move  / filter  Esc close", th.panel_dim));
                    }
                });
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
                            match &self.lines[i] {
                                HelpLine::Heading(h) => {
                                    if row > 0 {
                                        ui.add_space(6.0);
                                    }
                                    ui.label(text(h, th.panel_fg).strong().underline());
                                }
                                HelpLine::Binding { keys, text: t } => {
                                    ui.horizontal(|ui| {
                                        let pad = KEYS_COLS.saturating_sub(keys.chars().count());
                                        let k = format!("{keys}{}", " ".repeat(pad));
                                        ui.label(text(&k, th.panel_fg).strong());
                                        ui.add(egui::Label::new(text(t, th.panel_dim)).wrap());
                                    });
                                }
                                HelpLine::Text(t) => {
                                    ui.add(
                                        egui::Label::new(text(t, th.panel_dim).italics()).wrap(),
                                    );
                                }
                            }
                        });
                        if selected {
                            r.response.scroll_to_me(None);
                        }
                    }
                });
            });
    }
}
