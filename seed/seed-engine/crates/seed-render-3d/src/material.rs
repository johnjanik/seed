//! Material definitions for 3D rendering.

use seed_core::types::Color;

/// A material describing surface appearance.
#[derive(Debug, Clone)]
pub struct Material {
    /// Base color of the material.
    pub color: Color,
    /// Metallic factor (0.0 = dielectric, 1.0 = metal).
    pub metallic: f32,
    /// Roughness factor (0.0 = smooth/glossy, 1.0 = rough/matte).
    pub roughness: f32,
    /// Ambient occlusion factor.
    pub ambient_occlusion: f32,
    /// Emissive color (for glowing materials).
    pub emissive: Option<Color>,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            color: Color::rgb(0.8, 0.8, 0.8),
            metallic: 0.0,
            roughness: 0.5,
            ambient_occlusion: 1.0,
            emissive: None,
        }
    }
}

impl Material {
    /// Create a new material with the given color.
    pub fn new(color: Color) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }

    /// Create a metallic material.
    pub fn metal(color: Color) -> Self {
        Self {
            color,
            metallic: 1.0,
            roughness: 0.3,
            ..Default::default()
        }
    }

    /// Create a plastic-like material.
    pub fn plastic(color: Color) -> Self {
        Self {
            color,
            metallic: 0.0,
            roughness: 0.4,
            ..Default::default()
        }
    }

    /// Create a matte material.
    pub fn matte(color: Color) -> Self {
        Self {
            color,
            metallic: 0.0,
            roughness: 1.0,
            ..Default::default()
        }
    }

    /// Set the roughness.
    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.clamp(0.0, 1.0);
        self
    }

    /// Set the metallic factor.
    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic.clamp(0.0, 1.0);
        self
    }

    /// Set emissive color.
    pub fn with_emissive(mut self, color: Color) -> Self {
        self.emissive = Some(color);
        self
    }
}

/// A light source.
#[derive(Debug, Clone)]
pub enum Light {
    /// Directional light (like the sun).
    Directional {
        direction: [f32; 3],
        color: Color,
        intensity: f32,
    },
    /// Point light.
    Point {
        position: [f32; 3],
        color: Color,
        intensity: f32,
        range: f32,
    },
    /// Spot light.
    Spot {
        position: [f32; 3],
        direction: [f32; 3],
        color: Color,
        intensity: f32,
        range: f32,
        /// Inner cone angle in radians (full intensity)
        inner_angle: f32,
        /// Outer cone angle in radians (falloff to zero)
        outer_angle: f32,
    },
    /// Ambient light.
    Ambient {
        color: Color,
        intensity: f32,
    },
}

impl Light {
    /// Create a directional light.
    pub fn directional(direction: [f32; 3], color: Color, intensity: f32) -> Self {
        Light::Directional { direction, color, intensity }
    }

    /// Create a point light.
    pub fn point(position: [f32; 3], color: Color, intensity: f32, range: f32) -> Self {
        Light::Point { position, color, intensity, range }
    }

    /// Create a spot light.
    pub fn spot(
        position: [f32; 3],
        direction: [f32; 3],
        color: Color,
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Self {
        Light::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_angle,
            outer_angle,
        }
    }

    /// Create an ambient light.
    pub fn ambient(color: Color, intensity: f32) -> Self {
        Light::Ambient { color, intensity }
    }

    /// Create a default directional light (sun-like).
    pub fn default_sun() -> Self {
        Light::Directional {
            direction: [-0.5, -1.0, -0.5],
            color: Color::rgb(1.0, 0.98, 0.95),
            intensity: 1.0,
        }
    }

    /// Create a default ambient light.
    pub fn default_ambient() -> Self {
        Light::Ambient {
            color: Color::rgb(0.3, 0.35, 0.4),
            intensity: 0.3,
        }
    }
}

/// Calculate Fresnel reflectance using Schlick's approximation.
/// Returns the Fresnel factor at the given view angle.
pub fn fresnel_schlick(cos_theta: f32, f0: f32) -> f32 {
    f0 + (1.0 - f0) * (1.0 - cos_theta).powf(5.0)
}

/// Calculate Fresnel reflectance for RGB (metallic materials).
pub fn fresnel_schlick_rgb(cos_theta: f32, f0: [f32; 3]) -> [f32; 3] {
    let one_minus_cos = (1.0 - cos_theta).powf(5.0);
    [
        f0[0] + (1.0 - f0[0]) * one_minus_cos,
        f0[1] + (1.0 - f0[1]) * one_minus_cos,
        f0[2] + (1.0 - f0[2]) * one_minus_cos,
    ]
}

/// Apply gamma correction (linear to sRGB).
pub fn gamma_correct(color: Color) -> Color {
    Color {
        r: linear_to_srgb(color.r),
        g: linear_to_srgb(color.g),
        b: linear_to_srgb(color.b),
        a: color.a,
    }
}

/// Convert linear color to sRGB.
fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

/// Convert sRGB color to linear.
pub fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_default() {
        let mat = Material::default();
        assert!((mat.roughness - 0.5).abs() < 0.001);
        assert!(mat.metallic < 0.001);
    }

    #[test]
    fn test_material_metal() {
        let mat = Material::metal(Color::rgb(1.0, 0.8, 0.0));
        assert!(mat.metallic > 0.9);
    }

    #[test]
    fn test_light_directional() {
        let light = Light::directional([0.0, -1.0, 0.0], Color::rgb(1.0, 1.0, 1.0), 1.0);
        if let Light::Directional { direction, .. } = light {
            assert!((direction[1] - (-1.0)).abs() < 0.001);
        } else {
            panic!("Expected directional light");
        }
    }

    #[test]
    fn test_light_spot() {
        let light = Light::spot(
            [0.0, 10.0, 0.0],
            [0.0, -1.0, 0.0],
            Color::rgb(1.0, 1.0, 1.0),
            1.0,
            20.0,
            0.2,
            0.4,
        );
        if let Light::Spot { position, inner_angle, outer_angle, .. } = light {
            assert!((position[1] - 10.0).abs() < 0.001);
            assert!((inner_angle - 0.2).abs() < 0.001);
            assert!((outer_angle - 0.4).abs() < 0.001);
        } else {
            panic!("Expected spot light");
        }
    }

    #[test]
    fn test_fresnel_schlick() {
        // At normal incidence (cos_theta = 1), Fresnel should equal F0
        let f0 = 0.04;
        let fresnel = fresnel_schlick(1.0, f0);
        assert!((fresnel - f0).abs() < 0.001);

        // At grazing angle (cos_theta = 0), Fresnel should approach 1
        let fresnel_grazing = fresnel_schlick(0.0, f0);
        assert!(fresnel_grazing > 0.9);
    }

    #[test]
    fn test_gamma_correction() {
        // Linear 0.5 should become ~0.735 in sRGB
        let linear = Color::rgb(0.5, 0.5, 0.5);
        let srgb = gamma_correct(linear);
        assert!(srgb.r > 0.7 && srgb.r < 0.8);
        assert!(srgb.g > 0.7 && srgb.g < 0.8);
        assert!(srgb.b > 0.7 && srgb.b < 0.8);
    }

    #[test]
    fn test_srgb_to_linear() {
        // sRGB 0.5 should become ~0.214 in linear
        let linear = srgb_to_linear(0.5);
        assert!(linear > 0.2 && linear < 0.25);
    }
}
