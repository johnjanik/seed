//! glTF 2.0 format reader and writer.
//!
//! Supports both JSON (.gltf) and binary (.glb) variants.

mod schema;
mod reader;
mod writer;

pub use reader::GltfReader;
pub use writer::GltfWriter;
