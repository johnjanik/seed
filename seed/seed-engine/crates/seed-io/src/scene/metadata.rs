//! Metadata types for UnifiedScene.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Scene-level metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneMetadata {
    /// Scene name/title.
    pub name: Option<String>,
    /// Scene description.
    pub description: Option<String>,
    /// Author information.
    pub author: Option<String>,
    /// Copyright notice.
    pub copyright: Option<String>,
    /// Creation date (ISO 8601).
    pub created: Option<String>,
    /// Last modified date (ISO 8601).
    pub modified: Option<String>,
    /// Software that created the file.
    pub generator: Option<String>,
    /// Original file format.
    pub source_format: Option<String>,
    /// Original file path.
    pub source_path: Option<String>,
    /// Unit system.
    pub units: Units,
    /// Up axis.
    pub up_axis: Axis,
    /// Custom properties.
    pub custom: IndexMap<String, MetadataValue>,
}

/// Node-level metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Node name.
    pub name: Option<String>,
    /// Node description.
    pub description: Option<String>,
    /// Tags/labels.
    pub tags: Vec<String>,
    /// Custom properties.
    pub custom: IndexMap<String, MetadataValue>,
}

/// Unit system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Units {
    /// Millimeters (common in CAD).
    Millimeters,
    /// Centimeters.
    Centimeters,
    /// Meters (glTF default).
    #[default]
    Meters,
    /// Inches.
    Inches,
    /// Feet.
    Feet,
}

impl Units {
    /// Get the scale factor to convert to meters.
    pub fn to_meters_scale(&self) -> f32 {
        match self {
            Units::Millimeters => 0.001,
            Units::Centimeters => 0.01,
            Units::Meters => 1.0,
            Units::Inches => 0.0254,
            Units::Feet => 0.3048,
        }
    }

    /// Get the scale factor to convert from meters.
    pub fn from_meters_scale(&self) -> f32 {
        1.0 / self.to_meters_scale()
    }
}

/// Coordinate axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Axis {
    /// X axis.
    X,
    /// Y axis (glTF, most game engines).
    #[default]
    Y,
    /// Z axis (CAD, Blender).
    Z,
}

/// A metadata value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetadataValue {
    /// Boolean value.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Floating-point value.
    Float(f64),
    /// String value.
    String(String),
    /// Array of values.
    Array(Vec<MetadataValue>),
    /// Nested object.
    Object(IndexMap<String, MetadataValue>),
}

impl From<bool> for MetadataValue {
    fn from(v: bool) -> Self {
        MetadataValue::Bool(v)
    }
}

impl From<i64> for MetadataValue {
    fn from(v: i64) -> Self {
        MetadataValue::Int(v)
    }
}

impl From<f64> for MetadataValue {
    fn from(v: f64) -> Self {
        MetadataValue::Float(v)
    }
}

impl From<String> for MetadataValue {
    fn from(v: String) -> Self {
        MetadataValue::String(v)
    }
}

impl From<&str> for MetadataValue {
    fn from(v: &str) -> Self {
        MetadataValue::String(v.to_string())
    }
}
