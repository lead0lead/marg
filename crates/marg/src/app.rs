//! The marg window: page view and status bar.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Event, Key, Modifiers, MouseWheelUnit};
use marg_core::config::Config;
use marg_core::view::{Fit, View};
use marg_pdf::{DocInfo, DocThread, RenderKey, Request, Response};

use crate::pages::PageCache;

const BACKGROUND: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
const STATUS_BG: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const STATUS_FG: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);

/// A successfully opened document.
struct OpenDoc {
    info: DocInfo,
    generation: u64,
    file_name: String,
    view: View,
}

struct Message {
    text: String,
    until: Instant,
}

pub struct MargApp {
    config: Config,
    doc: DocThread,
    open: Option<OpenDoc>,
    /// Page to show once the pending document has opened (0-based).
    start_page: Option<usize>,
    cache: PageCache,
    /// `Rendered` responses received, for the document thread's bookkeeping.
    received: u64,
    last_request: Vec<RenderKey>,
    message: Option<Message>,
}

impl MargApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        path: PathBuf,
        start_page: Option<usize>,
        message: Option<String>,
    ) -> Self {
        // Ctrl +/- are marg's zoom keys, not egui's UI zoom.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let ctx = cc.egui_ctx.clone();
        let doc = DocThread::spawn(move || ctx.request_repaint());
        doc.send(Request::Open(path));
        let mut app = Self {
            cache: PageCache::new(&config.render),
            config,
            doc,
            open: None,
            start_page,
            received: 0,
            last_request: Vec::new(),
            message: None,
        };
        if let Some(m) = message {
            app.show_message(m);
        }
        app
    }

    fn show_message(&mut self, text: String) {
        let secs = Duration::from_secs_f32(self.config.ui.message_secs);
        self.message = Some(Message {
            text,
            until: Instant::now() + secs,
        });
    }

    fn handle_responses(&mut self, ctx: &egui::Context) {
        while let Some(resp) = self.doc.try_recv() {
            match resp {
                Response::Opened {
                    generation,
                    result,
                    took,
                } => match result {
                    Ok(info) => {
                        log::info!("opened {} in {took:?}", info.path.display());
                        self.cache.clear();
                        self.last_request.clear();
                        self.open = Some(self.open_doc(info, generation));
                    }
                    Err(e) => {
                        log::error!("open failed: {e}");
                        self.show_message(format!("Cannot open: {e}"));
                    }
                },
                Response::Rendered {
                    generation,
                    key,
                    result,
                    took,
                } => {
                    self.received += 1;
                    let Some(open) = &self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    match result {
                        Ok(px) => {
                            log::debug!("rendered {key:?} in {took:?}");
                            self.cache.insert(ctx, key, px);
                        }
                        Err(e) => log::error!("render {key:?} failed: {e}"),
                    }
                }
            }
        }
    }

    fn open_doc(&mut self, info: DocInfo, generation: u64) -> OpenDoc {
        let sizes: Vec<(f32, f32)> = info
            .page_sizes
            .iter()
            .map(|s| (s.width, s.height))
            .collect();
        // First page landscape: fit the page (slides); else fit the width.
        let fit = if sizes[0].0 > sizes[0].1 {
            Fit::Page
        } else {
            Fit::Width
        };
        let mut view = View::new(sizes, &self.config.view, fit, (800.0, 600.0));
        if let Some(p) = self.start_page.take() {
            view.goto_page(p);
        }
        let file_name = info
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        OpenDoc {
            info,
            generation,
            file_name,
            view,
        }
    }

    /// Temporary fixed keys until the keymap exists (Phase 1 step 5).
    /// `area` is the page view on screen; zoom anchors are relative to it.
    fn handle_input(&mut self, ctx: &egui::Context, area: egui::Rect) {
        let events = ctx.input(|i| i.events.clone());
        let pointer = ctx.input(|i| i.pointer.hover_pos()).map(|p| p - area.min);
        let Some(open) = &mut self.open else {
            if events
                .iter()
                .any(|e| matches!(e, Event::Text(t) if t == "q"))
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        };
        let v = &mut open.view;
        let cfg = &self.config.view;
        let step = cfg.scroll_step_px;
        for e in events {
            match e {
                Event::Text(t) => match t.as_str() {
                    "j" => v.scroll_by(0.0, step),
                    "k" => v.scroll_by(0.0, -step),
                    "h" => v.scroll_by(-step, 0.0),
                    "l" => v.scroll_by(step, 0.0),
                    "J" | " " => v.page_step(1),
                    "K" => v.page_step(-1),
                    "G" => v.goto_page(usize::MAX),
                    "+" => v.zoom_by(cfg.zoom_step),
                    "-" => v.zoom_by(1.0 / cfg.zoom_step),
                    "=" => v.set_zoom_at(1.0, (0.0, 0.0)),
                    "s" => v.set_fit(Fit::Width),
                    "a" => v.set_fit(Fit::Page),
                    "q" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    _ => {}
                },
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.matches_exact(Modifiers::CTRL) => match key {
                    Key::D => v.scroll_viewports(0.5),
                    Key::U => v.scroll_viewports(-0.5),
                    _ => {}
                },
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } => {
                    let lines = match unit {
                        MouseWheelUnit::Line => delta,
                        MouseWheelUnit::Point => delta / step,
                        MouseWheelUnit::Page => delta * v.viewport().1 / step,
                    };
                    if modifiers.command {
                        let anchor = pointer.map_or((0.0, 0.0), |p| (p.x, p.y));
                        v.set_zoom_at(v.zoom() * cfg.zoom_step.powf(lines.y), anchor);
                    } else if modifiers.shift && lines.x == 0.0 {
                        v.scroll_by(-lines.y * step, 0.0);
                    } else {
                        v.scroll_by(-lines.x * step, -lines.y * step);
                    }
                }
                Event::Zoom(factor) => {
                    let anchor = pointer.map_or((0.0, 0.0), |p| (p.x, p.y));
                    v.set_zoom_at(v.zoom() * factor, anchor);
                }
                _ => {}
            }
        }
    }

    fn status_line(&self) -> String {
        let Some(open) = &self.open else {
            return "NORMAL".into();
        };
        let v = &open.view;
        let page = v.current_page();
        let count = v.page_count();
        let page_text = match &open.info.labels {
            Some(labels) => format!("{} ({}/{count})", labels[page], page + 1),
            None => format!("{}/{count}", page + 1),
        };
        format!(
            "NORMAL  {}  {page_text}  {:.0}%  continuous",
            open.file_name,
            v.zoom() * 100.0
        )
    }
}

impl eframe::App for MargApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_responses(&ctx);
        if self
            .message
            .as_ref()
            .is_some_and(|m| Instant::now() >= m.until)
        {
            self.message = None;
        }
        if let Some(m) = &self.message {
            ctx.request_repaint_after(m.until.saturating_duration_since(Instant::now()));
        }

        egui::Panel::bottom("status")
            .exact_size(self.config.ui.status_bar_px)
            .frame(egui::Frame::new().fill(STATUS_BG).inner_margin(4))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        egui::RichText::new(self.status_line())
                            .monospace()
                            .color(STATUS_FG),
                    );
                    if let Some(m) = &self.message {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(&m.text).monospace().color(STATUS_FG));
                        });
                    }
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND))
            .show(ui, |ui| {
                let area = ui.max_rect();
                let ppp = ui.ctx().pixels_per_point();
                if let Some(open) = &mut self.open {
                    open.view.set_viewport(area.width(), area.height());
                }
                self.handle_input(&ctx, area);
                let Some(open) = &mut self.open else { return };
                let plan = self.cache.plan(&open.view, &self.config.render, ppp);
                if plan.keys != self.last_request {
                    self.doc.send(Request::Render {
                        keys: plan.keys.clone(),
                        received: self.received,
                    });
                    self.last_request = plan.keys;
                }
                let painter = ui.painter_at(area);
                self.cache
                    .draw(&painter, area, &open.view, &self.config.render);
            });
    }
}
