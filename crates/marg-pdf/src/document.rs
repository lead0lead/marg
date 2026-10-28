//! An open PDF and the operations the document thread performs on it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use marg_core::annot::{AnnotKind, AnnotSpec};
use marg_core::text::{BBox, Char, PageText};
use mupdf::color::AnnotationColor;
use mupdf::pdf::{PdfDocument, PdfObject, PdfWriteOptions};
use mupdf::{
    Colorspace, Device, Matrix, MetadataName, Pixmap, Point, Quad, TextBlockContent, TextPageFlags,
};

/// Page size in PDF points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSize {
    pub width: f32,
    pub height: f32,
}

/// What the UI needs to know about an open document.
#[derive(Debug, Clone)]
pub struct DocInfo {
    pub path: PathBuf,
    pub page_sizes: Vec<PageSize>,
    /// Page labels (e.g. "xiv"), if the PDF defines them.
    pub labels: Option<Vec<String>>,
    /// First element of the trailer `/ID` array, if present.
    pub id: Option<Vec<u8>>,
    /// Title from the metadata, if not empty.
    pub title: Option<String>,
}

/// A rectangle in device pixels of a page rendered at some scale, with
/// (0, 0) at the page's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Rendered pixels: opaque RGBA, tightly packed, `width * height * 4` bytes.
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// An open PDF. Not `Send`: lives on the thread that opened it.
pub struct Doc {
    doc: PdfDocument,
    /// The file marg saves to.
    path: PathBuf,
    /// The file the document was loaded from: `path`, except after a full
    /// save whose final rename failed.
    source: PathBuf,
    /// The file as marg last saw it (opened or saved).
    disk: Option<Stamp>,
    /// File name of the backup copy in the backup folder.
    backup_name: String,
    backed_up: bool,
}

/// What identifies a version of a file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl Stamp {
    pub fn of(path: &Path) -> Option<Stamp> {
        let m = std::fs::metadata(path).ok()?;
        Some(Stamp {
            modified: m.modified().ok(),
            len: m.len(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The file changed on disk since marg opened or saved it.
    ChangedOnDisk,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaveReport {
    /// The whole file was rewritten (not appended to).
    pub full: bool,
}

/// FNV-1a, for a stable short name from a path.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn backup_name(path: &Path, id: Option<&[u8]>) -> String {
    let file = path
        .file_name()
        .map_or("file.pdf".into(), |n| n.to_string_lossy().into_owned());
    let key = match id {
        Some(id) if !id.is_empty() => hex(id),
        _ => format!("path{:016x}", fnv1a(path.to_string_lossy().as_bytes())),
    };
    format!("{key}-{file}")
}

/// A temporary file next to `path` for a full save; unique per call.
fn temp_path(path: &Path) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let file = path
        .file_name()
        .map_or("file.pdf".into(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{file}.{}-{n}.marg-tmp", std::process::id()))
}

fn open_pdf(path: &Path) -> Result<PdfDocument, String> {
    let s = path.to_str().ok_or("path is not valid UTF-8")?;
    PdfDocument::open(s).map_err(err)
}

fn err(e: mupdf::Error) -> String {
    e.to_string()
}

/// A normalized `[x0 y0 x1 y1]` box from a page dictionary.
fn read_box(page: &PdfObject, key: &str) -> Result<Option<[f32; 4]>, mupdf::Error> {
    let Some(arr) = page.get_dict_inheritable(key)? else {
        return Ok(None);
    };
    if !arr.is_array()? || arr.len()? != 4 {
        return Ok(None);
    }
    let mut v = [0.0; 4];
    for (i, x) in v.iter_mut().enumerate() {
        *x = arr.get_array(i as i32)?.map_or(Ok(0.0), |n| n.as_float())?;
    }
    Ok(Some([
        v[0].min(v[2]),
        v[1].min(v[3]),
        v[0].max(v[2]),
        v[1].max(v[3]),
    ]))
}

/// Page size from the page dictionary alone (CropBox ∩ MediaBox, Rotate,
/// UserUnit), as MuPDF computes page bounds. Much faster than loading every
/// page at open. `None` if the boxes are missing or degenerate.
fn page_size_from_dict(page: &PdfObject) -> Result<Option<PageSize>, mupdf::Error> {
    let Some(media) = read_box(page, "MediaBox")? else {
        return Ok(None);
    };
    let b = match read_box(page, "CropBox")? {
        Some(c) => [
            c[0].max(media[0]),
            c[1].max(media[1]),
            c[2].min(media[2]),
            c[3].min(media[3]),
        ],
        None => media,
    };
    let unit = match page.get_dict("UserUnit")? {
        Some(u) if u.is_number()? && u.as_float()? > 0.0 => u.as_float()?,
        _ => 1.0,
    };
    let (w, h) = ((b[2] - b[0]) * unit, (b[3] - b[1]) * unit);
    if w <= 0.0 || h <= 0.0 {
        return Ok(None);
    }
    let rotate = page
        .get_dict_inheritable("Rotate")?
        .map_or(Ok(0), |r| r.as_int())?
        .rem_euclid(360);
    Ok(Some(if rotate == 90 || rotate == 270 {
        PageSize {
            width: h,
            height: w,
        }
    } else {
        PageSize {
            width: w,
            height: h,
        }
    }))
}

impl Doc {
    pub fn open(path: &Path) -> Result<(Self, DocInfo), String> {
        let path_str = path.to_str().ok_or("path is not valid UTF-8")?;
        let doc = PdfDocument::open(path_str).map_err(err)?;
        let count = doc.page_count().map_err(err)? as usize;
        let page_sizes = (0..count)
            .map(|i| Self::page_size(&doc, i))
            .collect::<Result<Vec<_>, _>>()?;
        let has_labels = doc
            .catalog()
            .and_then(|c| c.get_dict("PageLabels"))
            .map_err(err)?
            .is_some();
        let labels = if has_labels {
            Some(
                (0..count)
                    .map(|i| doc.page_label(i))
                    .collect::<Result<_, _>>()
                    .map_err(err)?,
            )
        } else {
            None
        };
        let id = match doc.trailer().and_then(|t| t.get_dict("ID")).map_err(err)? {
            Some(ids) => match ids.get_array(0).map_err(err)? {
                Some(first) => Some(first.as_bytes().map_err(err)?.to_vec()),
                None => None,
            },
            None => None,
        };
        let title = doc
            .metadata(MetadataName::Title)
            .ok()
            .filter(|t| !t.trim().is_empty());
        let info = DocInfo {
            path: path.to_path_buf(),
            page_sizes,
            labels,
            id,
            title,
        };
        let disk = Stamp::of(path);
        let doc = Self {
            doc,
            path: path.to_path_buf(),
            source: path.to_path_buf(),
            disk,
            backup_name: backup_name(path, info.id.as_deref()),
            backed_up: false,
        };
        Ok((doc, info))
    }

    fn page_size(doc: &PdfDocument, index: usize) -> Result<PageSize, String> {
        let obj = doc.find_page(index as i32).map_err(err)?;
        if let Some(size) = page_size_from_dict(&obj).map_err(err)? {
            return Ok(size);
        }
        let b = doc
            .load_page(index as i32)
            .and_then(|p| p.bounds())
            .map_err(err)?;
        Ok(PageSize {
            width: b.width(),
            height: b.height(),
        })
    }

    /// Page size from MuPDF's own page bounds (loads the page). For checking
    /// the fast path in tests and tools.
    pub fn loaded_page_size(&self, index: usize) -> Result<PageSize, String> {
        let b = self
            .doc
            .load_page(index as i32)
            .and_then(|p| p.bounds())
            .map_err(err)?;
        Ok(PageSize {
            width: b.width(),
            height: b.height(),
        })
    }

    /// Size of a page in device pixels at `scale` (pixels per point).
    pub fn page_pixels(size: PageSize, scale: f32) -> (u32, u32) {
        (
            (size.width * scale).ceil() as u32,
            (size.height * scale).ceil() as u32,
        )
    }

    /// Render part of page `index` at `scale` pixels per point, with
    /// annotations, on a white background.
    pub fn render(&self, index: usize, scale: f32, rect: PixelRect) -> Result<Pixels, String> {
        let page = self.doc.load_page(index as i32).map_err(err)?;
        // Bounds may not start at (0, 0) (offset boxes).
        let b = page.bounds().map_err(err)?;
        let (ox, oy) = (b.x0, b.y0);
        let mut pix = Pixmap::new(
            &Colorspace::device_rgb(),
            rect.x as i32,
            rect.y as i32,
            rect.width as i32,
            rect.height as i32,
            true,
        )
        .map_err(err)?;
        // White and opaque; with alpha 255 everywhere, premultiplied and
        // straight RGBA are the same.
        pix.clear_with(255).map_err(err)?;
        {
            let dev = Device::from_pixmap(&pix).map_err(err)?;
            let ctm = Matrix::new(scale, 0.0, 0.0, scale, -ox * scale, -oy * scale);
            page.run(&dev, &ctm).map_err(err)?;
        }
        let (w, h) = (rect.width as usize, rect.height as usize);
        let stride = pix.stride() as usize;
        let samples = pix.samples();
        let rgba = if stride == w * 4 {
            samples[..w * h * 4].to_vec()
        } else {
            let mut v = Vec::with_capacity(w * h * 4);
            for row in samples.chunks(stride).take(h) {
                v.extend_from_slice(&row[..w * 4]);
            }
            v
        };
        Ok(Pixels {
            width: rect.width,
            height: rect.height,
            rgba,
        })
    }

    /// The structured text of page `index`, in points from the page's
    /// top-left corner (the same frame as renders).
    pub fn text(&self, index: usize) -> Result<PageText, String> {
        let page = self.doc.load_page(index as i32).map_err(err)?;
        let b = page.bounds().map_err(err)?;
        let st = page
            .to_text_page(TextPageFlags::empty())
            .map_err(err)?
            .structured();
        let blocks = st
            .blocks
            .into_iter()
            .filter_map(|block| match block.content {
                TextBlockContent::Text { lines } => Some(lines),
                _ => None,
            })
            .map(|lines| {
                lines
                    .into_iter()
                    .map(|line| {
                        line.chars
                            .into_iter()
                            .map(|c| {
                                let q = c.quad;
                                let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
                                let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
                                let min = |v: [f32; 4]| v.into_iter().fold(f32::MAX, f32::min);
                                let max = |v: [f32; 4]| v.into_iter().fold(f32::MIN, f32::max);
                                Char {
                                    c: c.ch,
                                    bbox: BBox::new(
                                        min(xs) - b.x0,
                                        min(ys) - b.y0,
                                        max(xs) - b.x0,
                                        max(ys) - b.y0,
                                    ),
                                }
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        Ok(PageText::from_blocks(blocks))
    }

    /// The offset of page `index`'s top-left corner in MuPDF page space.
    fn page_origin(&self, index: usize) -> Result<(f32, f32), String> {
        let b = self
            .doc
            .load_page(index as i32)
            .and_then(|p| p.bounds())
            .map_err(err)?;
        Ok((b.x0, b.y0))
    }

    /// Create an annotation. Returns its object number.
    pub fn create_annotation(&self, spec: &AnnotSpec) -> Result<i32, String> {
        let (ox, oy) = self.page_origin(spec.page)?;
        let quads: Vec<Quad> = spec
            .rects
            .iter()
            .map(|r| {
                let (x0, y0, x1, y1) = (r.x0 + ox, r.y0 + oy, r.x1 + ox, r.y1 + oy);
                Quad::new(
                    Point::new(x0, y0),
                    Point::new(x1, y0),
                    Point::new(x0, y1),
                    Point::new(x1, y1),
                )
            })
            .collect();
        if quads.is_empty() {
            return Err("nothing to annotate".into());
        }
        let mut page = self.doc.load_pdf_page(spec.page as i32).map_err(err)?;
        let mut annot = match spec.kind {
            AnnotKind::Highlight => page.add_highlight_annotation(quads),
            AnnotKind::Underline => page.add_underline_annotation(quads),
        }
        .map_err(err)?;
        let [r, g, b] = spec.color.map(|c| c as f32 / 255.0);
        annot
            .set_color(AnnotationColor::Rgb {
                red: r,
                green: g,
                blue: b,
            })
            .map_err(err)?;
        if spec.kind == AnnotKind::Highlight {
            annot.set_opacity(spec.opacity).map_err(err)?;
        }
        annot.set_author(&spec.author).map_err(err)?;
        annot
            .object()
            .dict_put("Subj", PdfObject::new_string(&spec.subject).map_err(err)?)
            .map_err(err)?;
        annot.update().map_err(err)?;
        page.update().map_err(err)?;
        annot.xref().map_err(err)
    }

    /// Delete the annotation with object number `xref` from page `index`.
    pub fn delete_annotation(&self, index: usize, xref: i32) -> Result<(), String> {
        let mut page = self.doc.load_pdf_page(index as i32).map_err(err)?;
        let annot = page
            .annotations()
            .find(|a| a.xref().ok() == Some(xref))
            .ok_or("annotation not found")?;
        page.delete_annotation(annot).map_err(err)?;
        page.update().map_err(err)?;
        Ok(())
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.doc.has_unsaved_changes()
    }

    /// Copy the file into `dir` before its first modification, unless a
    /// backup of it exists already (backups are never overwritten).
    /// Returns the new backup's path.
    pub fn ensure_backup(&mut self, dir: &Path) -> Result<Option<PathBuf>, String> {
        if self.backed_up {
            return Ok(None);
        }
        let target = dir.join(&self.backup_name);
        if target.exists() {
            self.backed_up = true;
            return Ok(None);
        }
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        std::fs::copy(&self.path, &target).map_err(|e| format!("{}: {e}", target.display()))?;
        self.backed_up = true;
        Ok(Some(target))
    }

    /// Whether the file on disk differs from the version marg opened or
    /// last saved.
    pub fn changed_on_disk(&self) -> bool {
        Stamp::of(&self.path) != self.disk
    }

    /// Save the annotation changes. Incremental (appended) when possible;
    /// otherwise, or with `force`, the whole file is written to a temporary
    /// file and renamed over the original. Without `force`, a file changed
    /// on disk is not touched. `Ok(None)`: nothing to save.
    pub fn save(&mut self, force: bool) -> Result<Option<SaveReport>, SaveError> {
        let same_file = self.source == self.path;
        if same_file && !self.doc.has_unsaved_changes() && !force {
            return Ok(None);
        }
        let changed = self.changed_on_disk();
        if changed && !force {
            return Err(SaveError::ChangedOnDisk);
        }
        let incremental = !force && !changed && same_file && self.doc.can_be_saved_incrementally();
        if incremental {
            let mut opts = PdfWriteOptions::default();
            opts.set_incremental(true);
            let path = self
                .path
                .to_str()
                .ok_or(SaveError::Failed("path is not valid UTF-8".into()))?;
            self.doc
                .save_with_options(path, opts)
                .map_err(|e| SaveError::Failed(err(e)))?;
        } else {
            self.save_full().map_err(SaveError::Failed)?;
        }
        self.disk = Stamp::of(&self.path);
        Ok(Some(SaveReport { full: !incremental }))
    }

    /// Write the whole document to a temporary file, rename it over the
    /// original, and continue from the new file.
    fn save_full(&mut self) -> Result<(), String> {
        let tmp = temp_path(&self.path);
        let tmp_str = tmp.to_str().ok_or("path is not valid UTF-8")?;
        self.doc
            .save_with_options(tmp_str, PdfWriteOptions::default())
            .map_err(err)?;
        // Windows cannot rename over a file that is open (MuPDF has it
        // open), so on failure close the document and try again.
        let renamed = std::fs::rename(&tmp, &self.path).or_else(|_| {
            self.doc = PdfDocument::new();
            std::fs::rename(&tmp, &self.path)
        });
        // Continue from whichever file has the changes.
        let from = if renamed.is_ok() {
            self.path.clone()
        } else {
            tmp.clone()
        };
        self.doc = open_pdf(&from)?;
        self.source = from;
        renamed.map_err(|e| {
            format!(
                "cannot replace {} ({e}); the changes are in {}",
                self.path.display(),
                tmp.display()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_name_without_id_hashes_the_path() {
        let a = backup_name(Path::new("/papers/a/x.pdf"), None);
        let b = backup_name(Path::new("/papers/b/x.pdf"), None);
        assert!(a.starts_with("path") && a.ends_with("-x.pdf"), "{a}");
        assert_ne!(a, b);
        assert_eq!(a, backup_name(Path::new("/papers/a/x.pdf"), Some(&[])));
    }
}
