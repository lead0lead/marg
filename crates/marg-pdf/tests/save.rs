//! Saving annotations: incremental and full saves, backups, external
//! changes. Works on copies of the fixtures in Cargo's temp dir.

use std::path::{Path, PathBuf};

use marg_core::annot::{AnnotChange, AnnotKind, AnnotSpec};
use marg_core::text::BBox;
use marg_pdf::document::{Doc, PixelRect};
use marg_pdf::{Recolor, SaveError, SaveReport};
use mupdf::pdf::{PdfAnnotationType, PdfDocument};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// A fresh folder with a copy of `name`.
fn copy(name: &str, tag: &str) -> (PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("save-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::copy(fixture(name), &path).unwrap();
    (dir, path)
}

fn spec(kind: AnnotKind) -> AnnotSpec {
    AnnotSpec {
        page: 0,
        kind,
        rects: vec![
            BBox::new(72.0, 90.0, 300.0, 104.0),
            BBox::new(72.0, 104.0, 250.0, 118.0),
        ],
        color: [0xA2, 0x8A, 0xE5],
        opacity: 0.4,
        stroke_pt: 0.0,
        subject: "Question".into(),
        author: "tester".into(),
        contents: Some("why?".into()),
    }
}

/// (type, subject, author, opacity, quad count) of page 0's annotations
/// other than links.
fn annotations(path: &Path) -> Vec<(PdfAnnotationType, String, String, f32, usize)> {
    let doc = PdfDocument::open(path.to_str().unwrap()).unwrap();
    let page = doc.load_pdf_page(0).unwrap();
    page.annotations()
        .filter(|a| a.r#type().unwrap() != PdfAnnotationType::Link)
        .map(|a| {
            let subj = a
                .object()
                .get_dict("Subj")
                .unwrap()
                .map(|s| s.as_string().unwrap().to_string())
                .unwrap_or_default();
            (
                a.r#type().unwrap(),
                subj,
                a.author().unwrap().unwrap_or_default().to_string(),
                a.opacity().unwrap(),
                a.quad_points().unwrap().len(),
            )
        })
        .collect()
}

#[test]
fn incremental_save_round_trip_and_backup() {
    let (dir, path) = copy("text.pdf", "incremental");
    let original = std::fs::read(&path).unwrap();
    let backups = dir.join("backups");
    let (mut doc, _) = Doc::open(&path).unwrap();
    assert_eq!(doc.save(false), Ok(None), "nothing to save yet");

    let backup = doc.ensure_backup(&backups).unwrap().unwrap();
    assert_eq!(
        backup.file_name().unwrap(),
        "6d617267746578746669787475726531-text.pdf"
    );
    doc.create_annotation(&spec(AnnotKind::Highlight)).unwrap();
    doc.create_annotation(&spec(AnnotKind::Underline)).unwrap();
    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));

    let saved = std::fs::read(&path).unwrap();
    assert!(saved.len() > original.len());
    assert_eq!(
        &saved[..original.len()],
        &original[..],
        "appended, not rewritten"
    );
    assert_eq!(std::fs::read(&backup).unwrap(), original);

    let a = annotations(&path);
    assert_eq!(a.len(), 2, "{a:?}");
    assert_eq!(a[0].0, PdfAnnotationType::Highlight);
    assert_eq!(
        (a[0].1.as_str(), a[0].2.as_str(), a[0].4),
        ("Question", "tester", 2)
    );
    assert!((a[0].3 - 0.4).abs() < 1e-3);
    assert_eq!(a[1].0, PdfAnnotationType::Underline);

    // A second save with more changes appends again.
    doc.create_annotation(&spec(AnnotKind::Highlight)).unwrap();
    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));
    assert_eq!(annotations(&path).len(), 3);

    // A backup is never overwritten, also not by a new session.
    drop(doc);
    let (mut doc, _) = Doc::open(&path).unwrap();
    assert_eq!(doc.ensure_backup(&backups), Ok(None));
    assert_eq!(std::fs::read(&backup).unwrap(), original);
}

#[test]
fn changed_on_disk_blocks_save_until_forced() {
    let (_dir, path) = copy("columns.pdf", "changed");
    let (mut doc, _) = Doc::open(&path).unwrap();
    doc.create_annotation(&spec(AnnotKind::Highlight)).unwrap();
    // Another program appends to the file.
    let mut bytes = std::fs::read(&path).unwrap();
    bytes.extend_from_slice(b"\n% changed elsewhere\n");
    std::fs::write(&path, &bytes).unwrap();
    assert!(doc.changed_on_disk());
    assert!(doc.poll_disk());
    assert!(!doc.poll_disk(), "reported once");
    assert_eq!(doc.save(false), Err(SaveError::ChangedOnDisk));
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "untouched");

    assert_eq!(doc.save(true), Ok(Some(SaveReport { full: true })));
    assert!(!doc.changed_on_disk());
    assert!(!doc.poll_disk(), "own saves are not changes");
    assert_eq!(annotations(&path).len(), 1);
    // No temporary files left behind.
    let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".marg-tmp"))
        .collect();
    assert!(leftovers.is_empty());

    // After a full save, later saves are incremental again.
    let before = std::fs::read(&path).unwrap();
    doc.create_annotation(&spec(AnnotKind::Underline)).unwrap();
    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));
    let after = std::fs::read(&path).unwrap();
    assert_eq!(&after[..before.len()], &before[..]);
    assert_eq!(annotations(&path).len(), 2);
}

#[test]
fn backup_name_is_hex_id_and_file_name() {
    let (dir, path) = copy("boxes.pdf", "backupname");
    let (mut doc, _) = Doc::open(&path).unwrap();
    let b = doc.ensure_backup(&dir.join("b")).unwrap().unwrap();
    let name = b.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(name, "6d61726762786573666978747572653031-boxes.pdf");
}

#[test]
fn list_delete_restore_round_trip() {
    let (_dir, path) = copy("text.pdf", "restore");
    let (mut doc, _) = Doc::open(&path).unwrap();
    // text.pdf has two links; they are not listed.
    assert!(doc.annotations(0).unwrap().is_empty());
    let xref = doc.create_annotation(&spec(AnnotKind::Highlight)).unwrap();
    let a = doc.annotations(0).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!((a[0].xref, a[0].kind.as_str()), (xref, "Highlight"));
    assert_eq!(a[0].subject.as_deref(), Some("Question"));
    assert_eq!(a[0].color, Some([0xA2, 0x8A, 0xE5]));
    assert_eq!(a[0].author.as_deref(), Some("tester"));
    assert_eq!(a[0].contents.as_deref(), Some("why?"));
    assert!((a[0].opacity - 0.4).abs() < 1e-3);
    assert_eq!(a[0].quads.len(), 2);
    assert!((a[0].quads[0].x0 - 72.0).abs() < 0.5 && (a[0].quads[0].y0 - 90.0).abs() < 0.5);

    doc.delete_annotation(0, xref).unwrap();
    assert!(doc.annotations(0).unwrap().is_empty());
    doc.restore_annotation(0, xref).unwrap();
    let back = doc.annotations(0).unwrap();
    assert_eq!(back, a, "restored with everything it had");

    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));
    drop(doc);
    let (doc, _) = Doc::open(&path).unwrap();
    assert_eq!(doc.annotations(0).unwrap(), a);
}

/// Colored pixels in `b` (page points, rendered at scale 1), not white,
/// grey or black.
fn colored_pixels(doc: &Doc, b: BBox) -> usize {
    let px = doc
        .render(
            0,
            1.0,
            PixelRect {
                x: b.x0 as u32,
                y: b.y0 as u32,
                width: (b.x1 - b.x0) as u32,
                height: (b.y1 - b.y0) as u32,
            },
            None,
        )
        .unwrap();
    px.rgba
        .chunks(4)
        .filter(|p| p[..3].iter().max().unwrap() - p[..3].iter().min().unwrap() > 25)
        .count()
}

#[test]
fn update_in_place_round_trip() {
    let (_dir, path) = copy("text.pdf", "update");
    let (mut doc, _) = Doc::open(&path).unwrap();
    let xref = doc.create_annotation(&spec(AnnotKind::Highlight)).unwrap();
    let colored = |doc: &Doc| colored_pixels(doc, BBox::new(72.0, 90.0, 300.0, 104.0));
    let as_highlight = colored(&doc);
    doc.update_annotation(
        0,
        xref,
        &AnnotChange {
            color: Some([0xFF, 0x66, 0x66]),
            opacity: Some(1.0),
            subject: Some("Disagree".into()),
            contents: Some("no".into()),
            kind: Some(AnnotKind::Underline),
        },
    )
    .unwrap();
    let listed = doc.annotations(0).unwrap();
    let a = listed.iter().find(|a| a.xref == xref).unwrap();
    assert_eq!(a.kind, "Underline");
    assert_eq!(a.color, Some([0xFF, 0x66, 0x66]));
    assert_eq!(a.subject.as_deref(), Some("Disagree"));
    assert_eq!(a.contents.as_deref(), Some("no"));
    assert_eq!(a.quads.len(), 2, "quads kept");
    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));
    let a = annotations(&path);
    assert_eq!(a.len(), 1, "{a:?}");
    assert_eq!(
        (a[0].0, a[0].1.as_str(), a[0].2.as_str(), a[0].4),
        (PdfAnnotationType::Underline, "Disagree", "tester", 2)
    );
    assert!((a[0].3 - 1.0).abs() < 1e-3);
    // The new appearance is an underline: a thin line, not a filled box.
    let n = colored(&doc);
    assert!(n * 4 < as_highlight && n > 0, "{n} vs {as_highlight}");
    drop(doc);
    let (doc, _) = Doc::open(&path).unwrap();
    doc.update_annotation(
        0,
        xref,
        &AnnotChange {
            subject: Some(String::new()),
            contents: Some(String::new()),
            ..AnnotChange::default()
        },
    )
    .unwrap();
    let listed = doc.annotations(0).unwrap();
    assert_eq!(
        (listed[0].subject.as_ref(), listed[0].contents.as_ref()),
        (None, None)
    );
}

#[test]
fn box_and_note_round_trip() {
    let (_dir, path) = copy("text.pdf", "box");
    let (mut doc, _) = Doc::open(&path).unwrap();
    let mut b = spec(AnnotKind::Box);
    b.rects = vec![BBox::new(100.0, 200.0, 300.0, 260.0)];
    b.stroke_pt = 1.5;
    doc.create_annotation(&b).unwrap();
    let mut n = spec(AnnotKind::Note);
    n.rects = vec![BBox::new(10.0, 10.0, 30.0, 30.0)];
    doc.create_annotation(&n).unwrap();
    doc.save(false).unwrap();

    let pdf = PdfDocument::open(path.to_str().unwrap()).unwrap();
    let page = pdf.load_pdf_page(0).unwrap();
    let annots: Vec<_> = page
        .annotations()
        .filter(|a| a.r#type().unwrap() != PdfAnnotationType::Link)
        .collect();
    assert_eq!(annots.len(), 2);
    let sq = &annots[0];
    assert_eq!(sq.r#type().unwrap(), PdfAnnotationType::Square);
    assert!((sq.border_width().unwrap() - 1.5).abs() < 1e-3);
    assert!(sq.interior_color().unwrap().is_none(), "not filled");
    assert_eq!(sq.contents().unwrap(), Some("why?"));
    assert_eq!(annots[1].r#type().unwrap(), PdfAnnotationType::Text);
    let listed = Doc::open(&path).unwrap().0.annotations(0).unwrap();
    let r = listed[0].rect;
    // The rectangle may grow by half the line width; it covers the box.
    assert!(
        r.x0 <= 100.0 && r.y0 <= 200.0 && r.x1 >= 300.0 && r.y1 >= 260.0,
        "{r:?}"
    );
    assert!(r.x1 <= 302.0 && r.y1 <= 262.0, "{r:?}");
}

#[test]
fn recolored_pages_keep_annotation_colors() {
    let (_dir, path) = copy("text.pdf", "recolor");
    let (doc, _) = Doc::open(&path).unwrap();
    let mut hl = spec(AnnotKind::Highlight);
    hl.color = [0xFF, 0xD4, 0x00];
    doc.create_annotation(&hl).unwrap();
    let rc = Recolor {
        fg: [0xD0, 0xD0, 0xD0],
        bg: [0x1E, 0x1E, 0x1E],
    };
    let px = |x: u32, y: u32, recolor| {
        let p = doc
            .render(
                0,
                1.0,
                PixelRect {
                    x,
                    y,
                    width: 1,
                    height: 1,
                },
                recolor,
            )
            .unwrap();
        [p.rgba[0], p.rgba[1], p.rgba[2]]
    };
    // Empty paper: white, or the dark background.
    assert_eq!(px(500, 500, None), [255, 255, 255]);
    assert_eq!(px(500, 500, Some(rc)), rc.bg);
    // In the highlight, right of the text on its second line: yellowish
    // either way (red and green well above blue).
    for recolor in [None, Some(rc)] {
        let [r, g, b] = px(245, 110, recolor);
        assert!(
            r > 80 && g > 60 && i32::from(b) < i32::from(r) - 60,
            "{recolor:?}: {r},{g},{b}"
        );
    }
}
