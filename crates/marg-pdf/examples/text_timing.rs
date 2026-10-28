//! Time text extraction of every page of some PDFs (read-only).
//! `cargo run --release -p marg-pdf --example text_timing -- a.pdf b.pdf …`

use std::time::Instant;

use marg_pdf::document::Doc;

fn main() {
    let mut ms = Vec::new();
    for path in std::env::args_os().skip(1) {
        let Ok((doc, info)) = Doc::open(std::path::Path::new(&path)) else {
            eprintln!("cannot open {path:?}");
            continue;
        };
        for i in 0..info.page_sizes.len() {
            let start = Instant::now();
            let _ = doc.text(i);
            let took = start.elapsed().as_secs_f64() * 1000.0;
            if took > 50.0 {
                println!("{took:.0} ms: page {} of {}", i + 1, path.to_string_lossy());
            }
            ms.push(took);
        }
    }
    ms.sort_by(f64::total_cmp);
    let q = |p: f64| ms[((ms.len() - 1) as f64 * p) as usize];
    println!(
        "{} pages: median {:.2} ms, p90 {:.2} ms, max {:.2} ms",
        ms.len(),
        q(0.5),
        q(0.9),
        q(1.0)
    );
}
