//! Material types for UnifiedScene.

use glam::Vec4;
use serde::{Deserialize, Serialize};

/// PBR material definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    /// Material name.
    pub name: String,
    /// Base color factor (RGBA).
    pub base_color: Vec4,
    /// Base color texture.
    pub base_color_texture: Option<TextureRef>,
    /// Metallic factor (0.0 = dielectric, 1.0 = metallic).
    pub metallic: f32,
    /// Roughness factor (0.0 = smooth, 1.0 = rough).
    pub roughness: f32,
    /// Metallic-roughness texture (B=metallic, G=roughness).
    pub metallic_roughness_texture: Option<TextureRef>,
    /// Normal map texture.
    pub normal_texture: Option<TextureRef>,
    /// Normal map scale.
    pub normal_scale: f32,
    /// Occlusion texture.
    pub occlusion_texture: Option<TextureRef>,
    /// Occlusion strength.
    pub occlusion_strength: f32,
    /// Emissive color (RGB).
    pub emissive: [f32; 3],
    /// Emissive texture.
    pub emissive_texture: Option<TextureRef>,
    /// Alpha mode.
    pub alpha_mode: AlphaMode,
    /// Alpha cutoff (for MASK mode).
    pub alpha_cutoff: f32,
    /// Double-sided rendering.
    pub double_sided: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: Vec4::ONE,
            base_color_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            occlusion_texture: None,
            occlusion_strength: 1.0,
            emissive: [0.0, 0.0, 0.0],
            emissive_texture: None,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

impl Material {
    /// Create a new default material.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a simple colored material.
    pub fn colored(name: impl Into<String>, color: Vec4) -> Self {
        Self {
            name: name.into(),
            base_color: color,
            ..Default::default()
        }
    }

    /// Create a metallic material.
    pub fn metallic(name: impl Into<String>, color: Vec4, roughness: f32) -> Self {
        Self {
            name: name.into(),
            base_color: color,
            metallic: 1.0,
            roughness,
            ..Default::default()
        }
    }
}

/// Alpha blending mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AlphaMode {
    /// Fully opaque.
    #[default]
    Opaque,
    /// Masked (alpha test).
    Mask,
    /// Alpha blended.
    Blend,
}

/// Reference to a texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureRef {
    /// Index into the scene's texture array.
    pub texture_index: usize,
    /// Texture coordinate set to use.
    pub texcoord: u32,
}

/// Texture data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texture {
    /// Texture name.
    pub name: String,
    /// Image source.
    pub source: ImageSource,
    /// Sampler settings.
    pub sampler: Sampler,
}

/// Image source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSource {
    /// Embedded image data.
    Embedded {
        /// MIME type (e.g., "image/png").
        mime_type: String,
        /// Raw image data.
        data: Vec<u8>,
    },
    /// External file reference.
    External {
        /// File path or URI.
        uri: String,
    },
}

/// Texture sampler settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sampler {
    /// Magnification filter.
    pub mag_filter: Filter,
    /// Minification filter.
    pub min_filter: Filter,
    /// U (horizontal) wrapping mode.
    pub wrap_u: Wrap,
    /// V (vertical) wrapping mode.
    pub wrap_v: Wrap,
}

/// Texture filter mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Filter {
    /// Nearest neighbor.
    Nearest,
    /// Bilinear.
    #[default]
    Linear,
}

/// Texture wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Wrap {
    /// Clamp to edge.
    ClampToEdge,
    /// Repeat.
    #[default]
    Repeat,
    /// Mirrored repeat.
    MirroredRepeat,
}
