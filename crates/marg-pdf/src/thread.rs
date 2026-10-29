//! The document thread: owns the MuPDF document and serves requests from the
//! UI thread over channels. The UI never calls MuPDF.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use marg_core::annot::{AnnotChange, AnnotInfo, AnnotSpec};
use marg_core::outline::OutlineEntry;
use marg_core::search::{Hit, Query};
use marg_core::text::{BBox, PageText};

use crate::document::{Doc, DocInfo, PixelRect, Pixels, Recolor, SaveError, SaveReport};

/// Render scale as integer thousandths of a pixel per point, so it can be
/// used as a cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scale(pub u32);

impl Scale {
    pub fn from_f32(scale: f32) -> Self {
        Self((scale * 1000.0).round().max(1.0) as u32)
    }

    pub fn get(self) -> f32 {
        self.0 as f32 / 1000.0
    }
}

/// A square tile of `size` device pixels at column `col`, row `row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tile {
    pub col: u32,
    pub row: u32,
    pub size: u32,
}

/// One render job: a whole page, or one tile of it, at a scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderKey {
    /// Dark pages: recolor the page content (not annotations).
    pub recolor: Option<Recolor>,
    pub page: usize,
    /// The page's revision as the UI knows it: the number of annotation
    /// edits on it. Renders always show the current state; the revision
    /// only keeps renders from before an edit apart in caches.
    pub rev: u32,
    pub scale: Scale,
    pub tile: Option<Tile>,
}

/// A change to the document's annotations.
#[derive(Debug, Clone)]
pub enum Edit {
    Create(AnnotSpec),
    /// Delete the annotation with this object number from a page.
    Delete {
        page: usize,
        xref: i32,
    },
    /// Put a deleted annotation back (undo of `Delete`).
    Restore {
        page: usize,
        xref: i32,
    },
    /// Change an annotation in place.
    Update {
        page: usize,
        xref: i32,
        change: AnnotChange,
    },
}

pub enum Request {
    /// Open a document. On success it replaces the current one, clears the
    /// queue and starts a new generation; on failure the current one stays.
    Open(PathBuf),
    /// Replace the render queue. Jobs are done in order; jobs from an
    /// earlier queue that have not started are dropped. `received` is the
    /// number of `Rendered` responses the caller has taken so far: keys
    /// rendered after that are still in transit and are not rendered again.
    Render { keys: Vec<RenderKey>, received: u64 },
    /// Extract the text of a page. Served before renders.
    Text(usize),
    /// Change annotations. Served before text and renders. `tag` comes
    /// back in the response. The first edit of a document backs the file up
    /// first (if a backup folder is set); if that fails, the edit fails.
    Edit { tag: u64, edit: Edit },
    /// Save the changes (see `Doc::save`).
    Save { force: bool },
    /// Folder for backups of files before their first change.
    BackupDir(PathBuf),
    /// Check whether another program changed the file; answered with
    /// `DiskChanged` only if it did (once per change).
    CheckDisk,
    /// Close the document without saving. Starts a new generation, so
    /// responses still in transit can be told apart.
    Close,
    /// List the annotations of pages. Urgent requests are served with
    /// text (before renders); the others when nothing else is queued.
    Annots { pages: Vec<usize>, urgent: bool },
    /// Where images are drawn on a page (for box snapping). Served at once.
    Images(usize),
    /// The table of contents. Served at once.
    Outline,
    /// Search every page, starting at page `from` and wrapping around,
    /// after renders. Replaces the previous search; an empty query stops
    /// it. Hits come back page by page, then `SearchDone`.
    Search { id: u64, query: String, from: usize },
}

pub enum Response {
    Opened {
        generation: u64,
        result: Result<DocInfo, String>,
        took: Duration,
    },
    Rendered {
        /// Which successful `Open` this render belongs to; stale ones can be
        /// ignored.
        generation: u64,
        key: RenderKey,
        result: Result<Pixels, String>,
        took: Duration,
    },
    Text {
        generation: u64,
        page: usize,
        result: Result<PageText, String>,
    },
    Edited {
        generation: u64,
        tag: u64,
        page: usize,
        /// The page's revision after the edit.
        rev: u32,
        /// The object number of the created or deleted annotation.
        result: Result<i32, String>,
    },
    Saved {
        generation: u64,
        /// `Ok(None)`: there was nothing to save.
        result: Result<Option<SaveReport>, SaveError>,
    },
    DiskChanged {
        generation: u64,
    },
    Annots {
        generation: u64,
        page: usize,
        result: Result<Vec<AnnotInfo>, String>,
    },
    Images {
        generation: u64,
        page: usize,
        result: Result<Vec<BBox>, String>,
    },
    Outline {
        generation: u64,
        result: Result<Vec<OutlineEntry>, String>,
    },
    /// The hits of search `id` on one page (only pages with hits).
    Searched {
        generation: u64,
        id: u64,
        page: usize,
        hits: Vec<Hit>,
    },
    /// Search `id` has looked at every page.
    SearchDone {
        generation: u64,
        id: u64,
    },
}

/// Handle to the document thread. Dropping it stops the thread.
pub struct DocThread {
    tx: Option<Sender<Request>>,
    rx: Receiver<Response>,
    join: Option<JoinHandle<()>>,
}

impl DocThread {
    /// Start the thread. `wake` is called after every response is sent
    /// (e.g. to request a repaint).
    pub fn spawn(wake: impl Fn() + Send + 'static) -> Self {
        let (req_tx, req_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("marg-doc".into())
            .spawn(move || run(req_rx, resp_tx, wake))
            .expect("spawn document thread");
        Self {
            tx: Some(req_tx),
            rx: resp_rx,
            join: Some(join),
        }
    }

    pub fn send(&self, req: Request) {
        if let Some(tx) = &self.tx
            && tx.send(req).is_err()
        {
            log::error!("document thread has stopped");
        }
    }

    pub fn try_recv(&self) -> Option<Response> {
        self.rx.try_recv().ok()
    }

    /// Blocking receive, for tests and tools.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Response> {
        self.rx.recv_timeout(timeout).ok()
    }
}

impl Drop for DocThread {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn pixel_rect(info: &DocInfo, key: RenderKey) -> Result<PixelRect, String> {
    let size = *info.page_sizes.get(key.page).ok_or("page out of range")?;
    let (pw, ph) = Doc::page_pixels(size, key.scale.get());
    let Some(t) = key.tile else {
        return Ok(PixelRect {
            x: 0,
            y: 0,
            width: pw,
            height: ph,
        });
    };
    let (x, y) = (t.col * t.size, t.row * t.size);
    if x >= pw || y >= ph {
        return Err("tile outside page".into());
    }
    Ok(PixelRect {
        x,
        y,
        width: t.size.min(pw - x),
        height: t.size.min(ph - y),
    })
}

/// Back the file up before its first change.
fn backup(d: &mut Doc, dir: Option<&std::path::Path>) -> Result<(), String> {
    let Some(dir) = dir else { return Ok(()) };
    match d.ensure_backup(dir) {
        Ok(Some(path)) => {
            log::info!("backup: {}", path.display());
            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => Err(format!(
            "cannot back up the file, so it was not changed: {e}"
        )),
    }
}

fn run(rx: Receiver<Request>, tx: Sender<Response>, wake: impl Fn()) {
    let mut doc: Option<(Doc, DocInfo)> = None;
    let mut backup_dir: Option<PathBuf> = None;
    let mut generation = 0u64;
    let mut queue: VecDeque<RenderKey> = VecDeque::new();
    let mut text_queue: VecDeque<usize> = VecDeque::new();
    let mut annots_urgent: VecDeque<usize> = VecDeque::new();
    let mut annots_idle: VecDeque<usize> = VecDeque::new();
    // The running search: its id, query and the pages still to search.
    let mut search: Option<(u64, Query, VecDeque<usize>)> = None;
    // Annotation edits per page since the document was opened.
    let mut revs: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    // Rendered responses sent so far, and the keys of those the UI may not
    // have received yet, with their sequence numbers.
    let mut sent = 0u64;
    let mut in_transit: VecDeque<(u64, RenderKey)> = VecDeque::new();
    loop {
        // Block only when idle; otherwise take whatever arrived meanwhile.
        let mut incoming = Vec::new();
        if queue.is_empty()
            && text_queue.is_empty()
            && annots_urgent.is_empty()
            && annots_idle.is_empty()
            && search.is_none()
        {
            match rx.recv() {
                Ok(r) => incoming.push(r),
                Err(_) => return,
            }
        }
        loop {
            match rx.try_recv() {
                Ok(r) => incoming.push(r),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        for req in incoming {
            match req {
                Request::Open(path) => {
                    let start = Instant::now();
                    let info = match Doc::open(&path) {
                        Ok((d, info)) => {
                            queue.clear();
                            text_queue.clear();
                            annots_urgent.clear();
                            annots_idle.clear();
                            revs.clear();
                            search = None;
                            generation += 1;
                            doc = Some((d, info.clone()));
                            Ok(info)
                        }
                        Err(e) => Err(e),
                    };
                    let took = start.elapsed();
                    if tx
                        .send(Response::Opened {
                            generation,
                            result: info,
                            took,
                        })
                        .is_err()
                    {
                        return;
                    }
                    wake();
                }
                Request::Render { keys, received } => {
                    while in_transit.front().is_some_and(|&(seq, _)| seq <= received) {
                        in_transit.pop_front();
                    }
                    queue = keys
                        .into_iter()
                        .filter(|k| !in_transit.iter().any(|(_, t)| t == k))
                        .collect();
                }
                Request::Text(page) => {
                    if !text_queue.contains(&page) {
                        text_queue.push_back(page);
                    }
                }
                Request::Images(page) => {
                    let result = match &doc {
                        None => Err("no document open".into()),
                        Some((d, _)) => d.images(page),
                    };
                    let resp = Response::Images {
                        generation,
                        page,
                        result,
                    };
                    if tx.send(resp).is_err() {
                        return;
                    }
                    wake();
                }
                Request::Search { id, query, from } => {
                    let count = doc.as_ref().map_or(0, |(_, info)| info.page_sizes.len());
                    let from = from.min(count.saturating_sub(1));
                    search =
                        Query::new(&query).map(|q| (id, q, (from..count).chain(0..from).collect()));
                }
                Request::Outline => {
                    let result = match &doc {
                        None => Err("no document open".into()),
                        Some((d, _)) => d.outline(),
                    };
                    if tx.send(Response::Outline { generation, result }).is_err() {
                        return;
                    }
                    wake();
                }
                Request::Edit { tag, edit } => {
                    let page = match &edit {
                        Edit::Create(spec) => spec.page,
                        Edit::Delete { page, .. }
                        | Edit::Restore { page, .. }
                        | Edit::Update { page, .. } => *page,
                    };
                    let result = match &mut doc {
                        None => Err("no document open".into()),
                        Some((d, _)) => {
                            backup(d, backup_dir.as_deref()).and_then(|()| match edit {
                                Edit::Create(spec) => d.create_annotation(&spec),
                                Edit::Delete { page, xref } => {
                                    d.delete_annotation(page, xref).map(|()| xref)
                                }
                                Edit::Restore { page, xref } => {
                                    d.restore_annotation(page, xref).map(|()| xref)
                                }
                                Edit::Update { page, xref, change } => {
                                    d.update_annotation(page, xref, &change).map(|()| xref)
                                }
                            })
                        }
                    };
                    let rev = revs.entry(page).or_default();
                    if result.is_ok() {
                        *rev += 1;
                    }
                    let resp = Response::Edited {
                        generation,
                        tag,
                        page,
                        rev: *rev,
                        result,
                    };
                    if tx.send(resp).is_err() {
                        return;
                    }
                    wake();
                }
                Request::Save { force } => {
                    let result = match &mut doc {
                        None => Ok(None),
                        Some((d, _)) => d.save(force),
                    };
                    match &result {
                        Ok(Some(r)) => log::info!("saved (full: {})", r.full),
                        Ok(None) => {}
                        Err(e) => log::warn!("save failed: {e:?}"),
                    }
                    if tx.send(Response::Saved { generation, result }).is_err() {
                        return;
                    }
                    wake();
                }
                Request::BackupDir(dir) => backup_dir = Some(dir),
                Request::Annots { pages, urgent } => {
                    let q = if urgent {
                        &mut annots_urgent
                    } else {
                        &mut annots_idle
                    };
                    for p in pages {
                        if !q.contains(&p) {
                            q.push_back(p);
                        }
                    }
                }
                Request::Close => {
                    doc = None;
                    queue.clear();
                    text_queue.clear();
                    annots_urgent.clear();
                    annots_idle.clear();
                    revs.clear();
                    search = None;
                    generation += 1;
                }
                Request::CheckDisk => {
                    if let Some((d, _)) = &mut doc
                        && d.poll_disk()
                    {
                        if tx.send(Response::DiskChanged { generation }).is_err() {
                            return;
                        }
                        wake();
                    }
                }
            }
        }
        if let Some(page) = text_queue.pop_front() {
            let result = match &doc {
                None => Err("no document open".into()),
                Some((d, _)) => d.text(page),
            };
            if tx
                .send(Response::Text {
                    generation,
                    page,
                    result,
                })
                .is_err()
            {
                return;
            }
            wake();
            continue;
        }
        if annots_urgent.is_empty()
            && queue.is_empty()
            && let Some((id, query, pages)) = &mut search
        {
            let id = *id;
            match (pages.pop_front(), &doc) {
                (Some(page), Some((d, _))) => {
                    let hits = match d.text(page) {
                        Ok(t) => query.hits(page, &t),
                        Err(e) => {
                            log::error!("search: text of page {} failed: {e}", page + 1);
                            Vec::new()
                        }
                    };
                    if !hits.is_empty() {
                        let resp = Response::Searched {
                            generation,
                            id,
                            page,
                            hits,
                        };
                        if tx.send(resp).is_err() {
                            return;
                        }
                        wake();
                    }
                }
                _ => {
                    search = None;
                    if tx.send(Response::SearchDone { generation, id }).is_err() {
                        return;
                    }
                    wake();
                }
            }
            continue;
        }
        let annot_page = annots_urgent.pop_front().or_else(|| {
            if queue.is_empty() {
                annots_idle.pop_front()
            } else {
                None
            }
        });
        if let Some(page) = annot_page {
            let result = match &doc {
                None => Err("no document open".into()),
                Some((d, _)) => d.annotations(page),
            };
            let resp = Response::Annots {
                generation,
                page,
                result,
            };
            if tx.send(resp).is_err() {
                return;
            }
            // An idle scan of many pages wakes the UI only now and then.
            if annots_idle.len().is_multiple_of(64) {
                wake();
            }
            continue;
        }
        let Some(key) = queue.pop_front() else {
            continue;
        };
        let start = Instant::now();
        let result = match &doc {
            None => Err("no document open".into()),
            Some((d, info)) => pixel_rect(info, key)
                .and_then(|r| d.render(key.page, key.scale.get(), r, key.recolor)),
        };
        let resp = Response::Rendered {
            generation,
            key,
            result,
            took: start.elapsed(),
        };
        if tx.send(resp).is_err() {
            return;
        }
        sent += 1;
        in_transit.push_back((sent, key));
        wake();
    }
}
