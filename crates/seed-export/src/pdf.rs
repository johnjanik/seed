//! PDF export.

use seed_core::{Document, ExportError};
use seed_layout::LayoutTree;

/// Export a document to PDF.
pub fn export(_doc: &Document, _layout: &LayoutTree) -> Result<Vec<u8>, ExportError> {
    // TODO: Implement PDF export
    // Consider using pdf-writer or printpdf crate
    Err(ExportError::UnsupportedFormat {
        format: "PDF".to_string(),
    })
}
