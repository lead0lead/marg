//! Encrypted PDFs: opening with a password, and saving keeps the
//! encryption. Works on encrypted copies of the fixtures in Cargo's temp
//! dir.

use std::path::{Path, PathBuf};

use marg_core::annot::{AnnotKind, AnnotSpec};
use marg_core::text::BBox;
use marg_pdf::document::Doc;
use marg_pdf::{OpenError, SaveReport};
use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions, Permission};

/// `text.pdf` encrypted (AES-256) with user password "user" and owner
/// password "owner", in a fresh folder.
fn encrypted(tag: &str) -> PathBuf {
    encrypted_with(tag, "user", Permission::all())
}

fn encrypted_with(tag: &str, user_password: &str, permissions: Permission) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("password-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/text.pdf");
    let doc = PdfDocument::open(src.to_str().unwrap()).unwrap();
    let mut opts = PdfWriteOptions::default();
    opts.set_encryption(Encryption::Aes256)
        .set_user_password(user_password)
        .set_owner_password("owner")
        .set_permissions(permissions);
    let path = dir.join("locked.pdf");
    doc.save_with_options(path.to_str().unwrap(), opts).unwrap();
    path
}

fn highlight() -> AnnotSpec {
    AnnotSpec {
        page: 0,
        kind: AnnotKind::Highlight,
        rects: vec![BBox::new(72.0, 90.0, 300.0, 104.0)],
        color: [0xFF, 0xD4, 0x00],
        opacity: 0.4,
        stroke_pt: 0.0,
        subject: "Key point".into(),
        author: "tester".into(),
        contents: Some("secret note".into()),
    }
}

/// The file still needs a password, and with it has `n` highlights.
fn assert_encrypted_with(path: &Path, n: usize) {
    let bytes = std::fs::read(path).unwrap();
    assert!(
        !bytes.windows(11).any(|w| w == b"secret note"),
        "the comment is stored in clear text"
    );
    assert_eq!(
        Doc::open_with_password(path, None).err(),
        Some(OpenError::NeedsPassword)
    );
    let (doc, _) = Doc::open_with_password(path, Some("user")).unwrap();
    let a = doc.annotations(0).unwrap();
    let highlights: Vec<_> = a.iter().filter(|a| a.kind == "Highlight").collect();
    assert_eq!(highlights.len(), n, "{a:?}");
    assert!(
        highlights
            .iter()
            .all(|h| h.contents.as_deref() == Some("secret note"))
    );
}

#[test]
fn opens_only_with_a_password() {
    let path = encrypted("open");
    assert_eq!(
        Doc::open_with_password(&path, None).err(),
        Some(OpenError::NeedsPassword)
    );
    assert_eq!(
        Doc::open_with_password(&path, Some("nope")).err(),
        Some(OpenError::WrongPassword)
    );
    let (_, info) = Doc::open_with_password(&path, Some("user")).unwrap();
    assert_eq!(info.page_sizes.len(), 2);
    assert!(info.can_annotate && info.writable);
    assert!(Doc::open_with_password(&path, Some("owner")).is_ok());
    // A password is ignored for files that need none.
    let plain = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/text.pdf");
    assert!(Doc::open_with_password(&plain, Some("x")).is_ok());
}

#[test]
fn incremental_save_keeps_the_encryption() {
    let path = encrypted("incremental");
    let before = std::fs::read(&path).unwrap();
    let (mut doc, _) = Doc::open_with_password(&path, Some("user")).unwrap();
    doc.create_annotation(&highlight()).unwrap();
    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));
    let after = std::fs::read(&path).unwrap();
    assert_eq!(&after[..before.len()], &before[..], "appended");
    drop(doc);
    assert_encrypted_with(&path, 1);
}

#[test]
fn full_save_keeps_the_encryption_and_continues() {
    let path = encrypted("full");
    let (mut doc, _) = Doc::open_with_password(&path, Some("user")).unwrap();
    doc.create_annotation(&highlight()).unwrap();
    assert_eq!(doc.save(true), Ok(Some(SaveReport { full: true })));
    // The document was opened again from the new file, with the password.
    doc.create_annotation(&highlight()).unwrap();
    assert_eq!(doc.save(false), Ok(Some(SaveReport { full: false })));
    drop(doc);
    assert_encrypted_with(&path, 2);
}

#[test]
fn permissions_without_annotations_are_read() {
    // Like publishers' files: no password to open, only an owner password.
    let path = encrypted_with("forbid", "", Permission::all() - Permission::ANNOTATE);
    let (_, info) = Doc::open_with_password(&path, None).unwrap();
    assert!(!info.can_annotate);
    assert!(info.writable);
    let path = encrypted_with("allow", "", Permission::PRINT | Permission::ANNOTATE);
    let (_, info) = Doc::open_with_password(&path, None).unwrap();
    assert!(info.can_annotate);
}

#[cfg(unix)]
#[test]
fn a_write_protected_file_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;
    let path = encrypted_with("readonly", "", Permission::all());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    let (_, info) = Doc::open_with_password(&path, None).unwrap();
    // root may write anyway.
    let root = std::fs::OpenOptions::new().append(true).open(&path).is_ok();
    assert_eq!(info.writable, root);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
}
