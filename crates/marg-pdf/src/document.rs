//! An open PDF and the operations the document thread performs on it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use marg_core::annot::{AnnotChange, AnnotInfo, AnnotKind, AnnotSpec};
use marg_core::outline::{Dest, Link, LinkTarget, OutlineEntry};
use marg_core::text::{BBox, Char, PageText};
use mupdf::color::AnnotationColor;
use mupdf::pdf::{PdfAnnotationType, PdfDocument, PdfObject, PdfWriteOptions};
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

/// Page colors for dark pages: black becomes `fg`, white `bg`, and the
/// colors in between follow (each channel separately).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Recolor {
    pub fg: [u8; 3],
    pub bg: [u8; 3],
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
    /// The last external version reported by `poll_disk`.
    reported: Option<Stamp>,
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

/// A URI with a scheme (`https:`, `mailto:`), as opposed to `#page=3`.
fn is_external(uri: &str) -> bool {
    uri.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    })
}

/// A resolved link destination (MuPDF page space: y down from the top).
fn dest(d: mupdf::link::LinkDestination) -> Dest {
    use mupdf::DestinationKind as K;
    let (x, y) = match d.kind {
        K::XYZ { left, top, .. } => (left, top),
        K::FitH { top } | K::FitBH { top } => (None, top),
        K::FitV { left } | K::FitBV { left } => (left, None),
        K::FitR {
            left, bottom, top, ..
        } => (Some(left), Some(top.min(bottom))),
        K::Fit | K::FitB => (None, None),
    };
    Dest {
        page: d.loc.page_number as usize,
        x,
        y,
    }
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
            reported: disk,
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
    /// With `recolor`, the page content is tinted and annotations and form
    /// fields are drawn on top in their own colors.
    pub fn render(
        &self,
        index: usize,
        scale: f32,
        rect: PixelRect,
        recolor: Option<Recolor>,
    ) -> Result<Pixels, String> {
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
            match recolor {
                None => page.run(&dev, &ctm).map_err(err)?,
                Some(_) => page.run_contents(&dev, &ctm).map_err(err)?,
            }
        }
        if let Some(rc) = recolor {
            let int = |[r, g, b]: [u8; 3]| i32::from_be_bytes([0, r, g, b]);
            pix.tint(int(rc.fg), int(rc.bg)).map_err(err)?;
            // Annotations go on a transparent layer, then over the page:
            // drawn onto the dark page directly, highlights (multiply
            // blending) would almost vanish.
            let mut layer = Pixmap::new(
                &Colorspace::device_rgb(),
                rect.x as i32,
                rect.y as i32,
                rect.width as i32,
                rect.height as i32,
                true,
            )
            .map_err(err)?;
            layer.clear().map_err(err)?;
            {
                let dev = Device::from_pixmap(&layer).map_err(err)?;
                let ctm = Matrix::new(scale, 0.0, 0.0, scale, -ox * scale, -oy * scale);
                page.run_annotations(&dev, &ctm).map_err(err)?;
                page.run_widgets(&dev, &ctm).map_err(err)?;
            }
            // Both are premultiplied: out = layer + page × (1 − layer alpha).
            for (d, s) in pix
                .samples_mut()
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(layer.samples().as_chunks::<4>().0)
            {
                let keep = 255 - u16::from(s[3]);
                for i in 0..3 {
                    d[i] = s[i].saturating_add(((u16::from(d[i]) * keep + 127) / 255) as u8);
                }
            }
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

    /// The links on page `index`: internal ones with their destination,
    /// others with their URI (if it has a scheme, like `https:`).
    pub fn links(&self, index: usize) -> Result<Vec<Link>, String> {
        let page = self.doc.load_page(index as i32).map_err(err)?;
        let b = page.bounds().map_err(err)?;
        let mut out = Vec::new();
        for l in page.links().map_err(err)? {
            let target = match l.dest {
                Some(d) => LinkTarget::Page(dest(d)),
                None if is_external(&l.uri) => LinkTarget::Uri(l.uri),
                None => continue,
            };
            let r = l.bounds;
            out.push(Link {
                rect: BBox::new(r.x0 - b.x0, r.y0 - b.y0, r.x1 - b.x0, r.y1 - b.y0),
                target,
            });
        }
        Ok(out)
    }

    /// The table of contents, flattened in document order.
    pub fn outline(&self) -> Result<Vec<OutlineEntry>, String> {
        fn walk(items: &[mupdf::Outline], level: usize, out: &mut Vec<OutlineEntry>) {
            for o in items {
                out.push(OutlineEntry {
                    title: o.title.trim().to_string(),
                    level,
                    dest: o.dest.map(dest),
                });
                walk(&o.down, level + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.doc.outlines().map_err(err)?, 0, &mut out);
        Ok(out)
    }

    /// Where images are drawn on page `index`, in points. Separate from
    /// `text`: keeping images in the text page can take much longer (a
    /// book cover: 394 ms instead of under 80 ms).
    pub fn images(&self, index: usize) -> Result<Vec<BBox>, String> {
        let page = self.doc.load_page(index as i32).map_err(err)?;
        let b = page.bounds().map_err(err)?;
        let st = page
            .to_text_page(TextPageFlags::PRESERVE_IMAGES)
            .map_err(err)?
            .structured();
        Ok(st
            .blocks
            .iter()
            .filter(|block| matches!(block.content, TextBlockContent::Image { .. }))
            .map(|block| {
                let r = block.bounds;
                BBox::new(r.x0 - b.x0, r.y0 - b.y0, r.x1 - b.x0, r.y1 - b.y0)
            })
            .collect())
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
        let shifted: Vec<mupdf::Rect> = spec
            .rects
            .iter()
            .map(|r| mupdf::Rect::new(r.x0 + ox, r.y0 + oy, r.x1 + ox, r.y1 + oy))
            .collect();
        let Some(&first) = shifted.first() else {
            return Err("nothing to annotate".into());
        };
        let quads = || {
            shifted.iter().map(|r| {
                Quad::new(
                    Point::new(r.x0, r.y0),
                    Point::new(r.x1, r.y0),
                    Point::new(r.x0, r.y1),
                    Point::new(r.x1, r.y1),
                )
            })
        };
        let mut page = self.doc.load_pdf_page(spec.page as i32).map_err(err)?;
        let mut annot = match spec.kind {
            AnnotKind::Highlight => page.add_highlight_annotation(quads().collect::<Vec<_>>()),
            AnnotKind::Underline => page.add_underline_annotation(quads().collect::<Vec<_>>()),
            AnnotKind::Note => page.add_text_annotation(first, ""),
            AnnotKind::Box => page.add_square_annotation(first),
        }
        .map_err(err)?;
        if spec.kind == AnnotKind::Box {
            annot.set_border_width(spec.stroke_pt).map_err(err)?;
        }
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
        if let Some(c) = &spec.contents {
            annot.set_contents(c).map_err(err)?;
        }
        if !spec.subject.is_empty() {
            annot
                .object()
                .dict_put("Subj", PdfObject::new_string(&spec.subject).map_err(err)?)
                .map_err(err)?;
        }
        annot.update().map_err(err)?;
        page.update().map_err(err)?;
        annot.xref().map_err(err)
    }

    /// The annotations on page `index`, except links, form fields and
    /// popups.
    pub fn annotations(&self, index: usize) -> Result<Vec<AnnotInfo>, String> {
        let page = self.doc.load_pdf_page(index as i32).map_err(err)?;
        let b = page.bounds().map_err(err)?;
        let rel = |r: mupdf::Rect| BBox::new(r.x0 - b.x0, r.y0 - b.y0, r.x1 - b.x0, r.y1 - b.y0);
        let mut out = Vec::new();
        for a in page.annotations() {
            let kind = a.r#type().map_err(err)?;
            if matches!(
                kind,
                PdfAnnotationType::Link | PdfAnnotationType::Widget | PdfAnnotationType::Popup
            ) {
                continue;
            }
            let quads = if a.has_quads().unwrap_or(false) {
                a.quad_points()
                    .map_err(err)?
                    .into_iter()
                    .map(|q| {
                        let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
                        let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
                        let min = |v: [f32; 4]| v.into_iter().fold(f32::MAX, f32::min);
                        let max = |v: [f32; 4]| v.into_iter().fold(f32::MIN, f32::max);
                        BBox::new(
                            min(xs) - b.x0,
                            min(ys) - b.y0,
                            max(xs) - b.x0,
                            max(ys) - b.y0,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let color = match a.color().ok().flatten() {
                Some(AnnotationColor::Rgb { red, green, blue }) => {
                    Some([to_u8(red), to_u8(green), to_u8(blue)])
                }
                Some(AnnotationColor::Gray(g)) => Some([to_u8(g); 3]),
                Some(AnnotationColor::Cmyk {
                    cyan,
                    magenta,
                    yellow,
                    key,
                }) => Some([
                    to_u8((1.0 - cyan) * (1.0 - key)),
                    to_u8((1.0 - magenta) * (1.0 - key)),
                    to_u8((1.0 - yellow) * (1.0 - key)),
                ]),
                None => None,
            };
            let subject = a
                .object()
                .get_dict("Subj")
                .ok()
                .flatten()
                .and_then(|s| s.as_string().ok().map(|s| s.to_string()))
                .filter(|s| !s.is_empty());
            out.push(AnnotInfo {
                xref: a.xref().map_err(err)?,
                page: index,
                kind: format!("{kind:?}"),
                rect: rel(a.rect().map_err(err)?),
                quads,
                color,
                opacity: a.opacity().unwrap_or(1.0),
                subject,
                author: a
                    .author()
                    .ok()
                    .flatten()
                    .map(str::to_string)
                    .filter(|s| !s.is_empty()),
                contents: a
                    .contents()
                    .ok()
                    .flatten()
                    .map(str::to_string)
                    .filter(|s| !s.is_empty()),
            });
        }
        Ok(out)
    }

    /// Put a deleted annotation back on page `index`. Deleting only removes
    /// it (and its popup) from the page's `/Annots` array; the object stays,
    /// so this restores it with everything it had.
    pub fn restore_annotation(&self, index: usize, xref: i32) -> Result<(), String> {
        let page = self.doc.load_pdf_page(index as i32).map_err(err)?;
        let mut page_obj = page.object();
        let annot = self.doc.new_indirect(xref, 0).map_err(err)?;
        let mut arr = match page_obj.get_dict("Annots").map_err(err)? {
            Some(a) => a,
            None => {
                let a = self.doc.new_array().map_err(err)?;
                page_obj.dict_put("Annots", a).map_err(err)?;
                page_obj
                    .get_dict("Annots")
                    .map_err(err)?
                    .ok_or("cannot create /Annots")?
            }
        };
        let popup = annot
            .resolve()
            .map_err(err)?
            .ok_or("annotation object not found")?
            .get_dict("Popup")
            .map_err(err)?;
        arr.array_push(annot).map_err(err)?;
        if let Some(p) = popup {
            arr.array_push(p).map_err(err)?;
        }
        Ok(())
    }

    /// Change the annotation with object number `xref` on page `index` in
    /// place, so everything else it has (dates, `/NM`, popup) stays.
    pub fn update_annotation(
        &self,
        index: usize,
        xref: i32,
        change: &AnnotChange,
    ) -> Result<(), String> {
        let mut page = self.doc.load_pdf_page(index as i32).map_err(err)?;
        let mut annot = page
            .annotations()
            .find(|a| a.xref().ok() == Some(xref))
            .ok_or("annotation not found")?;
        if let Some(kind) = change.kind {
            let name = match kind {
                AnnotKind::Highlight => "Highlight",
                AnnotKind::Underline => "Underline",
                AnnotKind::Note => "Text",
                AnnotKind::Box => "Square",
            };
            let mut obj = annot.object();
            obj.dict_put("Subtype", PdfObject::new_name(name).map_err(err)?)
                .map_err(err)?;
            // The old appearance is for the old type; the setters below
            // mark the annotation for a new one.
            obj.dict_delete("AP").map_err(err)?;
        }
        if let Some([r, g, b]) = change.color.map(|c| c.map(|c| c as f32 / 255.0)) {
            annot
                .set_color(AnnotationColor::Rgb {
                    red: r,
                    green: g,
                    blue: b,
                })
                .map_err(err)?;
        }
        if let Some(o) = change.opacity {
            annot.set_opacity(o).map_err(err)?;
        }
        if let Some(c) = &change.contents {
            annot.set_contents(c).map_err(err)?;
        }
        if let Some(subj) = &change.subject {
            let mut obj = annot.object();
            if subj.is_empty() {
                obj.dict_delete("Subj").map_err(err)?;
            } else {
                obj.dict_put("Subj", PdfObject::new_string(subj).map_err(err)?)
                    .map_err(err)?;
            }
        }
        annot.update().map_err(err)?;
        page.update().map_err(err)?;
        Ok(())
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

    /// True once for each new version of the file written by another
    /// program.
    pub fn poll_disk(&mut self) -> bool {
        let now = Stamp::of(&self.path);
        if now == self.disk || now == self.reported {
            return false;
        }
        self.reported = now;
        true
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
        self.reported = self.disk;
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
