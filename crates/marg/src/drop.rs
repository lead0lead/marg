//! Files dropped on the window (concept §2.1): the first PDF opens in this
//! window, each further one in a new marg process.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What to do with one drop.
#[derive(Debug, PartialEq)]
pub struct Dropped {
    /// The PDFs, in the order they were dropped; empty if there were too
    /// many or none.
    pub pdfs: Vec<PathBuf>,
    /// The status message, if any.
    pub message: Option<String>,
}

/// Sort the dropped paths into PDFs and the rest. More than `max` PDFs
/// opens none of them.
pub fn sort(paths: Vec<PathBuf>, max: usize) -> Dropped {
    let (pdfs, others): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| is_pdf(p));
    if pdfs.len() > max {
        return Dropped {
            message: Some(format!(
                "{} PDFs dropped; at most {max} open at once",
                pdfs.len()
            )),
            pdfs: Vec::new(),
        };
    }
    let message = (!others.is_empty()).then(|| {
        let names: Vec<_> = others
            .iter()
            .map(|p| p.file_name().unwrap_or(p.as_os_str()).to_string_lossy())
            .collect();
        format!("Not a PDF: {}", names.join(", "))
    });
    Dropped { pdfs, message }
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Start another marg window for `path` without waiting for it.
pub fn open_in_new_window(path: &Path) -> std::io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Reap it when it exits.
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn keeps_pdfs_in_order_in_any_case() {
        let d = sort(paths(&["/a/b.pdf", "/a/c.PDF", "/a/d.Pdf"]), 10);
        assert_eq!(d.pdfs, paths(&["/a/b.pdf", "/a/c.PDF", "/a/d.Pdf"]));
        assert_eq!(d.message, None);
    }

    #[test]
    fn names_what_is_not_a_pdf() {
        let d = sort(paths(&["/a/notes.txt", "/a/b.pdf", "/a/dir", "/a/pdf"]), 10);
        assert_eq!(d.pdfs, paths(&["/a/b.pdf"]));
        assert_eq!(d.message.as_deref(), Some("Not a PDF: notes.txt, dir, pdf"));
    }

    #[test]
    fn too_many_opens_none() {
        let d = sort(paths(&["1.pdf", "2.pdf", "3.pdf", "x.txt"]), 2);
        assert!(d.pdfs.is_empty());
        assert_eq!(
            d.message.as_deref(),
            Some("3 PDFs dropped; at most 2 open at once")
        );
    }

    #[test]
    fn at_the_limit_opens_all() {
        assert_eq!(sort(paths(&["1.pdf", "2.pdf"]), 2).pdfs.len(), 2);
    }
}
