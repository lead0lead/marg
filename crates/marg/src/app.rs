//! The marg window: page view, status bar and command line.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Event, MouseWheelUnit};
use marg_core::command::{Command, parse_ex};
use marg_core::config::Config;
use marg_core::keymap::{Keymap, Matcher, Outcome};
use marg_core::keys::{KeyCode, KeyPress, Named};
use marg_core::lineedit::LineEdit;
use marg_core::view::{Fit, View};
use marg_pdf::{DocInfo, DocThread, RenderKey, Request, Response};

use crate::complete;
use crate::input;
use crate::pages::PageCache;

const BACKGROUND: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
const STATUS_BG: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const STATUS_FG: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);
const WARN_FG: Color32 = Color32::from_rgb(0xFF, 0xC0, 0x60);
const ERROR_FG: Color32 = Color32::from_rgb(0xFF, 0x70, 0x70);

/// A successfully opened document.
struct OpenDoc {
    info: DocInfo,
    generation: u64,
    file_name: String,
    view: View,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Info,
    Warn,
    Error,
}

struct Message {
    text: String,
    level: Level,
    until: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Command,
}

pub struct MargApp {
    config: Config,
    version: String,
    keymap: Keymap,
    matcher: Matcher,
    clock: Instant,
    mode: Mode,
    cmdline: LineEdit,
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

pub struct Startup {
    pub config: Config,
    pub version: String,
    pub path: PathBuf,
    pub start_page: Option<usize>,
    /// Problems found before the window existed (e.g. config errors).
    pub errors: Vec<String>,
}

impl MargApp {
    pub fn new(cc: &eframe::CreationContext<'_>, s: Startup) -> Self {
        // Ctrl +/- are not egui's UI zoom in marg.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let ctx = cc.egui_ctx.clone();
        let doc = DocThread::spawn(move || ctx.request_repaint());
        doc.send(Request::Open(s.path));
        let (keymap, warnings) = Keymap::normal(&s.config.keys.normal);
        for w in &warnings {
            log::warn!("{w}");
        }
        let mut app = Self {
            cache: PageCache::new(&s.config.render),
            config: s.config,
            version: s.version,
            keymap,
            matcher: Matcher::default(),
            clock: Instant::now(),
            mode: Mode::Normal,
            cmdline: LineEdit::default(),
            doc,
            open: None,
            start_page: s.start_page,
            received: 0,
            last_request: Vec::new(),
            message: None,
        };
        if let Some(e) = s.errors.first() {
            app.show(Level::Error, e.clone());
        } else if let Some(w) = warnings.first() {
            app.show(Level::Warn, w.clone());
        }
        app
    }

    fn show(&mut self, level: Level, text: String) {
        let secs = Duration::from_secs_f32(self.config.ui.message_secs);
        self.message = Some(Message {
            text,
            level,
            until: Instant::now() + secs,
        });
    }

    fn error(&mut self, text: String) {
        log::error!("{text}");
        self.show(Level::Error, text);
    }

    fn now_ms(&self) -> u64 {
        self.clock.elapsed().as_millis() as u64
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
                        let doc = self.open_doc(info, generation);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} — marg",
                            doc.file_name
                        )));
                        self.open = Some(doc);
                    }
                    Err(e) => self.error(format!("Cannot open: {e}")),
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
        let viewport = self
            .open
            .as_ref()
            .map_or((800.0, 600.0), |o| o.view.viewport());
        let mut view = View::new(sizes, &self.config.view, fit, viewport);
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

    /// Process this frame's input. `area` is the page view on screen; zoom
    /// anchors are relative to it.
    fn handle_input(&mut self, ctx: &egui::Context, area: egui::Rect) {
        let events = ctx.input(|i| i.events.clone());
        let pointer = ctx.input(|i| i.pointer.hover_pos()).map(|p| p - area.min);
        let mut skip_text = false;
        for e in &events {
            match e {
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } => self.wheel(
                    *unit,
                    *delta,
                    input::wheel_zooms(modifiers),
                    modifiers.shift,
                    pointer,
                ),
                Event::Zoom(factor) => {
                    if let Some(open) = &mut self.open {
                        let anchor = pointer.map_or((0.0, 0.0), |p| (p.x, p.y));
                        open.view.set_zoom_at(open.view.zoom() * factor, anchor);
                    }
                }
                _ if self.mode == Mode::Command => self.cmdline_event(ctx, e),
                Event::Text(_) if std::mem::take(&mut skip_text) => {}
                _ => {
                    if let Some(k) = input::key_press(e) {
                        // An Alt key event may be followed by its text event.
                        skip_text = k.alt && !k.ctrl;
                        self.normal_key(ctx, k);
                    }
                }
            }
        }
        if self
            .matcher
            .expire(self.now_ms(), self.config.ui.sequence_timeout_ms)
        {
            ctx.request_repaint();
        }
        if self.matcher.is_pending() {
            ctx.request_repaint_after(Duration::from_millis(self.config.ui.sequence_timeout_ms));
        }
    }

    fn wheel(
        &mut self,
        unit: MouseWheelUnit,
        delta: egui::Vec2,
        zoom: bool,
        horizontal: bool,
        pointer: Option<egui::Vec2>,
    ) {
        let Some(open) = &mut self.open else { return };
        let v = &mut open.view;
        let step = self.config.view.scroll_step_px;
        let lines = match unit {
            MouseWheelUnit::Line => delta,
            MouseWheelUnit::Point => delta / step,
            MouseWheelUnit::Page => delta * v.viewport().1 / step,
        };
        if zoom {
            let anchor = pointer.map_or((0.0, 0.0), |p| (p.x, p.y));
            v.set_zoom_at(v.zoom() * self.config.view.zoom_step.powf(lines.y), anchor);
        } else if horizontal && lines.x == 0.0 {
            v.scroll_by(-lines.y * step, 0.0);
        } else {
            v.scroll_by(-lines.x * step, -lines.y * step);
        }
    }

    fn normal_key(&mut self, ctx: &egui::Context, key: KeyPress) {
        let now = self.now_ms();
        let timeout = self.config.ui.sequence_timeout_ms;
        match self.matcher.feed(&self.keymap, key, now, timeout) {
            Outcome::Run { command, count } => self.run(ctx, command, count),
            Outcome::Pending | Outcome::Unbound | Outcome::Cancelled => {}
        }
    }

    fn cmdline_event(&mut self, ctx: &egui::Context, e: &Event) {
        match e {
            Event::Text(t) => self.cmdline.insert(t),
            Event::Key { pressed: true, .. } => {
                let Some(k) = input::key_press(e) else { return };
                match k.code {
                    KeyCode::Named(Named::Esc) => self.mode = Mode::Normal,
                    KeyCode::Named(Named::Enter) => {
                        self.mode = Mode::Normal;
                        let line = self.cmdline.submit();
                        match parse_ex(&line) {
                            Ok(c) => self.run(ctx, c, None),
                            Err(e) => self.error(e),
                        }
                    }
                    KeyCode::Named(Named::Backspace) if self.cmdline.is_empty() => {
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Named(Named::Backspace) => self.cmdline.backspace(),
                    KeyCode::Named(Named::Delete) => self.cmdline.delete(),
                    KeyCode::Named(Named::Left) => self.cmdline.left(),
                    KeyCode::Named(Named::Right) => self.cmdline.right(),
                    KeyCode::Named(Named::Home) => self.cmdline.home(),
                    KeyCode::Named(Named::End) => self.cmdline.end(),
                    KeyCode::Named(Named::Up) => self.cmdline.history_prev(),
                    KeyCode::Named(Named::Down) => self.cmdline.history_next(),
                    KeyCode::Named(Named::Tab) => self.complete(),
                    KeyCode::Named(Named::Space) => self.cmdline.insert(" "),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Tab completion of the path in `:e <path>`.
    fn complete(&mut self) {
        let Some(arg) = self.cmdline.text().strip_prefix("e ") else {
            return;
        };
        let c = complete::path(arg);
        self.cmdline.set(&format!("e {}", c.text));
        if c.candidates.len() > 1 {
            let mut list = c.candidates.join("  ");
            if list.len() > 200 {
                list.truncate(list.floor_char_boundary(200));
                list.push('…');
            }
            self.show(Level::Info, list);
        }
    }

    fn run(&mut self, ctx: &egui::Context, command: Command, count: Option<u32>) {
        let n = count.unwrap_or(1).max(1);
        match command {
            Command::CommandLine => {
                self.mode = Mode::Command;
                self.cmdline.clear();
                return;
            }
            Command::Quit | Command::QuitForce => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            Command::Open(path) => {
                self.open_path(&path);
                return;
            }
            Command::Version => {
                let v = self.version.clone();
                self.show(Level::Info, v);
                return;
            }
            Command::DebugToggle => {
                self.show(Level::Info, "Debug overlay not built yet".into());
                return;
            }
            _ => {}
        }
        let Some(open) = &mut self.open else { return };
        let v = &mut open.view;
        let cfg = &self.config.view;
        let step = cfg.scroll_step_px * n as f32;
        let page_of = |c: u32| c as usize - 1;
        match command {
            Command::ScrollDown => v.scroll_by(0.0, step),
            Command::ScrollUp => v.scroll_by(0.0, -step),
            Command::ScrollLeft => v.scroll_by(-step, 0.0),
            Command::ScrollRight => v.scroll_by(step, 0.0),
            Command::HalfPageDown => v.scroll_viewports(0.5 * n as f32),
            Command::HalfPageUp => v.scroll_viewports(-0.5 * n as f32),
            Command::PageNext => v.page_step(n as isize),
            Command::PagePrev => v.page_step(-(n as isize)),
            Command::PageFirst => v.goto_page(count.map_or(0, page_of)),
            Command::PageLast => v.goto_page(count.map_or(usize::MAX, page_of)),
            Command::PageGoto(p) => v.goto_page(p.saturating_sub(1)),
            Command::ZoomIn => v.zoom_by(cfg.zoom_step.powi(n as i32)),
            Command::ZoomOut => v.zoom_by(cfg.zoom_step.powi(-(n as i32))),
            Command::ZoomReset => {
                let c = v.viewport();
                v.set_zoom_at(1.0, (c.0 / 2.0, 0.0));
            }
            Command::FitWidth => v.set_fit(Fit::Width),
            Command::FitPage => v.set_fit(Fit::Page),
            Command::CommandLine
            | Command::Quit
            | Command::QuitForce
            | Command::Open(_)
            | Command::Version
            | Command::DebugToggle => {}
        }
    }

    fn open_path(&mut self, path: &str) {
        let path = complete::expand_tilde(path);
        if !Path::new(&path).is_file() {
            self.error(format!("No such file: {}", path.display()));
            return;
        }
        // Phase 1 has nothing to save before switching files.
        self.start_page = None;
        self.doc.send(Request::Open(path));
    }

    fn status_left(&self) -> String {
        let Some(open) = &self.open else {
            return "NORMAL".into();
        };
        let v = &open.view;
        let page = v.current_page();
        let count = v.page_count();
        let page_text = match &open.info.labels {
            Some(labels) if labels[page] != (page + 1).to_string() => {
                format!("{} ({}/{count})", labels[page], page + 1)
            }
            _ => format!("{}/{count}", page + 1),
        };
        format!(
            "NORMAL  {}  {page_text}  {:.0}%  continuous",
            open.file_name,
            v.zoom() * 100.0
        )
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        ui.horizontal_centered(|ui| {
            if self.mode == Mode::Command {
                let line = self.cmdline.text();
                let at = line
                    .char_indices()
                    .nth(self.cmdline.cursor())
                    .map_or(line.len(), |(i, _)| i);
                let (before, after) = line.split_at(at);
                ui.label(text(&format!(":{before}▏{after}"), STATUS_FG));
            } else {
                ui.label(text(&self.status_left(), STATUS_FG));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(m) = &self.message {
                    let color = match m.level {
                        Level::Info => STATUS_FG,
                        Level::Warn => WARN_FG,
                        Level::Error => ERROR_FG,
                    };
                    ui.label(text(&m.text, color));
                } else if self.matcher.is_pending() {
                    ui.label(text(&self.matcher.pending_text(), STATUS_FG));
                }
            });
        });
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

        // Input first, so the status bar and pages show its effect this frame.
        let status_h = self.config.ui.status_bar_px;
        let screen = ctx.content_rect();
        let area = egui::Rect::from_min_max(
            screen.min,
            egui::pos2(screen.max.x, screen.max.y - status_h),
        );
        self.handle_input(&ctx, area);

        egui::Panel::bottom("status")
            .exact_size(status_h)
            .frame(
                egui::Frame::new()
                    .fill(STATUS_BG)
                    .inner_margin(egui::Margin::symmetric(6, 0)),
            )
            .show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND))
            .show(ui, |ui| {
                let area = ui.max_rect();
                let ppp = ui.ctx().pixels_per_point();
                let Some(open) = &mut self.open else { return };
                open.view.set_viewport(area.width(), area.height());
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
