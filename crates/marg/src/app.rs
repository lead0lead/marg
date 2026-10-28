//! The marg window: page view, status bar and command line.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Event, MouseWheelUnit};
use marg_core::annot::{AnnotKind, AnnotSpec, parse_color};
use marg_core::caret::{self, Motion, Selection, TextPos};
use marg_core::command::{Command, parse_ex};
use marg_core::config::Config;
use marg_core::keymap::{KeyMode, Keymap, Matcher, Outcome};
use marg_core::keys::{KeyCode, KeyPress, Named};
use marg_core::lineedit::LineEdit;
use marg_core::state::{FileState, State, file_key};
use marg_core::stats::Rolling;
use marg_core::text::{BBox, PageText};
use marg_core::view::{Fit, View};
use marg_pdf::{DocInfo, DocThread, Edit, RenderKey, Request, Response, Scale};

use crate::complete;
use crate::input;
use crate::pages::PageCache;
use crate::recent::{self, RecentList};
use crate::select::{self, Caret, Hinting, Purpose};
use crate::statefile;

const BACKGROUND: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
const STATUS_BG: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
const STATUS_FG: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);
const WARN_FG: Color32 = Color32::from_rgb(0xFF, 0xC0, 0x60);
const ERROR_FG: Color32 = Color32::from_rgb(0xFF, 0x70, 0x70);

/// A successfully opened document.
struct OpenDoc {
    info: DocInfo,
    /// Key in the state file.
    key: String,
    generation: u64,
    file_name: String,
    view: View,
    /// Text of each page, once extracted.
    texts: Vec<Option<PageText>>,
    /// Pages whose text has been requested.
    text_requested: HashSet<usize>,
    /// Annotation edits per page, as confirmed by the document thread.
    revs: Vec<u32>,
    /// Annotations drawn by the UI until the page's new render arrives.
    provisional: Vec<Provisional>,
    /// Annotation changes not saved yet.
    unsaved: bool,
}

/// An annotation drawn by the UI while the document thread creates it and
/// re-renders its page.
struct Provisional {
    tag: u64,
    /// The page revision with the annotation, once confirmed.
    rev: Option<u32>,
    spec: AnnotSpec,
}

impl OpenDoc {
    fn request_text(&mut self, doc: &DocThread, page: usize) {
        if page < self.texts.len() && self.texts[page].is_none() && self.text_requested.insert(page)
        {
            doc.send(Request::Text(page));
        }
    }

    /// Ask for the text of the visible pages that do not have it yet.
    fn request_visible_text(&mut self, doc: &DocThread) {
        for p in self.view.visible_pages() {
            self.request_text(doc, p);
        }
    }
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
    Recent,
    Hint,
    Caret,
    Visual,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Normal | Mode::Command => "NORMAL",
            Mode::Recent => "RECENT",
            Mode::Hint => "HINT",
            Mode::Caret => "CARET",
            Mode::Visual => "VISUAL",
        }
    }
}

pub struct MargApp {
    config: Config,
    version: String,
    keymap: Keymap,
    caret_map: Keymap,
    visual_map: Keymap,
    matcher: Matcher,
    clock: Instant,
    mode: Mode,
    /// Mode to return to from the command line or hints.
    return_mode: Mode,
    /// The page view on screen, as of the last input.
    area: egui::Rect,
    caret: Option<Caret>,
    selection: Option<Selection>,
    hinting: Option<Hinting>,
    /// The selection before hints started, restored if they are cancelled.
    selection_before_hints: Option<Selection>,
    /// Where a mouse drag started, while the primary button is down.
    drag_anchor: Option<TextPos>,
    /// Last primary press: time, position, click count (1–3).
    last_click: Option<(Instant, egui::Pos2, u8)>,
    /// Category used by the last highlight; underlines use it.
    last_category: u8,
    /// Tag of the next annotation edit.
    next_tag: u64,
    /// Annotation author.
    author: String,
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
    state_path: Option<PathBuf>,
    state: State,
    recent: Option<RecentList>,
    debug: bool,
    /// Time spent in `ui()` per frame, in ms.
    frame_ms: Rolling,
    /// Last render time and scale per page, for the debug overlay.
    last_render: HashMap<usize, (Duration, Scale)>,
    timing: Option<Timing>,
}

/// Startup phase logging (`MARG_TIMING=1`).
struct Timing {
    start: Instant,
    opened: bool,
    shown: bool,
}

impl Timing {
    fn log(&self, phase: &str) {
        log::info!(
            "timing: {phase} at {:.1} ms",
            self.start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

pub struct Startup {
    pub config: Config,
    /// Started before the window, so the file opens while the window is
    /// being created.
    pub doc: DocThread,
    /// Set once the window exists; the document thread's wake-up uses it.
    pub repaint: Arc<OnceLock<egui::Context>>,
    pub version: String,
    /// The file being opened; `None`: show the recent files list.
    pub path: Option<PathBuf>,
    pub start_page: Option<usize>,
    pub state_path: Option<PathBuf>,
    /// Problems found before the window existed (e.g. config errors).
    pub errors: Vec<String>,
    /// Process start, when `MARG_TIMING` is set.
    pub timing_start: Option<Instant>,
}

impl MargApp {
    pub fn new(cc: &eframe::CreationContext<'_>, s: Startup) -> Self {
        // Ctrl +/- are not egui's UI zoom in marg.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let _ = s.repaint.set(cc.egui_ctx.clone());
        let doc = s.doc;
        let (keymap, mut warnings) = Keymap::build(KeyMode::Normal, &s.config.keys);
        let (caret_map, w) = Keymap::build(KeyMode::Caret, &s.config.keys);
        warnings.extend(w);
        let (visual_map, w) = Keymap::build(KeyMode::Visual, &s.config.keys);
        warnings.extend(w);
        for w in &warnings {
            log::warn!("{w}");
        }
        let mut errors = s.errors;
        let state = match s.state_path.as_deref().map(statefile::load) {
            Some(Ok(state)) => state,
            Some(Err(e)) => {
                errors.push(format!("state file: {e}"));
                State::default()
            }
            None => State::default(),
        };
        let (mode, recent) = match s.path {
            Some(_) => (Mode::Normal, None),
            None => (
                Mode::Recent,
                Some(RecentList::new(&state, s.config.ui.recent_files_max)),
            ),
        };
        let author = annotation_author(&s.config.annotation.author);
        let mut app = Self {
            cache: PageCache::new(&s.config.render),
            config: s.config,
            version: s.version,
            keymap,
            caret_map,
            visual_map,
            matcher: Matcher::default(),
            clock: Instant::now(),
            mode,
            return_mode: Mode::Normal,
            area: egui::Rect::NOTHING,
            caret: None,
            selection: None,
            hinting: None,
            selection_before_hints: None,
            drag_anchor: None,
            last_click: None,
            last_category: 1,
            next_tag: 1,
            author,
            cmdline: LineEdit::default(),
            doc,
            open: None,
            start_page: s.start_page,
            received: 0,
            last_request: Vec::new(),
            message: None,
            state_path: s.state_path,
            state,
            recent,
            debug: false,
            frame_ms: Rolling::new(120),
            last_render: HashMap::new(),
            timing: s.timing_start.map(|start| Timing {
                start,
                opened: false,
                shown: false,
            }),
        };
        if let Some(t) = &app.timing {
            t.log("window created");
        }
        for e in &errors {
            log::error!("{e}");
        }
        if let Some(e) = errors.first() {
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
                        if let Some(t) = &mut self.timing
                            && !t.opened
                        {
                            t.opened = true;
                            t.log(&format!("document opened (open took {took:?})"));
                        }
                        self.save_state();
                        self.cache.clear();
                        self.last_request.clear();
                        self.mode = Mode::Normal;
                        self.recent = None;
                        self.caret = None;
                        self.selection = None;
                        self.hinting = None;
                        let doc = self.open_doc(info, generation);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} — marg",
                            doc.file_name
                        )));
                        self.open = Some(doc);
                        // Record the open now, so the recent list has it even
                        // if marg does not exit cleanly.
                        self.save_state();
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
                            self.last_render.insert(key.page, (took, key.scale));
                            self.cache.insert(ctx, key, px);
                        }
                        Err(e) => log::error!("render {key:?} failed: {e}"),
                    }
                }
                Response::Edited {
                    generation,
                    tag,
                    page,
                    rev,
                    result,
                } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    match result {
                        Ok(_xref) => {
                            open.revs[page] = open.revs[page].max(rev);
                            open.unsaved = true;
                            for p in open.provisional.iter_mut().filter(|p| p.tag == tag) {
                                p.rev = Some(rev);
                            }
                        }
                        Err(e) => {
                            open.provisional.retain(|p| p.tag != tag);
                            self.error(format!("Annotation failed: {e}"));
                        }
                    }
                }
                Response::Text {
                    generation,
                    page,
                    result,
                } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    match result {
                        Ok(text) => open.texts[page] = Some(text),
                        Err(e) => log::error!("text of page {} failed: {e}", page + 1),
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
        let canonical = std::fs::canonicalize(&info.path).unwrap_or_else(|_| info.path.clone());
        let key = file_key(info.id.as_deref(), &canonical.to_string_lossy());
        let saved = self.state.files.get(&key);
        let fit = saved.map_or(fit, |f| f.fit);
        let mut view = View::new(sizes, &self.config.view, fit, viewport);
        if let Some(saved) = saved {
            if saved.fit == Fit::None {
                view.set_zoom_at(saved.zoom, (0.0, 0.0));
            }
            view.set_position(saved.position);
        }
        if let Some(p) = self.start_page.take() {
            view.goto_page(p);
        }
        let file_name = info
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        OpenDoc {
            info: DocInfo {
                path: canonical,
                ..info
            },
            key,
            generation,
            file_name,
            texts: vec![None; view.page_count()],
            text_requested: HashSet::new(),
            revs: vec![0; view.page_count()],
            provisional: Vec::new(),
            unsaved: false,
            view,
        }
    }

    /// Process this frame's input. `area` is the page view on screen; zoom
    /// anchors are relative to it.
    fn handle_input(&mut self, ctx: &egui::Context, area: egui::Rect) {
        self.area = area;
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
                _ if self.mode == Mode::Recent => self.recent_event(ctx, e),
                _ if self.mode == Mode::Hint => self.hint_event(e),
                Event::PointerButton { .. } | Event::PointerMoved(_)
                    if matches!(self.mode, Mode::Normal | Mode::Caret | Mode::Visual) =>
                {
                    self.mouse_event(e)
                }
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
        log::debug!("wheel {unit:?} {delta:?} zoom={zoom}");
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

    fn recent_event(&mut self, ctx: &egui::Context, e: &Event) {
        let Some(list) = &mut self.recent else { return };
        match list.event(e) {
            recent::Action::None => {}
            recent::Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            recent::Action::Open(path) => self.open_path(&path.to_string_lossy()),
        }
    }

    /// Write the open document's reading position to the state file.
    fn save_state(&mut self) {
        let Some(open) = &self.open else { return };
        let file = FileState {
            path: open.info.path.to_string_lossy().into_owned(),
            last_opened: statefile::now_secs(),
            position: open.view.position(),
            zoom: open.view.zoom(),
            fit: open.view.fit(),
            layout: marg_core::config::Layout::Continuous,
        };
        let max = self.config.ui.state_files_max;
        self.state.update(open.key.clone(), file.clone(), max);
        let Some(path) = &self.state_path else { return };
        if let Err(e) = statefile::save_entry(path, &open.key, file, max) {
            self.error(format!("Cannot save state: {e}"));
        }
    }

    fn normal_key(&mut self, ctx: &egui::Context, key: KeyPress) {
        let now = self.now_ms();
        let timeout = self.config.ui.sequence_timeout_ms;
        let map = match self.mode {
            Mode::Caret => &self.caret_map,
            Mode::Visual => &self.visual_map,
            _ => &self.keymap,
        };
        match self.matcher.feed(map, key, now, timeout) {
            Outcome::Run { command, count } => self.run(ctx, command, count),
            Outcome::Unbound if key == KeyPress::named(Named::Esc) => {
                if matches!(self.mode, Mode::Caret | Mode::Visual) {
                    self.leave_text_modes();
                }
            }
            Outcome::Pending | Outcome::Unbound | Outcome::Cancelled => {}
        }
    }

    /// Back to Normal mode: no caret, no selection.
    fn leave_text_modes(&mut self) {
        self.mode = Mode::Normal;
        self.caret = None;
        self.selection = None;
    }

    fn start_hints(&mut self, purpose: Purpose) {
        let Some(open) = &self.open else { return };
        match Hinting::start(
            purpose,
            &open.view,
            self.area,
            &open.texts,
            &self.config.hints,
        ) {
            Ok(h) => {
                if self.mode != Mode::Hint {
                    self.return_mode = self.mode;
                    self.selection_before_hints = self.selection;
                }
                self.hinting = Some(h);
                self.mode = Mode::Hint;
            }
            Err(msg) => {
                if self.mode == Mode::Hint {
                    self.cancel_hints();
                }
                self.show(Level::Info, msg.into());
            }
        }
    }

    fn cancel_hints(&mut self) {
        self.hinting = None;
        self.mode = self.return_mode;
        self.selection = self.selection_before_hints.take();
    }

    fn hint_event(&mut self, e: &Event) {
        let (Some(open), Some(h)) = (&self.open, &mut self.hinting) else {
            return;
        };
        match e {
            Event::Text(t) => {
                let picked = t
                    .chars()
                    .find_map(|c| h.push(c, &open.texts, &self.config.hints));
                if let Some((page, r)) = picked {
                    self.hint_picked(page, r);
                }
            }
            Event::Key { pressed: true, .. } => match input::key_press(e).map(|k| k.code) {
                Some(KeyCode::Named(Named::Esc)) => self.cancel_hints(),
                Some(KeyCode::Named(Named::Backspace)) => {
                    h.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn hint_picked(&mut self, page: usize, word: std::ops::Range<usize>) {
        let Some(h) = self.hinting.take() else { return };
        let start = TextPos::new(page, word.start);
        let end = TextPos::new(page, word.end - 1);
        match h.purpose {
            Purpose::Caret => {
                self.selection = None;
                self.set_caret(start);
                self.mode = Mode::Caret;
            }
            Purpose::SelectStart => {
                // Show the first word while the second is picked.
                self.selection = Some(Selection::new(start, end));
                self.start_hints(Purpose::SelectEnd(start, end));
            }
            Purpose::SelectEnd(s, e) => {
                self.selection = Some(Selection::new(s.min(start), e.max(end)));
                self.set_caret(e.max(end));
                self.mode = Mode::Visual;
            }
        }
    }

    /// Put the caret at `pos` and load the text of the pages around it.
    fn set_caret(&mut self, pos: TextPos) {
        let caret = Caret { pos, want_x: None };
        self.caret = Some(caret);
        if let Some(open) = &mut self.open {
            select::keep_visible(&mut open.view, &caret, &open.texts);
            open.request_text(&self.doc, pos.page.saturating_sub(1));
            open.request_text(&self.doc, pos.page + 1);
        }
    }

    fn move_caret(&mut self, motion: Motion, count: u32) {
        let (Some(open), Some(c)) = (&self.open, self.caret) else {
            return;
        };
        let (pos, want_x) = caret::apply_n(&open.texts, c.pos, motion, c.want_x, count);
        self.set_caret(pos);
        if let Some(c) = &mut self.caret {
            c.want_x = want_x;
        }
        if let Some(sel) = &mut self.selection {
            sel.head = pos;
        }
    }

    /// `v` / `V`: start a selection from Caret mode, switch its kind, or end
    /// it when it already has that kind.
    fn visual(&mut self, linewise: bool) {
        match (self.mode, &mut self.selection) {
            (Mode::Caret, _) => {
                let Some(c) = self.caret else { return };
                self.selection = Some(Selection {
                    linewise,
                    ..Selection::new(c.pos, c.pos)
                });
                self.mode = Mode::Visual;
            }
            (Mode::Visual, Some(sel)) if sel.linewise == linewise => {
                self.selection = None;
                self.mode = Mode::Caret;
            }
            (Mode::Visual, Some(sel)) => sel.linewise = linewise,
            _ => {}
        }
    }

    /// The character under a screen position, if its page's text is loaded
    /// (otherwise the text is requested).
    fn char_at_screen(&mut self, pos: egui::Pos2) -> Option<TextPos> {
        let open = self.open.as_mut()?;
        let rel = pos - self.area.min;
        let d = open.view.doc_pos_at(rel.x, rel.y);
        open.request_text(&self.doc, d.page);
        let t = open.texts[d.page].as_ref()?;
        t.char_at(d.x, d.y).map(|i| TextPos::new(d.page, i))
    }

    /// Mouse selection: drag selects characters, a double click a word, a
    /// triple click a line. The result works like a keyboard selection.
    fn mouse_event(&mut self, e: &Event) {
        match *e {
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } if self.area.contains(pos) => {
                let now = Instant::now();
                let multi = Duration::from_millis(self.config.ui.multi_click_ms);
                let count = match self.last_click {
                    Some((t, p, n)) if now - t < multi && p.distance(pos) < 4.0 => n % 3 + 1,
                    _ => 1,
                };
                self.last_click = Some((now, pos, count));
                let Some(at) = self.char_at_screen(pos) else {
                    self.leave_text_modes();
                    return;
                };
                match count {
                    1 => {
                        self.leave_text_modes();
                        self.drag_anchor = Some(at);
                    }
                    2 => {
                        let Some(t) = self.open.as_ref().and_then(|o| o.texts[at.page].as_ref())
                        else {
                            return;
                        };
                        let word = t
                            .words(t.line_of(at.idx))
                            .into_iter()
                            .find(|w| w.contains(&at.idx));
                        if let Some(w) = word {
                            let (a, b) = (
                                TextPos::new(at.page, w.start),
                                TextPos::new(at.page, w.end - 1),
                            );
                            self.mouse_select(Selection::new(a, b));
                        }
                    }
                    _ => self.mouse_select(Selection {
                        linewise: true,
                        ..Selection::new(at, at)
                    }),
                }
            }
            Event::PointerButton {
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => self.drag_anchor = None,
            Event::PointerMoved(pos) => {
                let Some(anchor) = self.drag_anchor else {
                    return;
                };
                if let Some(at) = self.char_at_screen(pos)
                    && (at != anchor || self.selection.is_some())
                {
                    self.mouse_select(Selection::new(anchor, at));
                }
            }
            _ => {}
        }
    }

    fn mouse_select(&mut self, sel: Selection) {
        self.selection = Some(sel);
        self.caret = Some(Caret {
            pos: sel.head,
            want_x: None,
        });
        self.mode = Mode::Visual;
    }

    /// Highlight or underline the selection in `category` (1-based), one
    /// annotation per page, and leave Visual mode.
    fn annotate(&mut self, kind: AnnotKind, category: u8) {
        let (Some(open), Some(sel)) = (&mut self.open, self.selection) else {
            return;
        };
        let Some(cat) = self.config.categories.get(usize::from(category) - 1) else {
            self.error(format!("No category {category}"));
            return;
        };
        let Some(color) = parse_color(&cat.color) else {
            let msg = format!("Bad color for category {}: {}", cat.name, cat.color);
            self.error(msg);
            return;
        };
        let mut by_page: BTreeMap<usize, Vec<BBox>> = BTreeMap::new();
        for (page, b) in sel.rects(&open.texts) {
            by_page.entry(page).or_default().push(b);
        }
        for (page, rects) in by_page {
            let spec = AnnotSpec {
                page,
                kind,
                rects,
                color,
                opacity: self.config.annotation.highlight_opacity,
                subject: cat.name.clone(),
                author: self.author.clone(),
            };
            let tag = self.next_tag;
            self.next_tag += 1;
            open.provisional.push(Provisional {
                tag,
                rev: None,
                spec: spec.clone(),
            });
            self.doc.send(Request::Edit {
                tag,
                edit: Edit::Create(spec),
            });
        }
        self.last_category = category;
        self.leave_text_modes();
    }

    fn copy_selection(&mut self, ctx: &egui::Context) {
        let (Some(open), Some(sel)) = (&self.open, &self.selection) else {
            return;
        };
        let text = sel.text(&open.texts);
        let n = text.chars().count();
        ctx.copy_text(text);
        self.show(Level::Info, format!("Copied {n} characters"));
    }

    fn cmdline_event(&mut self, ctx: &egui::Context, e: &Event) {
        match e {
            Event::Text(t) => self.cmdline.insert(t),
            Event::Key { pressed: true, .. } => {
                let Some(k) = input::key_press(e) else { return };
                match k.code {
                    KeyCode::Named(Named::Esc) => self.mode = self.return_mode,
                    KeyCode::Named(Named::Enter) => {
                        self.mode = self.return_mode;
                        let line = self.cmdline.submit();
                        match parse_ex(&line) {
                            Ok(c) => self.run(ctx, c, None),
                            Err(e) => self.error(e),
                        }
                    }
                    KeyCode::Named(Named::Backspace) if self.cmdline.is_empty() => {
                        self.mode = self.return_mode;
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
                self.return_mode = self.mode;
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
                self.debug = !self.debug;
                return;
            }
            Command::HintCaret => {
                self.start_hints(Purpose::Caret);
                return;
            }
            Command::HintSelect => {
                self.start_hints(Purpose::SelectStart);
                return;
            }
            Command::Motion(m) => {
                self.move_caret(m, n);
                return;
            }
            Command::VisualChar | Command::VisualLine => {
                self.visual(command == Command::VisualLine);
                return;
            }
            Command::VisualSwap => {
                if let Some(sel) = &mut self.selection {
                    sel.swap();
                    let head = sel.head;
                    self.set_caret(head);
                }
                return;
            }
            Command::Copy => {
                self.copy_selection(ctx);
                return;
            }
            Command::Highlight(n) => {
                self.annotate(AnnotKind::Highlight, n);
                return;
            }
            Command::Underline => {
                self.annotate(AnnotKind::Underline, self.last_category);
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
            Command::HintCaret
            | Command::HintSelect
            | Command::Motion(_)
            | Command::VisualChar
            | Command::VisualLine
            | Command::VisualSwap
            | Command::Copy
            | Command::Highlight(_)
            | Command::Underline => {}
            // Wired up in the next steps.
            Command::Undo
            | Command::Redo
            | Command::Save
            | Command::SaveForce
            | Command::Reload => {}
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
        let mode = self.mode.label();
        let Some(open) = self.open.as_ref().filter(|_| self.mode != Mode::Recent) else {
            return mode.into();
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
            "{mode}  {}  {page_text}  {:.0}%  continuous{}",
            open.file_name,
            v.zoom() * 100.0,
            if open.unsaved { "  ●" } else { "" }
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
                } else if let Some(h) = self.hinting.as_ref().filter(|h| !h.typed().is_empty()) {
                    ui.label(text(h.typed(), STATUS_FG));
                } else if self.matcher.is_pending() {
                    ui.label(text(&self.matcher.pending_text(), STATUS_FG));
                }
            });
        });
    }
}

impl eframe::App for MargApp {
    fn on_exit(&mut self) {
        self.save_state();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
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
                if self.mode == Mode::Recent
                    && let Some(list) = &mut self.recent
                {
                    list.draw(ui);
                    return;
                }
                let Some(open) = &mut self.open else { return };
                open.view.set_viewport(area.width(), area.height());
                let plan = self
                    .cache
                    .plan(&open.view, &self.config.render, ppp, &open.revs);
                if plan.keys != self.last_request {
                    self.doc.send(Request::Render {
                        keys: plan.keys.clone(),
                        received: self.received,
                    });
                    self.last_request = plan.keys;
                }
                open.request_visible_text(&self.doc);
                let painter = ui.painter_at(area);
                let (exact, stale) =
                    self.cache
                        .draw(&painter, area, &open.view, &self.config.render, &open.revs);
                let cache = &self.cache;
                open.provisional
                    .retain(|p| p.rev.is_none_or(|r| !cache.has_rev(p.spec.page, r)));
                for p in &open.provisional {
                    if p.rev.is_none() || stale.contains(&p.spec.page) {
                        select::draw_provisional(&painter, &open.view, area, &p.spec);
                    }
                }
                if let Some(sel) = &self.selection {
                    select::draw_selection(&painter, &open.view, area, sel, &open.texts);
                }
                if matches!(self.mode, Mode::Caret | Mode::Visual)
                    && let Some(c) = &self.caret
                {
                    select::draw_caret(&painter, &open.view, area, c, &open.texts);
                }
                if let Some(h) = &self.hinting {
                    h.draw(&painter, &open.view, area, &open.texts);
                }
                if let Some(t) = &mut self.timing
                    && !t.shown
                    && (exact || self.last_request.is_empty())
                {
                    t.shown = true;
                    t.log("first page shown");
                }
            });
        if self.debug {
            self.debug_overlay(&ctx);
        }
        self.frame_ms
            .push(frame_start.elapsed().as_secs_f32() * 1000.0);
    }
}

impl MargApp {
    fn debug_overlay(&self, ctx: &egui::Context) {
        let mut lines = Vec::new();
        let ms = |v: Option<f32>| v.map_or("-".into(), |v| format!("{v:.1}"));
        lines.push(format!(
            "ui frame  {} ms  (p95 {} ms)",
            ms(self.frame_ms.last()),
            ms(self.frame_ms.quantile(0.95))
        ));
        lines.push(format!("render queue  {}", self.last_request.len()));
        lines.push(format!(
            "cache  {:.1} / {} MB  ({} textures)",
            self.cache.bytes() as f64 / (1024.0 * 1024.0),
            self.config.render.cache_mb,
            self.cache.len()
        ));
        if let Some(open) = &self.open {
            let v = &open.view;
            let ppp = ctx.pixels_per_point();
            lines.push(format!(
                "zoom {:.0}%  scale {:.3}  ppp {ppp}  {}",
                v.zoom() * 100.0,
                v.zoom() * ppp,
                if v.zoom() > self.config.render.tile_above_zoom {
                    "tiled"
                } else {
                    "whole pages"
                }
            ));
            for p in v.visible_pages() {
                if let Some((took, scale)) = self.last_render.get(&p) {
                    lines.push(format!(
                        "page {}  last render {:.1} ms @ {:.3}",
                        p + 1,
                        took.as_secs_f64() * 1000.0,
                        scale.get()
                    ));
                }
            }
        }
        lines.push("MuPDF warnings: on stderr (not captured yet)".into());
        egui::Area::new(egui::Id::unique("marg-debug-overlay"))
            .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_black_alpha(200))
                    .inner_margin(8)
                    .show(ui, |ui| {
                        for l in lines {
                            ui.label(egui::RichText::new(l).monospace().color(STATUS_FG));
                        }
                    });
            });
    }
}

/// The annotation author: the configured one, else the OS user name.
fn annotation_author(configured: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.trim().to_string();
    }
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "marg".into())
}
