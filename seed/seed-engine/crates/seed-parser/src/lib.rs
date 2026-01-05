//! Parser for Seed documents.
//!
//! This crate provides parser combinators for parsing Seed markup into an AST.
//! Built on `nom` for composable, zero-copy parsing where possible.

mod lexer;
mod grammar;

pub use grammar::parse;

use seed_core::{Document, ParseError};

/// Parse a Seed document from source text.
///
/// # Example
///
/// ```ignore
/// use seed_parser::parse_document;
///
/// let source = r#"
/// Frame Button:
///   fill: #3B82F6
///   constraints:
///     - width = 120px
///     - height = 40px
/// "#;
///
/// let doc = parse_document(source)?;
/// ```
pub fn parse_document(source: &str) -> Result<Document, ParseError> {
    parse(source)
}
