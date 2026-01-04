//! WebAssembly bindings for the Seed rendering engine.
//!
//! This crate provides a JavaScript/TypeScript API for using the Seed engine
//! in web browsers.
//!
//! ## Example
//!
//! ```js
//! import { SeedEngine } from 'seed-engine';
//!
//! const engine = new SeedEngine();
//!
//! // Load design tokens
//! engine.loadTokens({
//!   colors: { primary: '#0066cc', text: '#333333' },
//!   spacing: { sm: 8, md: 16, lg: 24 }
//! });
//!
//! // Parse and render a document
//! engine.parse(`
//!   Frame {
//!     fill: $colors.primary
//!     width = 200px
//!     height = 100px
//!   }
//! `);
//!
//! // Export to SVG
//! const svg = engine.exportSvg();
//! ```

use wasm_bindgen::prelude::*;
use seed_core::{Document, TokenMap, ResolvedToken};
use seed_core::types::{Color, Length};
use seed_parser::parse_document;
use seed_resolver::{resolve_tokens, resolve_references};
use seed_expander::{ComponentRegistry, expand_components};
use seed_layout::{compute_layout, LayoutTree, LayoutOptions};

mod canvas;
mod types;

pub use canvas::*;
pub use types::*;

/// Initialize panic hook for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();
}

/// The main Seed engine interface for JavaScript.
#[wasm_bindgen]
pub struct SeedEngine {
    tokens: TokenMap,
    components: ComponentRegistry,
    last_document: Option<Document>,
    last_layout: Option<LayoutTree>,
    layout_options: LayoutOptions,
}

#[wasm_bindgen]
impl SeedEngine {
    /// Create a new Seed engine instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            tokens: TokenMap::new(),
            components: ComponentRegistry::new(),
            last_document: None,
            last_layout: None,
            layout_options: LayoutOptions::default(),
        }
    }

    /// Get the version of the engine.
    #[wasm_bindgen(js_name = version)]
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Load design tokens from a JSON object.
    #[wasm_bindgen(js_name = loadTokens)]
    pub fn load_tokens(&mut self, json: JsValue) -> Result<(), JsError> {
        let value: serde_json::Value = serde_wasm_bindgen::from_value(json)
            .map_err(|e| JsError::new(&format!("Invalid tokens: {}", e)))?;

        self.parse_tokens_recursive(&value, "");
        Ok(())
    }

    /// Load design tokens from a JSON string.
    #[wasm_bindgen(js_name = loadTokensFromString)]
    pub fn load_tokens_from_string(&mut self, json: &str) -> Result<(), JsError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| JsError::new(&format!("Invalid JSON: {}", e)))?;

        self.parse_tokens_recursive(&value, "");
        Ok(())
    }

    /// Clear all loaded tokens.
    #[wasm_bindgen(js_name = clearTokens)]
    pub fn clear_tokens(&mut self) {
        self.tokens = TokenMap::new();
    }

    /// Register a component definition.
    #[wasm_bindgen(js_name = registerComponent)]
    pub fn register_component(&mut self, definition: JsValue) -> Result<(), JsError> {
        let def: ComponentDefinitionJs = serde_wasm_bindgen::from_value(definition)
            .map_err(|e| JsError::new(&format!("Invalid component definition: {}", e)))?;

        let component = def.into_core();
        self.components.register(component);
        Ok(())
    }

    /// Check if a component is registered.
    #[wasm_bindgen(js_name = hasComponent)]
    pub fn has_component(&self, name: &str) -> bool {
        self.components.contains(name)
    }

    /// Get list of registered component names.
    #[wasm_bindgen(js_name = getComponentNames)]
    pub fn get_component_names(&self) -> Vec<String> {
        self.components.names().map(|s| s.to_string()).collect()
    }

    /// Parse a Seed document.
    #[wasm_bindgen]
    pub fn parse(&mut self, source: &str) -> Result<JsValue, JsError> {
        // Parse the document
        let doc = parse_document(source)
            .map_err(|e| JsError::new(&format!("Parse error: {}", e)))?;

        // Resolve tokens
        let doc = resolve_tokens(&doc, &self.tokens)
            .map_err(|e| JsError::new(&format!("Token resolution error: {}", e)))?;

        // Resolve element references
        let doc = resolve_references(&doc)
            .map_err(|e| JsError::new(&format!("Reference resolution error: {}", e)))?;

        // Expand components
        let doc = expand_components(&doc, &self.components)
            .map_err(|e| JsError::new(&format!("Component expansion error: {}", e)))?;

        let result = serde_wasm_bindgen::to_value(&doc)
            .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))?;

        self.last_document = Some(doc);
        self.last_layout = None; // Invalidate layout
        Ok(result)
    }

    /// Parse a Seed document without any resolution (raw AST).
    #[wasm_bindgen(js_name = parseRaw)]
    pub fn parse_raw(&self, source: &str) -> Result<JsValue, JsError> {
        let doc = parse_document(source)
            .map_err(|e| JsError::new(&format!("Parse error: {}", e)))?;

        serde_wasm_bindgen::to_value(&doc)
            .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
    }

    /// Set layout options.
    #[wasm_bindgen(js_name = setLayoutOptions)]
    pub fn set_layout_options(&mut self, options: JsValue) -> Result<(), JsError> {
        let opts: LayoutOptionsJs = serde_wasm_bindgen::from_value(options)
            .map_err(|e| JsError::new(&format!("Invalid layout options: {}", e)))?;

        self.layout_options = opts.into_core();
        Ok(())
    }

    /// Compute layout for the last parsed document.
    #[wasm_bindgen]
    pub fn layout(&mut self) -> Result<JsValue, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;

        let layout = compute_layout(doc, &self.layout_options)
            .map_err(|e| JsError::new(&format!("Layout error: {}", e)))?;

        // Convert to JS-friendly format
        let nodes: Vec<LayoutNodeJs> = layout.nodes().map(|node| {
            LayoutNodeJs {
                id: node.id.0,
                name: node.name.clone(),
                x: node.bounds.x,
                y: node.bounds.y,
                width: node.bounds.width,
                height: node.bounds.height,
                absolute_x: node.absolute_bounds.x,
                absolute_y: node.absolute_bounds.y,
                absolute_width: node.absolute_bounds.width,
                absolute_height: node.absolute_bounds.height,
            }
        }).collect();

        self.last_layout = Some(layout);

        serde_wasm_bindgen::to_value(&nodes)
            .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
    }

    /// Get layout bounds for the document.
    #[wasm_bindgen(js_name = getContentBounds)]
    pub fn get_content_bounds(&self) -> Result<JsValue, JsError> {
        let layout = self.last_layout.as_ref()
            .ok_or_else(|| JsError::new("Layout not computed. Call layout() first."))?;

        let bounds = layout.content_bounds();
        let result = BoundsJs {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        };

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
    }

    /// Perform hit testing at a point.
    #[wasm_bindgen(js_name = hitTest)]
    pub fn hit_test(&self, x: f64, y: f64) -> Result<JsValue, JsError> {
        let layout = self.last_layout.as_ref()
            .ok_or_else(|| JsError::new("Layout not computed. Call layout() first."))?;

        if let Some(node_id) = layout.hit_test(x, y) {
            if let Some(node) = layout.get(node_id) {
                let result = HitTestResultJs {
                    id: node.id.0,
                    name: node.name.clone(),
                    x: node.absolute_bounds.x,
                    y: node.absolute_bounds.y,
                    width: node.absolute_bounds.width,
                    height: node.absolute_bounds.height,
                };
                serde_wasm_bindgen::to_value(&Some(result))
                    .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
            } else {
                Ok(JsValue::NULL)
            }
        } else {
            Ok(JsValue::NULL)
        }
    }

    /// Export the document to SVG.
    #[wasm_bindgen(js_name = exportSvg)]
    pub fn export_svg(&self) -> Result<String, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;
        let layout = self.last_layout.as_ref()
            .ok_or_else(|| JsError::new("Layout not computed. Call layout() first."))?;

        seed_export::export_svg(doc, layout)
            .map_err(|e| JsError::new(&format!("SVG export error: {}", e)))
    }

    /// Export the document to PNG as a Uint8Array.
    #[wasm_bindgen(js_name = exportPng)]
    pub fn export_png(&self) -> Result<Vec<u8>, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;
        let layout = self.last_layout.as_ref()
            .ok_or_else(|| JsError::new("Layout not computed. Call layout() first."))?;

        seed_export::export_png(doc, layout)
            .map_err(|e| JsError::new(&format!("PNG export error: {}", e)))
    }

    /// Export the document to PNG with custom options.
    #[wasm_bindgen(js_name = exportPngWithOptions)]
    pub fn export_png_with_options(&self, options: JsValue) -> Result<Vec<u8>, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;
        let layout = self.last_layout.as_ref()
            .ok_or_else(|| JsError::new("Layout not computed. Call layout() first."))?;

        let opts: PngOptionsJs = serde_wasm_bindgen::from_value(options)
            .map_err(|e| JsError::new(&format!("Invalid PNG options: {}", e)))?;

        let core_opts = seed_export::PngOptions {
            scale: opts.scale.unwrap_or(1.0),
            background: opts.background.unwrap_or([255, 255, 255, 255]),
        };

        seed_export::export_png_with_options(doc, layout, &core_opts)
            .map_err(|e| JsError::new(&format!("PNG export error: {}", e)))
    }

    /// Check if the last parsed document is a 3D document.
    #[wasm_bindgen(js_name = is3D)]
    pub fn is_3d(&self) -> bool {
        self.last_document.as_ref()
            .and_then(|doc| doc.meta.as_ref())
            .map(|meta| meta.profile == seed_core::ast::Profile::Seed3D)
            .unwrap_or(false)
    }

    /// Export a 3D document to STL (binary).
    #[wasm_bindgen(js_name = exportStl)]
    pub fn export_stl(&self) -> Result<Vec<u8>, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;

        seed_export::export_stl(doc)
            .map_err(|e| JsError::new(&format!("STL export error: {}", e)))
    }

    /// Export a 3D document to STL (ASCII).
    #[wasm_bindgen(js_name = exportStlAscii)]
    pub fn export_stl_ascii(&self) -> Result<String, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;

        seed_export::export_stl_ascii(doc)
            .map_err(|e| JsError::new(&format!("STL export error: {}", e)))
    }
}

impl SeedEngine {
    fn parse_tokens_recursive(&mut self, value: &serde_json::Value, prefix: &str) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    self.parse_tokens_recursive(val, &path);
                }
            }
            serde_json::Value::String(s) => {
                // Try to parse as color first
                if let Some(color) = Color::from_hex(s) {
                    self.tokens.insert(prefix, ResolvedToken::Color(color));
                } else if let Some(length) = parse_length_string(s) {
                    self.tokens.insert(prefix, ResolvedToken::Length(length));
                } else {
                    self.tokens.insert(prefix, ResolvedToken::String(s.clone()));
                }
            }
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    self.tokens.insert(prefix, ResolvedToken::Number(f));
                }
            }
            _ => {}
        }
    }
}

impl Default for SeedEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a length string like "16px", "10mm", "50%"
fn parse_length_string(s: &str) -> Option<Length> {
    let s = s.trim();
    if s.ends_with("px") {
        s[..s.len()-2].parse::<f64>().ok().map(Length::px)
    } else if s.ends_with("mm") {
        s[..s.len()-2].parse::<f64>().ok().map(Length::mm)
    } else if s.ends_with("cm") {
        s[..s.len()-2].parse::<f64>().ok().map(|v| Length::mm(v * 10.0))
    } else if s.ends_with("in") {
        s[..s.len()-2].parse::<f64>().ok().map(|v| Length::mm(v * 25.4))
    } else if s.ends_with("%") {
        s[..s.len()-1].parse::<f64>().ok().map(Length::percent)
    } else {
        None
    }
}

/// Standalone function to parse a document (for simpler usage).
#[wasm_bindgen(js_name = parseDocument)]
pub fn parse_document_standalone(source: &str) -> Result<JsValue, JsError> {
    let doc = parse_document(source)
        .map_err(|e| JsError::new(&format!("Parse error: {}", e)))?;

    serde_wasm_bindgen::to_value(&doc)
        .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
}

/// Get the engine version.
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_new() {
        let engine = SeedEngine::new();
        assert!(engine.last_document.is_none());
    }

    #[test]
    fn test_parse_length_string() {
        assert!(parse_length_string("16px").is_some());
        assert!(parse_length_string("10mm").is_some());
        assert!(parse_length_string("2.5cm").is_some());
        assert!(parse_length_string("50%").is_some());
        assert!(parse_length_string("invalid").is_none());
    }

    #[test]
    fn test_version() {
        let version = SeedEngine::version();
        assert!(!version.is_empty());
    }
}
