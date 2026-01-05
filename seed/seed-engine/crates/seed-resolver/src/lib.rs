//! Token and reference resolution for Seed documents.
//!
//! This crate resolves:
//! - Token references ($color.primary -> actual color value)
//! - Element references (Parent, named elements)
//! - Import handling

mod tokens;
mod references;

pub use tokens::resolve_tokens;
pub use references::resolve_references;

use seed_core::{ast::ImageSource, Document, TokenMap, ResolveError};

/// Resolve all tokens and references in a document.
pub fn resolve(doc: &Document, tokens: &TokenMap) -> Result<Document, ResolveError> {
    let doc = resolve_tokens(doc, tokens)?;
    let doc = resolve_references(&doc)?;
    Ok(doc)
}

/// Helper to parse image source from a string value.
#[allow(dead_code)]
pub(crate) fn parse_image_source_value(s: &str) -> ImageSource {
    let s = s.trim();

    // Check for data URL
    if s.starts_with("data:") {
        if let Some(comma_pos) = s.find(',') {
            let header = &s[5..comma_pos];
            let data = &s[comma_pos + 1..];
            let mime_type = header.split(';').next().unwrap_or("application/octet-stream");
            return ImageSource::Data {
                mime_type: mime_type.to_string(),
                data: data.to_string(),
            };
        }
    }

    // Check for URL
    if s.starts_with("http://") || s.starts_with("https://") {
        return ImageSource::Url(s.to_string());
    }

    // Otherwise treat as file path
    ImageSource::File(s.to_string())
}
