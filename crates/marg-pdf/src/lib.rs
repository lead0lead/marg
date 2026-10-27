//! MuPDF adapter for marg.
//!
//! MuPDF objects are not `Send`: everything in this crate must run on the
//! thread that opened the document.

use std::path::Path;

use mupdf::{Colorspace, Document, Matrix};

/// An RGB render of one page.
pub struct RenderedPage {
    pub width: usize,
    pub height: usize,
    /// Tightly packed RGB, `width * height * 3` bytes.
    pub rgb: Vec<u8>,
    pub page_count: usize,
}

/// Render page `index` of the PDF at `path`, including annotations.
/// `scale` is pixels per PDF point.
pub fn render_page(path: &Path, index: usize, scale: f32) -> Result<RenderedPage, String> {
    let path = path.to_str().ok_or("path is not valid UTF-8")?;
    let doc = Document::open(path).map_err(|e| e.to_string())?;
    let page_count = doc.page_count().map_err(|e| e.to_string())? as usize;
    let page = doc.load_page(index as i32).map_err(|e| e.to_string())?;
    let pix = page
        .to_pixmap(
            &Matrix::new_scale(scale, scale),
            &Colorspace::device_rgb(),
            false,
            true,
        )
        .map_err(|e| e.to_string())?;
    let (width, height) = (pix.width() as usize, pix.height() as usize);
    let stride = pix.stride() as usize;
    let mut rgb = Vec::with_capacity(width * height * 3);
    for row in pix.samples().chunks(stride).take(height) {
        rgb.extend_from_slice(&row[..width * 3]);
    }
    Ok(RenderedPage {
        width,
        height,
        rgb,
        page_count,
    })
}
