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

use seed_core::{Document, TokenMap, ResolveError};

/// Resolve all tokens and references in a document.
pub fn resolve(doc: &Document, tokens: &TokenMap) -> Result<Document, ResolveError> {
    let doc = resolve_tokens(doc, tokens)?;
    let doc = resolve_references(&doc)?;
    Ok(doc)
}
