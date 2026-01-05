//! Shape tessellation for 2D rendering.

use lyon::geom::point;
use lyon::path::Path;
use lyon::path::builder::BorderRadii;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex,
    StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use crate::primitives::{
    CornerRadius, EllipsePrimitive, Fill, PathCommand, PathPrimitive,
    RectPrimitive, RoundedRectPrimitive, Stroke,
};

/// A vertex for rendering.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(x: f32, y: f32, color: [f32; 4]) -> Self {
        Self {
            position: [x, y],
            color,
        }
    }
}

/// Tessellated mesh ready for rendering.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Tessellator for converting primitives to meshes.
pub struct Tessellator {
    fill_tessellator: FillTessellator,
    stroke_tessellator: StrokeTessellator,
}

impl Default for Tessellator {
    fn default() -> Self {
        Self::new()
    }
}

impl Tessellator {
    pub fn new() -> Self {
        Self {
            fill_tessellator: FillTessellator::new(),
            stroke_tessellator: StrokeTessellator::new(),
        }
    }

    /// Tessellate a rectangle.
    pub fn tessellate_rect(&mut self, rect: &RectPrimitive, mesh: &mut Mesh) {
        if let Some(ref fill) = rect.fill {
            self.tessellate_rect_fill(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                fill,
                mesh,
            );
        }

        if let Some(ref stroke) = rect.stroke {
            self.tessellate_rect_stroke(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                stroke,
                mesh,
            );
        }
    }

    fn tessellate_rect_fill(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: &Fill,
        mesh: &mut Mesh,
    ) {
        let base = mesh.vertices.len() as u32;

        // Compute color at each vertex for proper gradient interpolation
        let c0 = fill_color_at(fill, x, y);
        let c1 = fill_color_at(fill, x + width, y);
        let c2 = fill_color_at(fill, x + width, y + height);
        let c3 = fill_color_at(fill, x, y + height);

        mesh.vertices.push(Vertex::new(x, y, c0));
        mesh.vertices.push(Vertex::new(x + width, y, c1));
        mesh.vertices.push(Vertex::new(x + width, y + height, c2));
        mesh.vertices.push(Vertex::new(x, y + height, c3));

        mesh.indices.extend_from_slice(&[
            base,
            base + 1,
            base + 2,
            base,
            base + 2,
            base + 3,
        ]);
    }

    fn tessellate_rect_stroke(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        stroke: &Stroke,
        mesh: &mut Mesh,
    ) {
        let mut path_builder = Path::builder();
        path_builder.begin(point(x, y));
        path_builder.line_to(point(x + width, y));
        path_builder.line_to(point(x + width, y + height));
        path_builder.line_to(point(x, y + height));
        path_builder.close();
        let path = path_builder.build();

        self.tessellate_stroke(&path, stroke, mesh);
    }

    /// Tessellate a rounded rectangle.
    pub fn tessellate_rounded_rect(&mut self, rect: &RoundedRectPrimitive, mesh: &mut Mesh) {
        let path = build_rounded_rect_path(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            &rect.radius,
        );

        if let Some(ref fill) = rect.fill {
            self.tessellate_fill(&path, fill, mesh);
        }

        if let Some(ref stroke) = rect.stroke {
            self.tessellate_stroke(&path, stroke, mesh);
        }
    }

    /// Tessellate an ellipse.
    pub fn tessellate_ellipse(&mut self, ellipse: &EllipsePrimitive, mesh: &mut Mesh) {
        let path = build_ellipse_path(
            ellipse.center_x,
            ellipse.center_y,
            ellipse.radius_x,
            ellipse.radius_y,
        );

        if let Some(ref fill) = ellipse.fill {
            self.tessellate_fill(&path, fill, mesh);
        }

        if let Some(ref stroke) = ellipse.stroke {
            self.tessellate_stroke(&path, stroke, mesh);
        }
    }

    /// Tessellate a path.
    pub fn tessellate_path(&mut self, path_prim: &PathPrimitive, mesh: &mut Mesh) {
        let path = build_path_from_commands(&path_prim.commands);

        if let Some(ref fill) = path_prim.fill {
            self.tessellate_fill(&path, fill, mesh);
        }

        if let Some(ref stroke) = path_prim.stroke {
            self.tessellate_stroke(&path, stroke, mesh);
        }
    }

    fn tessellate_fill(&mut self, path: &Path, fill: &Fill, mesh: &mut Mesh) {
        // Clone fill for closure
        let fill_ref = fill.clone();

        let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();

        let result = self.fill_tessellator.tessellate_path(
            path,
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
                let x = vertex.position().x;
                let y = vertex.position().y;
                let color = fill_color_at(&fill_ref, x, y);
                Vertex::new(x, y, color)
            }),
        );

        if result.is_ok() {
            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend(buffers.vertices);
            mesh.indices.extend(buffers.indices.iter().map(|i| i + base));
        }
    }

    fn tessellate_stroke(&mut self, path: &Path, stroke: &Stroke, mesh: &mut Mesh) {
        let color = [stroke.color.r, stroke.color.g, stroke.color.b, stroke.color.a];

        let mut buffers: VertexBuffers<Vertex, u32> = VertexBuffers::new();

        let options = StrokeOptions::default()
            .with_line_width(stroke.width)
            .with_line_cap(convert_line_cap(stroke.line_cap))
            .with_line_join(convert_line_join(stroke.line_join));

        let result = self.stroke_tessellator.tessellate_path(
            path,
            &options,
            &mut BuffersBuilder::new(&mut buffers, |vertex: StrokeVertex| {
                Vertex::new(vertex.position().x, vertex.position().y, color)
            }),
        );

        if result.is_ok() {
            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend(buffers.vertices);
            mesh.indices.extend(buffers.indices.iter().map(|i| i + base));
        }
    }
}

fn build_rounded_rect_path(x: f32, y: f32, width: f32, height: f32, radius: &CornerRadius) -> Path {
    let mut builder = Path::builder();

    let radii = BorderRadii {
        top_left: radius.top_left,
        top_right: radius.top_right,
        bottom_left: radius.bottom_left,
        bottom_right: radius.bottom_right,
    };

    let rect = lyon::geom::Box2D::new(
        point(x, y),
        point(x + width, y + height),
    );

    builder.add_rounded_rectangle(&rect, &radii, lyon::path::Winding::Positive);
    builder.build()
}

fn build_ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
    let mut builder = Path::builder();

    let center = point(cx, cy);
    let radii = lyon::geom::vector(rx, ry);

    builder.add_ellipse(center, radii, lyon::geom::Angle::zero(), lyon::path::Winding::Positive);
    builder.build()
}

fn build_path_from_commands(commands: &[PathCommand]) -> Path {
    let mut builder = Path::builder();

    for cmd in commands {
        match cmd {
            PathCommand::MoveTo(p) => {
                builder.begin(point(p.x, p.y));
            }
            PathCommand::LineTo(p) => {
                builder.line_to(point(p.x, p.y));
            }
            PathCommand::QuadTo { control, end } => {
                builder.quadratic_bezier_to(
                    point(control.x, control.y),
                    point(end.x, end.y),
                );
            }
            PathCommand::CubicTo { control1, control2, end } => {
                builder.cubic_bezier_to(
                    point(control1.x, control1.y),
                    point(control2.x, control2.y),
                    point(end.x, end.y),
                );
            }
            PathCommand::Close => {
                builder.close();
            }
        }
    }

    builder.build()
}

fn fill_to_color(fill: &Fill) -> [f32; 4] {
    match fill {
        Fill::Solid(c) => [c.r, c.g, c.b, c.a],
        Fill::LinearGradient(g) => {
            // For solid color fallback, use the first stop color
            g.stops.first()
                .map(|s| [s.color.r, s.color.g, s.color.b, s.color.a])
                .unwrap_or([1.0, 1.0, 1.0, 1.0])
        }
        Fill::RadialGradient(g) => {
            g.stops.first()
                .map(|s| [s.color.r, s.color.g, s.color.b, s.color.a])
                .unwrap_or([1.0, 1.0, 1.0, 1.0])
        }
    }
}

/// Sample a linear gradient at position t (0.0 to 1.0).
fn sample_linear_gradient(gradient: &crate::primitives::LinearGradient, t: f32) -> [f32; 4] {
    sample_gradient_stops(&gradient.stops, t)
}

/// Sample a radial gradient at distance t from center (0.0 to 1.0).
fn sample_radial_gradient(gradient: &crate::primitives::RadialGradient, t: f32) -> [f32; 4] {
    sample_gradient_stops(&gradient.stops, t)
}

/// Sample gradient stops at position t (0.0 to 1.0).
fn sample_gradient_stops(stops: &[crate::primitives::GradientStop], t: f32) -> [f32; 4] {
    if stops.is_empty() {
        return [1.0, 1.0, 1.0, 1.0];
    }
    if stops.len() == 1 {
        let c = &stops[0].color;
        return [c.r, c.g, c.b, c.a];
    }

    let t = t.clamp(0.0, 1.0);

    // Find surrounding stops
    let mut prev = &stops[0];
    for stop in stops.iter() {
        if stop.offset >= t {
            if stop.offset == prev.offset {
                let c = &stop.color;
                return [c.r, c.g, c.b, c.a];
            }
            // Interpolate between prev and stop
            let local_t = (t - prev.offset) / (stop.offset - prev.offset);
            return lerp_color(&prev.color, &stop.color, local_t);
        }
        prev = stop;
    }

    // Past the last stop
    let c = &stops.last().unwrap().color;
    [c.r, c.g, c.b, c.a]
}

/// Linear interpolation between two colors.
fn lerp_color(a: &seed_core::types::Color, b: &seed_core::types::Color, t: f32) -> [f32; 4] {
    [
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    ]
}

/// Calculate the gradient t value for a point given a linear gradient.
fn linear_gradient_t(gradient: &crate::primitives::LinearGradient, x: f32, y: f32) -> f32 {
    let dx = gradient.end.x - gradient.start.x;
    let dy = gradient.end.y - gradient.start.y;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.0001 {
        return 0.0;
    }
    let px = x - gradient.start.x;
    let py = y - gradient.start.y;
    ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
}

/// Calculate the gradient t value for a point given a radial gradient.
fn radial_gradient_t(gradient: &crate::primitives::RadialGradient, x: f32, y: f32) -> f32 {
    let dx = x - gradient.center.x;
    let dy = y - gradient.center.y;
    let dist = (dx * dx + dy * dy).sqrt();
    (dist / gradient.radius).clamp(0.0, 1.0)
}

/// Get the color at a specific position for a fill.
fn fill_color_at(fill: &Fill, x: f32, y: f32) -> [f32; 4] {
    match fill {
        Fill::Solid(c) => [c.r, c.g, c.b, c.a],
        Fill::LinearGradient(g) => {
            let t = linear_gradient_t(g, x, y);
            sample_linear_gradient(g, t)
        }
        Fill::RadialGradient(g) => {
            let t = radial_gradient_t(g, x, y);
            sample_radial_gradient(g, t)
        }
    }
}

fn convert_line_cap(cap: crate::primitives::LineCap) -> lyon::tessellation::LineCap {
    match cap {
        crate::primitives::LineCap::Butt => lyon::tessellation::LineCap::Butt,
        crate::primitives::LineCap::Round => lyon::tessellation::LineCap::Round,
        crate::primitives::LineCap::Square => lyon::tessellation::LineCap::Square,
    }
}

fn convert_line_join(join: crate::primitives::LineJoin) -> lyon::tessellation::LineJoin {
    match join {
        crate::primitives::LineJoin::Miter => lyon::tessellation::LineJoin::Miter,
        crate::primitives::LineJoin::Round => lyon::tessellation::LineJoin::Round,
        crate::primitives::LineJoin::Bevel => lyon::tessellation::LineJoin::Bevel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::types::Color;
    use glam::Vec2;
    use crate::primitives::{GradientStop, LinearGradient, RadialGradient};

    #[test]
    fn test_tessellate_rect() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let rect = RectPrimitive::new(0.0, 0.0, 100.0, 50.0)
            .with_fill(Fill::Solid(Color::rgb(1.0, 0.0, 0.0)));

        tessellator.tessellate_rect(&rect, &mut mesh);

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn test_tessellate_rounded_rect() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let rect = RoundedRectPrimitive::new(0.0, 0.0, 100.0, 50.0, 10.0)
            .with_fill(Fill::Solid(Color::rgb(0.0, 1.0, 0.0)));

        tessellator.tessellate_rounded_rect(&rect, &mut mesh);

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_tessellate_ellipse() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let ellipse = EllipsePrimitive::circle(50.0, 50.0, 25.0)
            .with_fill(Fill::Solid(Color::rgb(0.0, 0.0, 1.0)));

        tessellator.tessellate_ellipse(&ellipse, &mut mesh);

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    // Gradient sampling tests

    #[test]
    fn test_sample_gradient_stops_empty() {
        let stops: Vec<GradientStop> = vec![];
        let color = sample_gradient_stops(&stops, 0.5);
        // Empty stops should return white
        assert_eq!(color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_sample_gradient_stops_single() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
        ];
        let color = sample_gradient_stops(&stops, 0.5);
        // Single stop should return that color regardless of t
        assert!((color[0] - 1.0).abs() < 0.001);
        assert!((color[1] - 0.0).abs() < 0.001);
        assert!((color[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_sample_gradient_stops_two_stops_at_start() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
            GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
        ];
        let color = sample_gradient_stops(&stops, 0.0);
        // At t=0, should be red
        assert!((color[0] - 1.0).abs() < 0.001);
        assert!((color[1] - 0.0).abs() < 0.001);
        assert!((color[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_sample_gradient_stops_two_stops_at_end() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
            GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
        ];
        let color = sample_gradient_stops(&stops, 1.0);
        // At t=1, should be blue
        assert!((color[0] - 0.0).abs() < 0.001);
        assert!((color[1] - 0.0).abs() < 0.001);
        assert!((color[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sample_gradient_stops_two_stops_midpoint() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
            GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
        ];
        let color = sample_gradient_stops(&stops, 0.5);
        // At t=0.5, should be 50% red + 50% blue = purple
        assert!((color[0] - 0.5).abs() < 0.001);
        assert!((color[1] - 0.0).abs() < 0.001);
        assert!((color[2] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_sample_gradient_stops_three_stops() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
            GradientStop { offset: 0.5, color: Color::rgb(0.0, 1.0, 0.0) },
            GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
        ];

        // At t=0.25, should be between red and green
        let color = sample_gradient_stops(&stops, 0.25);
        assert!((color[0] - 0.5).abs() < 0.001);
        assert!((color[1] - 0.5).abs() < 0.001);
        assert!((color[2] - 0.0).abs() < 0.001);

        // At t=0.5, should be green
        let color = sample_gradient_stops(&stops, 0.5);
        assert!((color[0] - 0.0).abs() < 0.001);
        assert!((color[1] - 1.0).abs() < 0.001);
        assert!((color[2] - 0.0).abs() < 0.001);

        // At t=0.75, should be between green and blue
        let color = sample_gradient_stops(&stops, 0.75);
        assert!((color[0] - 0.0).abs() < 0.001);
        assert!((color[1] - 0.5).abs() < 0.001);
        assert!((color[2] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_sample_gradient_stops_clamps_below_zero() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
            GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
        ];
        let color = sample_gradient_stops(&stops, -0.5);
        // Below 0 should clamp to first stop
        assert!((color[0] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_sample_gradient_stops_clamps_above_one() {
        let stops = vec![
            GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
            GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
        ];
        let color = sample_gradient_stops(&stops, 1.5);
        // Above 1 should clamp to last stop
        assert!((color[2] - 1.0).abs() < 0.001);
    }

    // Linear gradient tests

    #[test]
    fn test_linear_gradient_t_horizontal() {
        let gradient = LinearGradient {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 0.0),
            stops: vec![],
        };

        assert!((linear_gradient_t(&gradient, 0.0, 0.0) - 0.0).abs() < 0.001);
        assert!((linear_gradient_t(&gradient, 50.0, 0.0) - 0.5).abs() < 0.001);
        assert!((linear_gradient_t(&gradient, 100.0, 0.0) - 1.0).abs() < 0.001);
        // Y position shouldn't matter for horizontal gradient
        assert!((linear_gradient_t(&gradient, 50.0, 100.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_linear_gradient_t_vertical() {
        let gradient = LinearGradient {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(0.0, 100.0),
            stops: vec![],
        };

        assert!((linear_gradient_t(&gradient, 0.0, 0.0) - 0.0).abs() < 0.001);
        assert!((linear_gradient_t(&gradient, 0.0, 50.0) - 0.5).abs() < 0.001);
        assert!((linear_gradient_t(&gradient, 0.0, 100.0) - 1.0).abs() < 0.001);
        // X position shouldn't matter for vertical gradient
        assert!((linear_gradient_t(&gradient, 100.0, 50.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_linear_gradient_t_diagonal() {
        let gradient = LinearGradient {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 100.0),
            stops: vec![],
        };

        assert!((linear_gradient_t(&gradient, 0.0, 0.0) - 0.0).abs() < 0.001);
        assert!((linear_gradient_t(&gradient, 50.0, 50.0) - 0.5).abs() < 0.001);
        assert!((linear_gradient_t(&gradient, 100.0, 100.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_linear_gradient_t_clamps() {
        let gradient = LinearGradient {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 0.0),
            stops: vec![],
        };

        // Before start should clamp to 0
        assert!((linear_gradient_t(&gradient, -50.0, 0.0) - 0.0).abs() < 0.001);
        // After end should clamp to 1
        assert!((linear_gradient_t(&gradient, 150.0, 0.0) - 1.0).abs() < 0.001);
    }

    // Radial gradient tests

    #[test]
    fn test_radial_gradient_t_at_center() {
        let gradient = RadialGradient {
            center: Vec2::new(50.0, 50.0),
            radius: 50.0,
            stops: vec![],
        };

        assert!((radial_gradient_t(&gradient, 50.0, 50.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_radial_gradient_t_at_edge() {
        let gradient = RadialGradient {
            center: Vec2::new(50.0, 50.0),
            radius: 50.0,
            stops: vec![],
        };

        // At radius distance from center
        assert!((radial_gradient_t(&gradient, 100.0, 50.0) - 1.0).abs() < 0.001);
        assert!((radial_gradient_t(&gradient, 50.0, 100.0) - 1.0).abs() < 0.001);
        assert!((radial_gradient_t(&gradient, 0.0, 50.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_radial_gradient_t_halfway() {
        let gradient = RadialGradient {
            center: Vec2::new(50.0, 50.0),
            radius: 50.0,
            stops: vec![],
        };

        // Halfway to edge
        assert!((radial_gradient_t(&gradient, 75.0, 50.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_radial_gradient_t_clamps() {
        let gradient = RadialGradient {
            center: Vec2::new(50.0, 50.0),
            radius: 50.0,
            stops: vec![],
        };

        // Beyond radius should clamp to 1
        assert!((radial_gradient_t(&gradient, 150.0, 50.0) - 1.0).abs() < 0.001);
    }

    // fill_color_at tests

    #[test]
    fn test_fill_color_at_solid() {
        let fill = Fill::Solid(Color::rgba(0.5, 0.6, 0.7, 0.8));
        let color = fill_color_at(&fill, 0.0, 0.0);

        assert!((color[0] - 0.5).abs() < 0.001);
        assert!((color[1] - 0.6).abs() < 0.001);
        assert!((color[2] - 0.7).abs() < 0.001);
        assert!((color[3] - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_fill_color_at_linear_gradient() {
        let fill = Fill::LinearGradient(LinearGradient {
            start: Vec2::new(0.0, 0.0),
            end: Vec2::new(100.0, 0.0),
            stops: vec![
                GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
                GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
            ],
        });

        // At start, should be red
        let color = fill_color_at(&fill, 0.0, 0.0);
        assert!((color[0] - 1.0).abs() < 0.001);
        assert!((color[2] - 0.0).abs() < 0.001);

        // At middle, should be purple
        let color = fill_color_at(&fill, 50.0, 0.0);
        assert!((color[0] - 0.5).abs() < 0.001);
        assert!((color[2] - 0.5).abs() < 0.001);

        // At end, should be blue
        let color = fill_color_at(&fill, 100.0, 0.0);
        assert!((color[0] - 0.0).abs() < 0.001);
        assert!((color[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_fill_color_at_radial_gradient() {
        let fill = Fill::RadialGradient(RadialGradient {
            center: Vec2::new(50.0, 50.0),
            radius: 50.0,
            stops: vec![
                GradientStop { offset: 0.0, color: Color::rgb(1.0, 1.0, 0.0) },
                GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 1.0) },
            ],
        });

        // At center, should be yellow
        let color = fill_color_at(&fill, 50.0, 50.0);
        assert!((color[0] - 1.0).abs() < 0.001);
        assert!((color[1] - 1.0).abs() < 0.001);
        assert!((color[2] - 0.0).abs() < 0.001);

        // At edge, should be blue
        let color = fill_color_at(&fill, 100.0, 50.0);
        assert!((color[0] - 0.0).abs() < 0.001);
        assert!((color[1] - 0.0).abs() < 0.001);
        assert!((color[2] - 1.0).abs() < 0.001);
    }

    // Tessellation with gradient tests

    #[test]
    fn test_tessellate_rect_with_linear_gradient() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let gradient = LinearGradient::new(Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0))
            .add_stop(0.0, Color::rgb(1.0, 0.0, 0.0))
            .add_stop(1.0, Color::rgb(0.0, 0.0, 1.0));

        let rect = RectPrimitive::new(0.0, 0.0, 100.0, 50.0)
            .with_fill(Fill::LinearGradient(gradient));

        tessellator.tessellate_rect(&rect, &mut mesh);

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);

        // Check that vertices have varying colors based on position
        // Left vertices should be more red
        // Right vertices should be more blue
        let left_vertices: Vec<_> = mesh.vertices.iter()
            .filter(|v| v.position[0] < 50.0)
            .collect();
        let right_vertices: Vec<_> = mesh.vertices.iter()
            .filter(|v| v.position[0] > 50.0)
            .collect();

        assert!(!left_vertices.is_empty());
        assert!(!right_vertices.is_empty());

        // Left should be redder than right
        let left_red: f32 = left_vertices.iter().map(|v| v.color[0]).sum::<f32>() / left_vertices.len() as f32;
        let right_red: f32 = right_vertices.iter().map(|v| v.color[0]).sum::<f32>() / right_vertices.len() as f32;
        assert!(left_red > right_red, "Left side should be more red");
    }

    #[test]
    fn test_tessellate_rect_with_radial_gradient() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let gradient = RadialGradient {
            center: Vec2::new(50.0, 25.0),
            radius: 50.0,
            stops: vec![
                GradientStop { offset: 0.0, color: Color::rgb(1.0, 1.0, 1.0) },
                GradientStop { offset: 1.0, color: Color::rgb(0.0, 0.0, 0.0) },
            ],
        };

        let rect = RectPrimitive::new(0.0, 0.0, 100.0, 50.0)
            .with_fill(Fill::RadialGradient(gradient));

        tessellator.tessellate_rect(&rect, &mut mesh);

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
    }

    #[test]
    fn test_tessellate_rounded_rect_with_gradient() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let gradient = LinearGradient::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, 100.0))
            .add_stop(0.0, Color::rgb(1.0, 0.5, 0.0))
            .add_stop(1.0, Color::rgb(0.5, 0.0, 1.0));

        let rect = RoundedRectPrimitive::new(0.0, 0.0, 100.0, 100.0, 20.0)
            .with_fill(Fill::LinearGradient(gradient));

        tessellator.tessellate_rounded_rect(&rect, &mut mesh);

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_tessellate_ellipse_with_radial_gradient() {
        let mut tessellator = Tessellator::new();
        let mut mesh = Mesh::new();

        let gradient = RadialGradient {
            center: Vec2::new(50.0, 50.0),
            radius: 40.0,
            stops: vec![
                GradientStop { offset: 0.0, color: Color::rgb(1.0, 0.0, 0.0) },
                GradientStop { offset: 0.5, color: Color::rgb(1.0, 1.0, 0.0) },
                GradientStop { offset: 1.0, color: Color::rgb(0.0, 1.0, 0.0) },
            ],
        };

        let ellipse = EllipsePrimitive::circle(50.0, 50.0, 40.0)
            .with_fill(Fill::RadialGradient(gradient));

        tessellator.tessellate_ellipse(&ellipse, &mut mesh);

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_lerp_color() {
        let a = Color::rgb(1.0, 0.0, 0.0);
        let b = Color::rgb(0.0, 0.0, 1.0);

        let mid = lerp_color(&a, &b, 0.5);
        assert!((mid[0] - 0.5).abs() < 0.001);
        assert!((mid[1] - 0.0).abs() < 0.001);
        assert!((mid[2] - 0.5).abs() < 0.001);

        let start = lerp_color(&a, &b, 0.0);
        assert!((start[0] - 1.0).abs() < 0.001);

        let end = lerp_color(&a, &b, 1.0);
        assert!((end[2] - 1.0).abs() < 0.001);
    }
}
