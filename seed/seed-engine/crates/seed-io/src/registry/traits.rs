//! Format reader and writer traits.

use crate::error::Result;
use crate::scene::UnifiedScene;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Trait for format readers.
///
/// Implement this trait to add support for reading a new file format.
pub trait FormatReader: Send + Sync {
    /// Get the format name (e.g., "glTF", "STEP", "USD").
    fn name(&self) -> &'static str;

    /// Get supported file extensions (e.g., ["gltf", "glb"]).
    fn extensions(&self) -> &[&'static str];

    /// Check if this reader can handle the given data.
    ///
    /// This should be a fast check (e.g., magic bytes) without parsing the whole file.
    fn can_read(&self, data: &[u8]) -> bool;

    /// Read the data and convert to UnifiedScene.
    fn read(&self, data: &[u8], options: &ReadOptions) -> Result<UnifiedScene>;
}

/// Trait for format writers.
///
/// Implement this trait to add support for writing a new file format.
pub trait FormatWriter: Send + Sync {
    /// Get the format name (e.g., "glTF", "STEP", "USD").
    fn name(&self) -> &'static str;

    /// Get the primary file extension (e.g., "glb").
    fn extension(&self) -> &'static str;

    /// Write a UnifiedScene to the format.
    fn write(&self, scene: &UnifiedScene, options: &WriteOptions) -> Result<Vec<u8>>;
}

/// Options for reading files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadOptions {
    /// Convert units to target system.
    pub target_units: Option<crate::scene::Units>,
    /// Convert up axis to target.
    pub target_up_axis: Option<crate::scene::Axis>,
    /// Compute normals if missing.
    pub compute_normals: bool,
    /// Tessellate B-rep geometry to meshes.
    pub tessellate_brep: bool,
    /// Tessellation tolerance for B-rep (in model units).
    pub tessellation_tolerance: f32,
    /// Maximum recursion depth for USD composition.
    pub max_composition_depth: u32,
    /// Format-specific options.
    pub format_options: IndexMap<String, String>,
}

impl ReadOptions {
    /// Create default read options.
    pub fn new() -> Self {
        Self {
            tessellation_tolerance: 0.01,
            max_composition_depth: 32,
            ..Default::default()
        }
    }

    /// Enable B-rep tessellation.
    pub fn with_tessellation(mut self, tolerance: f32) -> Self {
        self.tessellate_brep = true;
        self.tessellation_tolerance = tolerance;
        self
    }

    /// Set target units.
    pub fn with_units(mut self, units: crate::scene::Units) -> Self {
        self.target_units = Some(units);
        self
    }

    /// Set target up axis.
    pub fn with_up_axis(mut self, axis: crate::scene::Axis) -> Self {
        self.target_up_axis = Some(axis);
        self
    }
}

/// Options for writing files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WriteOptions {
    /// Use binary format when available (e.g., GLB instead of glTF).
    pub binary: bool,
    /// Embed all resources (textures, buffers) in the output.
    pub embed_resources: bool,
    /// Pretty-print text formats.
    pub pretty: bool,
    /// Compression level (0-9, format-dependent).
    pub compression: u8,
    /// Target units for output.
    pub target_units: Option<crate::scene::Units>,
    /// Target up axis for output.
    pub target_up_axis: Option<crate::scene::Axis>,
    /// Format-specific options.
    pub format_options: IndexMap<String, String>,
}

impl WriteOptions {
    /// Create default write options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Use binary format.
    pub fn binary(mut self) -> Self {
        self.binary = true;
        self
    }

    /// Embed all resources.
    pub fn embedded(mut self) -> Self {
        self.embed_resources = true;
        self
    }

    /// Pretty-print output.
    pub fn pretty(mut self) -> Self {
        self.pretty = true;
        self
    }

    /// Set compression level.
    pub fn compressed(mut self, level: u8) -> Self {
        self.compression = level.min(9);
        self
    }
}
