//! Outline and links.

use std::path::{Path, PathBuf};

use marg_pdf::document::Doc;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn outline_is_flattened_with_destinations() {
    let (doc, _) = Doc::open(&fixture("text.pdf")).unwrap();
    let toc = doc.outline().unwrap();
    let titles: Vec<_> = toc.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["Page one", "Page two"]);
    assert!(toc.iter().all(|e| e.level == 0));
    let d = toc[1].dest.unwrap();
    // `/XYZ 0 792` on a 792 pt page: its top, y down.
    assert_eq!((d.page, d.x, d.y), (1, Some(0.0), Some(0.0)));
    let (doc, _) = Doc::open(&fixture("columns.pdf")).unwrap();
    assert!(doc.outline().unwrap().is_empty());
}
