//! MuPDF adapter for marg.
//!
//! MuPDF objects are not `Send`: everything in [`document`] must run on the
//! thread that opened the document. The UI talks to that thread through
//! [`thread::DocThread`].

pub mod document;
pub mod thread;

pub use document::{DocInfo, PageSize, Pixels, SaveError, SaveReport};
pub use thread::{DocThread, Edit, RenderKey, Request, Response, Scale, Tile};
