//! Time opening a PDF and rendering its first page (read-only).
//! `cargo run --release -p marg-pdf --example open_timing -- file.pdf`

use std::time::{Duration, Instant};

use marg_pdf::document::Doc;
use marg_pdf::{DocThread, RenderKey, Request, Response, Scale};

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("usage: open_timing <file.pdf>");
    let t = DocThread::spawn(|| ());
    let start = Instant::now();
    t.send(Request::Open(path.clone().into()));
    let Some(Response::Opened { result, took, .. }) = t.recv_timeout(Duration::from_secs(60))
    else {
        panic!("no open response")
    };
    let info = result.expect("open");
    println!(
        "open: {} pages in {took:?} (labels: {}, id: {})",
        info.page_sizes.len(),
        info.labels.is_some(),
        info.id.is_some()
    );
    t.send(Request::Render {
        keys: vec![RenderKey {
            rev: 0,
            page: 0,
            scale: Scale::from_f32(1.5),
            tile: None,
        }],
        received: 0,
    });
    let Some(Response::Rendered { result, took, .. }) = t.recv_timeout(Duration::from_secs(60))
    else {
        panic!("no render response")
    };
    let px = result.expect("render");
    println!("page 1 at 1.5x: {}x{} in {took:?}", px.width, px.height);
    println!("total: {:?}", start.elapsed());

    // Check the fast page-size path against MuPDF's page bounds.
    let (doc, info) = Doc::open(std::path::Path::new(&path)).expect("open");
    let mismatches = (0..info.page_sizes.len())
        .filter(|&i| {
            let real = doc.loaded_page_size(i).expect("load page");
            let fast = info.page_sizes[i];
            (real.width - fast.width).abs() > 0.01 || (real.height - fast.height).abs() > 0.01
        })
        .count();
    println!("page size mismatches: {mismatches}");
}
