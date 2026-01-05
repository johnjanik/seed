//! Color types and conversion utilities for image analysis.

/// RGBA color with 8-bit components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// LAB color space for perceptual color comparison.
#[derive(Debug, Clone, Copy)]
pub struct LabColor {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Color {
    /// Create a new color from RGB values.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a new color from RGBA values.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create a color from a pixel array [r, g, b, a].
    pub fn from_pixel(pixel: [u8; 4]) -> Self {
        Self {
            r: pixel[0],
            g: pixel[1],
            b: pixel[2],
            a: pixel[3],
        }
    }

    /// Convert to hexadecimal string (e.g., "#ff5500" or "#ff550080" with alpha).
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Convert to LAB color space for perceptual comparisons.
    pub fn to_lab(&self) -> LabColor {
        // First convert to XYZ
        let xyz = self.to_xyz();

        // Then XYZ to LAB
        // Reference white point (D65)
        let ref_x = 95.047;
        let ref_y = 100.0;
        let ref_z = 108.883;

        let x = xyz.0 / ref_x;
        let y = xyz.1 / ref_y;
        let z = xyz.2 / ref_z;

        let fx = lab_f(x);
        let fy = lab_f(y);
        let fz = lab_f(z);

        LabColor {
            l: 116.0 * fy - 16.0,
            a: 500.0 * (fx - fy),
            b: 200.0 * (fy - fz),
        }
    }

    /// Convert to XYZ color space.
    fn to_xyz(&self) -> (f32, f32, f32) {
        // sRGB to linear RGB
        let r = srgb_to_linear(self.r as f32 / 255.0);
        let g = srgb_to_linear(self.g as f32 / 255.0);
        let b = srgb_to_linear(self.b as f32 / 255.0);

        // Linear RGB to XYZ (sRGB primaries, D65 white point)
        let x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375;
        let y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750;
        let z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041;

        (x * 100.0, y * 100.0, z * 100.0)
    }

    /// Calculate perceptual distance to another color using Delta E (CIE76).
    pub fn distance(&self, other: &Color) -> f32 {
        let lab1 = self.to_lab();
        let lab2 = other.to_lab();
        lab1.distance(&lab2)
    }

    /// Check if this color is similar to another (within threshold).
    pub fn is_similar(&self, other: &Color, threshold: f32) -> bool {
        self.distance(other) < threshold
    }

    /// Convert to f32 components [0.0, 1.0].
    pub fn to_f32(&self) -> (f32, f32, f32, f32) {
        (
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        )
    }

    /// Blend two colors together.
    pub fn blend(&self, other: &Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: ((1.0 - t) * self.r as f32 + t * other.r as f32).round() as u8,
            g: ((1.0 - t) * self.g as f32 + t * other.g as f32).round() as u8,
            b: ((1.0 - t) * self.b as f32 + t * other.b as f32).round() as u8,
            a: ((1.0 - t) * self.a as f32 + t * other.a as f32).round() as u8,
        }
    }
}

impl LabColor {
    /// Calculate Delta E distance (CIE76).
    pub fn distance(&self, other: &LabColor) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        (dl * dl + da * da + db * db).sqrt()
    }
}

/// LAB conversion helper function.
fn lab_f(t: f32) -> f32 {
    let delta: f32 = 6.0 / 29.0;
    if t > delta.powi(3) {
        t.powf(1.0 / 3.0)
    } else {
        t / (3.0 * delta * delta) + 4.0 / 29.0
    }
}

/// Convert sRGB component to linear RGB.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_to_hex() {
        let c = Color::rgb(255, 85, 0);
        assert_eq!(c.to_hex(), "#ff5500");

        let c = Color::rgba(255, 85, 0, 128);
        assert_eq!(c.to_hex(), "#ff550080");
    }

    #[test]
    fn test_color_distance() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let red = Color::rgb(255, 0, 0);

        // Black and white should be far apart
        let bw_dist = black.distance(&white);
        assert!(bw_dist > 90.0);

        // Same colors should have zero distance
        let same_dist = red.distance(&red);
        assert!(same_dist < 0.01);
    }

    #[test]
    fn test_color_similar() {
        let c1 = Color::rgb(100, 100, 100);
        let c2 = Color::rgb(102, 100, 101);

        // Very similar colors
        assert!(c1.is_similar(&c2, 5.0));

        // Dissimilar colors
        let c3 = Color::rgb(200, 100, 100);
        assert!(!c1.is_similar(&c3, 5.0));
    }
}
