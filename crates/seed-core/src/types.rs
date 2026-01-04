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
