//! STEP (ISO 10303) export for 3D models.

use seed_core::{Document, ExportError};

/// Export a document to STEP format.
pub fn export(_doc: &Document) -> Result<Vec<u8>, ExportError> {
    // TODO: Implement STEP export via OpenCASCADE
    Err(ExportError::UnsupportedFormat {
        format: "STEP".to_string(),
    })
}
