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
mod memory;
mod streaming;
mod events;
mod io;

#[cfg(feature = "webgpu")]
mod webgpu;

pub use canvas::*;
pub use types::*;
pub use memory::*;
pub use streaming::*;
pub use events::*;
pub use io::*;

#[cfg(feature = "webgpu")]
pub use webgpu::*;

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

    /// Export a 2D document to PDF.
    #[wasm_bindgen(js_name = exportPdf)]
    pub fn export_pdf(&self) -> Result<Vec<u8>, JsError> {
        let doc = self.last_document.as_ref()
            .ok_or_else(|| JsError::new("No document parsed. Call parse() first."))?;
        let layout = self.last_layout.as_ref()
            .ok_or_else(|| JsError::new("Layout not computed. Call layout() first."))?;

        seed_export::export_pdf(doc, layout)
            .map_err(|e| JsError::new(&format!("PDF export error: {}", e)))
    }

    /// Analyze a PNG image and generate Seed code.
    ///
    /// Takes PNG bytes as input and returns Seed markup as a string.
    /// The analyzer uses computer vision to detect UI elements including:
    /// - Frames/containers with colors and gradients
    /// - Text regions (with placeholder content)
    /// - Icons and images
    /// - Layout patterns (row, column, grid)
    /// - Corner radii, strokes, and shadows
    ///
    /// Works with both light and dark themed UIs.
    #[wasm_bindgen(js_name = analyzeImage)]
    pub fn analyze_image(&mut self, png_bytes: &[u8]) -> Result<String, JsError> {
        let seed_code = seed_analyze::analyze_image(png_bytes)
            .map_err(|e| JsError::new(&format!("Analysis error: {}", e)))?;

        // Optionally parse the result so it can be immediately rendered
        if let Ok(_) = self.parse(&seed_code) {
            // Parse succeeded, layout will be available
        }

        Ok(seed_code)
    }

    /// Analyze a PNG image with custom configuration.
    #[wasm_bindgen(js_name = analyzeImageWithConfig)]
    pub fn analyze_image_with_config(&mut self, png_bytes: &[u8], config: JsValue) -> Result<String, JsError> {
        let config_js: AnalyzeConfigJs = serde_wasm_bindgen::from_value(config)
            .map_err(|e| JsError::new(&format!("Invalid config: {}", e)))?;

        let config = seed_analyze::AnalyzeConfig {
            max_dimension: config_js.max_dimension.unwrap_or(800),
            color_threshold: config_js.color_threshold.unwrap_or(15.0),
            min_region_area: config_js.min_region_area.unwrap_or(100),
            palette_size: config_js.palette_size.unwrap_or(8),
            canny_low_threshold: config_js.canny_low_threshold.unwrap_or(30.0),
            canny_high_threshold: config_js.canny_high_threshold.unwrap_or(100.0),
            morph_kernel_size: config_js.morph_kernel_size.unwrap_or(3),
            use_edge_detection: config_js.use_edge_detection.unwrap_or(true),
            adaptive_dark_theme: config_js.adaptive_dark_theme.unwrap_or(true),
            use_clahe: config_js.use_clahe.unwrap_or(true),
            use_edge_constrained_fill: config_js.use_edge_constrained_fill.unwrap_or(true),
            dark_color_threshold_mult: config_js.dark_color_threshold_mult.unwrap_or(0.5),
        };

        let seed_code = seed_analyze::analyze_image_with_config(png_bytes, &config)
            .map_err(|e| JsError::new(&format!("Analysis error: {}", e)))?;

        Ok(seed_code)
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

/// Analyze a PNG image and generate Seed source code.
///
/// Takes raw PNG bytes and returns Seed markup that reproduces the image.
/// This is the reverse of rendering: image -> code.
#[wasm_bindgen(js_name = analyzePng)]
pub fn analyze_png(png_bytes: &[u8]) -> Result<String, JsError> {
    seed_analyze::analyze_image(png_bytes)
        .map_err(|e| JsError::new(&format!("Image analysis error: {}", e)))
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

    #[test]
    fn test_parse_only() {
        let source = "Frame:\n  width: 100px\n  height: 100px\n  fill: #ff0000";
        let result = parse_document(source);
        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        println!("Parse OK");
    }

    #[test]
    fn test_resolve_tokens_step() {
        use seed_core::TokenMap;
        use seed_resolver::resolve_tokens;

        let source = "Frame:\n  width: 100px\n  height: 100px\n  fill: #ff0000";
        let doc = parse_document(source).unwrap();
        println!("Parsed, resolving tokens...");

        let tokens = TokenMap::new();
        let doc = resolve_tokens(&doc, &tokens).unwrap();
        println!("Tokens resolved, {} elements", doc.elements.len());
    }

    #[test]
    fn test_resolve_refs_step() {
        use seed_core::TokenMap;
        use seed_resolver::{resolve_tokens, resolve_references};

        let source = "Frame:\n  width: 100px\n  height: 100px\n  fill: #ff0000";
        let doc = parse_document(source).unwrap();
        let tokens = TokenMap::new();
        let doc = resolve_tokens(&doc, &tokens).unwrap();
        println!("Resolving references...");

        let doc = resolve_references(&doc).unwrap();
        println!("References resolved");
    }

    #[test]
    fn test_expand_step() {
        use seed_core::TokenMap;
        use seed_resolver::{resolve_tokens, resolve_references};
        use seed_expander::{expand_components, ComponentRegistry};

        let source = "Frame:\n  width: 100px\n  height: 100px\n  fill: #ff0000";
        let doc = parse_document(source).unwrap();
        let tokens = TokenMap::new();
        let doc = resolve_tokens(&doc, &tokens).unwrap();
        let doc = resolve_references(&doc).unwrap();
        println!("Expanding components...");

        let components = ComponentRegistry::new();
        let doc = expand_components(&doc, &components).unwrap();
        println!("Expanded, {} elements", doc.elements.len());
    }

    #[test]
    fn test_full_render_pipeline() {
        use seed_core::TokenMap;
        use seed_resolver::{resolve_tokens, resolve_references};
        use seed_expander::{expand_components, ComponentRegistry};
        use seed_layout::{compute_layout, LayoutOptions};
        use seed_export::png;

        let source = "Frame:\n  width: 200px\n  height: 100px\n  fill: #4a90d9";
        let doc = parse_document(source).unwrap();
        let tokens = TokenMap::new();
        let doc = resolve_tokens(&doc, &tokens).unwrap();
        let doc = resolve_references(&doc).unwrap();
        let components = ComponentRegistry::new();
        let doc = expand_components(&doc, &components).unwrap();

        let layout = compute_layout(&doc, &LayoutOptions::default()).unwrap();
        let bounds = layout.content_bounds();

        println!("Layout bounds: {}x{} at ({}, {})", bounds.width, bounds.height, bounds.x, bounds.y);

        // Verify the layout computed the correct size from properties
        assert!((bounds.width - 200.0).abs() < 0.001, "Expected width 200, got {}", bounds.width);
        assert!((bounds.height - 100.0).abs() < 0.001, "Expected height 100, got {}", bounds.height);

        // Export to PNG
        let png_data = png::export(&doc, &layout).unwrap();

        // Verify PNG is reasonable size (200x100 = 80KB raw + overhead, should be < 150KB)
        println!("PNG size: {} bytes", png_data.len());
        assert!(png_data.len() < 150_000, "PNG too large: {} bytes", png_data.len());

        // Verify PNG signature
        assert_eq!(&png_data[0..8], &[137, 80, 78, 71, 13, 10, 26, 10], "Invalid PNG signature");

        // Parse and verify IHDR chunk
        let ihdr_len = u32::from_be_bytes([png_data[8], png_data[9], png_data[10], png_data[11]]);
        assert_eq!(ihdr_len, 13, "IHDR length should be 13");
        assert_eq!(&png_data[12..16], b"IHDR", "Missing IHDR chunk");

        let width = u32::from_be_bytes([png_data[16], png_data[17], png_data[18], png_data[19]]);
        let height = u32::from_be_bytes([png_data[20], png_data[21], png_data[22], png_data[23]]);
        println!("PNG dimensions in header: {}x{}", width, height);
        assert_eq!(width, 200, "PNG width mismatch");
        assert_eq!(height, 100, "PNG height mismatch");

        // Write to temp file for manual inspection
        std::fs::write("/tmp/test_output.png", &png_data).unwrap();
        println!("Wrote PNG to /tmp/test_output.png");
    }
}
