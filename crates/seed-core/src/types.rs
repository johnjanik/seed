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
