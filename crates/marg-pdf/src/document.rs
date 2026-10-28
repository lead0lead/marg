//! An open PDF and the operations the document thread performs on it.

use std::path::{Path, PathBuf};

use mupdf::pdf::{PdfDocument, PdfObject};
use mupdf::{Colorspace, Device, Matrix, MetadataName, Pixmap};

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
        Ok((Self { doc }, info))
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
}
