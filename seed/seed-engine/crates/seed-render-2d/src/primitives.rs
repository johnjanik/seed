//! Render primitives for 2D rendering.
//!
//! These are intermediate representations that get tessellated and rendered.

use glam::Vec2;
use seed_core::types::Color;

/// A render command representing something to draw.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Draw a filled rectangle
    Rect(RectPrimitive),
    /// Draw a rounded rectangle
    RoundedRect(RoundedRectPrimitive),
    /// Draw an ellipse/circle
    Ellipse(EllipsePrimitive),
    /// Draw a path
    Path(PathPrimitive),
    /// Draw text
    Text(TextPrimitive),
    /// Draw a shadow
    Shadow(ShadowPrimitive),
    /// Push a clip region
    PushClip(ClipRegion),
    /// Pop clip region
    PopClip,
    /// Set opacity for subsequent commands
    SetOpacity(f32),
}

/// A shadow primitive.
#[derive(Debug, Clone)]
pub struct ShadowPrimitive {
    /// The shape the shadow is cast from
    pub shape: ShadowShape,
    /// Horizontal offset
    pub offset_x: f32,
    /// Vertical offset
    pub offset_y: f32,
    /// Blur radius
    pub blur: f32,
    /// Spread radius
    pub spread: f32,
    /// Shadow color
    pub color: Color,
    /// Whether this is an inner shadow
    pub inset: bool,
}

/// Shape that a shadow is cast from.
#[derive(Debug, Clone)]
pub enum ShadowShape {
    /// Rectangle with optional corner radius
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: CornerRadius,
    },
    /// Ellipse
    Ellipse {
        center_x: f32,
        center_y: f32,
        radius_x: f32,
        radius_y: f32,
    },
}

impl ShadowPrimitive {
    /// Create a new drop shadow for a rectangle.
    pub fn rect(
        x: f32, y: f32, width: f32, height: f32,
        offset_x: f32, offset_y: f32, blur: f32, color: Color,
    ) -> Self {
        Self {
            shape: ShadowShape::Rect {
                x, y, width, height,
                radius: CornerRadius::uniform(0.0),
            },
            offset_x,
            offset_y,
            blur,
            spread: 0.0,
            color,
            inset: false,
        }
    }

    /// Create a new shadow with all parameters.
    pub fn new(
        shape: ShadowShape,
        offset_x: f32, offset_y: f32,
        blur: f32, spread: f32,
        color: Color, inset: bool,
    ) -> Self {
        Self {
            shape, offset_x, offset_y, blur, spread, color, inset,
        }
    }
}

/// A rectangle primitive.
#[derive(Debug, Clone)]
pub struct RectPrimitive {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

impl RectPrimitive {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            fill: None,
            stroke: None,
        }
    }

    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }
}

/// A rounded rectangle primitive.
#[derive(Debug, Clone)]
pub struct RoundedRectPrimitive {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub radius: CornerRadius,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

impl RoundedRectPrimitive {
    pub fn new(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            radius: CornerRadius::uniform(radius),
            fill: None,
            stroke: None,
        }
    }

    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }
}

/// Corner radius configuration.
#[derive(Debug, Clone, Copy)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadius {
    pub fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.top_left <= 0.0
            && self.top_right <= 0.0
            && self.bottom_right <= 0.0
            && self.bottom_left <= 0.0
    }
}

/// An ellipse/circle primitive.
#[derive(Debug, Clone)]
pub struct EllipsePrimitive {
    pub center_x: f32,
    pub center_y: f32,
    pub radius_x: f32,
    pub radius_y: f32,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

impl EllipsePrimitive {
    pub fn circle(center_x: f32, center_y: f32, radius: f32) -> Self {
        Self {
            center_x,
            center_y,
            radius_x: radius,
            radius_y: radius,
            fill: None,
            stroke: None,
        }
    }

    pub fn ellipse(center_x: f32, center_y: f32, radius_x: f32, radius_y: f32) -> Self {
        Self {
            center_x,
            center_y,
            radius_x,
            radius_y,
            fill: None,
            stroke: None,
        }
    }

    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }
}

/// A path primitive (bezier curves, lines, etc.).
#[derive(Debug, Clone)]
pub struct PathPrimitive {
    pub commands: Vec<PathCommand>,
    pub fill: Option<Fill>,
    pub stroke: Option<Stroke>,
}

/// Path command for building paths.
#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    MoveTo(Vec2),
    LineTo(Vec2),
    QuadTo { control: Vec2, end: Vec2 },
    CubicTo { control1: Vec2, control2: Vec2, end: Vec2 },
    Close,
}

impl PathPrimitive {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            fill: None,
            stroke: None,
        }
    }

    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::MoveTo(Vec2::new(x, y)));
        self
    }

    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::LineTo(Vec2::new(x, y)));
        self
    }

    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }

    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }
}

impl Default for PathPrimitive {
    fn default() -> Self {
        Self::new()
    }
}

/// A text primitive.
#[derive(Debug, Clone)]
pub struct TextPrimitive {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size: f32,
    pub font_family: String,
    pub color: Color,
    pub max_width: Option<f32>,
}

impl TextPrimitive {
    pub fn new(x: f32, y: f32, text: impl Into<String>) -> Self {
        Self {
            x,
            y,
            text: text.into(),
            font_size: 16.0,
            font_family: "sans-serif".to_string(),
            color: Color::rgb(0.0, 0.0, 0.0),
            max_width: None,
        }
    }

    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

/// Fill style.
#[derive(Debug, Clone)]
pub enum Fill {
    Solid(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
}

impl From<Color> for Fill {
    fn from(color: Color) -> Self {
        Fill::Solid(color)
    }
}

/// Linear gradient.
#[derive(Debug, Clone)]
pub struct LinearGradient {
    pub start: Vec2,
    pub end: Vec2,
    pub stops: Vec<GradientStop>,
}

impl LinearGradient {
    pub fn new(start: Vec2, end: Vec2) -> Self {
        Self {
            start,
            end,
            stops: Vec::new(),
        }
    }

    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        self.stops.push(GradientStop { offset, color });
        self
    }
}

/// Radial gradient.
#[derive(Debug, Clone)]
pub struct RadialGradient {
    pub center: Vec2,
    pub radius: f32,
    pub stops: Vec<GradientStop>,
}

/// A gradient color stop.
#[derive(Debug, Clone)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

/// Stroke style.
#[derive(Debug, Clone)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
}

impl Stroke {
    pub fn new(color: Color, width: f32) -> Self {
        Self {
            color,
            width,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
        }
    }
}

/// Line cap style.
#[derive(Debug, Clone, Copy, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

/// Line join style.
#[derive(Debug, Clone, Copy, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Clip region.
#[derive(Debug, Clone)]
pub struct ClipRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A scene containing all render commands.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub commands: Vec<RenderCommand>,
    pub width: f32,
    pub height: f32,
}

impl Scene {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            commands: Vec::new(),
            width,
            height,
        }
    }

    pub fn push(&mut self, command: RenderCommand) {
        self.commands.push(command);
    }

    pub fn rect(&mut self, rect: RectPrimitive) {
        self.commands.push(RenderCommand::Rect(rect));
    }

    pub fn rounded_rect(&mut self, rect: RoundedRectPrimitive) {
        self.commands.push(RenderCommand::RoundedRect(rect));
    }

    pub fn ellipse(&mut self, ellipse: EllipsePrimitive) {
        self.commands.push(RenderCommand::Ellipse(ellipse));
    }

    pub fn text(&mut self, text: TextPrimitive) {
        self.commands.push(RenderCommand::Text(text));
    }

    pub fn shadow(&mut self, shadow: ShadowPrimitive) {
        self.commands.push(RenderCommand::Shadow(shadow));
    }

    pub fn push_clip(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.commands.push(RenderCommand::PushClip(ClipRegion {
            x,
            y,
            width,
            height,
        }));
    }

    pub fn pop_clip(&mut self) {
        self.commands.push(RenderCommand::PopClip);
    }
}
