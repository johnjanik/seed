//! seed-io: Universal file I/O for Seed design documents.
//!
//! This crate provides bidirectional conversion between Seed documents
//! and various 3D/CAD file formats.
//!
//! # Supported Formats
//!
//! | Format | Read | Write | Status |
//! |--------|------|-------|--------|
//! | Seed (.seed) | Yes | Yes | Complete |
//! | glTF 2.0 (.gltf, .glb) | Yes | Yes | Stub |
//! | STEP (.step, .stp) | Yes | Yes | Stub |
//! | USD (.usd, .usda, .usdc) | Yes | Yes | Stub |
//!
//! # Quick Start
//!
//! ```ignore
//! use seed_io::{read, write, ReadOptions, WriteOptions};
//!
//! // Read any supported format (auto-detection)
//! let scene = read(&file_bytes, &ReadOptions::default())?;
//!
//! // Write to a specific format
//! let gltf_bytes = write(&scene, "gltf", &WriteOptions::default())?;
//! ```
//!
//! # Architecture
//!
//! All formats convert to/from a common `UnifiedScene` representation:
//!
//! ```text
//! STEP ─┐                  ┌─> STEP
//! glTF ─┼─> UnifiedScene ──┼─> glTF
//! USD  ─┤                  ├─> USD
//! Seed ─┘                  └─> Seed
//! ```
//!
//! This allows any format to be converted to any other format.
//!
//! # Plugin System
//!
//! The `FormatRegistry` allows registering custom format handlers:
//!
//! ```ignore
//! use seed_io::{FormatRegistry, FormatReader};
//!
//! let mut registry = FormatRegistry::new();
//! registry.register_reader(MyCustomReader::new());
//! ```

pub mod error;
pub mod scene;
pub mod registry;
pub mod formats;
pub mod convert;

pub use error::{IoError, Result};
pub use scene::{
    UnifiedScene, SceneNode, Geometry, TriangleMesh, BrepGeometry, PrimitiveGeometry,
    NurbsGeometry, Material, Texture, BoundingBox, SceneMetadata, NodeMetadata,
};
pub use registry::{FormatRegistry, FormatReader, FormatWriter, ReadOptions, WriteOptions};
pub use convert::{tessellate_brep, generate_primitive_mesh};

/// Read data with auto-detection of format.
///
/// Tries each registered reader's `can_read` method to find a compatible format.
///
/// # Example
///
/// ```ignore
/// use seed_io::{read, ReadOptions};
///
/// let scene = read(&file_bytes, &ReadOptions::default())?;
/// println!("Loaded {} nodes", scene.node_count());
/// ```
pub fn read(data: &[u8], options: &ReadOptions) -> Result<UnifiedScene> {
    default_registry().read(data, options)
}

/// Read data with explicit format hint.
///
/// # Example
///
/// ```ignore
/// use seed_io::{read_as, ReadOptions};
///
/// let scene = read_as(&file_bytes, "step", &ReadOptions::default())?;
/// ```
pub fn read_as(data: &[u8], format: &str, options: &ReadOptions) -> Result<UnifiedScene> {
    default_registry().read_as(data, format, options)
}

/// Read data with file extension hint.
///
/// # Example
///
/// ```ignore
/// use seed_io::{read_with_extension, ReadOptions};
///
/// let scene = read_with_extension(&file_bytes, ".gltf", &ReadOptions::default())?;
/// ```
pub fn read_with_extension(data: &[u8], extension: &str, options: &ReadOptions) -> Result<UnifiedScene> {
    default_registry().read_with_extension(data, extension, options)
}

/// Write scene to a format.
///
/// # Example
///
/// ```ignore
/// use seed_io::{write, WriteOptions};
///
/// let gltf_bytes = write(&scene, "gltf", &WriteOptions::default())?;
/// ```
pub fn write(scene: &UnifiedScene, format: &str, options: &WriteOptions) -> Result<Vec<u8>> {
    default_registry().write(scene, format, options)
}

/// Write scene with file extension hint.
///
/// # Example
///
/// ```ignore
/// use seed_io::{write_with_extension, WriteOptions};
///
/// let step_bytes = write_with_extension(&scene, ".step", &WriteOptions::default())?;
/// ```
pub fn write_with_extension(scene: &UnifiedScene, extension: &str, options: &WriteOptions) -> Result<Vec<u8>> {
    default_registry().write_with_extension(scene, extension, options)
}

/// Get the default format registry with all built-in formats.
fn default_registry() -> FormatRegistry {
    let mut registry = FormatRegistry::new();

    // Register Seed format
    #[cfg(feature = "seed")]
    {
        registry.register_reader(formats::seed::SeedReader::new());
        registry.register_writer(formats::seed::SeedWriter::new());
    }

    // Register glTF format
    #[cfg(feature = "gltf")]
    {
        registry.register_reader(formats::gltf::GltfReader::new());
        registry.register_writer(formats::gltf::GltfWriter::new());
    }

    // Register STEP format
    #[cfg(feature = "step")]
    {
        registry.register_reader(formats::step::StepReader::new());
        registry.register_writer(formats::step::StepWriter::new());
    }

    // Register USD format
    #[cfg(feature = "usd")]
    {
        registry.register_reader(formats::usd::UsdReader::new());
        registry.register_writer(formats::usd::UsdWriter::new());
    }

    registry
}

/// Convert a Seed document to a UnifiedScene.
#[cfg(feature = "seed")]
pub fn from_seed_document(doc: &seed_core::ast::Document) -> Result<UnifiedScene> {
    use formats::seed::SeedReader;

    // Re-serialize and parse through the reader
    // TODO: Direct conversion without serialization
    let reader = SeedReader::new();
    let text = format!("{:?}", doc); // Placeholder - need proper serialization
    reader.read(text.as_bytes(), &ReadOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry() {
        let registry = default_registry();

        #[cfg(feature = "seed")]
        assert!(registry.get_reader("seed").is_some());

        #[cfg(feature = "gltf")]
        assert!(registry.get_reader("gltf").is_some());

        #[cfg(feature = "step")]
        assert!(registry.get_reader("step").is_some());

        #[cfg(feature = "usd")]
        assert!(registry.get_reader("usd").is_some());
    }
}
