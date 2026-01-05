//! TypeScript-friendly type definitions for WASM bindings.

use serde::{Serialize, Deserialize};
use seed_core::ast::{
    ComponentDefinition, PropDefinition, PropType, SlotDefinition,
    Span, PropertyValue,
};
use seed_core::types::Identifier;
use seed_layout::LayoutOptions;

/// Layout node representation for JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutNodeJs {
    pub id: u64,
    pub name: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub absolute_x: f64,
    pub absolute_y: f64,
    pub absolute_width: f64,
    pub absolute_height: f64,
}

/// Bounds representation for JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundsJs {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Hit test result for JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HitTestResultJs {
    pub id: u64,
    pub name: Option<String>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Layout options from JavaScript.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutOptionsJs {
    /// Default width for the viewport.
    #[serde(default)]
    pub viewport_width: Option<f64>,
    /// Default height for the viewport.
    #[serde(default)]
    pub viewport_height: Option<f64>,
    /// Default font size.
    #[serde(default)]
    pub default_font_size: Option<f64>,
    /// Default line height.
    #[serde(default)]
    pub default_line_height: Option<f64>,
}

impl LayoutOptionsJs {
    pub fn into_core(self) -> LayoutOptions {
        let mut opts = LayoutOptions::default();
        if let Some(w) = self.viewport_width {
            opts.viewport_width = w;
        }
        if let Some(h) = self.viewport_height {
            opts.viewport_height = h;
        }
        if let Some(fs) = self.default_font_size {
            opts.default_font_size = fs;
        }
        if let Some(lh) = self.default_line_height {
            opts.default_line_height = lh;
        }
        opts
    }
}

/// PNG export options from JavaScript.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PngOptionsJs {
    /// Scale factor (1.0 = 1:1, 2.0 = 2x resolution).
    #[serde(default)]
    pub scale: Option<f32>,
    /// Background color as RGBA array [r, g, b, a].
    #[serde(default)]
    pub background: Option<[u8; 4]>,
}

/// Component definition from JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDefinitionJs {
    /// Component name.
    pub name: String,
    /// Props definitions.
    #[serde(default)]
    pub props: Vec<PropDefinitionJs>,
    /// Slot definitions.
    #[serde(default)]
    pub slots: Vec<SlotDefinitionJs>,
    /// Template as Seed source code.
    #[serde(default)]
    pub template: Option<String>,
}

impl ComponentDefinitionJs {
    pub fn into_core(self) -> ComponentDefinition {
        // Parse template if provided
        let template = if let Some(src) = &self.template {
            seed_parser::parse_document(src)
                .map(|doc| doc.elements)
                .unwrap_or_default()
        } else {
            vec![]
        };

        ComponentDefinition {
            name: Identifier(self.name),
            props: self.props.into_iter().map(|p| p.into_core()).collect(),
            slots: self.slots.into_iter().map(|s| s.into_core()).collect(),
            template,
            span: Span::default(),
        }
    }
}

/// Prop definition from JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropDefinitionJs {
    /// Prop name.
    pub name: String,
    /// Prop type: "color", "length", "number", "string", "boolean", "any".
    #[serde(rename = "type")]
    pub prop_type: String,
    /// Whether this prop is required.
    #[serde(default)]
    pub required: bool,
    /// Default value (as a string or number).
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

impl PropDefinitionJs {
    pub fn into_core(self) -> PropDefinition {
        let prop_type = match self.prop_type.as_str() {
            "color" => PropType::Color,
            "length" => PropType::Length,
            "number" => PropType::Number,
            "string" => PropType::String,
            "boolean" => PropType::Boolean,
            _ => PropType::Any,
        };

        let default = self.default.and_then(|v| json_to_property_value(&v, prop_type));

        PropDefinition {
            name: self.name,
            prop_type,
            default,
            required: self.required,
            span: Span::default(),
        }
    }
}

/// Slot definition from JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotDefinitionJs {
    /// Slot name (null for default slot).
    pub name: Option<String>,
}

impl SlotDefinitionJs {
    pub fn into_core(self) -> SlotDefinition {
        SlotDefinition {
            name: self.name,
            span: Span::default(),
        }
    }
}

/// Convert a JSON value to a PropertyValue based on the expected type.
fn json_to_property_value(value: &serde_json::Value, prop_type: PropType) -> Option<PropertyValue> {
    match prop_type {
        PropType::Color => {
            if let serde_json::Value::String(s) = value {
                seed_core::types::Color::from_hex(s)
                    .map(PropertyValue::Color)
            } else {
                None
            }
        }
        PropType::Length => {
            match value {
                serde_json::Value::Number(n) => {
                    n.as_f64().map(|v| PropertyValue::Length(seed_core::types::Length::px(v)))
                }
                serde_json::Value::String(s) => {
                    parse_length_js(s).map(PropertyValue::Length)
                }
                _ => None,
            }
        }
        PropType::Number => {
            if let serde_json::Value::Number(n) = value {
                n.as_f64().map(PropertyValue::Number)
            } else {
                None
            }
        }
        PropType::String => {
            if let serde_json::Value::String(s) = value {
                Some(PropertyValue::String(s.clone()))
            } else {
                None
            }
        }
        PropType::Boolean => {
            if let serde_json::Value::Bool(b) = value {
                Some(PropertyValue::Boolean(*b))
            } else {
                None
            }
        }
        PropType::Any => {
            // Try to infer the type
            match value {
                serde_json::Value::Bool(b) => Some(PropertyValue::Boolean(*b)),
                serde_json::Value::Number(n) => n.as_f64().map(PropertyValue::Number),
                serde_json::Value::String(s) => {
                    if let Some(color) = seed_core::types::Color::from_hex(s) {
                        Some(PropertyValue::Color(color))
                    } else if let Some(length) = parse_length_js(s) {
                        Some(PropertyValue::Length(length))
                    } else {
                        Some(PropertyValue::String(s.clone()))
                    }
                }
                _ => None,
            }
        }
    }
}

/// Parse a length string from JavaScript.
fn parse_length_js(s: &str) -> Option<seed_core::types::Length> {
    let s = s.trim();
    if s.ends_with("px") {
        s[..s.len()-2].parse::<f64>().ok().map(seed_core::types::Length::px)
    } else if s.ends_with("mm") {
        s[..s.len()-2].parse::<f64>().ok().map(seed_core::types::Length::mm)
    } else if s.ends_with("cm") {
        s[..s.len()-2].parse::<f64>().ok().map(|v| seed_core::types::Length::mm(v * 10.0))
    } else if s.ends_with("in") {
        s[..s.len()-2].parse::<f64>().ok().map(|v| seed_core::types::Length::mm(v * 25.4))
    } else if s.ends_with("%") {
        s[..s.len()-1].parse::<f64>().ok().map(seed_core::types::Length::percent)
    } else {
        None
    }
}

/// Render options for canvas rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptionsJs {
    /// Scale factor for high-DPI displays.
    #[serde(default)]
    pub device_pixel_ratio: Option<f64>,
    /// Offset X for panning.
    #[serde(default)]
    pub offset_x: Option<f64>,
    /// Offset Y for panning.
    #[serde(default)]
    pub offset_y: Option<f64>,
    /// Zoom level.
    #[serde(default)]
    pub zoom: Option<f64>,
    /// Whether to show debug outlines.
    #[serde(default)]
    pub debug: Option<bool>,
}

/// Configuration for image analysis from JavaScript.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeConfigJs {
    /// Maximum dimension for processing (larger images are scaled down).
    #[serde(default)]
    pub max_dimension: Option<u32>,
    /// Color distance threshold for flood fill.
    #[serde(default)]
    pub color_threshold: Option<f32>,
    /// Minimum region area to consider.
    #[serde(default)]
    pub min_region_area: Option<u64>,
    /// Number of colors to extract for palette.
    #[serde(default)]
    pub palette_size: Option<usize>,
    /// Canny edge detection: low threshold for hysteresis.
    #[serde(default)]
    pub canny_low_threshold: Option<f32>,
    /// Canny edge detection: high threshold for hysteresis.
    #[serde(default)]
    pub canny_high_threshold: Option<f32>,
    /// Morphological kernel size for edge cleanup.
    #[serde(default)]
    pub morph_kernel_size: Option<u32>,
    /// Use enhanced edge-based detection pipeline.
    #[serde(default)]
    pub use_edge_detection: Option<bool>,
    /// Enable adaptive preprocessing for dark themes.
    #[serde(default)]
    pub adaptive_dark_theme: Option<bool>,
    /// Use CLAHE contrast enhancement for dark themes.
    #[serde(default)]
    pub use_clahe: Option<bool>,
    /// Use edge-constrained flood fill.
    #[serde(default)]
    pub use_edge_constrained_fill: Option<bool>,
    /// Multiplier for color threshold on dark themes.
    #[serde(default)]
    pub dark_color_threshold_mult: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_options_default() {
        let opts = LayoutOptionsJs::default();
        let core = opts.into_core();
        assert!(core.viewport_width > 0.0);
    }

    #[test]
    fn test_parse_length_js() {
        assert!(parse_length_js("16px").is_some());
        assert!(parse_length_js("10mm").is_some());
        assert!(parse_length_js("50%").is_some());
        assert!(parse_length_js("invalid").is_none());
    }

    #[test]
    fn test_json_to_property_value() {
        let num = serde_json::json!(42.0);
        let result = json_to_property_value(&num, PropType::Number);
        assert!(matches!(result, Some(PropertyValue::Number(_))));

        let color = serde_json::json!("#ff0000");
        let result = json_to_property_value(&color, PropType::Color);
        assert!(matches!(result, Some(PropertyValue::Color(_))));
    }
}
