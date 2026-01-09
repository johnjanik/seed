//! USD (Universal Scene Description) format reader and writer.
//!
//! Supports both USDA (ASCII) and USDC (binary Crate) formats.

mod usda;
mod usdc;
mod reader;
mod writer;

pub use reader::UsdReader;
pub use writer::UsdWriter;
