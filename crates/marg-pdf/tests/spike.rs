//! Phase 0 spike: checks that the `mupdf` crate covers the API marg needs.
//! Works on a copy of `tests/fixtures/text.pdf` in Cargo's per-test temp dir.

use std::path::{Path, PathBuf};

use mupdf::color::AnnotationColor;
use mupdf::pdf::{PdfAnnotationType, PdfDocument, PdfWriteOptions};
use mupdf::{Colorspace, Matrix, Point, Quad, Rect, TextBlockContent, TextPageFlags};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn temp_copy(name: &str, tag: &str) -> PathBuf {
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{name}"));
    std::fs::copy(fixture(name), &dst).unwrap();
    dst
}

fn doc_id(doc: &PdfDocument) -> Vec<u8> {
    let trailer = doc.trailer().unwrap();
    let id = trailer.get_dict("ID").unwrap().expect("trailer has /ID");
    id.get_array(0).unwrap().unwrap().as_bytes().unwrap()
}

fn subj(annot: &mupdf::pdf::PdfAnnotation) -> Option<String> {
    let obj = annot.object();
    obj.get_dict("Subj")
        .unwrap()
        .map(|s| s.as_string().unwrap())
}

fn yellow() -> AnnotationColor {
    AnnotationColor::Rgb {
        red: 1.0,
        green: 0xD4 as f32 / 255.0,
        blue: 0.0,
    }
}

fn save_incremental(doc: &PdfDocument, path: &Path) {
    let mut opts = PdfWriteOptions::default();
    opts.set_incremental(true);
    doc.save_with_options(path.to_str().unwrap(), opts).unwrap();
}

#[test]
fn render_page_with_annotations() {
    let doc = PdfDocument::open(fixture("text.pdf").to_str().unwrap()).unwrap();
    let page = doc.load_page(0).unwrap();
    let bounds = page.bounds().unwrap();
    assert_eq!((bounds.width(), bounds.height()), (612.0, 792.0));
    let pix = page
        .to_pixmap(
            &Matrix::new_scale(2.0, 2.0),
            &Colorspace::device_rgb(),
            false,
            true,
        )
        .unwrap();
    assert_eq!((pix.width(), pix.height()), (1224, 1584));
    assert_eq!(pix.n(), 3);
    // Some pixels are dark (text was drawn).
    assert!(pix.samples().iter().any(|&b| b < 64));
}

#[test]
fn structured_text_has_char_boxes() {
    let doc = PdfDocument::open(fixture("text.pdf").to_str().unwrap()).unwrap();
    let page = doc.load_page(0).unwrap();
    let st = page
        .to_text_page(TextPageFlags::DEHYPHENATE)
        .unwrap()
        .structured();
    let lines: Vec<_> = st
        .blocks
        .iter()
        .filter_map(|b| match &b.content {
            TextBlockContent::Text { lines } => Some(lines),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(lines.iter().any(|l| l.text.contains("quick brown fox")));
    let line = lines.iter().find(|l| l.text.contains("quick")).unwrap();
    let c = &line.chars[0];
    assert_eq!(c.ch, 'T');
    // Page space, y down from the top: first body line baseline at 792-688 = 104.
    assert!(c.quad.ul.y < c.quad.ll.y);
    assert!((c.origin.y - 104.0).abs() < 0.5, "origin {:?}", c.origin);
}

#[test]
fn outline_links_labels_metadata() {
    let doc = PdfDocument::open(fixture("text.pdf").to_str().unwrap()).unwrap();
    let outline = doc.outlines().unwrap();
    assert_eq!(
        outline.iter().map(|o| o.title.as_str()).collect::<Vec<_>>(),
        ["Page one", "Page two"]
    );
    assert_eq!(outline[1].dest.unwrap().loc.page_number, 1);

    let page = doc.load_page(0).unwrap();
    let links: Vec<_> = page.links().unwrap().collect();
    assert_eq!(links.len(), 2);
    assert!(
        links
            .iter()
            .any(|l| l.dest.is_some_and(|d| d.loc.page_number == 1))
    );
    assert!(links.iter().any(|l| l.uri == "https://example.com/"));

    assert_eq!(doc.page_label(0).unwrap(), "i");
    assert_eq!(doc.page_label(1).unwrap(), "1");
    assert_eq!(
        doc.metadata(mupdf::MetadataName::Title).unwrap(),
        "marg text fixture"
    );
    assert_eq!(doc_id(&doc), b"margtextfixture1");
}

#[test]
fn annotations_round_trip_incremental() {
    let path = temp_copy("text.pdf", "roundtrip");
    let original_len = std::fs::metadata(&path).unwrap().len();
    let original = std::fs::read(&path).unwrap();

    {
        let doc = PdfDocument::open(path.to_str().unwrap()).unwrap();
        let id_before = doc_id(&doc);
        let mut page = doc.load_pdf_page(0).unwrap();

        // Highlight with two quads (one per line), color, opacity, subject, author.
        let q = |x0: f32, y0: f32, x1: f32, y1: f32| {
            Quad::new(
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x0, y1),
                Point::new(x1, y1),
            )
        };
        let mut hl = page
            .add_highlight_annotation(vec![
                q(72.0, 92.0, 300.0, 107.0),
                q(72.0, 108.0, 280.0, 123.0),
            ])
            .unwrap();
        hl.set_color(yellow()).unwrap();
        hl.set_opacity(0.4).unwrap();
        hl.set_author("marg-test").unwrap();
        hl.set_contents("a comment").unwrap();
        hl.object()
            .dict_put(
                "Subj",
                mupdf::pdf::PdfObject::new_string("Key point").unwrap(),
            )
            .unwrap();
        hl.update().unwrap();

        let mut ul = page
            .add_underline_annotation(vec![q(72.0, 124.0, 250.0, 139.0)])
            .unwrap();
        ul.set_color(yellow()).unwrap();
        ul.update().unwrap();

        let mut note = page
            .add_text_annotation(Rect::new(20.0, 20.0, 40.0, 40.0), "sticky")
            .unwrap();
        note.update().unwrap();

        let mut sq = page
            .add_square_annotation(Rect::new(300.0, 300.0, 400.0, 380.0))
            .unwrap();
        sq.set_color(yellow()).unwrap();
        sq.set_border_width(1.5).unwrap();
        sq.update().unwrap();

        // Delete the underline again.
        let ul = page
            .annotations()
            .find(|a| a.r#type().unwrap() == PdfAnnotationType::Underline)
            .unwrap();
        page.delete_annotation(ul).unwrap();
        page.update().unwrap();

        assert!(doc.has_unsaved_changes());
        assert!(doc.can_be_saved_incrementally());
        save_incremental(&doc, &path);
        assert_eq!(doc_id(&doc), id_before);
    }

    // Incremental: the original bytes are an untouched prefix of the new file.
    let saved = std::fs::read(&path).unwrap();
    assert!(saved.len() as u64 > original_len);
    assert_eq!(&saved[..original.len()], &original[..]);

    // Reopen and verify.
    let doc = PdfDocument::open(path.to_str().unwrap()).unwrap();
    assert_eq!(doc_id(&doc), b"margtextfixture1");
    let page = doc.load_pdf_page(0).unwrap();
    let annots: Vec<_> = page
        .annotations()
        .filter(|a| a.r#type().unwrap() != PdfAnnotationType::Link)
        .collect();
    let types: Vec<_> = annots.iter().map(|a| a.r#type().unwrap()).collect();
    assert_eq!(
        types,
        [
            PdfAnnotationType::Highlight,
            PdfAnnotationType::Text,
            PdfAnnotationType::Square
        ]
    );
    let hl = &annots[0];
    assert_eq!(hl.quad_points().unwrap().len(), 2);
    assert!((hl.opacity().unwrap() - 0.4).abs() < 1e-3);
    assert_eq!(hl.author().unwrap(), Some("marg-test"));
    assert_eq!(hl.contents().unwrap(), Some("a comment"));
    assert_eq!(subj(hl).as_deref(), Some("Key point"));
    match hl.color().unwrap() {
        Some(AnnotationColor::Rgb { red, green, blue }) => {
            assert!((red - 1.0).abs() < 1e-3 && (green - 0.83).abs() < 0.01 && blue < 1e-3)
        }
        other => panic!("unexpected color {other:?}"),
    }
    assert_eq!(annots[1].contents().unwrap(), Some("sticky"));
    assert!((annots[2].border_width().unwrap() - 1.5).abs() < 1e-3);
}
