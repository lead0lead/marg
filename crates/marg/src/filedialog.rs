//! The desktop's "open file" dialog (concept §2.1): created on the UI
//! thread with marg's window as its parent, awaited on a thread, so the
//! window keeps drawing while it is open.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use eframe::egui;

/// An open dialog.
pub struct Picking {
    rx: Receiver<Option<PathBuf>>,
}

/// Show the dialog for PDFs, starting in `dir`. `ctx` is repainted when
/// it closes.
pub fn start(parent: &eframe::Frame, dir: Option<&Path>, ctx: egui::Context) -> Picking {
    // Portals match the extensions case-sensitively.
    let mut dialog = rfd::AsyncFileDialog::new()
        .set_title("Open PDF")
        .add_filter("PDF", &["pdf", "PDF"])
        .set_parent(parent);
    if let Some(dir) = dir {
        dialog = dialog.set_directory(dir);
    }
    let task = dialog.pick_file();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let picked = block_on(task).map(|f| f.path().to_path_buf());
        let _ = tx.send(picked);
        ctx.request_repaint();
    });
    Picking { rx }
}

impl Picking {
    /// `Some` once the dialog is closed: the picked file, or `None` when
    /// it was cancelled (or could not be shown; rfd logs why).
    pub fn poll(&self) -> Option<Option<PathBuf>> {
        match self.rx.try_recv() {
            Ok(picked) => Some(picked),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(None),
        }
    }
}

struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Run `f` to completion on this thread.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(out) = f.as_mut().poll(&mut cx) {
            return out;
        }
        thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_on_waits_for_a_wake_from_another_thread() {
        let (tx, rx) = mpsc::channel::<u32>();
        let rx = std::sync::Mutex::new(rx);
        let mut sender = Some(tx);
        let f = std::future::poll_fn(|cx| match rx.lock().unwrap().try_recv() {
            Ok(n) => Poll::Ready(n),
            Err(_) => {
                if let Some(tx) = sender.take() {
                    let waker = cx.waker().clone();
                    thread::spawn(move || {
                        tx.send(7).unwrap();
                        waker.wake();
                    });
                }
                Poll::Pending
            }
        });
        assert_eq!(block_on(f), 7);
    }
}
