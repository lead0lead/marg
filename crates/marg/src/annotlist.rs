//! The annotation list: a side panel with every annotation (§2.10).

use eframe::egui::{self, Color32, Event};
use marg_core::annotlist::{CategoryFilter, Entry, filter};
use marg_core::keys::KeyCode;
use marg_core::list::{ListAction, ListNav};

use crate::input;
use crate::prompt;
use crate::theme::Theme;

/// Characters of marked text shown per row.
const TEXT_CHARS: usize = 80;

pub enum Action {
    None,
    /// Go to the annotation and select it (closes the list).
    Jump {
        page: usize,
        xref: i32,
    },
    /// Go to it and edit its comment (closes the list).
    Edit {
        page: usize,
        xref: i32,
    },
    Delete {
        page: usize,
        xref: i32,
    },
    Close,
}

pub struct AnnotList {
    entries: Vec<Entry>,
    nav: ListNav,
    /// Indices into `entries` that pass the filters, in order.
    shown: Vec<usize>,
    category: CategoryFilter,
    half_page: usize,
    /// The annotations or page texts changed: rebuild the rows.
    pub dirty: bool,
}

impl AnnotList {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            nav: ListNav::default(),
            shown: Vec::new(),
            category: CategoryFilter::All,
            half_page: 1,
            dirty: true,
        }
    }

    /// New rows; the selected annotation stays selected if it still exists.
    pub fn set_entries(&mut self, entries: Vec<Entry>) {
        let selected = self.selected().map(|e| (e.page, e.xref));
        self.entries = entries;
        self.dirty = false;
        self.refilter();
        if let Some(row) = selected.and_then(|(p, x)| {
            self.shown
                .iter()
                .position(|&i| self.entries[i].page == p && self.entries[i].xref == x)
        }) {
            self.nav.select(row);
        }
    }

    fn refilter(&mut self) {
        self.shown = filter(&self.entries, self.nav.filter(), self.category);
        self.nav.set_len(self.shown.len());
    }

    fn selected(&self) -> Option<&Entry> {
        let row = self.nav.selected()?;
        self.entries.get(*self.shown.get(row)?)
    }

    /// Moving and filtering as in `marg_core::list`; outside the filter,
    /// `e` edits, `x` deletes, `1`–`9` show one category, `0` all, `A`
    /// closes.
    pub fn event(&mut self, e: &Event, categories: usize) -> Action {
        let at = |e: &Entry| (e.page, e.xref);
        let action = match e {
            Event::Text(t) if self.nav.filtering() => self.nav.text(t),
            _ => {
                let Some(k) = input::key_press(e) else {
                    return Action::None;
                };
                match k.code {
                    KeyCode::Char(c) if !self.nav.filtering() && !k.ctrl && !k.alt => match c {
                        'e' | 'x' => {
                            let Some((page, xref)) = self.selected().map(at) else {
                                return Action::None;
                            };
                            return if c == 'e' {
                                Action::Edit { page, xref }
                            } else {
                                Action::Delete { page, xref }
                            };
                        }
                        'A' => return Action::Close,
                        '0' => {
                            self.category = CategoryFilter::All;
                            self.refilter();
                            return Action::None;
                        }
                        d @ '1'..='9' => {
                            let i = d as usize - '1' as usize;
                            if i < categories {
                                self.category = CategoryFilter::Only(i);
                                self.refilter();
                            }
                            return Action::None;
                        }
                        _ => self.nav.key(k, self.half_page),
                    },
                    _ => self.nav.key(k, self.half_page),
                }
            }
        };
        match action {
            ListAction::None => Action::None,
            ListAction::FilterChanged => {
                self.refilter();
                Action::None
            }
            ListAction::Open => match self.selected().map(at) {
                Some((page, xref)) => Action::Jump { page, xref },
                None => Action::None,
            },
            ListAction::Close => Action::Close,
        }
    }

    /// Draw the rows. `names`: category names; `labels`: page labels.
    pub fn draw(
        &mut self,
        ui: &mut egui::Ui,
        th: &Theme,
        names: &[String],
        labels: Option<&[String]>,
    ) {
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        egui::Frame::new()
            .fill(th.panel_bg)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                let filter = self.nav.filter();
                let shown_cat = match self.category {
                    CategoryFilter::All => String::new(),
                    CategoryFilter::Only(c) => {
                        format!("[{}] ", names.get(c).map_or("?", |n| n.as_str()))
                    }
                };
                if self.nav.filtering() {
                    let prefix = format!("{shown_cat}/");
                    prompt::show(ui, &prefix, filter, self.nav.cursor(), th.panel_fg);
                } else if !filter.is_empty() || !shown_cat.is_empty() {
                    let f = if filter.is_empty() {
                        String::new()
                    } else {
                        format!("/{filter}")
                    };
                    ui.label(text(&format!("{shown_cat}{f}"), th.panel_dim));
                } else {
                    ui.label(text(
                        "/ filter  1-5 category  0 all  ⏎ go  e edit  x delete",
                        th.panel_dim,
                    ));
                }
                ui.add_space(6.0);
                let row_h = 2.0 * ui.text_style_height(&egui::TextStyle::Monospace) + 8.0;
                self.half_page = ((ui.available_height() / row_h) as usize / 2).max(1);
                if self.entries.is_empty() {
                    ui.label(text("No annotations", th.panel_dim));
                    return;
                }
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
                            .inner_margin(egui::Margin::symmetric(6, 3));
                        let r = frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let swatch = e
                                    .color
                                    .map_or(th.panel_dim, |[r, g, b]| Color32::from_rgb(r, g, b));
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(10.0, 10.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 2.0, swatch);
                                let page = labels
                                    .and_then(|l| l.get(e.page))
                                    .cloned()
                                    .unwrap_or_else(|| (e.page + 1).to_string());
                                let cat = e
                                    .category
                                    .and_then(|c| names.get(c))
                                    .map_or("Uncategorised", |n| n.as_str());
                                ui.label(text(&format!("p. {page}  {cat}"), th.panel_dim));
                            });
                            let mut t: String = e.text.chars().take(TEXT_CHARS).collect();
                            if e.text.chars().count() > TEXT_CHARS {
                                t.push('…');
                            }
                            if !t.is_empty() {
                                ui.add(egui::Label::new(text(&t, th.panel_fg)).wrap());
                            }
                            if !e.comment.is_empty() {
                                let c = e.comment.replace('\n', " ");
                                ui.add(egui::Label::new(text(&c, th.panel_dim).italics()).wrap());
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
