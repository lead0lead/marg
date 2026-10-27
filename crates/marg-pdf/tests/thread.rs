//! Document thread: open, info, full-page and tile renders.

use std::path::{Path, PathBuf};
use std::time::Duration;

use marg_pdf::{DocInfo, DocThread, Pixels, RenderKey, Request, Response, Scale, Tile};

const WAIT: Duration = Duration::from_secs(10);

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn open(t: &DocThread, path: PathBuf) -> (u64, Result<DocInfo, String>) {
    t.send(Request::Open(path));
    match t.recv_timeout(WAIT).expect("open response") {
        Response::Opened {
            generation, result, ..
        } => (generation, result),
        Response::Rendered { .. } => panic!("expected Opened"),
    }
}

fn render(t: &DocThread, key: RenderKey) -> (u64, RenderKey, Result<Pixels, String>) {
    t.send(Request::Render(vec![key]));
    match t.recv_timeout(WAIT).expect("render response") {
        Response::Rendered {
            generation,
            key,
            result,
            ..
        } => (generation, key, result),
        Response::Opened { .. } => panic!("expected Rendered"),
    }
}

#[test]
fn open_reports_info() {
    let t = DocThread::spawn(|| ());
    let (generation, info) = open(&t, fixture("text.pdf"));
    let info = info.unwrap();
    assert_eq!(generation, 1);
    assert_eq!(info.page_sizes.len(), 2);
    assert_eq!(
        (info.page_sizes[0].width, info.page_sizes[0].height),
        (612.0, 792.0)
    );
    assert_eq!(info.labels.unwrap(), ["i", "1"]);
    assert_eq!(info.id.unwrap(), b"margtextfixture1");
    assert_eq!(info.title.as_deref(), Some("marg text fixture"));
}

#[test]
fn open_missing_file_is_an_error() {
    let t = DocThread::spawn(|| ());
    let (_, info) = open(&t, fixture("does-not-exist.pdf"));
    assert!(info.is_err());
    // Rendering without a document is an error, not a crash.
    let key = RenderKey {
        page: 0,
        scale: Scale::from_f32(1.0),
        tile: None,
    };
    assert!(render(&t, key).2.is_err());
}

#[test]
fn renders_whole_page_opaque() {
    let t = DocThread::spawn(|| ());
    open(&t, fixture("text.pdf")).1.unwrap();
    let key = RenderKey {
        page: 0,
        scale: Scale::from_f32(1.5),
        tile: None,
    };
    let (generation, got, px) = render(&t, key);
    let px = px.unwrap();
    assert_eq!((generation, got), (1, key));
    assert_eq!((px.width, px.height), (918, 1188));
    assert_eq!(px.rgba.len(), 918 * 1188 * 4);
    assert!(px.rgba.chunks(4).all(|p| p[3] == 255));
    assert!(px.rgba.chunks(4).any(|p| p[0] < 64));
}

#[test]
fn renders_edge_tile_clipped_to_page() {
    let t = DocThread::spawn(|| ());
    open(&t, fixture("text.pdf")).1.unwrap();
    // 612x792 at scale 1 with 512 px tiles: tile (1, 1) is 100x280.
    let key = RenderKey {
        page: 0,
        scale: Scale::from_f32(1.0),
        tile: Some(Tile {
            col: 1,
            row: 1,
            size: 512,
        }),
    };
    let px = render(&t, key).2.unwrap();
    assert_eq!((px.width, px.height), (100, 280));
    let outside = RenderKey {
        tile: Some(Tile {
            col: 2,
            row: 0,
            size: 512,
        }),
        ..key
    };
    assert!(render(&t, outside).2.is_err());
}

#[test]
fn reopen_bumps_generation() {
    let t = DocThread::spawn(|| ());
    assert_eq!(open(&t, fixture("text.pdf")).0, 1);
    assert_eq!(open(&t, fixture("text.pdf")).0, 2);
}

#[test]
fn page_sizes_follow_boxes_rotation_and_user_unit() {
    let path = fixture("boxes.pdf");
    let (doc, info) = marg_pdf::document::Doc::open(&path).unwrap();
    let sizes: Vec<_> = info
        .page_sizes
        .iter()
        .map(|s| (s.width, s.height))
        .collect();
    assert_eq!(
        sizes,
        [
            (792.0, 612.0),
            (500.0, 600.0),
            (300.0, 400.0),
            (595.0, 842.0),
            (600.0, 800.0)
        ]
    );
    for (i, fast) in info.page_sizes.iter().enumerate() {
        assert_eq!(doc.loaded_page_size(i).unwrap(), *fast, "page {i}");
    }
}

#[test]
fn offset_page_renders_from_its_top_left() {
    let t = DocThread::spawn(|| ());
    open(&t, fixture("boxes.pdf")).1.unwrap();
    let key = RenderKey {
        page: 2,
        scale: Scale::from_f32(1.0),
        tile: None,
    };
    let px = render(&t, key).2.unwrap();
    assert_eq!((px.width, px.height), (300, 400));
    // The text sits ~15 pt below the top edge, near the left edge.
    let w = px.width as usize;
    let dark_in_top_band = (0..40)
        .flat_map(|y| (0..150).map(move |x| (y * w + x) * 4))
        .any(|i| px.rgba[i] < 64);
    assert!(dark_in_top_band);
}
