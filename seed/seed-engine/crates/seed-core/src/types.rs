//! Core value types for the Seed language.

/// An identifier (element name, property name, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Identifier(pub String);

impl From<&str> for Identifier {
    fn from(s: &str) -> Self {
        Identifier(s.to_string())
    }
}

impl From<String> for Identifier {
    fn from(s: String) -> Self {
        Identifier(s)
    }
}

/// A length value with unit.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Length {
    pub value: f64,
    pub unit: LengthUnit,
}

impl Length {
    pub fn px(value: f64) -> Self {
        Self { value, unit: LengthUnit::Px }
    }

    pub fn mm(value: f64) -> Self {
        Self { value, unit: LengthUnit::Mm }
    }

    pub fn percent(value: f64) -> Self {
        Self { value, unit: LengthUnit::Percent }
    }

    /// Convert to pixels (assuming 96 DPI for physical units).
    pub fn to_px(&self, parent_px: Option<f64>) -> Option<f64> {
        match self.unit {
            LengthUnit::Px => Some(self.value),
            LengthUnit::Pt => Some(self.value * 96.0 / 72.0),
            LengthUnit::Mm => Some(self.value * 96.0 / 25.4),
            LengthUnit::Cm => Some(self.value * 96.0 / 2.54),
            LengthUnit::In => Some(self.value * 96.0),
            LengthUnit::Percent => parent_px.map(|p| p * self.value / 100.0),
            LengthUnit::Em => None, // Requires font context
            LengthUnit::Rem => None, // Requires root font context
        }
    }

    /// Convert to millimeters.
    pub fn to_mm(&self) -> Option<f64> {
        match self.unit {
            LengthUnit::Mm => Some(self.value),
            LengthUnit::Cm => Some(self.value * 10.0),
            LengthUnit::In => Some(self.value * 25.4),
            LengthUnit::Px => Some(self.value * 25.4 / 96.0),
            LengthUnit::Pt => Some(self.value * 25.4 / 72.0),
            LengthUnit::Percent | LengthUnit::Em | LengthUnit::Rem => None,
        }
    }
}

/// Length units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LengthUnit {
    /// Pixels (screen units)
    Px,
    /// Points (1/72 inch)
    Pt,
    /// Millimeters
    Mm,
    /// Centimeters
    Cm,
    /// Inches
    In,
    /// Percentage of parent
    Percent,
    /// Relative to font size
    Em,
    /// Relative to root font size
    Rem,
}

/// A color value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from 8-bit RGB values.
    pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Create from hex string (e.g., "#FF5733" or "FF5733").
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Self::from_rgb8(r, g, b))
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Self::rgba(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ))
        } else {
            None
        }
    }

    /// Convert to 8-bit RGBA tuple.
    pub fn to_rgba8(&self) -> (u8, u8, u8, u8) {
        (
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }

    /// Convert to hex string (e.g., "#FF5733").
    pub fn to_hex(&self) -> String {
        let (r, g, b, a) = self.to_rgba8();
        if a == 255 {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
        }
    }

    // Common colors
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Unique identifier for elements (used internally after parsing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ElementId(pub u64);

/// Unique identifier for tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenId(pub u64);

/// Unique identifier for constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConstraintId(pub u64);

/// A gradient fill.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gradient {
    Linear(LinearGradient),
    Radial(RadialGradient),
    Conic(ConicGradient),
}

/// A linear gradient.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LinearGradient {
    /// Angle in degrees (0 = right, 90 = up, 180 = left, 270 = down)
    pub angle: f64,
    /// Color stops
    pub stops: Vec<GradientStop>,
}

impl LinearGradient {
    /// Create a horizontal gradient (left to right).
    pub fn horizontal(stops: Vec<GradientStop>) -> Self {
        Self { angle: 0.0, stops }
    }

    /// Create a vertical gradient (top to bottom).
    pub fn vertical(stops: Vec<GradientStop>) -> Self {
        Self { angle: 270.0, stops }
    }

    /// Create a gradient with an angle.
    pub fn with_angle(angle: f64, stops: Vec<GradientStop>) -> Self {
        Self { angle, stops }
    }

    /// Sample the gradient at position t (0.0 to 1.0).
    pub fn sample(&self, t: f64) -> Color {
        sample_gradient(&self.stops, t)
    }
}

/// A radial gradient.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RadialGradient {
    /// Center X position (0.0 to 1.0, relative to bounds)
    pub center_x: f64,
    /// Center Y position (0.0 to 1.0, relative to bounds)
    pub center_y: f64,
    /// Radius X (1.0 = extend to edge)
    pub radius_x: f64,
    /// Radius Y (1.0 = extend to edge, same as radius_x for circles)
    pub radius_y: f64,
    /// Color stops
    pub stops: Vec<GradientStop>,
}

impl RadialGradient {
    /// Create a centered circular gradient.
    pub fn circle(stops: Vec<GradientStop>) -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            radius_x: 1.0,
            radius_y: 1.0,
            stops,
        }
    }

    /// Create a gradient with custom center.
    pub fn with_center(center_x: f64, center_y: f64, stops: Vec<GradientStop>) -> Self {
        Self {
            center_x,
            center_y,
            radius_x: 1.0,
            radius_y: 1.0,
            stops,
        }
    }

    /// Sample the gradient at distance t from center (0.0 to 1.0).
    pub fn sample(&self, t: f64) -> Color {
        sample_gradient(&self.stops, t)
    }
}

/// A conic (angular) gradient.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConicGradient {
    /// Center X position (0.0 to 1.0, relative to bounds)
    pub center_x: f64,
    /// Center Y position (0.0 to 1.0, relative to bounds)
    pub center_y: f64,
    /// Starting angle in degrees
    pub start_angle: f64,
    /// Color stops
    pub stops: Vec<GradientStop>,
}

impl ConicGradient {
    /// Create a centered conic gradient.
    pub fn centered(stops: Vec<GradientStop>) -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            start_angle: 0.0,
            stops,
        }
    }

    /// Sample the gradient at angle t (0.0 to 1.0 = 0 to 360 degrees).
    pub fn sample(&self, t: f64) -> Color {
        sample_gradient(&self.stops, t)
    }
}

/// A color stop in a gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientStop {
    /// Position along the gradient (0.0 to 1.0)
    pub position: f64,
    /// Color at this position
    pub color: Color,
}

/// A shadow effect (drop shadow or inner shadow).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shadow {
    /// Horizontal offset (positive = right)
    pub offset_x: f64,
    /// Vertical offset (positive = down)
    pub offset_y: f64,
    /// Blur radius (0 = sharp edge)
    pub blur: f64,
    /// Spread radius (positive = larger shadow, negative = smaller)
    pub spread: f64,
    /// Shadow color
    pub color: Color,
    /// Whether this is an inner shadow (inset)
    pub inset: bool,
}

impl Shadow {
    /// Create a new drop shadow.
    pub fn drop(offset_x: f64, offset_y: f64, blur: f64, color: Color) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            spread: 0.0,
            color,
            inset: false,
        }
    }

    /// Create a new inner shadow.
    pub fn inner(offset_x: f64, offset_y: f64, blur: f64, color: Color) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            spread: 0.0,
            color,
            inset: true,
        }
    }

    /// Create a shadow with all parameters.
    pub fn new(offset_x: f64, offset_y: f64, blur: f64, spread: f64, color: Color, inset: bool) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            spread,
            color,
            inset,
        }
    }

    /// Set the spread radius.
    pub fn with_spread(mut self, spread: f64) -> Self {
        self.spread = spread;
        self
    }
}

/// A 2D transform.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform {
    /// The list of transform operations (applied in order)
    pub operations: Vec<TransformOp>,
}

impl Transform {
    /// Create an identity transform.
    pub fn identity() -> Self {
        Self { operations: Vec::new() }
    }

    /// Create a rotation transform.
    pub fn rotate(angle: f64) -> Self {
        Self { operations: vec![TransformOp::Rotate(angle)] }
    }

    /// Create a scale transform.
    pub fn scale(x: f64, y: f64) -> Self {
        Self { operations: vec![TransformOp::Scale(x, y)] }
    }

    /// Create a translation transform.
    pub fn translate(x: f64, y: f64) -> Self {
        Self { operations: vec![TransformOp::Translate(x, y)] }
    }

    /// Add a rotation operation.
    pub fn then_rotate(mut self, angle: f64) -> Self {
        self.operations.push(TransformOp::Rotate(angle));
        self
    }

    /// Add a scale operation.
    pub fn then_scale(mut self, x: f64, y: f64) -> Self {
        self.operations.push(TransformOp::Scale(x, y));
        self
    }

    /// Add a translation operation.
    pub fn then_translate(mut self, x: f64, y: f64) -> Self {
        self.operations.push(TransformOp::Translate(x, y));
        self
    }

    /// Convert to a 2D transformation matrix [a, b, c, d, e, f].
    /// This is row-major: [a, c, e; b, d, f; 0, 0, 1]
    /// So a point (x, y) transforms to (ax + cy + e, bx + dy + f).
    pub fn to_matrix(&self) -> [f64; 6] {
        let mut m = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]; // identity

        for op in &self.operations {
            m = multiply_matrix(m, op.to_matrix());
        }

        m
    }
}

/// A single transform operation.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TransformOp {
    /// Rotation in degrees (around the origin)
    Rotate(f64),
    /// Rotation in degrees around a point
    RotateAround { angle: f64, cx: f64, cy: f64 },
    /// Scale by (x, y) factors
    Scale(f64, f64),
    /// Translation by (x, y)
    Translate(f64, f64),
    /// Skew by (x, y) angles in degrees
    Skew(f64, f64),
    /// Arbitrary matrix [a, b, c, d, e, f]
    Matrix([f64; 6]),
}

impl TransformOp {
    /// Convert this operation to a 2D transformation matrix.
    pub fn to_matrix(&self) -> [f64; 6] {
        match *self {
            TransformOp::Rotate(angle) => {
                let rad = angle.to_radians();
                let cos = rad.cos();
                let sin = rad.sin();
                [cos, sin, -sin, cos, 0.0, 0.0]
            }
            TransformOp::RotateAround { angle, cx, cy } => {
                // Translate to origin, rotate, translate back
                let rad = angle.to_radians();
                let cos = rad.cos();
                let sin = rad.sin();
                [
                    cos, sin,
                    -sin, cos,
                    cx - cx * cos + cy * sin,
                    cy - cx * sin - cy * cos,
                ]
            }
            TransformOp::Scale(sx, sy) => {
                [sx, 0.0, 0.0, sy, 0.0, 0.0]
            }
            TransformOp::Translate(tx, ty) => {
                [1.0, 0.0, 0.0, 1.0, tx, ty]
            }
            TransformOp::Skew(ax, ay) => {
                let tan_x = ax.to_radians().tan();
                let tan_y = ay.to_radians().tan();
                [1.0, tan_y, tan_x, 1.0, 0.0, 0.0]
            }
            TransformOp::Matrix(m) => m,
        }
    }
}

/// Multiply two 2D transformation matrices.
fn multiply_matrix(a: [f64; 6], b: [f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

impl GradientStop {
    /// Create a new gradient stop.
    pub fn new(position: f64, color: Color) -> Self {
        Self { position, color }
    }
}

/// Sample a gradient at position t using linear interpolation.
fn sample_gradient(stops: &[GradientStop], t: f64) -> Color {
    if stops.is_empty() {
        return Color::TRANSPARENT;
    }
    if stops.len() == 1 {
        return stops[0].color;
    }

    let t = t.clamp(0.0, 1.0);

    // Find surrounding stops
    let mut prev = &stops[0];
    for stop in stops.iter() {
        if stop.position >= t {
            if stop.position == prev.position {
                return stop.color;
            }
            // Interpolate between prev and stop
            let local_t = (t - prev.position) / (stop.position - prev.position);
            return lerp_color(prev.color, stop.color, local_t as f32);
        }
        prev = stop;
    }

    // Past the last stop
    stops.last().unwrap().color
}

/// Linear interpolation between two colors.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}
