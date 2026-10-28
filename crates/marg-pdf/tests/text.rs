//! Text extraction into the core text model.

use std::path::{Path, PathBuf};

use marg_pdf::document::Doc;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn lines(name: &str, page: usize) -> Vec<String> {
    let (doc, _) = Doc::open(&fixture(name)).unwrap();
    let t = doc.text(page).unwrap();
    t.lines.iter().map(|l| t.string(l.chars.clone())).collect()
}

#[test]
fn columns_come_in_reading_order() {
    let l = lines("columns.pdf", 0);
    assert_eq!(l[0], "Two columns: a reading order test");
    let left = l
        .iter()
        .position(|s| s.starts_with("tions in the PDF"))
        .unwrap();
    let right = l
        .iter()
        .position(|s| s.starts_with("The right column"))
        .unwrap();
    assert!(left < right, "{l:#?}");
    assert!(l.iter().any(|s| s.ends_with("annota-")));
    // Empty lines in the content stream do not become lines.
    assert!(l.iter().all(|s| !s.trim().is_empty()));
}

#[test]
fn boxes_are_page_relative_points() {
    let (doc, _) = Doc::open(&fixture("text.pdf")).unwrap();
    let t = doc.text(0).unwrap();
    let line = t
        .lines
        .iter()
        .find(|l| t.string(l.chars.clone()).starts_with("The quick"))
        .unwrap();
    let b = line.bbox;
    // Drawn at x = 72, baseline y = 792 - 688 = 104 (from the top).
    assert!((b.x0 - 72.0).abs() < 1.0, "{b:?}");
    assert!(b.y0 < 104.0 && b.y1 > 100.0, "{b:?}");
}

#[test]
fn offset_page_boxes_start_at_zero() {
    // boxes.pdf page 3 has MediaBox [100 200 400 600]; text at x = 105,
    // baseline 585 → 5 pt from the left, 15 pt from the top.
    let (doc, _) = Doc::open(&fixture("boxes.pdf")).unwrap();
    let t = doc.text(2).unwrap();
    let b = t.lines[0].bbox;
    assert!((b.x0 - 5.0).abs() < 1.0, "{b:?}");
    assert!(b.y1 > 10.0 && b.y1 < 20.0, "{b:?}");
}
