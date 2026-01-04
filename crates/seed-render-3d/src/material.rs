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
}
