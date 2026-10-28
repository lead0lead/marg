//! Saving annotations: incremental and full saves, backups, external
//! changes. Works on copies of the fixtures in Cargo's temp dir.

use std::path::{Path, PathBuf};

use marg_core::annot::{AnnotKind, AnnotSpec};
use marg_core::text::BBox;
use marg_pdf::document::Doc;
use marg_pdf::{SaveError, SaveReport};
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
        subject: "Question".into(),
        author: "tester".into(),
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
