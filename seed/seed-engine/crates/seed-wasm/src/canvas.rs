//! Canvas rendering for the Seed engine.
//!
//! Provides functionality to render Seed documents directly to an HTML canvas.

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use seed_core::{Document, ast::Element};
use seed_layout::{LayoutTree, LayoutNode, LayoutNodeId};
use crate::types::RenderOptionsJs;

/// Canvas renderer for 2D Seed documents.
#[wasm_bindgen]
pub struct CanvasRenderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    device_pixel_ratio: f64,
}

#[wasm_bindgen]
impl CanvasRenderer {
    /// Create a new canvas renderer from a canvas element.
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<CanvasRenderer, JsError> {
        let ctx = canvas
            .get_context("2d")
            .map_err(|_| JsError::new("Failed to get 2d context"))?
            .ok_or_else(|| JsError::new("Canvas 2d context not available"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| JsError::new("Failed to cast to CanvasRenderingContext2d"))?;

        Ok(CanvasRenderer {
            canvas,
            ctx,
            device_pixel_ratio: 1.0,
        })
    }

    /// Set the device pixel ratio for high-DPI displays.
    #[wasm_bindgen(js_name = setDevicePixelRatio)]
    pub fn set_device_pixel_ratio(&mut self, ratio: f64) {
        self.device_pixel_ratio = ratio;
    }

    /// Clear the canvas with a background color.
    pub fn clear(&self, color: &str) {
        let width = self.canvas.width() as f64;
        let height = self.canvas.height() as f64;

        self.ctx.set_fill_style_str(color);
        self.ctx.fill_rect(0.0, 0.0, width, height);
    }

    /// Resize the canvas to match its display size with DPI scaling.
    #[wasm_bindgen(js_name = resizeToDisplay)]
    pub fn resize_to_display(&self) -> bool {
        let display_width = self.canvas.client_width() as u32;
        let display_height = self.canvas.client_height() as u32;

        let scaled_width = (display_width as f64 * self.device_pixel_ratio) as u32;
        let scaled_height = (display_height as f64 * self.device_pixel_ratio) as u32;

        let needs_resize = self.canvas.width() != scaled_width
            || self.canvas.height() != scaled_height;

        if needs_resize {
            self.canvas.set_width(scaled_width);
            self.canvas.set_height(scaled_height);

            // Scale context for high-DPI
            let _ = self.ctx.scale(self.device_pixel_ratio, self.device_pixel_ratio);
        }

        needs_resize
    }

    /// Get canvas width in CSS pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> f64 {
        self.canvas.client_width() as f64
    }

    /// Get canvas height in CSS pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> f64 {
        self.canvas.client_height() as f64
    }
}

impl CanvasRenderer {
    /// Render a document with its layout to the canvas.
    pub fn render_document(
        &self,
        doc: &Document,
        layout: &LayoutTree,
        offset_x: f64,
        offset_y: f64,
        zoom: f64,
    ) {
        // Clear canvas
        self.clear("#ffffff");

        // Save context state
        self.ctx.save();

        // Apply transform
        let _ = self.ctx.translate(offset_x, offset_y);
        let _ = self.ctx.scale(zoom, zoom);

        // Render elements
        for (i, element) in doc.elements.iter().enumerate() {
            let node_id = LayoutNodeId(i as u64);
            self.render_element(element, layout, node_id);
        }

        // Restore context state
        self.ctx.restore();
    }

    fn render_element(&self, element: &Element, layout: &LayoutTree, node_id: LayoutNodeId) {
        let Some(node) = layout.get(node_id) else {
            return;
        };

        match element {
            Element::Frame(frame) => {
                self.render_frame_node(node, &frame.properties);

                // Render children - zip AST children with layout children
                for (child, &child_id) in frame.children.iter().zip(node.children.iter()) {
                    self.render_element(child, layout, child_id);
                }
            }
            Element::Text(text) => {
                self.render_text_node(node, &text.content, &text.properties);
            }
            Element::Svg(svg) => {
                self.render_svg_node(node, svg);
            }
            Element::Image(image) => {
                self.render_image_node(node, image);
            }
            Element::Icon(icon) => {
                self.render_icon_node(node, icon);
            }
            Element::Part(_) | Element::Component(_) | Element::Slot(_) => {
                // These are either 3D or should be expanded
            }
        }
    }

    fn render_frame_node(&self, node: &LayoutNode, properties: &[seed_core::ast::Property]) {
        let bounds = &node.absolute_bounds;

        // Extract fill color
        let fill = properties.iter()
            .find(|p| p.name == "fill")
            .and_then(|p| match &p.value {
                seed_core::ast::PropertyValue::Color(c) => Some(color_to_css(c)),
                _ => None,
            });

        // Extract stroke color
        let stroke = properties.iter()
            .find(|p| p.name == "stroke")
            .and_then(|p| match &p.value {
                seed_core::ast::PropertyValue::Color(c) => Some(color_to_css(c)),
                _ => None,
            });

        // Extract corner radius
        let radius = properties.iter()
            .find(|p| p.name == "corner-radius" || p.name == "cornerRadius")
            .and_then(|p| match &p.value {
                seed_core::ast::PropertyValue::Length(l) => l.to_px(None),
                seed_core::ast::PropertyValue::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(0.0);

        // Draw rounded rectangle
        if radius > 0.0 {
            self.draw_rounded_rect(bounds.x, bounds.y, bounds.width, bounds.height, radius);
        } else {
            self.ctx.begin_path();
            self.ctx.rect(bounds.x, bounds.y, bounds.width, bounds.height);
        }

        // Fill
        if let Some(fill_color) = fill {
            self.ctx.set_fill_style_str(&fill_color);
            self.ctx.fill();
        }

        // Stroke
        if let Some(stroke_color) = stroke {
            self.ctx.set_stroke_style_str(&stroke_color);
            self.ctx.stroke();
        }
    }

    fn render_text_node(
        &self,
        node: &LayoutNode,
        content: &seed_core::ast::TextContent,
        properties: &[seed_core::ast::Property],
    ) {
        let bounds = &node.absolute_bounds;

        // Get text content
        let text = match content {
            seed_core::ast::TextContent::Literal(s) => s.clone(),
            seed_core::ast::TextContent::TokenRef(_) => return, // Should be resolved
        };

        // Extract text color
        let color = properties.iter()
            .find(|p| p.name == "color" || p.name == "fill")
            .and_then(|p| match &p.value {
                seed_core::ast::PropertyValue::Color(c) => Some(color_to_css(c)),
                _ => None,
            })
            .unwrap_or_else(|| "#000000".to_string());

        // Extract font size
        let font_size = properties.iter()
            .find(|p| p.name == "font-size" || p.name == "fontSize")
            .and_then(|p| match &p.value {
                seed_core::ast::PropertyValue::Length(l) => l.to_px(None),
                seed_core::ast::PropertyValue::Number(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(16.0);

        // Extract font family
        let font_family = properties.iter()
            .find(|p| p.name == "font-family" || p.name == "fontFamily")
            .and_then(|p| match &p.value {
                seed_core::ast::PropertyValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "sans-serif".to_string());

        // Set font
        let font = format!("{}px {}", font_size, font_family);
        self.ctx.set_font(&font);
        self.ctx.set_fill_style_str(&color);
        self.ctx.set_text_baseline("top");

        // Draw text
        let _ = self.ctx.fill_text(&text, bounds.x, bounds.y);
    }

    fn render_svg_node(&self, node: &LayoutNode, svg: &seed_core::ast::SvgElement) {
        use seed_core::ast::SvgPathCommand;

        let bounds = &node.absolute_bounds;

        // Get viewBox or use default
        let (vb_x, vb_y, vb_w, vb_h) = svg.view_box
            .as_ref()
            .map(|vb| (vb.min_x, vb.min_y, vb.width, vb.height))
            .unwrap_or((0.0, 0.0, 24.0, 24.0));

        // Calculate scale factors
        let scale_x = bounds.width / vb_w;
        let scale_y = bounds.height / vb_h;

        for path in &svg.paths {
            self.ctx.begin_path();

            // Track current position for relative commands
            let mut cur_x = 0.0;
            let mut cur_y = 0.0;
            let mut start_x = 0.0;
            let mut start_y = 0.0;

            for cmd in &path.commands {
                match cmd {
                    SvgPathCommand::MoveTo { x, y } => {
                        let px = bounds.x + (*x - vb_x) * scale_x;
                        let py = bounds.y + (*y - vb_y) * scale_y;
                        self.ctx.move_to(px, py);
                        cur_x = *x;
                        cur_y = *y;
                        start_x = cur_x;
                        start_y = cur_y;
                    }
                    SvgPathCommand::MoveToRel { dx, dy } => {
                        cur_x += *dx;
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.move_to(px, py);
                        start_x = cur_x;
                        start_y = cur_y;
                    }
                    SvgPathCommand::LineTo { x, y } => {
                        let px = bounds.x + (*x - vb_x) * scale_x;
                        let py = bounds.y + (*y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                        cur_x = *x;
                        cur_y = *y;
                    }
                    SvgPathCommand::LineToRel { dx, dy } => {
                        cur_x += *dx;
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                    }
                    SvgPathCommand::HorizontalTo { x } => {
                        cur_x = *x;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                    }
                    SvgPathCommand::HorizontalToRel { dx } => {
                        cur_x += *dx;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                    }
                    SvgPathCommand::VerticalTo { y } => {
                        cur_y = *y;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                    }
                    SvgPathCommand::VerticalToRel { dy } => {
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                    }
                    SvgPathCommand::CubicTo { x1, y1, x2, y2, x, y } => {
                        let px1 = bounds.x + (*x1 - vb_x) * scale_x;
                        let py1 = bounds.y + (*y1 - vb_y) * scale_y;
                        let px2 = bounds.x + (*x2 - vb_x) * scale_x;
                        let py2 = bounds.y + (*y2 - vb_y) * scale_y;
                        let px = bounds.x + (*x - vb_x) * scale_x;
                        let py = bounds.y + (*y - vb_y) * scale_y;
                        self.ctx.bezier_curve_to(px1, py1, px2, py2, px, py);
                        cur_x = *x;
                        cur_y = *y;
                    }
                    SvgPathCommand::CubicToRel { dx1, dy1, dx2, dy2, dx, dy } => {
                        let px1 = bounds.x + (cur_x + *dx1 - vb_x) * scale_x;
                        let py1 = bounds.y + (cur_y + *dy1 - vb_y) * scale_y;
                        let px2 = bounds.x + (cur_x + *dx2 - vb_x) * scale_x;
                        let py2 = bounds.y + (cur_y + *dy2 - vb_y) * scale_y;
                        cur_x += *dx;
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.bezier_curve_to(px1, py1, px2, py2, px, py);
                    }
                    SvgPathCommand::SmoothCubicTo { x2, y2, x, y } => {
                        // Simplified: use current point as first control
                        let px1 = bounds.x + (cur_x - vb_x) * scale_x;
                        let py1 = bounds.y + (cur_y - vb_y) * scale_y;
                        let px2 = bounds.x + (*x2 - vb_x) * scale_x;
                        let py2 = bounds.y + (*y2 - vb_y) * scale_y;
                        let px = bounds.x + (*x - vb_x) * scale_x;
                        let py = bounds.y + (*y - vb_y) * scale_y;
                        self.ctx.bezier_curve_to(px1, py1, px2, py2, px, py);
                        cur_x = *x;
                        cur_y = *y;
                    }
                    SvgPathCommand::SmoothCubicToRel { dx2, dy2, dx, dy } => {
                        let px1 = bounds.x + (cur_x - vb_x) * scale_x;
                        let py1 = bounds.y + (cur_y - vb_y) * scale_y;
                        let px2 = bounds.x + (cur_x + *dx2 - vb_x) * scale_x;
                        let py2 = bounds.y + (cur_y + *dy2 - vb_y) * scale_y;
                        cur_x += *dx;
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.bezier_curve_to(px1, py1, px2, py2, px, py);
                    }
                    SvgPathCommand::QuadTo { x1, y1, x, y } => {
                        let px1 = bounds.x + (*x1 - vb_x) * scale_x;
                        let py1 = bounds.y + (*y1 - vb_y) * scale_y;
                        let px = bounds.x + (*x - vb_x) * scale_x;
                        let py = bounds.y + (*y - vb_y) * scale_y;
                        self.ctx.quadratic_curve_to(px1, py1, px, py);
                        cur_x = *x;
                        cur_y = *y;
                    }
                    SvgPathCommand::QuadToRel { dx1, dy1, dx, dy } => {
                        let px1 = bounds.x + (cur_x + *dx1 - vb_x) * scale_x;
                        let py1 = bounds.y + (cur_y + *dy1 - vb_y) * scale_y;
                        cur_x += *dx;
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.quadratic_curve_to(px1, py1, px, py);
                    }
                    SvgPathCommand::SmoothQuadTo { x, y } => {
                        // Simplified: use line
                        let px = bounds.x + (*x - vb_x) * scale_x;
                        let py = bounds.y + (*y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                        cur_x = *x;
                        cur_y = *y;
                    }
                    SvgPathCommand::SmoothQuadToRel { dx, dy } => {
                        cur_x += *dx;
                        cur_y += *dy;
                        let px = bounds.x + (cur_x - vb_x) * scale_x;
                        let py = bounds.y + (cur_y - vb_y) * scale_y;
                        self.ctx.line_to(px, py);
                    }
                    SvgPathCommand::ArcTo { rx, ry, x_rotation: _, large_arc: _, sweep: _, x, y } => {
                        // Canvas doesn't have direct arc support with SVG params
                        // Approximate with line for now
                        if *rx > 0.0 && *ry > 0.0 {
                            // Use ellipse approximation via arc_to
                            let end_x = bounds.x + (*x - vb_x) * scale_x;
                            let end_y = bounds.y + (*y - vb_y) * scale_y;
                            self.ctx.line_to(end_x, end_y);
                        } else {
                            let px = bounds.x + (*x - vb_x) * scale_x;
                            let py = bounds.y + (*y - vb_y) * scale_y;
                            self.ctx.line_to(px, py);
                        }
                        cur_x = *x;
                        cur_y = *y;
                    }
                    SvgPathCommand::ArcToRel { rx, ry, x_rotation: _, large_arc: _, sweep: _, dx, dy } => {
                        let end_x = cur_x + *dx;
                        let end_y = cur_y + *dy;
                        if *rx > 0.0 && *ry > 0.0 {
                            let px = bounds.x + (end_x - vb_x) * scale_x;
                            let py = bounds.y + (end_y - vb_y) * scale_y;
                            self.ctx.line_to(px, py);
                        } else {
                            let px = bounds.x + (end_x - vb_x) * scale_x;
                            let py = bounds.y + (end_y - vb_y) * scale_y;
                            self.ctx.line_to(px, py);
                        }
                        cur_x = end_x;
                        cur_y = end_y;
                    }
                    SvgPathCommand::ClosePath => {
                        self.ctx.close_path();
                        cur_x = start_x;
                        cur_y = start_y;
                    }
                }
            }

            // Apply fill
            if let Some(color) = path.fill {
                let css = color_to_css(&color);
                self.ctx.set_fill_style_str(&css);
                self.ctx.fill();
            }

            // Apply stroke
            if let Some(color) = path.stroke {
                let css = color_to_css(&color);
                self.ctx.set_stroke_style_str(&css);
                if let Some(width) = path.stroke_width {
                    self.ctx.set_line_width(width * scale_x.min(scale_y));
                }
                self.ctx.stroke();
            }
        }
    }

    fn draw_rounded_rect(&self, x: f64, y: f64, width: f64, height: f64, radius: f64) {
        let r = radius.min(width / 2.0).min(height / 2.0);

        self.ctx.begin_path();
        self.ctx.move_to(x + r, y);
        self.ctx.line_to(x + width - r, y);
        self.ctx.arc_to(x + width, y, x + width, y + r, r).unwrap_or(());
        self.ctx.line_to(x + width, y + height - r);
        self.ctx.arc_to(x + width, y + height, x + width - r, y + height, r).unwrap_or(());
        self.ctx.line_to(x + r, y + height);
        self.ctx.arc_to(x, y + height, x, y + height - r, r).unwrap_or(());
        self.ctx.line_to(x, y + r);
        self.ctx.arc_to(x, y, x + r, y, r).unwrap_or(());
        self.ctx.close_path();
    }

    fn render_image_node(&self, node: &LayoutNode, _image: &seed_core::ast::ImageElement) {
        let bounds = &node.absolute_bounds;

        // For now, render a placeholder rectangle with X pattern
        // Full image loading would require async fetch
        self.ctx.set_fill_style_str("#c8c8c8");
        self.ctx.fill_rect(bounds.x, bounds.y, bounds.width, bounds.height);

        self.ctx.set_stroke_style_str("#969696");
        self.ctx.set_line_width(1.0);
        self.ctx.stroke_rect(bounds.x, bounds.y, bounds.width, bounds.height);

        // Draw X pattern
        self.ctx.begin_path();
        self.ctx.move_to(bounds.x, bounds.y);
        self.ctx.line_to(bounds.x + bounds.width, bounds.y + bounds.height);
        self.ctx.move_to(bounds.x + bounds.width, bounds.y);
        self.ctx.line_to(bounds.x, bounds.y + bounds.height);
        self.ctx.stroke();
    }

    fn render_icon_node(&self, node: &LayoutNode, icon: &seed_core::ast::IconElement) {
        use seed_core::ast::SvgPathCommand;

        let bounds = &node.absolute_bounds;

        // Get color
        let color = icon.color.unwrap_or(seed_core::types::Color::BLACK);
        let css = color_to_css(&color);

        match &icon.icon {
            seed_core::ast::IconSource::Svg(paths) => {
                // Render inline SVG paths
                let scale_x = bounds.width / 24.0;
                let scale_y = bounds.height / 24.0;

                for path in paths {
                    self.ctx.begin_path();

                    let mut cur_x = 0.0;
                    let mut cur_y = 0.0;
                    let mut start_x = 0.0;
                    let mut start_y = 0.0;

                    for cmd in &path.commands {
                        match cmd {
                            SvgPathCommand::MoveTo { x, y } => {
                                let px = bounds.x + *x * scale_x;
                                let py = bounds.y + *y * scale_y;
                                self.ctx.move_to(px, py);
                                cur_x = *x;
                                cur_y = *y;
                                start_x = cur_x;
                                start_y = cur_y;
                            }
                            SvgPathCommand::LineTo { x, y } => {
                                let px = bounds.x + *x * scale_x;
                                let py = bounds.y + *y * scale_y;
                                self.ctx.line_to(px, py);
                                cur_x = *x;
                                cur_y = *y;
                            }
                            SvgPathCommand::LineToRel { dx, dy } => {
                                cur_x += *dx;
                                cur_y += *dy;
                                let px = bounds.x + cur_x * scale_x;
                                let py = bounds.y + cur_y * scale_y;
                                self.ctx.line_to(px, py);
                            }
                            SvgPathCommand::ClosePath => {
                                self.ctx.close_path();
                                cur_x = start_x;
                                cur_y = start_y;
                            }
                            // Skip other commands for simplicity
                            _ => {}
                        }
                    }

                    // Apply fill
                    let fill_color = path.fill.unwrap_or(color);
                    self.ctx.set_fill_style_str(&color_to_css(&fill_color));
                    self.ctx.fill();

                    // Apply stroke if present
                    if let Some(stroke) = path.stroke {
                        self.ctx.set_stroke_style_str(&color_to_css(&stroke));
                        if let Some(width) = path.stroke_width {
                            self.ctx.set_line_width(width * scale_x.min(scale_y));
                        }
                        self.ctx.stroke();
                    }
                }
            }
            _ => {
                // Named icons or token refs: render placeholder circle
                let cx = bounds.x + bounds.width / 2.0;
                let cy = bounds.y + bounds.height / 2.0;
                let r = bounds.width.min(bounds.height) / 2.0;

                self.ctx.begin_path();
                self.ctx.arc(cx, cy, r, 0.0, std::f64::consts::PI * 2.0).unwrap_or(());
                self.ctx.set_fill_style_str(&css);
                self.ctx.fill();
            }
        }
    }
}

/// Convert a Color to a CSS color string.
fn color_to_css(color: &seed_core::types::Color) -> String {
    let (r, g, b, a) = color.to_rgba8();
    if a == 255 {
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
        format!("rgba({}, {}, {}, {})", r, g, b, a as f64 / 255.0)
    }
}

/// Render a Seed document to a canvas element.
///
/// This is a convenience function for simple rendering.
#[wasm_bindgen(js_name = renderToCanvas)]
pub fn render_to_canvas(
    canvas: HtmlCanvasElement,
    document_json: JsValue,
    _layout_json: JsValue,
    options: JsValue,
) -> Result<(), JsError> {
    let _doc: Document = serde_wasm_bindgen::from_value(document_json)
        .map_err(|e| JsError::new(&format!("Invalid document: {}", e)))?;

    let opts: RenderOptionsJs = serde_wasm_bindgen::from_value(options)
        .unwrap_or_default();

    let renderer = CanvasRenderer::new(canvas)?;

    // Apply device pixel ratio
    if let Some(dpr) = opts.device_pixel_ratio {
        let _ = renderer.ctx.scale(dpr, dpr);
    }

    // Clear with background
    renderer.clear("#ffffff");

    // Note: Full rendering would require properly deserializing the layout
    // This is a simplified version

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_to_css() {
        let color = seed_core::types::Color::rgb(1.0, 0.0, 0.0);
        let css = color_to_css(&color);
        assert_eq!(css, "#ff0000");
    }

    #[test]
    fn test_color_to_css_with_alpha() {
        let color = seed_core::types::Color::rgba(1.0, 0.0, 0.0, 0.5);
        let css = color_to_css(&color);
        assert!(css.starts_with("rgba("));
    }
}
