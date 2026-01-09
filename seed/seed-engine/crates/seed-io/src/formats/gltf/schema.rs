//! glTF 2.0 JSON schema types.
//!
//! These types are derived from the glTF 2.0 specification.
//! Some constants and methods are defined for spec completeness.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root glTF object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gltf {
    /// Asset information.
    pub asset: Asset,
    /// Default scene index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<usize>,
    /// Scenes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    /// Nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    /// Meshes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<Mesh>,
    /// Accessors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accessors: Vec<Accessor>,
    /// Buffer views.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffer_views: Vec<BufferView>,
    /// Buffers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffers: Vec<Buffer>,
    /// Materials.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<GltfMaterial>,
    /// Textures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<Texture>,
    /// Images.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    /// Samplers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samplers: Vec<Sampler>,
    /// Animations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<Animation>,
    /// Skins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skins: Vec<Skin>,
    /// Cameras.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<Camera>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extensions: HashMap<String, serde_json::Value>,
    /// Extension names used.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    /// Required extension names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_required: Vec<String>,
}

/// Asset metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    /// glTF version.
    pub version: String,
    /// Minimum glTF version required.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    /// Generator name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    /// Copyright.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
}

impl Default for Asset {
    fn default() -> Self {
        Self {
            version: "2.0".to_string(),
            min_version: None,
            generator: None,
            copyright: None,
        }
    }
}

/// A scene containing root nodes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Scene {
    /// Scene name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Root node indices.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<usize>,
}

/// A node in the scene graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Node name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Child node indices.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<usize>,
    /// Mesh index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<usize>,
    /// Camera index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<usize>,
    /// Skin index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<usize>,
    /// Local transformation matrix (column-major).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f32; 16]>,
    /// Translation (TRS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<[f32; 3]>,
    /// Rotation quaternion (TRS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f32; 4]>,
    /// Scale (TRS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f32; 3]>,
    /// Morph target weights.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<f32>>,
}

/// A mesh containing primitives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mesh {
    /// Mesh name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Mesh primitives.
    pub primitives: Vec<Primitive>,
    /// Morph target weights.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<f32>>,
}

/// A mesh primitive.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Primitive {
    /// Vertex attributes (POSITION, NORMAL, TEXCOORD_0, etc.).
    pub attributes: HashMap<String, usize>,
    /// Index accessor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<usize>,
    /// Material index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<usize>,
    /// Rendering mode (0=POINTS, 1=LINES, 4=TRIANGLES, etc.).
    #[serde(default = "default_primitive_mode")]
    pub mode: u32,
    /// Morph targets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<HashMap<String, usize>>>,
}

fn default_primitive_mode() -> u32 {
    4 // TRIANGLES
}

/// An accessor for typed buffer data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    /// Buffer view index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_view: Option<usize>,
    /// Byte offset within buffer view.
    #[serde(default)]
    pub byte_offset: usize,
    /// Component type (5120=BYTE, 5121=UNSIGNED_BYTE, 5122=SHORT, 5123=UNSIGNED_SHORT, 5125=UNSIGNED_INT, 5126=FLOAT).
    pub component_type: u32,
    /// Number of elements.
    pub count: usize,
    /// Element type ("SCALAR", "VEC2", "VEC3", "VEC4", "MAT2", "MAT3", "MAT4").
    #[serde(rename = "type")]
    pub accessor_type: String,
    /// Whether values are normalized.
    #[serde(default)]
    pub normalized: bool,
    /// Minimum values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f64>>,
    /// Maximum values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f64>>,
    /// Sparse accessor data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse: Option<serde_json::Value>,
    /// Accessor name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A view into a buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    /// Buffer index.
    pub buffer: usize,
    /// Byte offset into buffer.
    #[serde(default)]
    pub byte_offset: usize,
    /// Byte length.
    pub byte_length: usize,
    /// Byte stride for vertex data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_stride: Option<usize>,
    /// Target (34962=ARRAY_BUFFER, 34963=ELEMENT_ARRAY_BUFFER).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
    /// Buffer view name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A buffer containing binary data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    /// Byte length.
    pub byte_length: usize,
    /// URI (data URI or external file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Buffer name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A PBR material.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GltfMaterial {
    /// Material name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// PBR metallic-roughness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pbr_metallic_roughness: Option<PbrMetallicRoughness>,
    /// Normal texture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normal_texture: Option<NormalTextureInfo>,
    /// Occlusion texture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occlusion_texture: Option<OcclusionTextureInfo>,
    /// Emissive texture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emissive_texture: Option<TextureInfo>,
    /// Emissive factor.
    #[serde(default = "default_emissive_factor")]
    pub emissive_factor: [f32; 3],
    /// Alpha mode ("OPAQUE", "MASK", "BLEND").
    #[serde(default = "default_alpha_mode")]
    pub alpha_mode: String,
    /// Alpha cutoff for MASK mode.
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    /// Double-sided rendering.
    #[serde(default)]
    pub double_sided: bool,
}

fn default_emissive_factor() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}

fn default_alpha_mode() -> String {
    "OPAQUE".to_string()
}

fn default_alpha_cutoff() -> f32 {
    0.5
}

/// PBR metallic-roughness properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PbrMetallicRoughness {
    /// Base color factor.
    #[serde(default = "default_base_color_factor")]
    pub base_color_factor: [f32; 4],
    /// Base color texture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<TextureInfo>,
    /// Metallic factor.
    #[serde(default = "default_metallic_factor")]
    pub metallic_factor: f32,
    /// Roughness factor.
    #[serde(default = "default_roughness_factor")]
    pub roughness_factor: f32,
    /// Metallic-roughness texture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<TextureInfo>,
}

impl Default for PbrMetallicRoughness {
    fn default() -> Self {
        Self {
            base_color_factor: default_base_color_factor(),
            base_color_texture: None,
            metallic_factor: default_metallic_factor(),
            roughness_factor: default_roughness_factor(),
            metallic_roughness_texture: None,
        }
    }
}

fn default_base_color_factor() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_metallic_factor() -> f32 {
    1.0
}

fn default_roughness_factor() -> f32 {
    1.0
}

/// Texture reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    /// Texture index.
    pub index: usize,
    /// Texture coordinate set.
    #[serde(default)]
    pub tex_coord: u32,
}

/// Normal texture reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalTextureInfo {
    /// Texture index.
    pub index: usize,
    /// Texture coordinate set.
    #[serde(default)]
    pub tex_coord: u32,
    /// Normal scale.
    #[serde(default = "default_normal_scale")]
    pub scale: f32,
}

fn default_normal_scale() -> f32 {
    1.0
}

/// Occlusion texture reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcclusionTextureInfo {
    /// Texture index.
    pub index: usize,
    /// Texture coordinate set.
    #[serde(default)]
    pub tex_coord: u32,
    /// Occlusion strength.
    #[serde(default = "default_occlusion_strength")]
    pub strength: f32,
}

fn default_occlusion_strength() -> f32 {
    1.0
}

/// A texture.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Texture {
    /// Sampler index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampler: Option<usize>,
    /// Image source index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<usize>,
    /// Texture name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// An image.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    /// URI (data URI or external file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Buffer view index (for GLB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_view: Option<usize>,
    /// Image name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A texture sampler.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    /// Magnification filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mag_filter: Option<u32>,
    /// Minification filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_filter: Option<u32>,
    /// S (U) wrap mode.
    #[serde(default = "default_wrap_mode")]
    pub wrap_s: u32,
    /// T (V) wrap mode.
    #[serde(default = "default_wrap_mode")]
    pub wrap_t: u32,
    /// Sampler name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn default_wrap_mode() -> u32 {
    10497 // REPEAT
}

/// An animation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    /// Animation name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Animation channels.
    #[serde(default)]
    pub channels: Vec<AnimationChannel>,
    /// Animation samplers.
    #[serde(default)]
    pub samplers: Vec<AnimationSampler>,
}

/// An animation channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationChannel {
    /// Sampler index.
    pub sampler: usize,
    /// Target.
    pub target: AnimationTarget,
}

/// Animation target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationTarget {
    /// Node index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<usize>,
    /// Target path ("translation", "rotation", "scale", "weights").
    pub path: String,
}

/// Animation sampler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimationSampler {
    /// Input accessor (time).
    pub input: usize,
    /// Output accessor (values).
    pub output: usize,
    /// Interpolation ("LINEAR", "STEP", "CUBICSPLINE").
    #[serde(default = "default_interpolation")]
    pub interpolation: String,
}

fn default_interpolation() -> String {
    "LINEAR".to_string()
}

/// A skin for skeletal animation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// Skin name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Inverse bind matrices accessor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse_bind_matrices: Option<usize>,
    /// Skeleton root node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skeleton: Option<usize>,
    /// Joint node indices.
    #[serde(default)]
    pub joints: Vec<usize>,
}

/// A camera.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Camera {
    /// Camera name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Camera type ("perspective" or "orthographic").
    #[serde(rename = "type")]
    pub camera_type: String,
    /// Perspective camera properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perspective: Option<PerspectiveCamera>,
    /// Orthographic camera properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orthographic: Option<OrthographicCamera>,
}

/// Perspective camera properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerspectiveCamera {
    /// Aspect ratio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f32>,
    /// Vertical FOV in radians.
    pub yfov: f32,
    /// Near clipping plane.
    pub znear: f32,
    /// Far clipping plane.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zfar: Option<f32>,
}

/// Orthographic camera properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrthographicCamera {
    /// Horizontal magnification.
    pub xmag: f32,
    /// Vertical magnification.
    pub ymag: f32,
    /// Near clipping plane.
    pub znear: f32,
    /// Far clipping plane.
    pub zfar: f32,
}

// Component type constants
pub const COMPONENT_BYTE: u32 = 5120;
pub const COMPONENT_UNSIGNED_BYTE: u32 = 5121;
pub const COMPONENT_SHORT: u32 = 5122;
pub const COMPONENT_UNSIGNED_SHORT: u32 = 5123;
pub const COMPONENT_UNSIGNED_INT: u32 = 5125;
pub const COMPONENT_FLOAT: u32 = 5126;

impl Accessor {
    /// Get the byte size of a single component.
    pub fn component_size(&self) -> usize {
        match self.component_type {
            COMPONENT_BYTE | COMPONENT_UNSIGNED_BYTE => 1,
            COMPONENT_SHORT | COMPONENT_UNSIGNED_SHORT => 2,
            COMPONENT_UNSIGNED_INT | COMPONENT_FLOAT => 4,
            _ => 4,
        }
    }

    /// Get the number of components per element.
    pub fn component_count(&self) -> usize {
        match self.accessor_type.as_str() {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT2" => 4,
            "MAT3" => 9,
            "MAT4" => 16,
            _ => 1,
        }
    }

    /// Get the total byte size of all elements.
    pub fn byte_size(&self) -> usize {
        self.count * self.component_count() * self.component_size()
    }
}
