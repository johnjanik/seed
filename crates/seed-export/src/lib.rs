//! Export formats for Seed documents.
//!
//! Supported formats:
//! - SVG (2D vector graphics)
//! - PNG (2D raster)
//! - PDF (2D print-ready) - planned
//! - STEP (3D CAD interchange) - planned
//! - STL (3D printing)

#[cfg(feature = "svg")]
pub mod svg;

#[cfg(feature = "pdf")]
pub mod pdf;

pub mod png;
pub mod step;
pub mod stl;

use seed_core::{Document, ExportError};
use seed_layout::LayoutTree;

// Re-export commonly used types
pub use png::PngOptions;
pub use stl::{mesh_to_stl, mesh_to_stl_ascii};

/// Export a 2D document to SVG.
#[cfg(feature = "svg")]
pub fn export_svg(doc: &Document, layout: &LayoutTree) -> Result<String, ExportError> {
    svg::export(doc, layout)
}

/// Export a 2D document to PNG.
pub fn export_png(doc: &Document, layout: &LayoutTree) -> Result<Vec<u8>, ExportError> {
    png::export(doc, layout)
}

/// Export a 2D document to PNG with custom options.
pub fn export_png_with_options(
    doc: &Document,
    layout: &LayoutTree,
    options: &PngOptions,
) -> Result<Vec<u8>, ExportError> {
    png::export_with_options(doc, layout, options)
}

/// Export a 2D document to PDF.
#[cfg(feature = "pdf")]
pub fn export_pdf(doc: &Document, layout: &LayoutTree) -> Result<Vec<u8>, ExportError> {
    pdf::export(doc, layout)
}

/// Export a 3D document to STEP.
pub fn export_step(doc: &Document) -> Result<Vec<u8>, ExportError> {
    step::export(doc)
}

/// Export a 3D document to STL (binary).
pub fn export_stl(doc: &Document) -> Result<Vec<u8>, ExportError> {
    stl::export(doc)
}

/// Export a 3D document to STL (ASCII).
pub fn export_stl_ascii(doc: &Document) -> Result<String, ExportError> {
    stl::export_ascii(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::Span;

    fn empty_doc() -> Document {
        Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        }
    }

    #[test]
    #[cfg(feature = "svg")]
    fn test_export_svg() {
        let doc = empty_doc();
        let layout = LayoutTree::new();
        let result = export_svg(&doc, &layout);
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_png() {
        let doc = empty_doc();
        let layout = LayoutTree::new();
        let result = export_png(&doc, &layout);
        assert!(result.is_ok());
    }

    #[test]
    fn test_export_stl_no_geometry() {
        let doc = empty_doc();
        let result = export_stl(&doc);
        assert!(result.is_err());
    }
}
