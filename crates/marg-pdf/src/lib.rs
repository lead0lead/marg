//! MuPDF adapter for marg.
//!
//! MuPDF objects are not `Send`: everything in [`document`] must run on the
//! thread that opened the document. The UI talks to that thread through
//! [`thread::DocThread`].

pub mod document;
pub mod thread;

pub use document::{DocInfo, PageSize, Pixels};
pub use thread::{DocThread, RenderKey, Request, Response, Scale, Tile};
