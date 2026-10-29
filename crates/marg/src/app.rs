//! The marg window: page view, status bar and command line.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Event, MouseWheelUnit};
use marg_core::annot::{
    AnnotChange, AnnotFrom, AnnotInfo, AnnotKind, AnnotSpec, annot_at, category_of, erase,
    parse_color, step_annot,
};
use marg_core::caret::{self, Motion, Selection, TextPos};
use marg_core::command::{Command, parse_ex};
use marg_core::config::{Config, Layout};
use marg_core::help;
use marg_core::jumps::JumpList;
use marg_core::keymap::{KeyMode, Keymap, Matcher, Outcome};
use marg_core::keys::{KeyCode, KeyPress, Named};
use marg_core::lineedit::LineEdit;
use marg_core::outline::{Dest, Link, LinkTarget, OutlineEntry, link_at};
use marg_core::search::Hit;
use marg_core::state::{FileState, State, file_key};
use marg_core::stats::Rolling;
use marg_core::text::{BBox, PageText};
use marg_core::undo::UndoStack;
use marg_core::view::{DocPos, Fit, View};
use marg_pdf::{
    DocInfo, DocThread, Edit, OpenError, Recolor, RenderKey, Request, Response, SaveError, Scale,
};

use crate::annotlist::{self, AnnotList};
use crate::boxmode::BoxMode;
use crate::browser;
use crate::comment::{self, CommentEditor};
use crate::complete;
use crate::help::HelpView;
use crate::input;
use crate::outline::{self, OutlineList};
use crate::pages::PageCache;
use crate::recent::{self, RecentList};
use crate::select::{self, Caret, Hinting, Picked, Purpose};
use crate::startup;
use crate::statefile;
use crate::theme::Theme;
use marg_core::theme::{StatusPart, status_line};

/// A successfully opened document.
struct OpenDoc {
    info: DocInfo,
    jumps: JumpList,
    /// The table of contents, once read.
    outline: Option<Vec<OutlineEntry>>,
    /// Links of each page, once listed.
    links: Vec<Option<Vec<Link>>>,
    links_requested: HashSet<usize>,
    /// The last search and its hits; `None` after Esc.
    search: Option<SearchState>,
    /// Marks `a`–`z`, saved in the state file.
    marks: BTreeMap<char, DocPos>,
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
    /// Successful annotation edits, and how many of them are saved. The
    /// document thread answers in order, so a save covers every edit
    /// confirmed before it.
    edits: u64,
    saved_edits: u64,
    /// A save request is on its way.
    saving: bool,
    /// When the next autosave is due.
    save_due: Option<Instant>,
    /// The file changed on disk while marg has unsaved changes; saving
    /// waits for `:w!` or `:e!`.
    conflict: bool,
    /// Annotation actions of this session.
    undo: UndoStack<UndoOp>,
    /// Object numbers of created annotations, by edit tag.
    xrefs: HashMap<u64, i32>,
    /// The annotations of each page, once listed by the document thread.
    annots: Vec<Option<Vec<AnnotInfo>>>,
    /// What to do with created annotations once their creation is
    /// confirmed, by edit tag.
    pending: HashMap<u64, AfterCreate>,
    /// Where images are on each page, once asked for (box snapping).
    images: Vec<Option<Vec<BBox>>>,
    /// The password the file was opened with, for reloading it.
    password: Option<String>,
    /// Pages whose annotations or text could not be read.
    annots_failed: HashSet<usize>,
    text_failed: HashSet<usize>,
    /// `:export md`, waiting for annotations and text.
    export: Option<PendingExport>,
}

/// A Markdown export waiting for the annotations of every page and the
/// text of the annotated ones.
struct PendingExport {
    path: PathBuf,
    /// Pages not listed again yet (so that recent edits are included).
    relist: HashSet<usize>,
}

/// An undoable annotation action: annotations it deleted, created and
/// changed (a highlight creates one per page, `x` deletes one, erasing
/// does both, recoloring changes one).
#[derive(Default)]
struct UndoOp {
    /// (page, object number). Undo puts the same objects back.
    deleted: Vec<(usize, i32)>,
    created: Vec<Created>,
    changed: Vec<Changed>,
}

/// An annotation changed in place by an undoable action.
struct Changed {
    page: usize,
    xref: i32,
    before: AnnotChange,
    after: AnnotChange,
}

/// An annotation created by an undoable action. Undo deletes it and redo
/// puts the same object back, so its object number never changes.
struct Created {
    spec: AnnotSpec,
    /// Tag of the edit that created it.
    tag: u64,
}

impl OpenDoc {
    fn unsaved(&self) -> bool {
        self.edits > self.saved_edits
    }
}

/// What to do with an annotation once its creation is confirmed.
enum AfterCreate {
    Comment(String),
    Delete,
}

/// What to do once the pending save has succeeded.
#[derive(Debug, Clone, PartialEq)]
enum AfterSave {
    Nothing,
    /// `:w`: say "Saved".
    Report,
    Quit,
    Open(PathBuf),
    Close,
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

    /// Ask for the text and links of the visible pages that do not have
    /// them yet.
    fn request_visible_text(&mut self, doc: &DocThread) {
        for p in self.view.visible_pages() {
            self.request_text(doc, p);
            if self.links[p].is_none() && self.links_requested.insert(p) {
                doc.send(Request::Links(p));
            }
        }
    }
}

/// A search (`/`) and the hits found so far.
struct SearchState {
    id: u64,
    query: String,
    /// In page order, then reading order.
    hits: Vec<Hit>,
    /// Every page has been searched.
    done: bool,
    current: Option<usize>,
    /// Go to the first hit at or after this position once it arrives.
    seek: Option<DocPos>,
}

impl SearchState {
    /// Index of the first hit at or after `pos`.
    fn first_from(&self, pos: DocPos) -> Option<usize> {
        self.hits
            .iter()
            .position(|h| h.page > pos.page || (h.page == pos.page && h.top().1 >= pos.y - 0.5))
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
    /// Visual mode, asking which category to highlight with.
    Category,
    /// The comment editor is open.
    Insert,
    /// Drawing a box.
    Box,
    /// The annotation list is open.
    Annots,
    /// The outline is open, or asked for.
    Outline,
    /// Asking whether to open an external link.
    Confirm,
    /// The help overlay or page is open.
    Help,
    /// Typing a search.
    Search,
    /// Asking for the password of an encrypted file.
    Password,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Normal | Mode::Command | Mode::Search | Mode::Confirm | Mode::Password => {
                "NORMAL"
            }
            Mode::Help => "HELP",
            Mode::Recent => "RECENT",
            Mode::Hint => "HINT",
            Mode::Caret => "CARET",
            Mode::Visual | Mode::Category => "VISUAL",
            Mode::Insert => "INSERT",
            Mode::Box => "BOX",
            Mode::Annots => "ANNOTATIONS",
            Mode::Outline => "OUTLINE",
        }
    }
}

pub struct MargApp {
    config: Config,
    /// `config.colors` as egui colors.
    theme: Theme,
    /// `[statusbar]` parts, left and right.
    status_parts: (Vec<StatusPart>, Vec<StatusPart>),
    version: String,
    keymap: Keymap,
    caret_map: Keymap,
    visual_map: Keymap,
    /// Normal mode while an annotation is selected.
    annot_map: Keymap,
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
    /// The comment editor (Insert mode).
    editor: Option<CommentEditor>,
    /// The box being drawn (Box mode).
    boxing: Option<BoxMode>,
    /// The annotation list (Annots mode).
    annot_list: Option<AnnotList>,
    /// The outline overlay (Outline mode).
    outline_list: Option<OutlineList>,
    /// The help (Help mode), and whether it is the whole page.
    help: Option<(HelpView, bool)>,
    /// The selection before hints started, restored if they are cancelled.
    selection_before_hints: Option<Selection>,
    /// The external link waiting for `y` (Confirm mode).
    pending_url: Option<String>,
    /// A press on a link: where, and the link; a release there follows it.
    link_press: Option<(egui::Pos2, Link)>,
    /// `m` or `'` was pressed: the next key names the mark (true: set).
    pending_mark: Option<bool>,
    /// Where a mouse drag started, while the primary button is down.
    drag_anchor: Option<TextPos>,
    /// Last primary press: time, position, click count (1–3).
    last_click: Option<(Instant, egui::Pos2, u8)>,
    /// Category used by the last highlight; underlines use it.
    last_category: u8,
    /// The selected annotation: (page, object number).
    selected_annot: Option<(usize, i32)>,
    /// Tag of the next annotation edit.
    next_tag: u64,
    /// Annotation author.
    author: String,
    after_save: AfterSave,
    /// `:q!`: close without saving.
    discard: bool,
    /// When to ask whether the file changed on disk next.
    next_disk_check: Instant,
    /// The config and theme files as last read, and when to look again.
    config_stamp: startup::Stamp,
    next_config_check: Instant,
    /// The pending open is a reload of the current file.
    reloading: bool,
    cmdline: LineEdit,
    searchline: LineEdit,
    /// The password prompt (no history).
    passline: LineEdit,
    /// The file being opened and the password sent with it; the file the
    /// password prompt is for.
    opening: Option<(PathBuf, Option<String>)>,
    /// Where the view was when the search prompt opened.
    search_origin: Option<DocPos>,
    next_search_id: u64,
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
    config_path: Option<PathBuf>,
    state: State,
    recent: Option<RecentList>,
    debug: bool,
    /// Time spent in `ui()` per frame, in ms.
    frame_ms: Rolling,
    /// Last render time and scale per page, for the debug overlay.
    last_render: HashMap<usize, (Duration, Scale)>,
    /// Dark pages are on.
    recolor: bool,
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
    pub config_path: Option<PathBuf>,
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
        let ([keymap, caret_map, visual_map, annot_map], warnings) = build_keymaps(&s.config);
        for w in &warnings {
            log::warn!("{w}");
        }
        let mut errors = s.errors;
        let (left, right, w) = s.config.statusbar.parts();
        errors.extend(w);
        errors.extend(crate::theme::install_fonts(&cc.egui_ctx, &s.config.fonts));
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
            theme: Theme::new(&s.config.colors, &s.config.fonts),
            status_parts: (left, right),
            config: s.config,
            version: s.version,
            keymap,
            caret_map,
            visual_map,
            annot_map,
            matcher: Matcher::default(),
            clock: Instant::now(),
            mode,
            return_mode: Mode::Normal,
            area: egui::Rect::NOTHING,
            caret: None,
            selection: None,
            hinting: None,
            editor: None,
            boxing: None,
            annot_list: None,
            outline_list: None,
            help: None,
            selection_before_hints: None,
            pending_mark: None,
            pending_url: None,
            link_press: None,
            drag_anchor: None,
            last_click: None,
            last_category: 1,
            selected_annot: None,
            next_tag: 1,
            after_save: AfterSave::Nothing,
            discard: false,
            next_disk_check: Instant::now(),
            config_stamp: Vec::new(),
            next_config_check: Instant::now(),
            reloading: false,
            author,
            cmdline: LineEdit::default(),
            searchline: LineEdit::default(),
            passline: LineEdit::default(),
            opening: s.path.clone().map(|p| (p, None)),
            search_origin: None,
            next_search_id: 0,
            doc,
            open: None,
            start_page: s.start_page,
            received: 0,
            last_request: Vec::new(),
            message: None,
            state_path: s.state_path,
            config_path: s.config_path,
            state,
            recent,
            debug: false,
            frame_ms: Rolling::new(120),
            last_render: HashMap::new(),
            recolor: false,
            timing: s.timing_start.map(|start| Timing {
                start,
                opened: false,
                shown: false,
            }),
        };
        app.set_recolor(app.config.view.recolor);
        app.config_stamp = startup::stamp(&app.config_files());
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
            if matches!(resp, Response::Annots { .. } | Response::Text { .. })
                && let Some(list) = &mut self.annot_list
            {
                list.dirty = true;
            }
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
                        if std::mem::take(&mut self.reloading) {
                            self.show(Level::Info, "Reloaded from disk".into());
                        }
                        self.after_save = AfterSave::Nothing;
                        self.cache.clear();
                        self.last_request.clear();
                        self.mode = Mode::Normal;
                        self.recent = None;
                        self.caret = None;
                        self.selection = None;
                        self.hinting = None;
                        let mut doc = self.open_doc(info, generation);
                        doc.password = self.opening.take().and_then(|(_, pw)| pw);
                        self.selected_annot = None;
                        self.doc.send(Request::Annots {
                            pages: (0..doc.view.page_count()).collect(),
                            urgent: false,
                        });
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                            "{} — marg",
                            doc.file_name
                        )));
                        self.open = Some(doc);
                        // Record the open now, so the recent list has it even
                        // if marg does not exit cleanly.
                        self.save_state();
                    }
                    Err(e @ (OpenError::NeedsPassword | OpenError::WrongPassword)) => {
                        self.reloading = false;
                        let Some((path, _)) = &self.opening else {
                            continue;
                        };
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        log::info!("{}: {e}", path.display());
                        if e == OpenError::WrongPassword {
                            self.show(Level::Warn, format!("Wrong password for {name}"));
                        }
                        self.passline.clear();
                        if self.mode != Mode::Password {
                            self.return_mode = self.mode;
                        }
                        self.mode = Mode::Password;
                    }
                    Err(e) => {
                        self.reloading = false;
                        self.opening = None;
                        self.error(format!("Cannot open: {e}"));
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
                        Ok(xref) => {
                            open.xrefs.insert(tag, xref);
                            if let Some(after) = open.pending.remove(&tag) {
                                let edit = match after {
                                    AfterCreate::Comment(text) => comment_edit(page, xref, text),
                                    AfterCreate::Delete => Edit::Delete { page, xref },
                                };
                                self.doc.send(Request::Edit {
                                    tag: self.next_tag,
                                    edit,
                                });
                                self.next_tag += 1;
                            }
                            self.doc.send(Request::Annots {
                                pages: vec![page],
                                urgent: true,
                            });
                            open.revs[page] = open.revs[page].max(rev);
                            open.edits += 1;
                            open.save_due = Some(
                                Instant::now()
                                    + Duration::from_secs_f32(self.config.annotation.autosave_secs),
                            );
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
                Response::Searched {
                    generation,
                    id,
                    page,
                    hits,
                } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    let Some(s) = open.search.as_mut().filter(|s| s.id == id) else {
                        continue;
                    };
                    let at = s.hits.partition_point(|h| h.page < page);
                    if let Some(c) = s.current.as_mut().filter(|c| **c >= at) {
                        *c += hits.len();
                    }
                    let n = hits.len();
                    s.hits.splice(at..at, hits);
                    if let Some(from) = s.seek {
                        // Pages come from `from` on, then wrap around.
                        let found = if page < from.page {
                            Some(at)
                        } else {
                            s.first_from(from).filter(|&i| i >= at && i < at + n)
                        };
                        if let Some(i) = found {
                            s.seek = None;
                            s.current = Some(i);
                            let h = &s.hits[i];
                            select::reveal(&mut open.view, h.page, h.rects[0]);
                        }
                    }
                }
                Response::SearchDone { generation, id } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    let Some(s) = open.search.as_mut().filter(|s| s.id == id) else {
                        continue;
                    };
                    s.done = true;
                    if let Some(from) = s.seek.take() {
                        if s.hits.is_empty() {
                            open.view.set_position(from);
                            let msg = format!("Not found: {}", s.query);
                            self.show(Level::Info, msg);
                        } else {
                            // Only hits before the start: wrap to the first.
                            s.current = Some(0);
                            let h = &s.hits[0];
                            select::reveal(&mut open.view, h.page, h.rects[0]);
                        }
                    }
                }
                Response::Outline { generation, result } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    match result {
                        Ok(toc) => {
                            open.outline = Some(toc);
                            if self.mode == Mode::Outline {
                                self.show_outline();
                            }
                        }
                        Err(e) => {
                            if self.mode == Mode::Outline {
                                self.mode = Mode::Normal;
                            }
                            self.error(format!("Cannot read the outline: {e}"));
                        }
                    }
                }
                Response::Links {
                    generation,
                    page,
                    result,
                } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    match result {
                        Ok(links) => open.links[page] = Some(links),
                        Err(e) => {
                            log::error!("links of page {} failed: {e}", page + 1);
                            open.links[page] = Some(Vec::new());
                        }
                    }
                }
                Response::Images {
                    generation,
                    page,
                    result,
                } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    match result {
                        Ok(images) => {
                            open.images[page] = Some(images);
                            if self.boxing.as_ref().is_some_and(|b| {
                                b.snap_waiting && b.sel.is_some_and(|s| s.page == page)
                            }) {
                                self.snap_box();
                            }
                        }
                        Err(e) => self.error(format!("Cannot find the images: {e}")),
                    }
                }
                Response::DiskChanged { generation } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    if open.unsaved() {
                        open.conflict = true;
                        self.show(
                            Level::Warn,
                            "File changed on disk; :w! keeps marg's version, :e! reloads".into(),
                        );
                    } else {
                        self.reload();
                    }
                }
                Response::Saved { generation, result } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    open.saving = false;
                    let after = std::mem::replace(&mut self.after_save, AfterSave::Nothing);
                    match result {
                        Ok(_) => {
                            open.saved_edits = open.edits;
                            open.conflict = false;
                            if !open.unsaved() {
                                open.save_due = None;
                            }
                            match after {
                                AfterSave::Nothing => {}
                                AfterSave::Report => self.show(Level::Info, "Saved".into()),
                                AfterSave::Quit => {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                                }
                                AfterSave::Open(path) => self.send_open(path, None),
                                AfterSave::Close => self.close_doc(ctx),
                            }
                        }
                        Err(e) => {
                            let what = match after {
                                AfterSave::Quit => "Not quitting: ",
                                AfterSave::Open(_) => "Not opening the other file: ",
                                AfterSave::Close => "Not closing: ",
                                _ => "",
                            };
                            let msg = match e {
                                SaveError::ChangedOnDisk => {
                                    open.conflict = true;
                                    format!(
                                        "{what}file changed on disk; :w! keeps marg's version, :e! reloads"
                                    )
                                }
                                SaveError::Failed(e) => {
                                    open.save_due = Some(
                                        Instant::now()
                                            + Duration::from_secs_f32(
                                                self.config.annotation.autosave_secs,
                                            ),
                                    );
                                    format!("{what}save failed: {e}; will retry")
                                }
                            };
                            self.error(msg);
                        }
                    }
                }
                Response::Annots {
                    generation,
                    page,
                    result,
                } => {
                    let Some(open) = &mut self.open else { continue };
                    if generation != open.generation {
                        continue;
                    }
                    if let Some(x) = &mut open.export {
                        x.relist.remove(&page);
                    }
                    match result {
                        Ok(list) => {
                            if self.selected_annot.is_some_and(|(p, x)| {
                                p == page && !list.iter().any(|a| a.xref == x)
                            }) {
                                self.selected_annot = None;
                            }
                            open.annots[page] = Some(list);
                            open.annots_failed.remove(&page);
                        }
                        Err(e) => {
                            log::error!("annotations of page {} failed: {e}", page + 1);
                            open.annots_failed.insert(page);
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
                        Err(e) => {
                            log::error!("text of page {} failed: {e}", page + 1);
                            open.text_failed.insert(page);
                        }
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
        let cfg = &self.config.view;
        let (layout, fit) = if sizes[0].0 > sizes[0].1 {
            (cfg.default_layout_landscape, Fit::Page)
        } else {
            (cfg.default_layout_portrait, Fit::Width)
        };
        let viewport = self
            .open
            .as_ref()
            .map_or((800.0, 600.0), |o| o.view.viewport());
        let canonical = std::fs::canonicalize(&info.path).unwrap_or_else(|_| info.path.clone());
        let key = file_key(info.id.as_deref(), &canonical.to_string_lossy());
        let saved = self.state.files.get(&key);
        let fit = saved.map_or(fit, |f| f.fit);
        let layout = saved.map_or(layout, |f| f.layout);
        let marks = saved.map(|f| f.marks.clone()).unwrap_or_default();
        let mut view = View::new(sizes, &self.config.view, layout, fit, viewport);
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
            jumps: JumpList::new(self.config.view.jump_history),
            outline: None,
            search: None,
            marks,
            texts: vec![None; view.page_count()],
            text_requested: HashSet::new(),
            revs: vec![0; view.page_count()],
            provisional: Vec::new(),
            edits: 0,
            saved_edits: 0,
            saving: false,
            save_due: None,
            conflict: false,
            undo: UndoStack::new(self.config.annotation.undo_steps),
            xrefs: HashMap::new(),
            annots: vec![None; view.page_count()],
            pending: HashMap::new(),
            images: vec![None; view.page_count()],
            password: None,
            annots_failed: HashSet::new(),
            text_failed: HashSet::new(),
            export: None,
            links: vec![None; view.page_count()],
            links_requested: HashSet::new(),
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
        if self.mode == Mode::Insert {
            self.editor_keys(ctx);
        }
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
                _ if self.mode == Mode::Search => self.search_event(e),
                _ if self.mode == Mode::Password => self.password_event(e),
                _ if self.mode == Mode::Confirm => self.confirm_event(e),
                _ if self.mode == Mode::Help => {
                    if self.help.as_mut().is_none_or(|(h, _)| h.event(e)) {
                        self.help = None;
                        self.mode = self.return_mode;
                    }
                }
                _ if self.mode == Mode::Recent => self.recent_event(ctx, e),
                _ if self.mode == Mode::Hint => self.hint_event(e),
                _ if self.mode == Mode::Category => self.category_event(e),
                // The comment editor's text field takes the keys.
                _ if self.mode == Mode::Insert => {}
                _ if self.mode == Mode::Box => self.box_event(e),
                _ if self.mode == Mode::Annots => self.annot_list_event(e),
                _ if self.mode == Mode::Outline => self.outline_event(e),
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
        self.drag_scroll(ctx, area);
        let hover = ctx.input(|i| i.pointer.hover_pos());
        if matches!(self.mode, Mode::Normal | Mode::Caret | Mode::Visual)
            && hover.is_some_and(|p| area.contains(p) && self.link_at_screen(p).is_some())
        {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
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
            layout: open.view.layout(),
            marks: open.marks.clone(),
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
        if let Some(set) = self.pending_mark.take() {
            self.mark_key(key, set);
            return;
        }
        let map = match self.mode {
            Mode::Caret => &self.caret_map,
            Mode::Visual => &self.visual_map,
            Mode::Normal if self.selected_annot.is_some() => &self.annot_map,
            _ => &self.keymap,
        };
        match self.matcher.feed(map, key, now, timeout) {
            Outcome::Run { command, count } => self.run(ctx, command, count),
            Outcome::Unbound if key == KeyPress::named(Named::Esc) => {
                if matches!(self.mode, Mode::Caret | Mode::Visual) {
                    self.leave_text_modes();
                } else if self.selected_annot.is_some() {
                    self.selected_annot = None;
                } else if let Some(open) = &mut self.open {
                    // Hide the search hits; `n` searches again.
                    open.search = None;
                }
            }
            Outcome::Pending | Outcome::Unbound | Outcome::Cancelled => {}
        }
    }

    /// The key after `m` (`set`) or `'`: a mark name `a`–`z`; anything
    /// else cancels.
    fn mark_key(&mut self, key: KeyPress, set: bool) {
        let Some(open) = &mut self.open else { return };
        let name = match key.code {
            KeyCode::Char(c @ 'a'..='z') if !key.ctrl && !key.alt => c,
            _ => return,
        };
        let here = open.view.position();
        if set {
            open.marks.insert(name, here);
            self.save_state();
            self.show(Level::Info, format!("Mark {name} set"));
        } else if let Some(&to) = open.marks.get(&name) {
            open.jumps.push(here);
            open.view.set_position(to);
        } else {
            self.show(Level::Info, format!("No mark {name}"));
        }
    }

    /// Open the outline overlay once the outline is read.
    fn show_outline(&mut self) {
        let Some(open) = &self.open else { return };
        let toc = open.outline.clone().unwrap_or_default();
        if toc.is_empty() {
            self.mode = Mode::Normal;
            self.show(Level::Info, "This PDF has no outline".into());
            return;
        }
        self.outline_list = Some(OutlineList::new(toc, open.view.current_page()));
    }

    /// A key in the outline.
    fn outline_event(&mut self, e: &Event) {
        let Some(list) = &mut self.outline_list else {
            // Still reading the outline: Esc gives up.
            if input::key_press(e) == Some(KeyPress::named(Named::Esc)) {
                self.mode = Mode::Normal;
            }
            return;
        };
        match list.event(e) {
            outline::Action::None => {}
            outline::Action::Close => {
                self.outline_list = None;
                self.mode = Mode::Normal;
            }
            outline::Action::Jump(dest) => {
                self.outline_list = None;
                self.mode = Mode::Normal;
                self.jump_to(dest);
            }
        }
    }

    /// Go to a link or outline destination, as a jump.
    fn jump_to(&mut self, dest: Dest) {
        let Some(open) = &mut self.open else { return };
        if dest.page >= open.view.page_count() {
            return;
        }
        open.jumps.push(open.view.position());
        open.view.set_position(DocPos {
            page: dest.page,
            x: dest.x.unwrap_or(0.0),
            y: dest.y.unwrap_or(0.0),
        });
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

    /// A key or mouse event in Box mode.
    fn box_event(&mut self, e: &Event) {
        let (Some(open), Some(b)) = (&self.open, &mut self.boxing) else {
            self.mode = Mode::Normal;
            return;
        };
        let view = &open.view;
        let cfg = &self.config.annotation;
        match e {
            Event::Text(t) => {
                for c in t.chars() {
                    if !b.adjusting() {
                        b.push(c, view);
                        continue;
                    }
                    let step = cfg.box_step_pt;
                    match c {
                        'h' => b.nudge(true, -step, 0.0, view),
                        'l' => b.nudge(true, step, 0.0, view),
                        'k' => b.nudge(true, 0.0, -step, view),
                        'j' => b.nudge(true, 0.0, step, view),
                        'H' => b.nudge(false, -step, 0.0, view),
                        'L' => b.nudge(false, step, 0.0, view),
                        'K' => b.nudge(false, 0.0, -step, view),
                        'J' => b.nudge(false, 0.0, step, view),
                        's' => {
                            b.snap_waiting = true;
                            self.snap_box();
                            return;
                        }
                        _ => {
                            let n = self.config.categories.len().min(9);
                            if let Some(d) = c.to_digit(10).filter(|&d| d >= 1 && d as usize <= n) {
                                b.category = d as u8;
                            }
                        }
                    }
                }
            }
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let fine = cfg.box_fine_step_pt;
                match key {
                    egui::Key::Escape => {
                        self.boxing = None;
                        self.mode = Mode::Normal;
                    }
                    egui::Key::Backspace if !b.adjusting() => b.pop(),
                    egui::Key::Enter if b.adjusting() => self.confirm_box(),
                    // Ctrl drops the text event, so fine steps come from
                    // the key; Shift moves the top-left corner.
                    egui::Key::H | egui::Key::J | egui::Key::K | egui::Key::L
                        if modifiers.ctrl && b.adjusting() =>
                    {
                        let (dx, dy) = match key {
                            egui::Key::H => (-fine, 0.0),
                            egui::Key::L => (fine, 0.0),
                            egui::Key::K => (0.0, -fine),
                            _ => (0.0, fine),
                        };
                        b.nudge(!modifiers.shift, dx, dy, view);
                    }
                    _ => {}
                }
            }
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                ..
            } => {
                if !*pressed {
                    b.drag_end();
                } else if self.area.contains(*pos) {
                    let p = *pos - self.area.min;
                    b.drag_start(view, p.x, p.y);
                }
            }
            Event::PointerMoved(pos) => {
                let p = *pos - self.area.min;
                b.drag_to(view, p.x, p.y);
            }
            _ => {}
        }
    }

    /// `s` in Box mode: snap the box to the image it overlaps most, once
    /// the page's images are known (asks for them otherwise).
    fn snap_box(&mut self) {
        let (Some(open), Some(b)) = (&self.open, &mut self.boxing) else {
            return;
        };
        let Some(page) = b.sel.map(|s| s.page) else {
            return;
        };
        let Some(images) = &open.images[page] else {
            self.doc.send(Request::Images(page));
            return;
        };
        if !b.snap(images, &open.view) {
            self.show(Level::Info, "No image under the box".into());
        }
    }

    /// Enter in Box mode: create the box (one undo step).
    fn confirm_box(&mut self) {
        let Some(b) = self.boxing.take() else { return };
        self.mode = Mode::Normal;
        let (Some(sel), Some(open)) = (b.sel, &mut self.open) else {
            return;
        };
        let Some(cat) = self.config.categories.get(usize::from(b.category) - 1) else {
            return;
        };
        let Some(color) = parse_color(&cat.color) else {
            let msg = format!("Bad color for category {}: {}", cat.name, cat.color);
            self.error(msg);
            return;
        };
        let spec = AnnotSpec {
            page: sel.page,
            kind: AnnotKind::Box,
            rects: vec![sel.rect],
            color,
            opacity: 1.0,
            stroke_pt: self.config.annotation.box_stroke_pt,
            subject: cat.name.clone(),
            author: self.author.clone(),
            contents: None,
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
            edit: Edit::Create(spec.clone()),
        });
        open.undo.push(UndoOp {
            created: vec![Created { spec, tag }],
            ..UndoOp::default()
        });
        self.last_category = b.category;
        self.show(Level::Info, format!("Box: {}", cat.name));
    }

    /// Dark pages on or off; the visible pages are rendered again.
    fn set_recolor(&mut self, on: bool) {
        self.recolor = on;
        let rgb = |c: marg_core::theme::Rgba| [c.0[0], c.0[1], c.0[2]];
        let colors = &self.config.colors;
        self.cache.set_recolor(on.then(|| Recolor {
            fg: rgb(colors.recolor_fg),
            bg: rgb(colors.recolor_bg),
        }));
    }

    /// `A`: open the annotation list; asks for the text of annotated
    /// pages that do not have it yet.
    fn open_annot_list(&mut self) {
        let Some(open) = &mut self.open else { return };
        let pages: Vec<usize> = open
            .annots
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_ref().is_some_and(|a| !a.is_empty()))
            .map(|(p, _)| p)
            .collect();
        for p in pages {
            open.request_text(&self.doc, p);
        }
        self.leave_text_modes();
        self.annot_list = Some(AnnotList::new());
        self.mode = Mode::Annots;
    }

    /// Rebuild the rows of the annotation list if they changed.
    fn refresh_annot_list(&mut self) {
        let Some(list) = self.annot_list.as_mut().filter(|l| l.dirty) else {
            return;
        };
        let Some(open) = &self.open else { return };
        let all: Vec<AnnotInfo> = open.annots.iter().flatten().flatten().cloned().collect();
        let cats: Vec<_> = self
            .config
            .categories
            .iter()
            .map(|c| (c.name.as_str(), parse_color(&c.color)))
            .collect();
        list.set_entries(marg_core::annotlist::entries(&all, &open.texts, &cats));
    }

    /// A key in the annotation list.
    fn annot_list_event(&mut self, e: &Event) {
        let Some(list) = &mut self.annot_list else {
            self.mode = Mode::Normal;
            return;
        };
        let find = |s: &Self, page: usize, xref: i32| {
            s.open.as_ref().and_then(|o| {
                o.annots[page]
                    .as_ref()
                    .and_then(|l| l.iter().find(|a| a.xref == xref))
                    .cloned()
            })
        };
        match list.event(e, self.config.categories.len().min(9)) {
            annotlist::Action::None => {}
            annotlist::Action::Close => {
                self.annot_list = None;
                self.mode = Mode::Normal;
            }
            annotlist::Action::Jump { page, xref } => {
                self.annot_list = None;
                self.mode = Mode::Normal;
                if let Some(a) = find(self, page, xref) {
                    self.reveal_annot(&a);
                }
            }
            annotlist::Action::Edit { page, xref } => {
                self.annot_list = None;
                self.mode = Mode::Normal;
                if let Some(a) = find(self, page, xref) {
                    self.reveal_annot(&a);
                    self.edit_comment();
                }
            }
            annotlist::Action::Delete { page, xref } => {
                self.selected_annot = Some((page, xref));
                self.delete_annotation();
            }
        }
    }

    /// The status line in Box mode.
    fn box_prompt(&self) -> String {
        let Some(b) = &self.boxing else {
            return String::new();
        };
        if !b.adjusting() {
            return "BOX  type a cell for the first corner, then the second (or drag)".into();
        }
        let name = self
            .config
            .categories
            .get(usize::from(b.category) - 1)
            .map_or("", |c| c.name.as_str());
        format!(
            "BOX  {name}  hjkl/HJKL corners (Ctrl: fine)  s snap  1-5 category  ⏎ save  Esc cancel"
        )
    }

    /// `ga`: hints on the visible annotations, in reading order.
    fn annot_hints(&mut self) {
        let Some(open) = &self.open else { return };
        let mut found: Vec<&AnnotInfo> = Vec::new();
        for page in open.view.visible_pages() {
            let Some(list) = &open.annots[page] else {
                self.show(Level::Info, "Still reading the annotations".into());
                return;
            };
            found.extend(list.iter().filter(|a| {
                a.boxes()
                    .iter()
                    .any(|&b| select::on_screen(&open.view, self.area, page, b))
            }));
        }
        if found.is_empty() {
            self.show(Level::Info, "No annotations in view".into());
            return;
        }
        found.sort_by(|a, b| {
            (a.page, a.rect.y0, a.rect.x0)
                .partial_cmp(&(b.page, b.rect.y0, b.rect.x0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let boxes = found.iter().map(|a| (a.page, a.boxes()[0])).collect();
        let ids = found.iter().map(|a| (a.page, a.xref)).collect();
        let h = Hinting::boxes(Purpose::Annot(ids), boxes, &self.config.hints);
        if self.mode != Mode::Hint {
            self.return_mode = self.mode;
            self.selection_before_hints = self.selection;
        }
        self.hinting = Some(h);
        self.mode = Mode::Hint;
    }

    /// `?`: the keys of the current mode; `:help` (`page`): everything.
    fn show_help(&mut self, page: bool) {
        let view = if page {
            let names: Vec<String> = self
                .config
                .categories
                .iter()
                .map(|c| c.name.clone())
                .collect();
            let maps = help::Maps {
                normal: &self.keymap,
                caret: &self.caret_map,
                visual: &self.visual_map,
                annot: &self.annot_map,
            };
            HelpView::new("marg help".into(), help::page(&maps, &names))
        } else {
            let (title, map) = match self.mode {
                Mode::Caret => ("Caret mode keys", &self.caret_map),
                Mode::Visual => ("Visual mode keys", &self.visual_map),
                Mode::Normal if self.selected_annot.is_some() => {
                    ("Keys with an annotation selected", &self.annot_map)
                }
                _ => ("Normal mode keys", &self.keymap),
            };
            let mut lines = help::section("", map, None);
            lines.push(help::HelpLine::Text(
                ":help shows every mode, the : commands and more.".into(),
            ));
            HelpView::new(title.into(), lines)
        };
        if self.mode != Mode::Help {
            self.return_mode = self.mode;
        }
        self.help = Some((view, page));
        self.mode = Mode::Help;
    }

    /// `gl`: hints on the visible links.
    fn link_hints(&mut self) {
        let Some(open) = &self.open else { return };
        let mut found: Vec<(usize, Link)> = Vec::new();
        for page in open.view.visible_pages() {
            let Some(list) = &open.links[page] else {
                self.show(Level::Info, "Still reading the links".into());
                return;
            };
            found.extend(
                list.iter()
                    .filter(|l| select::on_screen(&open.view, self.area, page, l.rect))
                    .map(|l| (page, l.clone())),
            );
        }
        if found.is_empty() {
            self.show(Level::Info, "No links in view".into());
            return;
        }
        found.sort_by(|(pa, a), (pb, b)| {
            (pa, a.rect.y0, a.rect.x0)
                .partial_cmp(&(pb, b.rect.y0, b.rect.x0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let boxes = found.iter().map(|(p, l)| (*p, l.rect)).collect();
        let links = found.into_iter().map(|(_, l)| l).collect();
        let h = Hinting::boxes(Purpose::Link(links), boxes, &self.config.hints);
        if self.mode != Mode::Hint {
            self.return_mode = self.mode;
            self.selection_before_hints = self.selection;
        }
        self.hinting = Some(h);
        self.mode = Mode::Hint;
    }

    /// Follow a link: jump to its destination, or ask before opening an
    /// external one.
    fn follow(&mut self, link: Link) {
        match link.target {
            LinkTarget::Page(dest) => self.jump_to(dest),
            LinkTarget::Uri(url) => {
                self.return_mode = self.mode;
                self.pending_url = Some(url);
                self.mode = Mode::Confirm;
            }
        }
    }

    /// Confirm mode: `y` opens the link, anything else does not.
    fn confirm_event(&mut self, e: &Event) {
        let Some(k) = input::key_press(e) else { return };
        let Some(url) = self.pending_url.take() else {
            self.mode = self.return_mode;
            return;
        };
        self.mode = self.return_mode;
        if k.code == KeyCode::Char('y') && !k.ctrl && !k.alt {
            match browser::open(&url) {
                Ok(()) => self.show(Level::Info, format!("Opened {url}")),
                Err(e) => self.error(format!("Cannot open {url}: {e}")),
            }
        }
    }

    /// The link under a point of the window, if its page's links are known.
    fn link_at_screen(&self, pos: egui::Pos2) -> Option<Link> {
        let open = self.open.as_ref()?;
        let rel = pos - self.area.min;
        let d = open.view.doc_pos_at(rel.x, rel.y);
        // `doc_pos_at` clamps to the page: check the point is on it.
        let r = open.view.page_rect_on_screen(d.page);
        if rel.x < r.x || rel.x > r.right() || rel.y < r.y || rel.y > r.bottom() {
            return None;
        }
        link_at(open.links[d.page].as_ref()?, d.x, d.y).cloned()
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
                if let Some(p) = picked {
                    self.hint_picked(p);
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

    fn hint_picked(&mut self, picked: Picked) {
        let Some(h) = self.hinting.take() else { return };
        let (page, word) = match (picked, &h.purpose) {
            (Picked::Box(i), Purpose::Annot(annots)) => {
                let (page, xref) = annots[i];
                self.leave_text_modes();
                let a = self.open.as_ref().and_then(|o| {
                    o.annots[page]
                        .as_ref()
                        .and_then(|l| l.iter().find(|a| a.xref == xref))
                        .cloned()
                });
                self.select_annot(a.as_ref());
                return;
            }
            (Picked::Box(i), Purpose::Link(links)) => {
                self.mode = self.return_mode;
                self.selection = self.selection_before_hints.take();
                self.follow(links[i].clone());
                return;
            }
            (Picked::Word(page, word), _) => (page, word),
            (Picked::Box(_), _) => return,
        };
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
            Purpose::Annot(_) | Purpose::Link(_) => {}
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
                self.link_press = self.link_at_screen(pos).map(|l| (pos, l));
                if count == 1 {
                    // A click selects the annotation under it (or none).
                    let hit = self.annot_at_screen(pos);
                    self.leave_text_modes();
                    self.select_annot(hit.as_ref());
                }
                let Some(at) = self.char_at_screen(pos) else {
                    return;
                };
                match count {
                    1 => self.drag_anchor = Some(at),
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
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                ..
            } => {
                self.drag_anchor = None;
                // A click (not a drag) on a link follows it.
                if let Some((at, link)) = self.link_press.take()
                    && at.distance(pos) < 4.0
                    && self.selection.is_none()
                {
                    self.follow(link);
                }
            }
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

    /// While a mouse selection is dragged into the top or bottom edge zone
    /// (or past it), scroll and extend the selection, every frame.
    fn drag_scroll(&mut self, ctx: &egui::Context, area: egui::Rect) {
        let Some(anchor) = self.drag_anchor else {
            return;
        };
        let (pos, down, dt) = ctx.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_down(),
                i.stable_dt.min(0.1),
            )
        });
        let (Some(p), true) = (pos, down) else { return };
        let edge = self.config.ui.drag_scroll_edge_px;
        let over = if p.y < area.min.y + edge {
            p.y - (area.min.y + edge)
        } else if p.y > area.max.y - edge {
            p.y - (area.max.y - edge)
        } else {
            return;
        };
        let Some(open) = &mut self.open else { return };
        open.view
            .scroll_by(0.0, over * self.config.ui.drag_scroll_rate * dt);
        let inside = egui::pos2(p.x, p.y.clamp(area.min.y + 1.0, area.max.y - 1.0));
        if let Some(at) = self.char_at_screen(inside) {
            self.mouse_select(Selection::new(anchor, at));
        }
        ctx.request_repaint();
    }

    fn mouse_select(&mut self, sel: Selection) {
        self.selected_annot = None;
        self.selection = Some(sel);
        self.caret = Some(Caret {
            pos: sel.head,
            want_x: None,
        });
        self.mode = Mode::Visual;
    }

    /// Highlight or underline the selection in `category` (1-based), one
    /// annotation per page, and leave Visual mode.
    /// Returns the edit tag and spec of the first annotation.
    fn annotate(&mut self, kind: AnnotKind, category: u8) -> Option<(u64, AnnotSpec)> {
        let (Some(open), Some(sel)) = (&mut self.open, self.selection) else {
            return None;
        };
        let Some(cat) = self.config.categories.get(usize::from(category) - 1) else {
            self.error(format!("No category {category}"));
            return None;
        };
        let Some(color) = parse_color(&cat.color) else {
            let msg = format!("Bad color for category {}: {}", cat.name, cat.color);
            self.error(msg);
            return None;
        };
        let mut by_page: BTreeMap<usize, Vec<BBox>> = BTreeMap::new();
        for (page, b) in sel.rects(&open.texts) {
            by_page.entry(page).or_default().push(b);
        }
        let mut group = Vec::new();
        for (page, rects) in by_page {
            let spec = AnnotSpec {
                page,
                kind,
                rects,
                color,
                opacity: self.config.annotation.highlight_opacity,
                stroke_pt: 0.0,
                subject: cat.name.clone(),
                author: self.author.clone(),
                contents: None,
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
                edit: Edit::Create(spec.clone()),
            });
            group.push(Created { spec, tag });
        }
        let first = group.first().map(|c| (c.tag, c.spec.clone()));
        open.undo.push(UndoOp {
            created: group,
            ..UndoOp::default()
        });
        self.last_category = category;
        self.leave_text_modes();
        first
    }

    /// Visual `c`: highlight in the last-used category, then edit the
    /// comment of its first part. The comment belongs to the same undo
    /// step; cancelling keeps the highlight.
    fn comment_selection(&mut self) {
        let Some((tag, spec)) = self.annotate(AnnotKind::Highlight, self.last_category) else {
            return;
        };
        let anchor = spec
            .rects
            .iter()
            .copied()
            .reduce(BBox::union)
            .unwrap_or_default();
        let target = comment::Target::Created {
            page: spec.page,
            tag,
            note: false,
        };
        self.editor = Some(CommentEditor::new(target, anchor, String::new()));
        self.mode = Mode::Insert;
    }

    /// `i`: a note in the last-used category with its icon above the
    /// caret, or at the top left of the current page's visible part; the
    /// comment editor opens on it.
    fn new_note(&mut self) {
        let Some(open) = &mut self.open else { return };
        let Some(cat) = self
            .config
            .categories
            .get(usize::from(self.last_category) - 1)
        else {
            return;
        };
        let Some(color) = parse_color(&cat.color) else {
            let msg = format!("Bad color for category {}: {}", cat.name, cat.color);
            self.error(msg);
            return;
        };
        let size = self.config.annotation.note_size_pt;
        let at_caret = self.caret.and_then(|c| {
            let t = open.texts.get(c.pos.page)?.as_ref()?;
            let b = t.chars.get(c.pos.idx)?.bbox;
            Some((c.pos.page, b.x0, (b.y0 - size).max(0.0)))
        });
        let (page, x, y) = at_caret.unwrap_or_else(|| {
            let page = open.view.current_page();
            let top = -open.view.page_rect_on_screen(page).y / open.view.zoom();
            (page, size / 2.0, top.max(0.0) + size / 2.0)
        });
        let spec = AnnotSpec {
            page,
            kind: AnnotKind::Note,
            rects: vec![BBox::new(x, y, x + size, y + size)],
            color,
            opacity: 1.0,
            stroke_pt: 0.0,
            subject: cat.name.clone(),
            author: self.author.clone(),
            contents: None,
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
            edit: Edit::Create(spec.clone()),
        });
        open.undo.push(UndoOp {
            created: vec![Created {
                spec: spec.clone(),
                tag,
            }],
            ..UndoOp::default()
        });
        self.leave_text_modes();
        let target = comment::Target::Created {
            page,
            tag,
            note: true,
        };
        self.editor = Some(CommentEditor::new(target, spec.rects[0], String::new()));
        self.mode = Mode::Insert;
    }

    /// Undo the last annotation action, or redo the last undone one.
    fn undo(&mut self, redo: bool) {
        let Some(open) = &mut self.open else { return };
        let step = if redo {
            open.undo.redo()
        } else {
            open.undo.undo()
        };
        let Some(op) = step else {
            let msg = if redo {
                "Nothing to redo"
            } else {
                "Nothing to undo"
            };
            self.show(Level::Info, msg.into());
            return;
        };
        let mut edits = Vec::new();
        if redo {
            for &(page, xref) in &op.deleted {
                edits.push(Edit::Delete { page, xref });
            }
            for c in &op.changed {
                edits.push(Edit::Update {
                    page: c.page,
                    xref: c.xref,
                    change: c.after.clone(),
                });
            }
            // Undo needed the object numbers, so they are known here.
            for c in &op.created {
                if let Some(&xref) = open.xrefs.get(&c.tag) {
                    edits.push(Edit::Restore {
                        page: c.spec.page,
                        xref,
                    });
                }
            }
        } else {
            let created: Option<Vec<Edit>> = op
                .created
                .iter()
                .map(|c| {
                    open.xrefs.get(&c.tag).map(|&xref| Edit::Delete {
                        page: c.spec.page,
                        xref,
                    })
                })
                .collect();
            let Some(created) = created else {
                // The creation has not been confirmed yet; try again later.
                open.undo.redo();
                return;
            };
            edits.extend(created);
            for c in op.changed.iter().rev() {
                edits.push(Edit::Update {
                    page: c.page,
                    xref: c.xref,
                    change: c.before.clone(),
                });
            }
            for &(page, xref) in &op.deleted {
                edits.push(Edit::Restore { page, xref });
            }
        }
        for edit in edits {
            let tag = self.next_tag;
            self.next_tag += 1;
            self.doc.send(Request::Edit { tag, edit });
        }
        self.selected_annot = None;
        self.show(Level::Info, if redo { "Redone" } else { "Undone" }.into());
    }

    /// All annotations listed so far.
    fn known_annots(&self) -> Vec<AnnotInfo> {
        self.open
            .as_ref()
            .map(|o| o.annots.iter().flatten().flatten().cloned().collect())
            .unwrap_or_default()
    }

    /// The category of an annotation (§2.8), 1-based; `None`: Uncategorised.
    fn category(&self, a: &AnnotInfo) -> Option<u8> {
        let cats: Vec<_> = self
            .config
            .categories
            .iter()
            .map(|c| (c.name.as_str(), parse_color(&c.color)))
            .collect();
        category_of(a, &cats).and_then(|i| u8::try_from(i + 1).ok())
    }

    fn category_name(&self, a: &AnnotInfo) -> &str {
        self.category(a)
            .and_then(|c| self.config.categories.get(usize::from(c) - 1))
            .map_or("Uncategorised", |c| c.name.as_str())
    }

    /// Select an annotation (or none), and say what it is.
    fn select_annot(&mut self, a: Option<&AnnotInfo>) {
        self.selected_annot = a.map(|a| (a.page, a.xref));
        let Some(a) = a else { return };
        let kind = match a.kind.as_str() {
            "Text" => "Note",
            "Square" => "Box",
            k => k,
        };
        let mut text = format!("{kind} p. {}  {}", a.page + 1, self.category_name(a));
        if let Some(c) = &a.contents {
            let c: String = c.chars().take(80).collect();
            text.push_str(&format!("  “{}”", c.replace('\n', " ")));
        }
        self.show(Level::Info, text);
    }

    /// `]a` / `[a`: the next or previous annotation in reading order, from
    /// the selected one or from the top of the view; scrolls to it.
    fn step_annotation(&mut self, forward: bool, count: u32) {
        let Some(open) = &self.open else { return };
        let all = self.known_annots();
        let mut from = match self.selected_annot {
            Some((page, xref)) => AnnotFrom::Selected { page, xref },
            None => {
                let top = open.view.position();
                AnnotFrom::Pos {
                    page: top.page,
                    y: top.y,
                }
            }
        };
        let mut found = None;
        for _ in 0..count.max(1) {
            match step_annot(&all, from, forward) {
                Some(a) => {
                    from = AnnotFrom::Selected {
                        page: a.page,
                        xref: a.xref,
                    };
                    found = Some(a.clone());
                }
                None => break,
            }
        }
        let Some(a) = found else {
            let loading = open.annots.iter().any(|p| p.is_none());
            let msg = match (loading, forward) {
                (true, _) => "Still reading the annotations",
                (false, true) => "No more annotations below",
                (false, false) => "No more annotations above",
            };
            self.show(Level::Info, msg.into());
            return;
        };
        self.reveal_annot(&a);
    }

    /// Scroll an annotation into view (a third from the top, if it was
    /// not visible) and select it.
    fn reveal_annot(&mut self, a: &AnnotInfo) {
        if let Some(open) = &mut self.open {
            let v = &mut open.view;
            let r = select::screen_rect(
                v,
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::from(v.viewport())),
                a.page,
                a.rect,
            );
            let (vw, vh) = v.viewport();
            if r.min.y < 0.0 || r.max.y > vh || r.max.x < 0.0 || r.min.x > vw {
                let x = r.min.x.clamp(0.0, vw);
                v.put_doc_pos_at(
                    marg_core::view::DocPos {
                        page: a.page,
                        x: a.rect.x0,
                        y: a.rect.y0,
                    },
                    x,
                    vh / 3.0,
                );
            }
        }
        self.leave_text_modes();
        self.select_annot(Some(a));
    }

    /// Visual `d`: remove highlights and underlines from the selected text.
    /// The parts of them outside the selection become new annotations with
    /// the same color, category and author (the comment stays with the first
    /// part). One undo step.
    /// With `whole` (Visual `D`), the touched annotations are deleted
    /// completely instead.
    fn erase_selection(&mut self, whole: bool) {
        let (Some(open), Some(sel)) = (&mut self.open, self.selection) else {
            return;
        };
        let mut by_page: BTreeMap<usize, HashSet<usize>> = BTreeMap::new();
        for (page, _, r) in sel.line_parts(&open.texts) {
            by_page.entry(page).or_default().extend(r);
        }
        if by_page.keys().any(|&p| open.annots[p].is_none()) {
            self.show(Level::Info, "Still reading the annotations".into());
            return;
        }
        let mut op = UndoOp::default();
        let mut edits = Vec::new();
        for (page, selected) in &by_page {
            let (Some(list), Some(t)) = (&open.annots[*page], &open.texts[*page]) else {
                continue;
            };
            for a in list {
                let Some(kind) = annot_kind(&a.kind) else {
                    continue;
                };
                let Some(groups) = erase(a, t, selected) else {
                    continue;
                };
                op.deleted.push((*page, a.xref));
                edits.push((
                    self.next_tag,
                    Edit::Delete {
                        page: *page,
                        xref: a.xref,
                    },
                ));
                self.next_tag += 1;
                let groups = if whole { Vec::new() } else { groups };
                for (i, rects) in groups.into_iter().enumerate() {
                    let spec = AnnotSpec {
                        page: *page,
                        kind,
                        rects,
                        color: a.color.unwrap_or([0xFF, 0xD4, 0x00]),
                        opacity: a.opacity,
                        stroke_pt: 0.0,
                        subject: a.subject.clone().unwrap_or_default(),
                        author: a.author.clone().unwrap_or_else(|| self.author.clone()),
                        contents: if i == 0 { a.contents.clone() } else { None },
                    };
                    let tag = self.next_tag;
                    self.next_tag += 1;
                    open.provisional.push(Provisional {
                        tag,
                        rev: None,
                        spec: spec.clone(),
                    });
                    edits.push((tag, Edit::Create(spec.clone())));
                    op.created.push(Created { spec, tag });
                }
            }
        }
        if op.deleted.is_empty() {
            self.show(Level::Info, "Nothing highlighted in the selection".into());
            return;
        }
        let n = op.deleted.len();
        open.undo.push(op);
        for (tag, edit) in edits {
            self.doc.send(Request::Edit { tag, edit });
        }
        self.leave_text_modes();
        let what = if n == 1 { "annotation" } else { "annotations" };
        let verb = if whole { "Deleted" } else { "Erased from" };
        self.show(Level::Info, format!("{verb} {n} {what} (u to undo)"));
    }

    /// `x`: delete the selected annotation (undo puts it back).
    fn delete_annotation(&mut self) {
        let Some((page, xref)) = self.selected_annot.take() else {
            self.show(
                Level::Info,
                "No annotation selected (]a, [a or click)".into(),
            );
            return;
        };
        let Some(open) = &mut self.open else { return };
        open.undo.push(UndoOp {
            deleted: vec![(page, xref)],
            ..UndoOp::default()
        });
        let tag = self.next_tag;
        self.next_tag += 1;
        self.doc.send(Request::Edit {
            tag,
            edit: Edit::Delete { page, xref },
        });
        self.show(Level::Info, "Deleted (u to undo)".into());
    }

    /// The selected annotation, if it is listed.
    fn selected_info(&self) -> Option<&AnnotInfo> {
        let (page, xref) = self.selected_annot?;
        self.open.as_ref()?.annots[page]
            .as_ref()?
            .iter()
            .find(|a| a.xref == xref)
    }

    /// Change the selected annotation in place (one undo step); `change`
    /// gives the new values, and the old ones are taken from the listing.
    fn change_annotation(&mut self, change: AnnotChange, msg: String) {
        let Some(a) = self.selected_info().cloned() else {
            self.show(
                Level::Info,
                "No annotation selected (]a, [a or click)".into(),
            );
            return;
        };
        self.change_listed(&a, change, msg);
    }

    /// Change a listed annotation in place (one undo step).
    fn change_listed(&mut self, a: &AnnotInfo, change: AnnotChange, msg: String) {
        let before = AnnotChange {
            color: change.color.and(a.color),
            opacity: change.opacity.map(|_| a.opacity),
            subject: change
                .subject
                .as_ref()
                .map(|_| a.subject.clone().unwrap_or_default()),
            contents: change
                .contents
                .as_ref()
                .map(|_| a.contents.clone().unwrap_or_default()),
            kind: change.kind.and(annot_kind(&a.kind)),
        };
        let Some(open) = &mut self.open else { return };
        let edit = Edit::Update {
            page: a.page,
            xref: a.xref,
            change: change.clone(),
        };
        open.undo.push(UndoOp {
            changed: vec![Changed {
                page: a.page,
                xref: a.xref,
                before,
                after: change,
            }],
            ..UndoOp::default()
        });
        let tag = self.next_tag;
        self.next_tag += 1;
        self.doc.send(Request::Edit { tag, edit });
        self.show(Level::Info, msg);
    }

    /// `1`–`5` on a selected annotation: give it that category.
    fn recolor_annotation(&mut self, category: u8) {
        let Some(cat) = self.config.categories.get(usize::from(category) - 1) else {
            self.error(format!("No category {category}"));
            return;
        };
        let Some(color) = parse_color(&cat.color) else {
            let msg = format!("Bad color for category {}: {}", cat.name, cat.color);
            self.error(msg);
            return;
        };
        let change = AnnotChange {
            color: Some(color),
            subject: Some(cat.name.clone()),
            ..AnnotChange::default()
        };
        let msg = format!("Category: {}", cat.name);
        self.change_annotation(change, msg);
    }

    /// `U` on a selected annotation: switch between highlight and underline.
    fn toggle_annotation_type(&mut self) {
        let Some(a) = self.selected_info() else {
            self.show(Level::Info, "No annotation selected".into());
            return;
        };
        let (kind, opacity, msg) = match annot_kind(&a.kind) {
            Some(AnnotKind::Highlight) => (AnnotKind::Underline, 1.0, "Underline"),
            Some(AnnotKind::Underline) => (
                AnnotKind::Highlight,
                self.config.annotation.highlight_opacity,
                "Highlight",
            ),
            _ => {
                let msg = format!("Only highlights and underlines can switch ({})", a.kind);
                self.show(Level::Info, msg);
                return;
            }
        };
        let change = AnnotChange {
            kind: Some(kind),
            opacity: Some(opacity),
            ..AnnotChange::default()
        };
        self.change_annotation(change, msg.into());
    }

    /// `e`: edit the comment of the selected annotation.
    fn edit_comment(&mut self) {
        let Some(a) = self.selected_info() else {
            self.show(
                Level::Info,
                "No annotation selected (]a, [a or click)".into(),
            );
            return;
        };
        let target = comment::Target::Annot {
            page: a.page,
            xref: a.xref,
        };
        let text = a.contents.clone().unwrap_or_default();
        self.editor = Some(CommentEditor::new(target, a.rect, text));
        self.mode = Mode::Insert;
    }

    /// `Ctrl-Enter` and Esc in the comment editor.
    fn editor_keys(&mut self, ctx: &egui::Context) {
        let Some(ed) = &mut self.editor else {
            self.mode = Mode::Normal;
            return;
        };
        // Cancelling saves nothing (a new note without text is removed).
        let (text, save) = match ed.keys(ctx) {
            comment::Outcome::Editing => return,
            comment::Outcome::Cancel => (String::new(), false),
            comment::Outcome::Save(text) => (text, true),
        };
        let Some(ed) = self.editor.take() else { return };
        self.mode = Mode::Normal;
        if let comment::Target::Created {
            page,
            tag,
            note: true,
        } = ed.target
            && text.is_empty()
        {
            self.drop_new_note(page, tag);
            return;
        }
        if !save || text == ed.original() {
            return;
        }
        match ed.target {
            comment::Target::Annot { page, xref } => {
                let Some(a) = self.open.as_ref().and_then(|o| {
                    o.annots[page]
                        .as_ref()
                        .and_then(|l| l.iter().find(|a| a.xref == xref))
                        .cloned()
                }) else {
                    self.error("The annotation is gone; comment not saved".into());
                    return;
                };
                let change = AnnotChange {
                    contents: Some(text),
                    ..AnnotChange::default()
                };
                self.change_listed(&a, change, "Comment saved".into());
            }
            comment::Target::Created { page, tag, .. } => {
                // Part of the creation's undo step: redo puts the same
                // object back, comment included.
                let Some(open) = &mut self.open else { return };
                match open.xrefs.get(&tag) {
                    Some(&xref) => {
                        self.doc.send(Request::Edit {
                            tag: self.next_tag,
                            edit: comment_edit(page, xref, text),
                        });
                        self.next_tag += 1;
                    }
                    None => {
                        open.pending.insert(tag, AfterCreate::Comment(text));
                    }
                }
                self.show(Level::Info, "Comment saved".into());
            }
        }
    }

    /// A new note left without text: delete it, and forget its undo step.
    fn drop_new_note(&mut self, page: usize, tag: u64) {
        let Some(open) = &mut self.open else { return };
        if open
            .undo
            .discard_last()
            .is_some_and(|op| !op.created.iter().any(|c| c.tag == tag))
        {
            log::warn!("dropping a new note: the last undo step was not its creation");
        }
        open.provisional.retain(|p| p.tag != tag);
        match open.xrefs.get(&tag) {
            Some(&xref) => {
                self.doc.send(Request::Edit {
                    tag: self.next_tag,
                    edit: Edit::Delete { page, xref },
                });
                self.next_tag += 1;
            }
            None => {
                open.pending.insert(tag, AfterCreate::Delete);
            }
        }
        self.show(Level::Info, "Empty note removed".into());
    }

    /// The comment of the annotation under the mouse, else of the selected
    /// one, next to it. Not while editing or picking hints.
    fn comment_tooltip(&self, ctx: &egui::Context, area: egui::Rect, screen_h: f32) {
        if !matches!(self.mode, Mode::Normal | Mode::Caret | Mode::Visual) {
            return;
        }
        let Some(open) = &self.open else { return };
        let hovered = ctx
            .input(|i| i.pointer.hover_pos())
            .filter(|p| area.contains(*p))
            .and_then(|p| self.annot_at_screen(p))
            .filter(|a| a.contents.is_some());
        let Some(a) = hovered.or_else(|| self.selected_info().cloned()) else {
            return;
        };
        let Some(text) = &a.contents else { return };
        let anchor = select::screen_rect(&open.view, area, a.page, a.rect);
        let ui_cfg = &self.config.ui;
        comment::tooltip(
            ctx,
            &self.theme,
            text,
            anchor,
            area,
            ui_cfg.comment_width_px,
            ui_cfg.comment_max_height * screen_h,
        );
    }

    /// The annotation under a screen position, if any.
    fn annot_at_screen(&self, pos: egui::Pos2) -> Option<AnnotInfo> {
        let open = self.open.as_ref()?;
        let rel = pos - self.area.min;
        let d = open.view.doc_pos_at(rel.x, rel.y);
        let list = open.annots[d.page].as_ref()?;
        annot_at(list, d.page, d.x, d.y).cloned()
    }

    /// The category prompt: a digit highlights with that category, Enter
    /// with the last one, Esc goes back to the selection.
    fn category_event(&mut self, e: &Event) {
        let Some(k) = input::key_press(e) else { return };
        match k.code {
            KeyCode::Char(c) if !k.ctrl && !k.alt => {
                let n = self.config.categories.len().min(9);
                if let Some(d) = c.to_digit(10).filter(|&d| d >= 1 && d as usize <= n) {
                    self.annotate(AnnotKind::Highlight, d as u8);
                }
            }
            KeyCode::Named(Named::Enter) => {
                self.annotate(AnnotKind::Highlight, self.last_category);
            }
            KeyCode::Named(Named::Esc) => self.mode = Mode::Visual,
            _ => {}
        }
    }

    /// The status line of the category prompt.
    fn category_prompt(&self) -> String {
        let mut out = String::from("highlight:");
        for (i, c) in self.config.categories.iter().take(9).enumerate() {
            out.push_str(&format!("  {} {}", i + 1, c.name));
        }
        if let Some(last) = self
            .config
            .categories
            .get(usize::from(self.last_category) - 1)
        {
            out.push_str(&format!("   ⏎ {}", last.name));
        }
        out
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

    /// A key in the search prompt. The search runs as the query is typed;
    /// Enter keeps it, Esc goes back to where the search started.
    fn search_event(&mut self, e: &Event) {
        match e {
            Event::Text(t) => {
                self.searchline.insert(t);
                self.search_typed();
            }
            Event::Key { pressed: true, .. } => {
                let Some(k) = input::key_press(e) else { return };
                match k.code {
                    KeyCode::Named(Named::Esc) => self.cancel_search(),
                    KeyCode::Named(Named::Backspace) if self.searchline.is_empty() => {
                        self.cancel_search()
                    }
                    KeyCode::Named(Named::Enter) => {
                        self.mode = self.return_mode;
                        let typed = self.searchline.submit();
                        let (Some(open), Some(origin)) = (&mut self.open, self.search_origin)
                        else {
                            return;
                        };
                        if typed.trim().is_empty() {
                            // Like vim: an empty search repeats the last one.
                            let last = self.searchline.last().unwrap_or_default();
                            if last.trim().is_empty() {
                                open.search = None;
                                return;
                            }
                            self.start_search(&last, origin);
                        }
                        if let Some(open) = &mut self.open {
                            open.jumps.push(origin);
                        }
                    }
                    KeyCode::Named(n) => {
                        match n {
                            Named::Backspace => self.searchline.backspace(),
                            Named::Delete => self.searchline.delete(),
                            Named::Left => self.searchline.left(),
                            Named::Right => self.searchline.right(),
                            Named::Home => self.searchline.home(),
                            Named::End => self.searchline.end(),
                            Named::Up => self.searchline.history_prev(),
                            Named::Down => self.searchline.history_next(),
                            _ => return,
                        }
                        self.search_typed();
                    }
                    KeyCode::Char(_) => {}
                }
            }
            _ => {}
        }
    }

    /// The search prompt changed: search for the new text.
    fn search_typed(&mut self) {
        let (Some(open), Some(origin)) = (&mut self.open, self.search_origin) else {
            return;
        };
        let query = self.searchline.text().to_string();
        if query.trim().is_empty() {
            open.search = None;
            self.doc.send(Request::Search {
                id: 0,
                query: String::new(),
                from: 0,
            });
            open.view.set_position(origin);
            return;
        }
        if open.search.as_ref().is_some_and(|s| s.query == query) {
            return;
        }
        self.start_search(&query, origin);
    }

    /// Search for `query`; go to the first hit at or after `from`.
    fn start_search(&mut self, query: &str, from: DocPos) {
        let Some(open) = &mut self.open else { return };
        self.next_search_id += 1;
        let id = self.next_search_id;
        self.doc.send(Request::Search {
            id,
            query: query.to_string(),
            from: from.page,
        });
        open.search = Some(SearchState {
            id,
            query: query.to_string(),
            hits: Vec::new(),
            done: false,
            current: None,
            seek: Some(from),
        });
    }

    fn cancel_search(&mut self) {
        self.mode = self.return_mode;
        if let (Some(open), Some(origin)) = (&mut self.open, self.search_origin) {
            open.search = None;
            open.view.set_position(origin);
        }
    }

    /// `n` / `N`: the next or previous hit, from the current one if it is
    /// in view, else from the view. After Esc, `n` searches again.
    fn step_hit(&mut self, forward: bool, count: u32) {
        let Some(open) = &mut self.open else { return };
        let Some(s) = &mut open.search else {
            match self.searchline.last().filter(|q| !q.trim().is_empty()) {
                Some(q) => {
                    let from = open.view.position();
                    open.jumps.push(from);
                    self.start_search(&q, from);
                }
                None => self.show(Level::Info, "No search".into()),
            }
            return;
        };
        if s.hits.is_empty() {
            let msg = if s.done {
                format!("Not found: {}", s.query)
            } else {
                "Still searching".into()
            };
            self.show(Level::Info, msg);
            return;
        }
        let len = s.hits.len();
        let visible = open.view.visible_pages();
        let base = s.current.filter(|&c| visible.contains(&s.hits[c].page));
        let n = count.max(1) as usize % len;
        let i = match (base, forward) {
            (Some(c), true) => (c + n) % len,
            (Some(c), false) => (c + len - n) % len,
            (None, true) => {
                let pos = open.view.position();
                (s.first_from(pos).unwrap_or(0) + n - 1) % len
            }
            (None, false) => {
                let pos = open.view.position();
                let first = s.first_from(pos).unwrap_or(len);
                (first + len - n) % len
            }
        };
        open.jumps.push(open.view.position());
        s.current = Some(i);
        s.seek = None;
        let h = &s.hits[i];
        select::reveal(&mut open.view, h.page, h.rects[0]);
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
            Command::Quit => {
                if self.open.as_ref().is_some_and(|o| o.unsaved()) {
                    self.save(false, AfterSave::Quit);
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                return;
            }
            Command::QuitForce => {
                self.discard = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
            Command::Save | Command::SaveForce => {
                self.save(command == Command::SaveForce, AfterSave::Report);
                return;
            }
            Command::Reload => {
                self.reload();
                return;
            }
            Command::AnnotNext | Command::AnnotPrev => {
                self.step_annotation(command == Command::AnnotNext, n);
                return;
            }
            Command::AnnotDelete => {
                self.delete_annotation();
                return;
            }
            Command::AnnotRecolor(c) => {
                self.recolor_annotation(c);
                return;
            }
            Command::AnnotToggleType => {
                self.toggle_annotation_type();
                return;
            }
            Command::AnnotEdit => {
                self.edit_comment();
                return;
            }
            Command::Erase | Command::EraseWhole => {
                self.erase_selection(command == Command::EraseWhole);
                return;
            }
            Command::Close => {
                if self.open.as_ref().is_some_and(|o| o.unsaved()) {
                    self.save(false, AfterSave::Close);
                } else {
                    self.close_doc(ctx);
                }
                return;
            }
            Command::Undo | Command::Redo => {
                self.undo(command == Command::Redo);
                return;
            }
            Command::Open(path) => {
                self.open_path(&path);
                return;
            }
            Command::WriteConfig => {
                self.write_config();
                return;
            }
            Command::Export(path) => {
                self.start_export(path.as_deref());
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
            Command::HintAnnot => {
                self.annot_hints();
                return;
            }
            Command::HintLinks => {
                self.link_hints();
                return;
            }
            Command::Help | Command::HelpPage => {
                self.show_help(command == Command::HelpPage);
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
            Command::Comment => {
                self.comment_selection();
                return;
            }
            Command::Note => {
                self.new_note();
                return;
            }
            Command::AnnotList => {
                self.open_annot_list();
                return;
            }
            Command::Search => {
                let Some(open) = &self.open else { return };
                self.search_origin = Some(open.view.position());
                self.return_mode = self.mode;
                self.mode = Mode::Search;
                self.searchline.clear();
                return;
            }
            Command::SearchNext | Command::SearchPrev => {
                self.step_hit(command == Command::SearchNext, n);
                return;
            }
            Command::Outline => {
                let Some(read) = self.open.as_ref().map(|o| o.outline.is_some()) else {
                    return;
                };
                self.leave_text_modes();
                self.selected_annot = None;
                self.mode = Mode::Outline;
                if read {
                    self.show_outline();
                } else {
                    self.doc.send(Request::Outline);
                }
                return;
            }
            Command::MarkSet | Command::MarkJump => {
                if self.open.is_some() {
                    self.pending_mark = Some(command == Command::MarkSet);
                }
                return;
            }
            Command::JumpBack | Command::JumpForward => {
                let Some(open) = &mut self.open else { return };
                let here = open.view.position();
                let to = if command == Command::JumpBack {
                    open.jumps.back(here)
                } else {
                    open.jumps.forward(here)
                };
                match to {
                    Some(to) => open.view.set_position(to),
                    None => self.show(Level::Info, "No more jumps".into()),
                }
                return;
            }
            Command::Recolor => {
                self.set_recolor(!self.recolor);
                let msg = if self.recolor {
                    "Dark pages on"
                } else {
                    "Dark pages off"
                };
                self.show(Level::Info, msg.into());
                return;
            }
            Command::BoxMode => {
                self.leave_text_modes();
                self.selected_annot = None;
                let cfg = &self.config.annotation;
                let chars: Vec<char> = self.config.hints.chars.chars().collect();
                self.boxing = Some(BoxMode::new(
                    cfg.box_grid_cols,
                    cfg.box_grid_rows,
                    &dedup(chars),
                    self.last_category,
                ));
                self.mode = Mode::Box;
                return;
            }
            Command::PickCategory => {
                if self.selection.is_some() {
                    self.mode = Mode::Category;
                }
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
        let from = v.position();
        if matches!(
            command,
            Command::PageFirst | Command::PageLast | Command::PageGoto(_)
        ) {
            open.jumps.push(from);
        }
        match command {
            Command::ScrollDown => v.scroll_by(0.0, step),
            Command::ScrollUp => v.scroll_by(0.0, -step),
            Command::ScrollLeft => v.scroll_by(-step, 0.0),
            Command::ScrollRight => v.scroll_by(step, 0.0),
            Command::HalfPageDown => v.scroll_viewports(0.5 * n as f32),
            Command::HalfPageUp => v.scroll_viewports(-0.5 * n as f32),
            Command::PageNext => v.page_step(n as isize),
            Command::PagePrev => v.page_step(-(n as isize)),
            Command::ScreenDown => (0..n).for_each(|_| v.screen_step(true)),
            Command::ScreenUp => (0..n).for_each(|_| v.screen_step(false)),
            Command::SetLayout(l) => v.set_layout(l),
            Command::LayoutCycle => v.set_layout(match v.layout() {
                Layout::Continuous => Layout::Single,
                Layout::Single => Layout::Spread,
                Layout::Spread => Layout::Continuous,
            }),
            Command::SpreadFirstAlone => v.set_first_page_alone(!v.first_page_alone()),
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
            | Command::Export(_)
            | Command::WriteConfig
            | Command::Version
            | Command::DebugToggle => {}
            Command::HintCaret
            | Command::HintSelect
            | Command::HintAnnot
            | Command::HintLinks
            | Command::Help
            | Command::HelpPage
            | Command::Motion(_)
            | Command::VisualChar
            | Command::VisualLine
            | Command::VisualSwap
            | Command::Copy
            | Command::Highlight(_)
            | Command::Underline
            | Command::PickCategory
            | Command::Save
            | Command::SaveForce
            | Command::Reload
            | Command::Close
            | Command::AnnotNext
            | Command::AnnotPrev
            | Command::AnnotDelete
            | Command::AnnotRecolor(_)
            | Command::AnnotToggleType
            | Command::AnnotEdit
            | Command::Comment
            | Command::Note
            | Command::BoxMode
            | Command::AnnotList
            | Command::Recolor
            | Command::JumpBack
            | Command::JumpForward
            | Command::MarkSet
            | Command::MarkJump
            | Command::Outline
            | Command::Search
            | Command::SearchNext
            | Command::SearchPrev
            | Command::Erase
            | Command::EraseWhole
            | Command::Undo
            | Command::Redo => {}
        }
    }

    fn open_path(&mut self, path: &str) {
        let path = complete::expand_tilde(path);
        if !Path::new(&path).is_file() {
            self.error(format!("No such file: {}", path.display()));
            return;
        }
        self.start_page = None;
        if self.open.as_ref().is_some_and(|o| o.unsaved()) {
            self.save(false, AfterSave::Open(path));
        } else {
            self.send_open(path, None);
        }
    }

    /// `:config`: write the commented default config, unless a config
    /// file exists.
    fn write_config(&mut self) {
        let Some(path) = self.config_path.clone() else {
            self.error("No config folder on this system".into());
            return;
        };
        let write = || -> std::io::Result<()> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut f = std::fs::File::create_new(&path)?;
            std::io::Write::write_all(&mut f, marg_core::default_config::text().as_bytes())
        };
        match write() {
            Ok(()) => self.show(Level::Info, format!("Wrote {}", complete::tilde(&path))),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => self.show(
                Level::Info,
                format!("{} exists; not changed", complete::tilde(&path)),
            ),
            Err(e) => self.error(format!("Cannot write {}: {e}", complete::tilde(&path))),
        }
    }

    /// `:export md [path]`: list the annotations again, then write the file once everything is read
    /// (`poll_export`). `path` may be a folder.
    fn start_export(&mut self, path: Option<&str>) {
        let Some(open) = &mut self.open else {
            self.error("No file open".into());
            return;
        };
        let pdf = &open.info.path;
        let default_name = || {
            let stem = pdf.file_stem().unwrap_or_default().to_string_lossy();
            format!("{stem}.notes.md")
        };
        let path = match path {
            None => pdf.with_file_name(default_name()),
            Some(p) => {
                let p = complete::expand_tilde(p);
                if p.is_dir() {
                    p.join(default_name())
                } else {
                    p
                }
            }
        };
        // Edits are applied as they arrive and listings answered later, so
        // listing every page again includes every edit sent so far. Idle:
        // renders go first.
        let pages: Vec<usize> = (0..open.annots.len()).collect();
        let relist = pages.iter().copied().collect();
        self.doc.send(Request::Annots {
            pages,
            urgent: false,
        });
        open.export = Some(PendingExport { path, relist });
        self.show(Level::Info, "Exporting…".into());
        self.poll_export();
    }

    /// Write the pending export once the annotations of every page and the
    /// text of every annotated page are read (or failed).
    fn poll_export(&mut self) {
        let Some(open) = &mut self.open else { return };
        let Some(x) = &open.export else { return };
        if !x.relist.is_empty() {
            return;
        }
        let annotated: Vec<usize> = (0..open.annots.len())
            .filter(|&p| open.annots[p].as_ref().is_some_and(|a| !a.is_empty()))
            .collect();
        let mut waiting = false;
        for &p in &annotated {
            if open.texts[p].is_none() && !open.text_failed.contains(&p) {
                open.request_text(&self.doc, p);
                waiting = true;
            }
        }
        if waiting {
            return;
        }
        let Some(x) = open.export.take() else { return };
        let all: Vec<AnnotInfo> = open.annots.iter().flatten().flatten().cloned().collect();
        let cats: Vec<_> = self
            .config
            .categories
            .iter()
            .map(|c| (c.name.as_str(), parse_color(&c.color)))
            .collect();
        let entries = marg_core::annotlist::entries(&all, &open.texts, &cats);
        let names: Vec<&str> = cats.iter().map(|c| c.0).collect();
        let md = marg_core::export::markdown(
            &marg_core::export::Export {
                file_name: &open.file_name,
                title: open.info.title.as_deref(),
                date: &marg_core::export::date(statefile::now_secs()),
                labels: open.info.labels.as_deref(),
                categories: &names,
            },
            &entries,
        );
        let unread = annotated
            .iter()
            .filter(|p| open.text_failed.contains(p))
            .count()
            + open.annots_failed.len();
        let shown = x.path.file_name().map_or_else(
            || x.path.display().to_string(),
            |n| n.to_string_lossy().into(),
        );
        match std::fs::write(&x.path, md) {
            Ok(()) if unread > 0 => self.show(
                Level::Warn,
                format!(
                    "Exported {} annotations to {shown}; {unread} pages could not be read",
                    entries.len()
                ),
            ),
            Ok(()) => self.show(
                Level::Info,
                format!("Exported {} annotations to {shown}", entries.len()),
            ),
            Err(e) => self.error(format!("Export failed: {}: {e}", x.path.display())),
        }
    }

    /// Close the file (saved or not: callers save first) and show the
    /// recent files list.
    fn close_doc(&mut self, ctx: &egui::Context) {
        if self.open.is_none() {
            return;
        }
        self.selected_annot = None;
        self.save_state();
        self.doc.send(Request::Close);
        self.open = None;
        self.cache.clear();
        self.last_request.clear();
        self.caret = None;
        self.selection = None;
        self.hinting = None;
        self.matcher.reset();
        self.recent = Some(RecentList::new(
            &self.state,
            self.config.ui.recent_files_max,
        ));
        self.mode = Mode::Recent;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("marg".into()));
    }

    /// Open the current file again from disk, keeping the position and
    /// dropping unsaved changes.
    fn reload(&mut self) {
        let Some(open) = &self.open else { return };
        let (path, password) = (open.info.path.clone(), open.password.clone());
        self.save_state();
        self.reloading = true;
        self.send_open(path, password);
    }

    /// Ask the document thread to open `path`; remembered for the password
    /// prompt and for reloading.
    fn send_open(&mut self, path: PathBuf, password: Option<String>) {
        self.opening = Some((path.clone(), password.clone()));
        self.doc.send(Request::Open { path, password });
    }

    /// A key in the password prompt: Enter tries the password, Esc gives
    /// up on the file.
    fn password_event(&mut self, e: &Event) {
        match e {
            Event::Text(t) => self.passline.insert(t),
            Event::Key { pressed: true, .. } => {
                let Some(k) = input::key_press(e) else { return };
                match k.code {
                    KeyCode::Named(Named::Esc) => {
                        self.passline.clear();
                        self.opening = None;
                        self.mode = self.return_mode;
                        if self.open.is_none() {
                            self.recent = Some(RecentList::new(
                                &self.state,
                                self.config.ui.recent_files_max,
                            ));
                            self.mode = Mode::Recent;
                        }
                        self.show(Level::Info, "Not opened".into());
                    }
                    KeyCode::Named(Named::Enter) => {
                        let password = self.passline.text().to_string();
                        self.passline.clear();
                        if let Some((path, _)) = self.opening.clone() {
                            self.send_open(path, Some(password));
                        }
                    }
                    KeyCode::Named(Named::Backspace) => self.passline.backspace(),
                    KeyCode::Named(Named::Delete) => self.passline.delete(),
                    KeyCode::Named(Named::Left) => self.passline.left(),
                    KeyCode::Named(Named::Right) => self.passline.right(),
                    KeyCode::Named(Named::Home) => self.passline.home(),
                    KeyCode::Named(Named::End) => self.passline.end(),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Ask the document thread whether the file changed, every
    /// `file_check_secs`.
    fn check_disk(&mut self, ctx: &egui::Context) {
        if self.open.is_none() {
            return;
        }
        let now = Instant::now();
        let every = Duration::from_secs_f32(self.config.ui.file_check_secs);
        if now >= self.next_disk_check {
            self.doc.send(Request::CheckDisk);
            self.next_disk_check = now + every;
        }
        ctx.request_repaint_after(self.next_disk_check - now);
    }

    /// The files the config comes from: the config and its theme.
    fn config_files(&self) -> Vec<PathBuf> {
        let Some(path) = &self.config_path else {
            return Vec::new();
        };
        let mut files = vec![path.clone()];
        files.extend(startup::theme_file(path, &self.config));
        files
    }

    /// Every `ui.file_check_secs`: reload the config if it or its theme
    /// changed.
    fn check_config(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if now >= self.next_config_check {
            self.next_config_check =
                now + Duration::from_secs_f32(self.config.ui.file_check_secs.max(0.1));
            let stamp = startup::stamp(&self.config_files());
            if stamp != self.config_stamp {
                self.reload_config(ctx);
            }
        }
        ctx.request_repaint_after(self.next_config_check - now);
    }

    /// Read the config again. A broken file keeps the current config.
    /// Settings used when a file opens (`[view]` layout and size values,
    /// `jump_history`, `undo_steps`) apply to the next file; `cache_mb` on
    /// the next start.
    fn reload_config(&mut self, ctx: &egui::Context) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        let result = startup::read_config(&path);
        // The theme may have changed with the config.
        let (config, mut problems) = match result {
            Ok(c) => c,
            Err(e) => {
                self.config_stamp = startup::stamp(&self.config_files());
                self.error(format!("Config not reloaded: {e}"));
                return;
            }
        };
        let old = std::mem::replace(&mut self.config, config);
        self.config_stamp = startup::stamp(&self.config_files());
        let ([keymap, caret_map, visual_map, annot_map], warnings) = build_keymaps(&self.config);
        (self.keymap, self.caret_map, self.visual_map, self.annot_map) =
            (keymap, caret_map, visual_map, annot_map);
        self.matcher.reset();
        let (left, right, w) = self.config.statusbar.parts();
        problems.extend(w);
        self.status_parts = (left, right);
        if self.config.fonts != old.fonts {
            // From the built-in fonts, also if the new file fails.
            ctx.set_fonts(egui::FontDefinitions::default());
            problems.extend(crate::theme::install_fonts(ctx, &self.config.fonts));
        }
        self.theme = Theme::new(&self.config.colors, &self.config.fonts);
        self.author = annotation_author(&self.config.annotation.author);
        if self.config.view.recolor != old.view.recolor {
            self.set_recolor(self.config.view.recolor);
        } else if self.recolor
            && (self.config.colors.recolor_fg, self.config.colors.recolor_bg)
                != (old.colors.recolor_fg, old.colors.recolor_bg)
        {
            self.set_recolor(true);
        }
        for p in &problems {
            log::error!("{p}");
        }
        for w in &warnings {
            log::warn!("{w}");
        }
        log::info!("config reloaded");
        if let Some(p) = problems.first() {
            self.show(Level::Error, p.clone());
        } else if let Some(w) = warnings.first() {
            self.show(Level::Warn, w.clone());
        } else {
            self.show(Level::Info, "Config reloaded".into());
        }
    }

    /// Ask the document thread to save, then do `after` if it worked.
    fn save(&mut self, force: bool, after: AfterSave) {
        let Some(open) = &mut self.open else { return };
        if after == AfterSave::Report && !open.unsaved() && !force {
            self.show(Level::Info, "Nothing to save".into());
            return;
        }
        self.after_save = after;
        open.saving = true;
        self.doc.send(Request::Save { force });
    }

    /// Autosave once it is due, unless a save is running or the file
    /// changed on disk.
    fn autosave(&mut self, ctx: &egui::Context) {
        let Some(open) = &self.open else { return };
        let Some(due) = open.save_due else { return };
        if open.saving || open.conflict || !open.unsaved() {
            return;
        }
        let now = Instant::now();
        if now >= due {
            self.save(false, AfterSave::Nothing);
        } else {
            ctx.request_repaint_after(due - now);
        }
    }

    /// The text of one status bar part (empty: left out).
    fn status_part(&self, part: StatusPart) -> String {
        let open = self.open.as_ref().filter(|_| self.mode != Mode::Recent);
        match (part, open) {
            (StatusPart::Mode, _) => self.mode.label().into(),
            (StatusPart::Search, Some(open)) => match &open.search {
                None => String::new(),
                Some(s) if s.hits.is_empty() => if s.done { "no hits" } else { "searching" }.into(),
                Some(s) => format!(
                    "hit {}/{}{}",
                    s.current.map_or("-".into(), |c| (c + 1).to_string()),
                    s.hits.len(),
                    if s.done { "" } else { "+" }
                ),
            },
            (StatusPart::Keys, _) => {
                if let Some(set) = self.pending_mark {
                    if set { "m" } else { "'" }.into()
                } else if let Some(h) = self.hinting.as_ref().filter(|h| !h.typed().is_empty()) {
                    h.typed().into()
                } else if let Some(b) = self.boxing.as_ref().filter(|b| !b.typed().is_empty()) {
                    b.typed().into()
                } else if self.matcher.is_pending() {
                    self.matcher.pending_text()
                } else {
                    String::new()
                }
            }
            (_, None) => String::new(),
            (StatusPart::File, Some(open)) => open.file_name.clone(),
            (StatusPart::Page, Some(open)) => {
                let v = &open.view;
                let pages = v.current_pages();
                let count = v.page_count();
                let span = |f: &dyn Fn(usize) -> String| {
                    let last = pages.end - 1;
                    if last > pages.start {
                        format!("{}–{}", f(pages.start), f(last))
                    } else {
                        f(pages.start)
                    }
                };
                let number = span(&|p| (p + 1).to_string());
                match &open.info.labels {
                    Some(labels) if pages.clone().any(|p| labels[p] != (p + 1).to_string()) => {
                        format!("{} ({number}/{count})", span(&|p| labels[p].clone()))
                    }
                    _ => format!("{number}/{count}"),
                }
            }
            (StatusPart::Zoom, Some(open)) => format!("{:.0}%", open.view.zoom() * 100.0),
            (StatusPart::Layout, Some(open)) => match open.view.layout() {
                Layout::Continuous => "continuous".into(),
                Layout::Single => "single".into(),
                Layout::Spread => "spread".into(),
            },
            (StatusPart::Unsaved, Some(open)) => {
                if open.unsaved() {
                    "●".into()
                } else {
                    String::new()
                }
            }
        }
    }

    fn status_left(&self) -> String {
        let sep = &self.config.statusbar.separator;
        status_line(&self.status_parts.0, sep, |p| self.status_part(p))
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        let th = &self.theme;
        let text = |s: &str, c: Color32| egui::RichText::new(s).monospace().color(c);
        ui.horizontal_centered(|ui| {
            if let Some((prompt, edit)) = match self.mode {
                Mode::Command => Some((':', &self.cmdline)),
                Mode::Search => Some(('/', &self.searchline)),
                _ => None,
            } {
                let line = edit.text();
                let at = line
                    .char_indices()
                    .nth(edit.cursor())
                    .map_or(line.len(), |(i, _)| i);
                let (before, after) = line.split_at(at);
                ui.label(text(&format!("{prompt}{before}▏{after}"), th.statusbar_fg));
            } else if self.mode == Mode::Password {
                let name = self
                    .opening
                    .as_ref()
                    .and_then(|(p, _)| p.file_name())
                    .unwrap_or_default()
                    .to_string_lossy();
                let len = self.passline.text().chars().count();
                let at = self.passline.cursor();
                let (before, after) = ("•".repeat(at), "•".repeat(len - at));
                ui.label(text(
                    &format!("Password for {name}: {before}▏{after}"),
                    th.statusbar_fg,
                ));
            } else {
                let left = if self.mode == Mode::Category {
                    self.category_prompt()
                } else if self.mode == Mode::Box {
                    self.box_prompt()
                } else if let Some(url) = self
                    .pending_url
                    .as_ref()
                    .filter(|_| self.mode == Mode::Confirm)
                {
                    format!("Open {url} ?  y: yes  any other key: no")
                } else {
                    self.status_left()
                };
                ui.label(text(&left, th.statusbar_fg));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let editing = self.editor.as_ref().filter(|_| self.mode == Mode::Insert);
                if let Some(ed) =
                    editing.filter(|_| self.message.as_ref().is_none_or(|m| m.level == Level::Info))
                {
                    ui.label(text(ed.status(), th.statusbar_fg));
                } else if let Some(m) = &self.message {
                    let color = match m.level {
                        Level::Info => th.statusbar_fg,
                        Level::Warn => th.statusbar_warn,
                        Level::Error => th.statusbar_error,
                    };
                    ui.add(egui::Label::new(text(&m.text, color)).truncate());
                } else {
                    let sep = &self.config.statusbar.separator;
                    let right = status_line(&self.status_parts.1, sep, |p| self.status_part(p));
                    ui.label(text(&right, th.statusbar_fg));
                }
            });
        });
    }
}

impl eframe::App for MargApp {
    fn on_exit(&mut self) {
        self.save_state();
        // Last resort: the window closed without going through `q` (which
        // saves first). Save and wait for the result.
        if self.discard || !self.open.as_ref().is_some_and(|o| o.unsaved()) {
            return;
        }
        self.doc.send(Request::Save { force: false });
        let deadline = Instant::now() + Duration::from_secs(10);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match self.doc.recv_timeout(left) {
                Some(Response::Saved { result, .. }) => {
                    match result {
                        Ok(_) => log::info!("saved on exit"),
                        Err(e) => log::error!("save on exit failed: {e:?}; changes lost"),
                    }
                    return;
                }
                Some(_) => {}
                None => break,
            }
        }
        log::error!("save on exit timed out; changes lost");
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        let th = self.theme.clone();
        let ctx = ui.ctx().clone();
        self.handle_responses(&ctx);
        self.poll_export();
        // Closing the window saves first, like `q`.
        if ctx.input(|i| i.viewport().close_requested())
            && !self.discard
            && self.open.as_ref().is_some_and(|o| o.unsaved())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.after_save != AfterSave::Quit {
                self.save(false, AfterSave::Quit);
            }
        }
        self.autosave(&ctx);
        self.check_disk(&ctx);
        self.check_config(&ctx);
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
        // Tall enough for the font.
        let status_h = self
            .config
            .ui
            .status_bar_px
            .max(self.config.fonts.size.clamp(4.0, 64.0) + 8.0);
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
                    .fill(th.statusbar_bg)
                    .inner_margin(egui::Margin::symmetric(6, 0)),
            )
            .show(ui, |ui| self.status_bar(ui));
        if let Some((h, page)) = self.help.as_mut().filter(|_| self.mode == Mode::Help) {
            // The page fills the view; the overlay sits in the middle.
            let rect = if *page {
                area
            } else {
                egui::Rect::from_center_size(
                    area.center(),
                    egui::vec2((area.width() * 0.7).min(760.0), area.height() * 0.85),
                )
            };
            egui::Area::new(egui::Id::unique("marg-help"))
                .fixed_pos(rect.min)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    ui.set_min_size(rect.size());
                    ui.set_max_size(rect.size());
                    h.draw(ui, &th);
                });
        }
        if let Some(list) = self
            .outline_list
            .as_mut()
            .filter(|_| self.mode == Mode::Outline)
        {
            let labels = self.open.as_ref().and_then(|o| o.info.labels.clone());
            let width = screen.width() * self.config.ui.outline_width;
            egui::Area::new(egui::Id::unique("marg-outline"))
                .fixed_pos(area.min)
                .order(egui::Order::Foreground)
                .show(&ctx, |ui| {
                    ui.set_min_size(egui::vec2(width, area.height()));
                    ui.set_max_size(egui::vec2(width, area.height()));
                    list.draw(ui, &th, labels.as_deref());
                });
        }
        if self.mode == Mode::Annots {
            self.refresh_annot_list();
            let names: Vec<String> = self
                .config
                .categories
                .iter()
                .map(|c| c.name.clone())
                .collect();
            let labels = self.open.as_ref().and_then(|o| o.info.labels.clone());
            let width = screen.width() * self.config.ui.annot_list_width;
            // Over the pages, not beside them: the view keeps its size, so
            // pages are not re-fitted and re-rendered.
            if let Some(list) = &mut self.annot_list {
                egui::Area::new(egui::Id::unique("marg-annotations"))
                    .fixed_pos([area.right() - width, area.top()])
                    .order(egui::Order::Foreground)
                    .show(&ctx, |ui| {
                        ui.set_min_size(egui::vec2(width, area.height()));
                        ui.set_max_size(egui::vec2(width, area.height()));
                        list.draw(ui, &th, &names, labels.as_deref());
                    });
            }
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(th.background))
            .show(ui, |ui| {
                let area = ui.max_rect();
                let ppp = ui.ctx().pixels_per_point();
                if self.mode == Mode::Recent
                    && let Some(list) = &mut self.recent
                {
                    list.draw(ui, &th);
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
                let (exact, stale) = self.cache.draw(
                    &painter,
                    area,
                    &open.view,
                    &self.config.render,
                    &open.revs,
                    th.page_edge,
                );
                let cache = &self.cache;
                open.provisional
                    .retain(|p| p.rev.is_none_or(|r| !cache.has_rev(p.spec.page, r)));
                for p in &open.provisional {
                    if p.rev.is_none() || stale.contains(&p.spec.page) {
                        select::draw_provisional(&painter, &open.view, area, &p.spec);
                    }
                }
                if let Some(s) = &open.search {
                    select::draw_hits(&painter, &th, &open.view, area, &s.hits, s.current);
                }
                if let Some(sel) = &self.selection {
                    select::draw_selection(&painter, &th, &open.view, area, sel, &open.texts);
                }
                if matches!(self.mode, Mode::Caret | Mode::Visual)
                    && let Some(c) = &self.caret
                {
                    select::draw_caret(&painter, &th, &open.view, area, c, &open.texts);
                }
                if let Some((page, xref)) = self.selected_annot
                    && let Some(a) = open.annots[page]
                        .as_ref()
                        .and_then(|l| l.iter().find(|a| a.xref == xref))
                {
                    select::draw_selected_annot(&painter, &open.view, area, a);
                }
                if let Some(h) = &self.hinting {
                    h.draw(&painter, &th, &open.view, area, &open.texts);
                }
                if let Some(b) = &self.boxing {
                    let color = self
                        .config
                        .categories
                        .get(usize::from(b.category) - 1)
                        .and_then(|c| parse_color(&c.color))
                        .map_or(Color32::RED, |[r, g, b]| Color32::from_rgb(r, g, b));
                    b.draw(&painter, &th, &open.view, area, color);
                }
                if let Some(t) = &mut self.timing
                    && !t.shown
                    && (exact || self.last_request.is_empty())
                {
                    t.shown = true;
                    t.log("first page shown");
                }
            });
        self.comment_tooltip(&ctx, area, screen.height());
        if let (Some(ed), Some(open)) = (&mut self.editor, &self.open) {
            let anchor = select::screen_rect(&open.view, area, ed.target.page(), ed.anchor);
            let ui_cfg = &self.config.ui;
            ed.show(
                &ctx,
                &th,
                anchor,
                area,
                ui_cfg.comment_width_px,
                ui_cfg.comment_max_height * screen.height(),
            );
        }
        if self.debug {
            self.debug_overlay(&ctx);
        }
        self.frame_ms
            .push(frame_start.elapsed().as_secs_f32() * 1000.0);
    }
}

impl MargApp {
    fn debug_overlay(&self, ctx: &egui::Context) {
        let th = &self.theme;
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
                    .fill(th.panel_bg)
                    .inner_margin(8)
                    .show(ui, |ui| {
                        for l in lines {
                            ui.label(egui::RichText::new(l).monospace().color(th.panel_fg));
                        }
                    });
            });
    }
}

/// Characters without repeats, in order.
fn dedup(chars: Vec<char>) -> Vec<char> {
    let mut out = Vec::new();
    for c in chars {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// The edit that sets an annotation's comment.
fn comment_edit(page: usize, xref: i32, text: String) -> Edit {
    Edit::Update {
        page,
        xref,
        change: AnnotChange {
            contents: Some(text),
            ..AnnotChange::default()
        },
    }
}

/// The kind of a listed annotation, if marg can create it.
fn annot_kind(subtype: &str) -> Option<AnnotKind> {
    match subtype {
        "Highlight" => Some(AnnotKind::Highlight),
        "Underline" => Some(AnnotKind::Underline),
        _ => None,
    }
}

/// The annotation author: the configured one, else the OS user name.
/// The Normal, Caret, Visual and Annot keymaps, and warnings about the
/// `[keys.*]` tables.
fn build_keymaps(config: &Config) -> ([Keymap; 4], Vec<String>) {
    let mut warnings = Vec::new();
    let maps = [
        KeyMode::Normal,
        KeyMode::Caret,
        KeyMode::Visual,
        KeyMode::Annot,
    ]
    .map(|m| {
        let (map, w) = Keymap::build(m, &config.keys);
        warnings.extend(w);
        map
    });
    (maps, warnings)
}

fn annotation_author(configured: &str) -> String {
    if !configured.trim().is_empty() {
        return configured.trim().to_string();
    }
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "marg".into())
}
