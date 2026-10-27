//! Phase 0 spike: copy a PDF and add one yellow highlight over the first
//! non-blank text line of page 1, saved incrementally. For checking in other
//! viewers.
//!
//! Usage: cargo run -p marg-pdf --example highlight_copy -- <in.pdf> <out.pdf>

use mupdf::color::AnnotationColor;
use mupdf::pdf::{PdfDocument, PdfObject, PdfWriteOptions};
use mupdf::{TextBlockContent, TextPageFlags};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, dst] = args.as_slice() else {
        return Err("usage: highlight_copy <in.pdf> <out.pdf>".into());
    };
    if std::path::Path::new(dst).exists() {
        return Err(format!("{dst} exists; refusing to overwrite").into());
    }
    std::fs::copy(src, dst)?;

    let doc = PdfDocument::open(dst.as_str())?;
    let mut page = doc.load_pdf_page(0)?;
    let st = page.to_text_page(TextPageFlags::empty())?.structured();
    let line = st
        .blocks
        .iter()
        .find_map(|b| match &b.content {
            // Skip whitespace-only lines (some PDFs start with a lone space).
            TextBlockContent::Text { lines } => lines
                .iter()
                .find(|l| l.text.trim().chars().count() >= 3)
                .cloned(),
            _ => None,
        })
        .ok_or("no text on page 1")?;
    let quad = mupdf::Quad::from(line.bounds);

    let mut hl = page.add_highlight_annotation(vec![quad])?;
    hl.set_color(AnnotationColor::Rgb {
        red: 1.0,
        green: 0xD4 as f32 / 255.0,
        blue: 0.0,
    })?;
    hl.set_opacity(0.4)?;
    hl.set_author("marg")?;
    hl.set_contents("marg phase 0 spike")?;
    hl.object()
        .dict_put("Subj", PdfObject::new_string("Key point")?)?;
    hl.update()?;

    let mut opts = PdfWriteOptions::default();
    opts.set_incremental(true);
    doc.save_with_options(dst, opts)?;
    println!("highlighted: {:?}", line.text);
    Ok(())
}
