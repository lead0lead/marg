//! Outline and links.

use std::path::{Path, PathBuf};

use marg_core::outline::LinkTarget;
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

#[test]
fn links_have_targets_and_page_boxes() {
    let (doc, _) = Doc::open(&fixture("text.pdf")).unwrap();
    let links = doc.links(0).unwrap();
    assert_eq!(links.len(), 2);
    let internal = links
        .iter()
        .find_map(|l| match &l.target {
            LinkTarget::Page(d) => Some((l.rect, *d)),
            LinkTarget::Uri(_) => None,
        })
        .unwrap();
    assert_eq!(internal.1.page, 1);
    // On the "Go to page two." line, in page space (y down).
    let text = doc.text(0).unwrap();
    let line = text
        .lines
        .iter()
        .find(|l| text.string(l.chars.clone()).starts_with("Go to page"))
        .unwrap();
    let (r, lb) = (internal.0, line.bbox);
    assert!(
        r.y0 < lb.center_y() && lb.center_y() < r.y1,
        "{r:?} vs {lb:?}"
    );
    assert!(
        links
            .iter()
            .any(|l| l.target == LinkTarget::Uri("https://example.com/".into()))
    );
}
