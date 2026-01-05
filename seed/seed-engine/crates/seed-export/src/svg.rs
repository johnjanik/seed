//! SVG export for 2D documents.

use seed_core::{
    Document, ExportError,
    ast::{Element, FrameElement, TextElement, Property, PropertyValue},
    types::{Color, Gradient, LinearGradient, RadialGradient, ConicGradient, GradientStop},
};
use seed_layout::{LayoutTree, LayoutNodeId};
use std::collections::HashMap;

/// Export a document to SVG.
pub fn export(doc: &Document, layout: &LayoutTree) -> Result<String, ExportError> {
    let bounds = layout.content_bounds();
    let width = bounds.width.max(1.0);
    let height = bounds.height.max(1.0);

    // First pass: collect all gradients
    let mut gradient_collector = GradientCollector::new();
    for element in &doc.elements {
        gradient_collector.collect_from_element(element);
    }

    let mut svg = String::new();

    // XML declaration and SVG root
    svg.push_str(&format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">
"#,
        width, height, width, height
    ));

    // Write gradient definitions if any
    if !gradient_collector.gradients.is_empty() {
        svg.push_str("  <defs>\n");
        for (id, gradient) in &gradient_collector.gradients {
            write_gradient_def(&mut svg, id, gradient);
        }
        svg.push_str("  </defs>\n");
    }

    // Export elements
    let mut builder = SvgBuilder {
        svg: &mut svg,
        layout,
        indent: 1,
        gradients: &gradient_collector.gradients,
    };

    for (idx, element) in doc.elements.iter().enumerate() {
        let node_id = layout.roots().get(idx).copied();
        builder.export_element(element, node_id)?;
    }

    svg.push_str("</svg>\n");

    Ok(svg)
}

/// Collects gradients from a document and assigns unique IDs.
struct GradientCollector {
    gradients: HashMap<String, Gradient>,
    counter: usize,
}

impl GradientCollector {
    fn new() -> Self {
        Self {
            gradients: HashMap::new(),
            counter: 0,
        }
    }

    fn collect_from_element(&mut self, element: &Element) {
        match element {
            Element::Frame(frame) => {
                self.collect_from_properties(&frame.properties);
                for child in &frame.children {
                    self.collect_from_element(child);
                }
            }
            Element::Text(text) => {
                self.collect_from_properties(&text.properties);
            }
            _ => {}
        }
    }

    fn collect_from_properties(&mut self, properties: &[Property]) {
        for prop in properties {
            if let PropertyValue::Gradient(gradient) = &prop.value {
                let id = self.get_or_create_id(gradient);
                let _ = id; // ID is already stored
            }
        }
    }

    fn get_or_create_id(&mut self, gradient: &Gradient) -> String {
        // Check if we already have this gradient
        for (id, existing) in &self.gradients {
            if gradients_equal(existing, gradient) {
                return id.clone();
            }
        }

        // Create new ID
        self.counter += 1;
        let id = format!("gradient{}", self.counter);
        self.gradients.insert(id.clone(), gradient.clone());
        id
    }

    #[allow(dead_code)]
    fn find_gradient_id(&self, gradient: &Gradient) -> Option<String> {
        for (id, existing) in &self.gradients {
            if gradients_equal(existing, gradient) {
                return Some(id.clone());
            }
        }
        None
    }
}

fn gradients_equal(a: &Gradient, b: &Gradient) -> bool {
    // Simple equality check
    a == b
}

/// Write a gradient definition to the SVG defs section.
fn write_gradient_def(svg: &mut String, id: &str, gradient: &Gradient) {
    match gradient {
        Gradient::Linear(linear) => write_linear_gradient_def(svg, id, linear),
        Gradient::Radial(radial) => write_radial_gradient_def(svg, id, radial),
        Gradient::Conic(conic) => write_conic_gradient_def(svg, id, conic),
    }
}

fn write_linear_gradient_def(svg: &mut String, id: &str, gradient: &LinearGradient) {
    // Convert angle to x1, y1, x2, y2
    // SVG uses coordinates where (0,0) is top-left
    let angle_rad = gradient.angle.to_radians();
    let (x1, y1, x2, y2) = angle_to_coordinates(angle_rad);

    svg.push_str(&format!(
        "    <linearGradient id=\"{}\" x1=\"{}%\" y1=\"{}%\" x2=\"{}%\" y2=\"{}%\">\n",
        id,
        (x1 * 100.0).round(),
        (y1 * 100.0).round(),
        (x2 * 100.0).round(),
        (y2 * 100.0).round()
    ));

    for stop in &gradient.stops {
        write_gradient_stop(svg, stop);
    }

    svg.push_str("    </linearGradient>\n");
}

fn write_radial_gradient_def(svg: &mut String, id: &str, gradient: &RadialGradient) {
    svg.push_str(&format!(
        "    <radialGradient id=\"{}\" cx=\"{}%\" cy=\"{}%\" r=\"{}%\" fx=\"{}%\" fy=\"{}%\">\n",
        id,
        (gradient.center_x * 100.0).round(),
        (gradient.center_y * 100.0).round(),
        ((gradient.radius_x + gradient.radius_y) / 2.0 * 50.0).round(), // Average radius as %
        (gradient.center_x * 100.0).round(),
        (gradient.center_y * 100.0).round()
    ));

    for stop in &gradient.stops {
        write_gradient_stop(svg, stop);
    }

    svg.push_str("    </radialGradient>\n");
}

fn write_conic_gradient_def(svg: &mut String, id: &str, gradient: &ConicGradient) {
    // SVG doesn't natively support conic gradients
    // We'll approximate with a radial gradient or just use the first/last colors
    // For now, create a simple two-color radial as fallback
    svg.push_str(&format!(
        "    <!-- Conic gradient approximation (SVG doesn't support conic gradients natively) -->\n"
    ));
    svg.push_str(&format!(
        "    <radialGradient id=\"{}\" cx=\"{}%\" cy=\"{}%\" r=\"50%\">\n",
        id,
        (gradient.center_x * 100.0).round(),
        (gradient.center_y * 100.0).round()
    ));

    for stop in &gradient.stops {
        write_gradient_stop(svg, stop);
    }

    svg.push_str("    </radialGradient>\n");
}

fn write_gradient_stop(svg: &mut String, stop: &GradientStop) {
    let (r, g, b, a) = stop.color.to_rgba8();
    let color_str = format!("#{:02x}{:02x}{:02x}", r, g, b);
    let offset = (stop.position * 100.0).round();

    if a == 255 {
        svg.push_str(&format!(
            "      <stop offset=\"{}%\" stop-color=\"{}\" />\n",
            offset, color_str
        ));
    } else {
        svg.push_str(&format!(
            "      <stop offset=\"{}%\" stop-color=\"{}\" stop-opacity=\"{}\" />\n",
            offset, color_str, a as f64 / 255.0
        ));
    }
}

/// Convert angle in radians to SVG gradient coordinates.
fn angle_to_coordinates(angle_rad: f64) -> (f64, f64, f64, f64) {
    // SVG coordinates: (0,0) is top-left, y increases downward
    // Angle 0 = right, 90 = up, 180 = left, 270 = down
    let cos = angle_rad.cos();
    let sin = angle_rad.sin();

    // Start from center, extend to edges
    let x1 = 0.5 - cos * 0.5;
    let y1 = 0.5 + sin * 0.5; // Invert for SVG coordinates
    let x2 = 0.5 + cos * 0.5;
    let y2 = 0.5 - sin * 0.5;

    (x1, y1, x2, y2)
}

struct SvgBuilder<'a> {
    svg: &'a mut String,
    layout: &'a LayoutTree,
    indent: usize,
    gradients: &'a HashMap<String, Gradient>,
}

impl<'a> SvgBuilder<'a> {
    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.svg.push_str("  ");
        }
    }

    fn export_element(&mut self, element: &Element, node_id: Option<LayoutNodeId>) -> Result<(), ExportError> {
        match element {
            Element::Frame(frame) => self.export_frame(frame, node_id),
            Element::Text(text) => self.export_text(text, node_id),
            Element::Part(_) => Ok(()), // 3D parts don't render in 2D SVG
            Element::Component(_) => Ok(()), // Components should be expanded
            Element::Slot(_) => Ok(()), // Slots should be expanded
        }
    }

    fn export_frame(&mut self, frame: &FrameElement, node_id: Option<LayoutNodeId>) -> Result<(), ExportError> {
        // Get bounds from layout
        let (x, y, width, height) = if let Some(id) = node_id {
            if let Some(node) = self.layout.get(id) {
                let b = node.absolute_bounds;
                (b.x, b.y, b.width, b.height)
            } else {
                (0.0, 0.0, 100.0, 100.0)
            }
        } else {
            (0.0, 0.0, 100.0, 100.0)
        };

        // Extract style properties
        let fill = get_fill(&frame.properties, self.gradients);
        let stroke = get_stroke_color(&frame.properties);
        let stroke_width = get_stroke_width(&frame.properties);
        let corner_radius = get_corner_radius(&frame.properties);
        let opacity = get_opacity(&frame.properties);

        // Build style attributes
        let mut attrs = Vec::new();

        attrs.push(format!("x=\"{}\"", x));
        attrs.push(format!("y=\"{}\"", y));
        attrs.push(format!("width=\"{}\"", width));
        attrs.push(format!("height=\"{}\"", height));

        if let Some(r) = corner_radius {
            attrs.push(format!("rx=\"{}\"", r));
            attrs.push(format!("ry=\"{}\"", r));
        }

        match fill {
            FillValue::Color(color) => {
                attrs.push(format!("fill=\"{}\"", color_to_svg(&color)));
                if color.a < 1.0 {
                    attrs.push(format!("fill-opacity=\"{}\"", color.a));
                }
            }
            FillValue::Gradient(gradient_id) => {
                attrs.push(format!("fill=\"url(#{})\"", gradient_id));
            }
            FillValue::None => {
                attrs.push("fill=\"none\"".to_string());
            }
        }

        if let Some(color) = stroke {
            attrs.push(format!("stroke=\"{}\"", color_to_svg(&color)));
            if color.a < 1.0 {
                attrs.push(format!("stroke-opacity=\"{}\"", color.a));
            }
        }

        if let Some(width) = stroke_width {
            attrs.push(format!("stroke-width=\"{}\"", width));
        }

        if let Some(op) = opacity {
            if op < 1.0 {
                attrs.push(format!("opacity=\"{}\"", op));
            }
        }

        // Check if we have children
        if frame.children.is_empty() {
            // Self-closing rect
            self.write_indent();
            self.svg.push_str(&format!("<rect {} />\n", attrs.join(" ")));
        } else {
            // Group with rect and children
            self.write_indent();
            self.svg.push_str("<g");
            if let Some(op) = opacity {
                if op < 1.0 {
                    self.svg.push_str(&format!(" opacity=\"{}\"", op));
                }
            }
            self.svg.push_str(">\n");

            self.indent += 1;

            // Draw the rect (without opacity, as it's on the group)
            let rect_attrs: Vec<_> = attrs.iter()
                .filter(|a| !a.starts_with("opacity="))
                .cloned()
                .collect();
            self.write_indent();
            self.svg.push_str(&format!("<rect {} />\n", rect_attrs.join(" ")));

            // Export children
            if let Some(parent_id) = node_id {
                if let Some(parent_node) = self.layout.get(parent_id) {
                    for (idx, child) in frame.children.iter().enumerate() {
                        let child_id = parent_node.children.get(idx).copied();
                        self.export_element(child, child_id)?;
                    }
                }
            } else {
                for child in &frame.children {
                    self.export_element(child, None)?;
                }
            }

            self.indent -= 1;
            self.write_indent();
            self.svg.push_str("</g>\n");
        }

        Ok(())
    }

    fn export_text(&mut self, text: &TextElement, node_id: Option<LayoutNodeId>) -> Result<(), ExportError> {
        let (x, y) = if let Some(id) = node_id {
            if let Some(node) = self.layout.get(id) {
                let b = node.absolute_bounds;
                (b.x, b.y)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        let content = match &text.content {
            seed_core::ast::TextContent::Literal(s) => escape_xml(s),
            seed_core::ast::TextContent::TokenRef(path) => format!("[{}]", path.0.join(".")),
        };

        let color = get_color_property(&text.properties, "color")
            .unwrap_or(Color::BLACK);
        let font_size = get_number_property(&text.properties, "font-size")
            .unwrap_or(16.0);
        let font_family = get_string_property(&text.properties, "font-family")
            .unwrap_or_else(|| "sans-serif".to_string());

        self.write_indent();
        self.svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-size=\"{}\" font-family=\"{}\">{}</text>\n",
            x,
            y + font_size, // Adjust for baseline
            color_to_svg(&color),
            font_size,
            font_family,
            content
        ));

        Ok(())
    }
}

// Helper functions

/// Represents a fill value (color, gradient, or none).
enum FillValue {
    Color(Color),
    Gradient(String), // Gradient ID
    None,
}

fn color_to_svg(color: &Color) -> String {
    let (r, g, b, _) = color.to_rgba8();
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn get_fill(properties: &[Property], gradients: &HashMap<String, Gradient>) -> FillValue {
    for prop in properties {
        if prop.name == "fill" || prop.name == "background" || prop.name == "background-color" {
            match &prop.value {
                PropertyValue::Color(c) => return FillValue::Color(*c),
                PropertyValue::Gradient(gradient) => {
                    // Find the gradient ID
                    for (id, existing) in gradients {
                        if existing == gradient {
                            return FillValue::Gradient(id.clone());
                        }
                    }
                }
                PropertyValue::String(s) => {
                    if let Some(color) = Color::from_hex(s) {
                        return FillValue::Color(color);
                    }
                }
                _ => {}
            }
        }
    }
    FillValue::None
}

#[allow(dead_code)]
fn get_fill_color(properties: &[Property]) -> Option<Color> {
    get_color_property(properties, "fill")
        .or_else(|| get_color_property(properties, "background"))
        .or_else(|| get_color_property(properties, "background-color"))
}

fn get_stroke_color(properties: &[Property]) -> Option<Color> {
    get_color_property(properties, "stroke")
        .or_else(|| get_color_property(properties, "border-color"))
}

fn get_stroke_width(properties: &[Property]) -> Option<f64> {
    get_length_property(properties, "stroke-width")
        .or_else(|| get_length_property(properties, "border-width"))
}

fn get_corner_radius(properties: &[Property]) -> Option<f64> {
    get_length_property(properties, "corner-radius")
        .or_else(|| get_length_property(properties, "border-radius"))
}

fn get_opacity(properties: &[Property]) -> Option<f64> {
    get_number_property(properties, "opacity")
}

fn get_color_property(properties: &[Property], name: &str) -> Option<Color> {
    properties.iter()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            PropertyValue::Color(c) => Some(*c),
            PropertyValue::String(s) => Color::from_hex(s),
            _ => None,
        })
}

fn get_length_property(properties: &[Property], name: &str) -> Option<f64> {
    properties.iter()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            PropertyValue::Length(l) => l.to_px(None),
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        })
}

fn get_number_property(properties: &[Property], name: &str) -> Option<f64> {
    properties.iter()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        })
}

fn get_string_property(properties: &[Property], name: &str) -> Option<String> {
    properties.iter()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            PropertyValue::String(s) => Some(s.clone()),
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::Span;

    #[test]
    fn test_color_to_svg() {
        let color = Color::rgb(1.0, 0.5, 0.0);
        assert_eq!(color_to_svg(&color), "#ff7f00");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
    }

    #[test]
    fn test_export_empty_document() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };
        let layout = LayoutTree::new();

        let svg = export(&doc, &layout).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }
}
