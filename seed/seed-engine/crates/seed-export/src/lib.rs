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

    #[test]
    fn test_complex_nested_ui_export() {
        use seed_parser::parse_document;
        use seed_layout::{compute_layout, LayoutOptions};

        let source = r#"Frame:
  width: 375px
  height: 400px
  fill: #0d0d1a

  Frame:
    x: 0px
    y: 44px
    width: 375px
    height: 80px
    fill: #0d0d1a

    Text:
      content: "Presets"
      x: 20px
      y: 8px
      color: #ffffff
      font-size: 28px

  Frame:
    x: 20px
    y: 134px
    width: 335px
    height: 44px
    fill: #1a1a2e
    corner-radius: 10px

  Frame:
    x: 20px
    y: 200px
    width: 335px
    height: 180px
    fill: linear-gradient(135deg, #1e3a5f 0%, #0d1f33 100%)
    corner-radius: 20px

    Frame:
      x: 24px
      y: 24px
      width: 44px
      height: 44px
      fill: #2a5080
      corner-radius: 22px

    Text:
      content: "Deep Focus"
      x: 24px
      y: 88px
      color: #ffffff
      font-size: 20px
"#;

        // Parse
        let doc = parse_document(source).expect("Should parse complex nested UI");
        assert_eq!(doc.elements.len(), 1, "Should have 1 root frame");

        // Layout
        let options = LayoutOptions::default();
        let layout = compute_layout(&doc, &options).expect("Should compute layout");
        let bounds = layout.content_bounds();
        assert!(bounds.width > 0.0, "Should have positive width");
        assert!(bounds.height > 0.0, "Should have positive height");

        // Export PNG
        let png = export_png(&doc, &layout).expect("Should export to PNG");

        // Verify PNG header
        assert!(png.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]), "Invalid PNG header");

        // Verify reasonable size
        assert!(png.len() > 1000, "PNG should be > 1KB for this content");
    }
}
