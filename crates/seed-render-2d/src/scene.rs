//! Scene building from documents and layouts.

use seed_core::{
    ast::{Element, FrameElement, TextElement, Property, PropertyValue},
    types::Color,
    Document,
};
use seed_layout::{LayoutTree, LayoutNodeId};

use crate::primitives::{
    Scene, RenderCommand, RectPrimitive, RoundedRectPrimitive, TextPrimitive,
    Fill, Stroke, CornerRadius,
};

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

        // Get fill and stroke from properties
        let fill = get_fill_from_properties(&frame.properties);
        let stroke = get_stroke_from_properties(&frame.properties);
        let corner_radius = get_corner_radius_from_properties(&frame.properties);

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

        // Render children
        for &child_id in &node.children {
            for child in &frame.children {
                self.build_element(child, child_id);
            }
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

fn get_fill_from_properties(properties: &[Property]) -> Option<Fill> {
    get_color_from_properties(properties, "fill")
        .or_else(|| get_color_from_properties(properties, "background"))
        .or_else(|| get_color_from_properties(properties, "background-color"))
        .map(Fill::Solid)
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
