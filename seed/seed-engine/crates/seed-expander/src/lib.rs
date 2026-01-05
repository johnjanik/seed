//! Component expansion for Seed documents.
//!
//! This crate handles:
//! - Component instantiation
//! - Prop substitution
//! - Slot injection
//! - Circular reference detection

mod registry;
mod expander;

pub use registry::ComponentRegistry;
pub use expander::expand_components;

use seed_core::{Document, ExpandError};

/// Expand all component instances in a document using the provided registry.
pub fn expand(doc: &Document, registry: &ComponentRegistry) -> Result<Document, ExpandError> {
    expand_components(doc, registry)
}
