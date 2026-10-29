//! Time opening a PDF, rendering its first page and extracting its text
//! (read-only).
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
    t.send(Request::Open {
        path: path.clone().into(),
        password: None,
    });
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
            recolor: None,
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
    t.send(Request::Render {
        keys: vec![RenderKey {
            recolor: Some(marg_pdf::Recolor {
                fg: [0xD0; 3],
                bg: [0x1E; 3],
            }),
            rev: 0,
            page: 0,
            scale: Scale::from_f32(1.5),
            tile: None,
        }],
        received: 1,
    });
    let Some(Response::Rendered { result, took, .. }) = t.recv_timeout(Duration::from_secs(60))
    else {
        panic!("no render response")
    };
    result.expect("render");
    println!("page 1 at 1.5x, dark: {took:?}");
    let text_start = Instant::now();
    t.send(Request::Text(0));
    let Some(Response::Text { result, .. }) = t.recv_timeout(Duration::from_secs(60)) else {
        panic!("no text response")
    };
    let text = result.expect("text");
    println!(
        "page 1 text: {} lines, {} chars in {:?}",
        text.lines.len(),
        text.chars.len(),
        text_start.elapsed()
    );
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
