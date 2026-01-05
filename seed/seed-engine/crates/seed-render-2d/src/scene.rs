//! Scene building from documents and layouts.

use seed_core::{
    ast::{Element, FrameElement, TextElement, Property, PropertyValue},
    types::{Color, Gradient},
    Document,
};
use seed_layout::{LayoutTree, LayoutNodeId};

use crate::primitives::{
    Scene, RenderCommand, RectPrimitive, RoundedRectPrimitive, TextPrimitive,
    EllipsePrimitive, Fill, Stroke, CornerRadius, LinearGradient, RadialGradient,
    GradientStop, ShadowPrimitive, ShadowShape,
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
            Element::Svg(svg) => self.build_svg(svg, node_id),
            Element::Image(image) => self.build_image(image, node_id),
            Element::Icon(icon) => self.build_icon(icon, node_id),
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

    fn build_svg(&mut self, svg: &seed_core::ast::SvgElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;

        // Get fill and stroke from properties
        let fill = get_fill_from_properties(&svg.properties, bounds.x as f32, bounds.y as f32, bounds.width as f32, bounds.height as f32);
        let stroke = get_stroke_from_properties(&svg.properties);

        // Calculate transform from viewBox to bounds
        let view_box = svg.view_box.unwrap_or_default();
        let scale_x = bounds.width / view_box.width;
        let scale_y = bounds.height / view_box.height;
        let offset_x = bounds.x - view_box.min_x * scale_x;
        let offset_y = bounds.y - view_box.min_y * scale_y;

        // Render each path
        for path in &svg.paths {
            self.render_svg_path(
                path,
                offset_x as f32,
                offset_y as f32,
                scale_x as f32,
                scale_y as f32,
                fill.clone(),
                stroke.clone(),
            );
        }
    }

    fn build_image(&mut self, _image: &seed_core::ast::ImageElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;

        // For now, render a placeholder rectangle with a cross pattern
        // to indicate an image placeholder. Full image loading requires
        // async handling which will be added later.
        let fill_color = Color::rgb(0.78, 0.78, 0.78); // Light gray placeholder

        // Draw placeholder rectangle
        let mut rect = RectPrimitive::new(
            bounds.x as f32,
            bounds.y as f32,
            bounds.width as f32,
            bounds.height as f32,
        );
        rect.fill = Some(Fill::Solid(fill_color));
        rect.stroke = Some(Stroke::new(Color::rgb(0.59, 0.59, 0.59), 1.0));

        self.scene.rect(rect);

        // Draw diagonal lines to indicate placeholder using a path
        let line_color = Color::rgb(0.7, 0.7, 0.7);

        // Create a path for the X pattern
        self.scene.polygon(
            vec![
                Vec2::new(bounds.x as f32, bounds.y as f32),
                Vec2::new((bounds.x + bounds.width) as f32, (bounds.y + bounds.height) as f32),
            ],
            None,
            Some(Stroke::new(line_color, 1.0)),
        );

        self.scene.polygon(
            vec![
                Vec2::new((bounds.x + bounds.width) as f32, bounds.y as f32),
                Vec2::new(bounds.x as f32, (bounds.y + bounds.height) as f32),
            ],
            None,
            Some(Stroke::new(line_color, 1.0)),
        );
    }

    fn build_icon(&mut self, icon: &seed_core::ast::IconElement, node_id: LayoutNodeId) {
        let Some(node) = self.layout.get(node_id) else {
            return;
        };

        if !node.visible || node.opacity <= 0.0 {
            return;
        }

        let bounds = node.absolute_bounds;

        // Get icon color (default to black if not specified)
        let color = icon.color.as_ref()
            .map(|c| Color::rgba(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.a as f32 / 255.0))
            .unwrap_or_else(|| Color::rgb(0.0, 0.0, 0.0));

        // For named icons, we'd need to load them from an icon library.
        // For now, render a placeholder circle to indicate an icon.
        match &icon.icon {
            seed_core::ast::IconSource::Svg(paths) => {
                // Render inline SVG paths
                let view_size = icon.size
                    .as_ref()
                    .and_then(|l| l.to_px(None))
                    .unwrap_or(24.0);

                let scale_x = bounds.width / view_size;
                let scale_y = bounds.height / view_size;

                for path in paths {
                    self.render_svg_path(
                        path,
                        bounds.x as f32,
                        bounds.y as f32,
                        scale_x as f32,
                        scale_y as f32,
                        Some(Fill::Solid(color)),
                        None,
                    );
                }
            }
            _ => {
                // Render placeholder circle for named icons
                let center_x = bounds.x + bounds.width / 2.0;
                let center_y = bounds.y + bounds.height / 2.0;
                let radius = bounds.width.min(bounds.height) / 2.0 * 0.8;

                let mut ellipse = EllipsePrimitive::circle(
                    center_x as f32,
                    center_y as f32,
                    radius as f32,
                );
                ellipse.fill = Some(Fill::Solid(color));
                ellipse.stroke = Some(Stroke::new(Color::rgb(0.4, 0.4, 0.4), 1.0));

                self.scene.ellipse(ellipse);
            }
        }
    }

    fn render_svg_path(
        &mut self,
        path: &seed_core::ast::SvgPath,
        offset_x: f32,
        offset_y: f32,
        scale_x: f32,
        scale_y: f32,
        default_fill: Option<Fill>,
        default_stroke: Option<Stroke>,
    ) {
        use seed_core::ast::SvgPathCommand;

        // Build polygon from path commands
        let mut vertices: Vec<Vec2> = Vec::new();
        let mut current_x = 0.0f32;
        let mut current_y = 0.0f32;
        let mut start_x = 0.0f32;
        let mut start_y = 0.0f32;
        let mut last_control_x = 0.0f32;
        let mut last_control_y = 0.0f32;
        let mut last_command_was_curve = false;

        for cmd in &path.commands {
            match cmd {
                SvgPathCommand::MoveTo { x, y } => {
                    // If we have vertices from a previous subpath, flush them
                    if !vertices.is_empty() {
                        self.flush_path_vertices(&vertices, path, default_fill.clone(), default_stroke.clone());
                        vertices.clear();
                    }
                    current_x = *x as f32 * scale_x + offset_x;
                    current_y = *y as f32 * scale_y + offset_y;
                    start_x = current_x;
                    start_y = current_y;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::MoveToRel { dx, dy } => {
                    if !vertices.is_empty() {
                        self.flush_path_vertices(&vertices, path, default_fill.clone(), default_stroke.clone());
                        vertices.clear();
                    }
                    current_x += *dx as f32 * scale_x;
                    current_y += *dy as f32 * scale_y;
                    start_x = current_x;
                    start_y = current_y;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::LineTo { x, y } => {
                    current_x = *x as f32 * scale_x + offset_x;
                    current_y = *y as f32 * scale_y + offset_y;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::LineToRel { dx, dy } => {
                    current_x += *dx as f32 * scale_x;
                    current_y += *dy as f32 * scale_y;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::HorizontalTo { x } => {
                    current_x = *x as f32 * scale_x + offset_x;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::HorizontalToRel { dx } => {
                    current_x += *dx as f32 * scale_x;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::VerticalTo { y } => {
                    current_y = *y as f32 * scale_y + offset_y;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::VerticalToRel { dy } => {
                    current_y += *dy as f32 * scale_y;
                    vertices.push(Vec2::new(current_x, current_y));
                    last_command_was_curve = false;
                }
                SvgPathCommand::CubicTo { x1, y1, x2, y2, x, y } => {
                    // Approximate cubic bezier with line segments
                    let cx1 = *x1 as f32 * scale_x + offset_x;
                    let cy1 = *y1 as f32 * scale_y + offset_y;
                    let cx2 = *x2 as f32 * scale_x + offset_x;
                    let cy2 = *y2 as f32 * scale_y + offset_y;
                    let end_x = *x as f32 * scale_x + offset_x;
                    let end_y = *y as f32 * scale_y + offset_y;

                    approximate_cubic_bezier(&mut vertices, current_x, current_y, cx1, cy1, cx2, cy2, end_x, end_y);

                    last_control_x = cx2;
                    last_control_y = cy2;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::CubicToRel { dx1, dy1, dx2, dy2, dx, dy } => {
                    let cx1 = current_x + *dx1 as f32 * scale_x;
                    let cy1 = current_y + *dy1 as f32 * scale_y;
                    let cx2 = current_x + *dx2 as f32 * scale_x;
                    let cy2 = current_y + *dy2 as f32 * scale_y;
                    let end_x = current_x + *dx as f32 * scale_x;
                    let end_y = current_y + *dy as f32 * scale_y;

                    approximate_cubic_bezier(&mut vertices, current_x, current_y, cx1, cy1, cx2, cy2, end_x, end_y);

                    last_control_x = cx2;
                    last_control_y = cy2;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::SmoothCubicTo { x2, y2, x, y } => {
                    // Reflect last control point
                    let cx1 = if last_command_was_curve {
                        2.0 * current_x - last_control_x
                    } else {
                        current_x
                    };
                    let cy1 = if last_command_was_curve {
                        2.0 * current_y - last_control_y
                    } else {
                        current_y
                    };
                    let cx2 = *x2 as f32 * scale_x + offset_x;
                    let cy2 = *y2 as f32 * scale_y + offset_y;
                    let end_x = *x as f32 * scale_x + offset_x;
                    let end_y = *y as f32 * scale_y + offset_y;

                    approximate_cubic_bezier(&mut vertices, current_x, current_y, cx1, cy1, cx2, cy2, end_x, end_y);

                    last_control_x = cx2;
                    last_control_y = cy2;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::SmoothCubicToRel { dx2, dy2, dx, dy } => {
                    let cx1 = if last_command_was_curve {
                        2.0 * current_x - last_control_x
                    } else {
                        current_x
                    };
                    let cy1 = if last_command_was_curve {
                        2.0 * current_y - last_control_y
                    } else {
                        current_y
                    };
                    let cx2 = current_x + *dx2 as f32 * scale_x;
                    let cy2 = current_y + *dy2 as f32 * scale_y;
                    let end_x = current_x + *dx as f32 * scale_x;
                    let end_y = current_y + *dy as f32 * scale_y;

                    approximate_cubic_bezier(&mut vertices, current_x, current_y, cx1, cy1, cx2, cy2, end_x, end_y);

                    last_control_x = cx2;
                    last_control_y = cy2;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::QuadTo { x1, y1, x, y } => {
                    let cx = *x1 as f32 * scale_x + offset_x;
                    let cy = *y1 as f32 * scale_y + offset_y;
                    let end_x = *x as f32 * scale_x + offset_x;
                    let end_y = *y as f32 * scale_y + offset_y;

                    approximate_quadratic_bezier(&mut vertices, current_x, current_y, cx, cy, end_x, end_y);

                    last_control_x = cx;
                    last_control_y = cy;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::QuadToRel { dx1, dy1, dx, dy } => {
                    let cx = current_x + *dx1 as f32 * scale_x;
                    let cy = current_y + *dy1 as f32 * scale_y;
                    let end_x = current_x + *dx as f32 * scale_x;
                    let end_y = current_y + *dy as f32 * scale_y;

                    approximate_quadratic_bezier(&mut vertices, current_x, current_y, cx, cy, end_x, end_y);

                    last_control_x = cx;
                    last_control_y = cy;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::SmoothQuadTo { x, y } => {
                    let cx = if last_command_was_curve {
                        2.0 * current_x - last_control_x
                    } else {
                        current_x
                    };
                    let cy = if last_command_was_curve {
                        2.0 * current_y - last_control_y
                    } else {
                        current_y
                    };
                    let end_x = *x as f32 * scale_x + offset_x;
                    let end_y = *y as f32 * scale_y + offset_y;

                    approximate_quadratic_bezier(&mut vertices, current_x, current_y, cx, cy, end_x, end_y);

                    last_control_x = cx;
                    last_control_y = cy;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::SmoothQuadToRel { dx, dy } => {
                    let cx = if last_command_was_curve {
                        2.0 * current_x - last_control_x
                    } else {
                        current_x
                    };
                    let cy = if last_command_was_curve {
                        2.0 * current_y - last_control_y
                    } else {
                        current_y
                    };
                    let end_x = current_x + *dx as f32 * scale_x;
                    let end_y = current_y + *dy as f32 * scale_y;

                    approximate_quadratic_bezier(&mut vertices, current_x, current_y, cx, cy, end_x, end_y);

                    last_control_x = cx;
                    last_control_y = cy;
                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = true;
                }
                SvgPathCommand::ArcTo { rx, ry, x_rotation, large_arc, sweep, x, y } => {
                    let end_x = *x as f32 * scale_x + offset_x;
                    let end_y = *y as f32 * scale_y + offset_y;

                    approximate_arc(&mut vertices, current_x, current_y,
                        *rx as f32 * scale_x, *ry as f32 * scale_y,
                        *x_rotation as f32, *large_arc, *sweep, end_x, end_y);

                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = false;
                }
                SvgPathCommand::ArcToRel { rx, ry, x_rotation, large_arc, sweep, dx, dy } => {
                    let end_x = current_x + *dx as f32 * scale_x;
                    let end_y = current_y + *dy as f32 * scale_y;

                    approximate_arc(&mut vertices, current_x, current_y,
                        *rx as f32 * scale_x, *ry as f32 * scale_y,
                        *x_rotation as f32, *large_arc, *sweep, end_x, end_y);

                    current_x = end_x;
                    current_y = end_y;
                    last_command_was_curve = false;
                }
                SvgPathCommand::ClosePath => {
                    // Close the path by connecting back to start
                    if (current_x - start_x).abs() > 0.001 || (current_y - start_y).abs() > 0.001 {
                        vertices.push(Vec2::new(start_x, start_y));
                    }
                    current_x = start_x;
                    current_y = start_y;
                    last_command_was_curve = false;
                }
            }
        }

        // Flush any remaining vertices
        if !vertices.is_empty() {
            self.flush_path_vertices(&vertices, path, default_fill, default_stroke);
        }
    }

    fn flush_path_vertices(
        &mut self,
        vertices: &[Vec2],
        path: &seed_core::ast::SvgPath,
        default_fill: Option<Fill>,
        default_stroke: Option<Stroke>,
    ) {
        if vertices.len() < 2 {
            return;
        }

        // Determine fill
        let fill = path.fill
            .map(Fill::Solid)
            .or(default_fill);

        // Determine stroke
        let stroke = if path.stroke.is_some() || path.stroke_width.is_some() {
            Some(Stroke::new(
                path.stroke.unwrap_or(Color::BLACK),
                path.stroke_width.unwrap_or(1.0) as f32,
            ))
        } else {
            default_stroke
        };

        // Add polygon to scene
        self.scene.polygon(vertices.to_vec(), fill, stroke);
    }
}

// Bezier curve approximation helpers

fn approximate_cubic_bezier(
    vertices: &mut Vec<Vec2>,
    x0: f32, y0: f32,
    x1: f32, y1: f32,
    x2: f32, y2: f32,
    x3: f32, y3: f32,
) {
    const SEGMENTS: usize = 16;
    for i in 1..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;

        let x = mt3 * x0 + 3.0 * mt2 * t * x1 + 3.0 * mt * t2 * x2 + t3 * x3;
        let y = mt3 * y0 + 3.0 * mt2 * t * y1 + 3.0 * mt * t2 * y2 + t3 * y3;
        vertices.push(Vec2::new(x, y));
    }
}

fn approximate_quadratic_bezier(
    vertices: &mut Vec<Vec2>,
    x0: f32, y0: f32,
    x1: f32, y1: f32,
    x2: f32, y2: f32,
) {
    const SEGMENTS: usize = 12;
    for i in 1..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let mt = 1.0 - t;

        let x = mt * mt * x0 + 2.0 * mt * t * x1 + t * t * x2;
        let y = mt * mt * y0 + 2.0 * mt * t * y1 + t * t * y2;
        vertices.push(Vec2::new(x, y));
    }
}

fn approximate_arc(
    vertices: &mut Vec<Vec2>,
    x0: f32, y0: f32,
    rx: f32, ry: f32,
    _x_rotation: f32,
    _large_arc: bool,
    _sweep: bool,
    x1: f32, y1: f32,
) {
    // Simplified arc approximation - just use line segments for now
    // A proper implementation would use the SVG arc parametrization
    const SEGMENTS: usize = 16;
    for i in 1..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let x = x0 + (x1 - x0) * t;
        let y = y0 + (y1 - y0) * t;
        // Add some curvature based on radii (simplified)
        let curve = 4.0 * t * (1.0 - t); // parabolic curve factor
        let offset_x = curve * rx * 0.5 * (y1 - y0).signum();
        let offset_y = curve * ry * 0.5 * (x0 - x1).signum();
        vertices.push(Vec2::new(x + offset_x, y + offset_y));
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
    use seed_core::types::Identifier;
    use seed_layout::{compute_layout, LayoutOptions};

    fn make_frame(name: &str, width: f64, height: f64, children: Vec<Element>) -> Element {
        Element::Frame(FrameElement {
            name: Some(Identifier(name.to_string())),
            properties: vec![
                Property {
                    name: "fill".to_string(),
                    value: PropertyValue::Color(Color::rgb(0.5, 0.5, 0.5)),
                    span: Span::default(),
                },
            ],
            constraints: vec![
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "width".to_string(),
                        value: Expression::Literal(width),
                    },
                    priority: None,
                    span: Span::default(),
                },
                Constraint {
                    kind: ConstraintKind::Equality {
                        property: "height".to_string(),
                        value: Expression::Literal(height),
                    },
                    priority: None,
                    span: Span::default(),
                },
            ],
            children,
            span: Span::default(),
        })
    }

    fn make_doc(elements: Vec<Element>) -> Document {
        Document {
            meta: None,
            tokens: None,
            elements,
            span: Span::default(),
        }
    }

    #[test]
    fn test_build_empty_scene() {
        let doc = make_doc(vec![]);
        let layout = LayoutTree::new();

        let scene = build_scene(&doc, &layout);
        assert!(scene.commands.is_empty());
    }

    #[test]
    fn test_build_single_frame_scene() {
        let doc = make_doc(vec![make_frame("root", 200.0, 100.0, vec![])]);
        let layout = compute_layout(&doc, &LayoutOptions::default()).unwrap();

        let scene = build_scene(&doc, &layout);

        // Should have exactly one rect command for the single frame
        let rect_count = scene.commands.iter().filter(|cmd| {
            matches!(cmd, RenderCommand::Rect(_) | RenderCommand::RoundedRect(_))
        }).count();
        assert_eq!(rect_count, 1, "Expected 1 rect command for single frame");
    }

    #[test]
    fn test_build_nested_frames_scene() {
        // Create parent with one child
        let child = make_frame("child", 80.0, 40.0, vec![]);
        let parent = make_frame("parent", 200.0, 100.0, vec![child]);

        let doc = make_doc(vec![parent]);
        let layout = compute_layout(&doc, &LayoutOptions::default()).unwrap();

        let scene = build_scene(&doc, &layout);

        // Should have exactly 2 rect commands: one for parent, one for child
        let rect_count = scene.commands.iter().filter(|cmd| {
            matches!(cmd, RenderCommand::Rect(_) | RenderCommand::RoundedRect(_))
        }).count();
        assert_eq!(rect_count, 2, "Expected 2 rect commands for parent + child");
    }

    #[test]
    fn test_build_multiple_children_scene() {
        // Create parent with multiple children
        let child1 = make_frame("child1", 50.0, 30.0, vec![]);
        let child2 = make_frame("child2", 50.0, 30.0, vec![]);
        let child3 = make_frame("child3", 50.0, 30.0, vec![]);
        let parent = make_frame("parent", 200.0, 100.0, vec![child1, child2, child3]);

        let doc = make_doc(vec![parent]);
        let layout = compute_layout(&doc, &LayoutOptions::default()).unwrap();

        let scene = build_scene(&doc, &layout);

        // Should have exactly 4 rect commands: parent + 3 children
        let rect_count = scene.commands.iter().filter(|cmd| {
            matches!(cmd, RenderCommand::Rect(_) | RenderCommand::RoundedRect(_))
        }).count();
        assert_eq!(rect_count, 4, "Expected 4 rect commands for parent + 3 children");
    }

    #[test]
    fn test_build_deeply_nested_frames_scene() {
        // Create a 4-level deep nesting: root > level1 > level2 > level3
        let level3 = make_frame("level3", 20.0, 20.0, vec![]);
        let level2 = make_frame("level2", 40.0, 40.0, vec![level3]);
        let level1 = make_frame("level1", 80.0, 80.0, vec![level2]);
        let root = make_frame("root", 200.0, 200.0, vec![level1]);

        let doc = make_doc(vec![root]);
        let layout = compute_layout(&doc, &LayoutOptions::default()).unwrap();

        let scene = build_scene(&doc, &layout);

        // Should have exactly 4 rect commands: one per level
        let rect_count = scene.commands.iter().filter(|cmd| {
            matches!(cmd, RenderCommand::Rect(_) | RenderCommand::RoundedRect(_))
        }).count();
        assert_eq!(rect_count, 4, "Expected 4 rect commands for 4 levels of nesting");
    }

    #[test]
    fn test_build_complex_tree_scene() {
        // Create a more complex tree:
        // root
        // ├── branch1
        // │   ├── leaf1a
        // │   └── leaf1b
        // └── branch2
        //     └── leaf2a
        let leaf1a = make_frame("leaf1a", 20.0, 20.0, vec![]);
        let leaf1b = make_frame("leaf1b", 20.0, 20.0, vec![]);
        let leaf2a = make_frame("leaf2a", 20.0, 20.0, vec![]);
        let branch1 = make_frame("branch1", 60.0, 50.0, vec![leaf1a, leaf1b]);
        let branch2 = make_frame("branch2", 60.0, 50.0, vec![leaf2a]);
        let root = make_frame("root", 200.0, 150.0, vec![branch1, branch2]);

        let doc = make_doc(vec![root]);
        let layout = compute_layout(&doc, &LayoutOptions::default()).unwrap();

        let scene = build_scene(&doc, &layout);

        // Should have exactly 6 rect commands: root + 2 branches + 3 leaves
        let rect_count = scene.commands.iter().filter(|cmd| {
            matches!(cmd, RenderCommand::Rect(_) | RenderCommand::RoundedRect(_))
        }).count();
        assert_eq!(rect_count, 6, "Expected 6 rect commands for complex tree");
    }
}
