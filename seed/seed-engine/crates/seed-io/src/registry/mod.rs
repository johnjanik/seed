//! Format registry and traits.

mod registry;
mod traits;

pub use registry::FormatRegistry;
pub use traits::{FormatReader, FormatWriter, ReadOptions, WriteOptions};
