//! Scene building from documents and layouts.

use seed_core::{
    ast::{Element, FrameElement, TextElement, Property, PropertyValue},
    types::{Color, Gradient, LinearGradient as AstLinearGradient, RadialGradient as AstRadialGradient, ConicGradient as AstConicGradient},
    Document,
};
use seed_layout::{LayoutTree, LayoutNodeId};

use crate::primitives::{
    Scene, RenderCommand, RectPrimitive, RoundedRectPrimitive, TextPrimitive,
    Fill, Stroke, CornerRadius, LinearGradient, RadialGradient, GradientStop,
    ShadowPrimitive, ShadowShape,
};
use glam::Vec2;
use seed_core::types::Shadow as AstShadow;

/// Build a renderable scene from a document and its layout.
pub fn build_scene(doc: &Document, layout: &LayoutTree) -> Scene {
    let bounds = layout.content_bounds();
    let mut scene = Scene::new(bounds.width as f32, bounds.height as f32);

    let mut builder = SceneBuilder {
        scene: &mut scene,
        layout,
    };

    // Render all root elements
    for &root_id in layout.roots() {
        if layout.get(root_id).is_some() {
            // Find the corresponding element
            for element in &doc.elements {
                builder.build_element(element, root_id);
            }
        }
    }

    scene
}

struct SceneBuilder<'a> {
    scene: &'a mut Scene,
    layout: &'a LayoutTree,
}

impl<'a> SceneBuilder<'a> {
    fn build_element(&mut self, element: &Element, node_id: LayoutNodeId) {
        match element {
            Element::Frame(frame) => self.build_frame(frame, node_id),
            Element::Text(text) => self.build_text(text, node_id),
            Element::Part(_) => {
                // 3D parts don't render in 2D
            }
            Element::Component(_) => {
                // Components should be expanded before rendering
            }
            Element::Slot(_) => {
                // Slots should be expanded before rendering
            }
        }
    }

    fn build_frame(&mut self, frame: &FrameElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        let bounds = node.absolute_bounds;

        // Check visibility
        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        // Set opacity if not fully opaque
        if node.opacity < 1.0 {
            self.scene.push(RenderCommand::SetOpacity(node.opacity as f32));
        }

        // Push clip if needed
        if node.clips_children {
            self.scene.push_clip(
                bounds.x as f32,
                bounds.y as f32,
                bounds.width as f32,
                bounds.height as f32,
            );
        }

        // Get fill, stroke, shadow from properties
        let fill = get_fill_from_properties(&frame.properties, bounds.x as f32, bounds.y as f32, bounds.width as f32, bounds.height as f32);
        let stroke = get_stroke_from_properties(&frame.properties);
        let corner_radius = get_corner_radius_from_properties(&frame.properties);
        let shadow = get_shadow_from_properties(&frame.properties);

        // Render shadow first (behind the shape)
        if let Some(shadow) = shadow {
            let radius = corner_radius.unwrap_or_else(|| CornerRadius::uniform(0.0));
            let shadow_prim = ShadowPrimitive::new(
                ShadowShape::Rect {
                    x: bounds.x as f32,
                    y: bounds.y as f32,
                    width: bounds.width as f32,
                    height: bounds.height as f32,
                    radius,
                },
                shadow.offset_x as f32,
                shadow.offset_y as f32,
                shadow.blur as f32,
                shadow.spread as f32,
                shadow.color,
                shadow.inset,
            );
            self.scene.shadow(shadow_prim);
        }

        // Only render if there's something to draw
        if fill.is_some() || stroke.is_some() {
            if let Some(radius) = corner_radius {
                if !radius.is_zero() {
                    let mut rect = RoundedRectPrimitive::new(
                        bounds.x as f32,
                        bounds.y as f32,
                        bounds.width as f32,
                        bounds.height as f32,
                        0.0,
                    );
                    rect.radius = radius;
                    rect.fill = fill;
                    rect.stroke = stroke;
                    self.scene.rounded_rect(rect);
                } else {
                    let mut rect = RectPrimitive::new(
                        bounds.x as f32,
                        bounds.y as f32,
                        bounds.width as f32,
                        bounds.height as f32,
                    );
                    rect.fill = fill;
                    rect.stroke = stroke;
                    self.scene.rect(rect);
                }
            } else {
                let mut rect = RectPrimitive::new(
                    bounds.x as f32,
                    bounds.y as f32,
                    bounds.width as f32,
                    bounds.height as f32,
                );
                rect.fill = fill;
                rect.stroke = stroke;
                self.scene.rect(rect);
            }
        }

        // Render children - zip AST children with layout children
        for (child, &child_id) in frame.children.iter().zip(node.children.iter()) {
            self.build_element(child, child_id);
        }

        // Pop clip if we pushed one
        if node.clips_children {
            self.scene.pop_clip();
        }

        // Reset opacity
        if node.opacity < 1.0 {
            self.scene.push(RenderCommand::SetOpacity(1.0));
        }
    }

    fn build_text(&mut self, text: &TextElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;

        // Get text content
        let content = match &text.content {
            seed_core::ast::TextContent::Literal(s) => s.clone(),
            seed_core::ast::TextContent::TokenRef(_) => "[token]".to_string(),
        };

        // Get text properties
        let color = get_color_from_properties(&text.properties, "color")
            .unwrap_or_else(|| Color::rgb(0.0, 0.0, 0.0));
        let font_size = get_length_from_properties(&text.properties, "font-size")
            .unwrap_or(16.0);

        let text_prim = TextPrimitive::new(bounds.x as f32, bounds.y as f32, content)
            .with_font_size(font_size as f32)
            .with_color(color);

        self.scene.text(text_prim);
    }
}

// Property extraction helpers

fn get_fill_from_properties(properties: &[Property], x: f32, y: f32, width: f32, height: f32) -> Option<Fill> {
    // First check for gradient fills
    for prop in properties {
        if prop.name == "fill" || prop.name == "background" || prop.name == "background-color" {
            if let PropertyValue::Gradient(gradient) = &prop.value {
                return Some(convert_gradient(gradient, x, y, width, height));
            }
        }
    }

    // Fall back to solid color
    get_color_from_properties(properties, "fill")
        .or_else(|| get_color_from_properties(properties, "background"))
        .or_else(|| get_color_from_properties(properties, "background-color"))
        .map(Fill::Solid)
}

/// Convert an AST gradient to a render primitive gradient.
fn convert_gradient(gradient: &Gradient, x: f32, y: f32, width: f32, height: f32) -> Fill {
    match gradient {
        Gradient::Linear(linear) => {
            // Convert angle to start/end points
            // Angle: 0 = right (→), 90 = up (↑), 180 = left (←), 270 = down (↓)
            let angle_rad = linear.angle.to_radians();
            let cos_a = angle_rad.cos() as f32;
            let sin_a = angle_rad.sin() as f32;

            // Calculate gradient line endpoints
            // The gradient line passes through the center and extends to the edges
            let cx = x + width / 2.0;
            let cy = y + height / 2.0;

            // Calculate the length needed to cover the rectangle
            let half_diag = ((width / 2.0).powi(2) + (height / 2.0).powi(2)).sqrt();

            let start = Vec2::new(cx - cos_a * half_diag, cy + sin_a * half_diag);
            let end = Vec2::new(cx + cos_a * half_diag, cy - sin_a * half_diag);

            let stops: Vec<GradientStop> = linear.stops.iter().map(|s| {
                GradientStop {
                    offset: s.position as f32,
                    color: s.color,
                }
            }).collect();

            Fill::LinearGradient(LinearGradient { start, end, stops })
        }
        Gradient::Radial(radial) => {
            // Convert relative center to absolute coordinates
            let cx = x + width * radial.center_x as f32;
            let cy = y + height * radial.center_y as f32;

            // Use the larger dimension for radius
            let radius = (width.max(height) / 2.0) * radial.radius_x as f32;

            let stops: Vec<GradientStop> = radial.stops.iter().map(|s| {
                GradientStop {
                    offset: s.position as f32,
                    color: s.color,
                }
            }).collect();

            Fill::RadialGradient(RadialGradient {
                center: Vec2::new(cx, cy),
                radius,
                stops,
            })
        }
        Gradient::Conic(conic) => {
            // For conic gradients, we'll approximate with a radial gradient for now
            // A proper implementation would require angular sampling
            let cx = x + width * conic.center_x as f32;
            let cy = y + height * conic.center_y as f32;
            let radius = width.max(height) / 2.0;

            let stops: Vec<GradientStop> = conic.stops.iter().map(|s| {
                GradientStop {
                    offset: s.position as f32,
                    color: s.color,
                }
            }).collect();

            Fill::RadialGradient(RadialGradient {
                center: Vec2::new(cx, cy),
                radius,
                stops,
            })
        }
    }
}

fn get_stroke_from_properties(properties: &[Property]) -> Option<Stroke> {
    let color = get_color_from_properties(properties, "stroke")
        .or_else(|| get_color_from_properties(properties, "border-color"))?;

    let width = get_length_from_properties(properties, "stroke-width")
        .or_else(|| get_length_from_properties(properties, "border-width"))
        .unwrap_or(1.0);

    Some(Stroke::new(color, width as f32))
}

fn get_corner_radius_from_properties(properties: &[Property]) -> Option<CornerRadius> {
    // Try uniform radius first
    if let Some(radius) = get_length_from_properties(properties, "corner-radius")
        .or_else(|| get_length_from_properties(properties, "border-radius"))
    {
        return Some(CornerRadius::uniform(radius as f32));
    }

    // Try individual corners
    let top_left = get_length_from_properties(properties, "corner-radius-top-left").unwrap_or(0.0);
    let top_right = get_length_from_properties(properties, "corner-radius-top-right").unwrap_or(0.0);
    let bottom_right = get_length_from_properties(properties, "corner-radius-bottom-right").unwrap_or(0.0);
    let bottom_left = get_length_from_properties(properties, "corner-radius-bottom-left").unwrap_or(0.0);

    if top_left > 0.0 || top_right > 0.0 || bottom_right > 0.0 || bottom_left > 0.0 {
        Some(CornerRadius::new(
            top_left as f32,
            top_right as f32,
            bottom_right as f32,
            bottom_left as f32,
        ))
    } else {
        None
    }
}

fn get_shadow_from_properties(properties: &[Property]) -> Option<AstShadow> {
    // Check for shadow, box-shadow, or drop-shadow properties
    for prop in properties {
        if prop.name == "shadow" || prop.name == "box-shadow" || prop.name == "drop-shadow" {
            if let PropertyValue::Shadow(shadow) = &prop.value {
                return Some(*shadow);
            }
        }
    }
    None
}

fn get_color_from_properties(properties: &[Property], name: &str) -> Option<Color> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Color(c) => Some(*c),
            PropertyValue::String(s) => Color::from_hex(s),
            _ => None,
        }
    })
}

fn get_length_from_properties(properties: &[Property], name: &str) -> Option<f64> {
    properties.iter().find(|p| p.name == name).and_then(|p| {
        match &p.value {
            PropertyValue::Length(l) => l.to_px(None),
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::*;

    #[test]
    fn test_build_empty_scene() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };
        let layout = LayoutTree::new();

        let scene = build_scene(&doc, &layout);
        assert!(scene.commands.is_empty());
    }
}
