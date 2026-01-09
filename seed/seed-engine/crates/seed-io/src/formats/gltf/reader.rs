//! glTF reader implementation.

use crate::error::{IoError, Result};
use crate::registry::{FormatReader, ReadOptions};
use crate::scene::{
    AlphaMode, Filter, Geometry, ImageSource, Material, SceneNode, Texture as SceneTexture,
    TextureRef, TriangleMesh, UnifiedScene, Wrap,
};
use base64::Engine;
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

use super::schema::{self, Gltf, COMPONENT_FLOAT, COMPONENT_UNSIGNED_INT, COMPONENT_UNSIGNED_SHORT};

/// GLB magic number.
const GLB_MAGIC: u32 = 0x46546C67; // "glTF" in little-endian
/// GLB version 2.
const GLB_VERSION: u32 = 2;
/// JSON chunk type.
const GLB_CHUNK_JSON: u32 = 0x4E4F534A; // "JSON" in little-endian
/// Binary chunk type.
const GLB_CHUNK_BIN: u32 = 0x004E4942; // "BIN\0" in little-endian

/// Reader for glTF 2.0 files.
pub struct GltfReader;

impl GltfReader {
    /// Create a new glTF reader.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GltfReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatReader for GltfReader {
    fn name(&self) -> &'static str {
        "gltf"
    }

    fn extensions(&self) -> &[&'static str] {
        &["gltf", "glb"]
    }

    fn can_read(&self, data: &[u8]) -> bool {
        // Check for GLB magic number
        if data.len() >= 4 {
            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            if magic == GLB_MAGIC {
                return true;
            }
        }

        // Check for JSON glTF
        if let Ok(text) = std::str::from_utf8(data) {
            let trimmed = text.trim_start();
            if trimmed.starts_with('{') && trimmed.contains("\"asset\"") {
                return true;
            }
        }

        false
    }

    fn read(&self, data: &[u8], options: &ReadOptions) -> Result<UnifiedScene> {
        // Check if GLB or JSON
        if data.len() >= 4 {
            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            if magic == GLB_MAGIC {
                return self.read_glb(data, options);
            }
        }
        self.read_json(data, options)
    }
}

impl GltfReader {
    /// Read a GLB file.
    fn read_glb(&self, data: &[u8], options: &ReadOptions) -> Result<UnifiedScene> {
        if data.len() < 12 {
            return Err(IoError::InvalidData("GLB file too short".into()));
        }

        // Parse header
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let _length = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);

        if magic != GLB_MAGIC {
            return Err(IoError::InvalidData("invalid GLB magic".into()));
        }
        if version != GLB_VERSION {
            return Err(IoError::Unsupported(format!("GLB version {} not supported", version)));
        }

        // Parse chunks
        let mut offset = 12;
        let mut json_data: Option<&[u8]> = None;
        let mut bin_data: Option<&[u8]> = None;

        while offset + 8 <= data.len() {
            let chunk_length =
                u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
                    as usize;
            let chunk_type = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            offset += 8;

            if offset + chunk_length > data.len() {
                return Err(IoError::InvalidData("GLB chunk extends past end of file".into()));
            }

            match chunk_type {
                GLB_CHUNK_JSON => {
                    json_data = Some(&data[offset..offset + chunk_length]);
                }
                GLB_CHUNK_BIN => {
                    bin_data = Some(&data[offset..offset + chunk_length]);
                }
                _ => {
                    // Unknown chunk type, skip
                }
            }

            offset += chunk_length;
            // Chunks are 4-byte aligned
            offset = (offset + 3) & !3;
        }

        let json_data = json_data.ok_or_else(|| IoError::InvalidData("GLB missing JSON chunk".into()))?;

        // Parse JSON
        let gltf: Gltf = serde_json::from_slice(json_data)?;

        // Load buffers
        let buffers = self.load_buffers_glb(&gltf, bin_data)?;

        // Convert to UnifiedScene
        self.convert_gltf(&gltf, &buffers, options)
    }

    /// Read a JSON glTF file.
    fn read_json(&self, data: &[u8], options: &ReadOptions) -> Result<UnifiedScene> {
        let gltf: Gltf = serde_json::from_slice(data)?;

        // Load buffers (from data URIs)
        let buffers = self.load_buffers_json(&gltf)?;

        // Convert to UnifiedScene
        self.convert_gltf(&gltf, &buffers, options)
    }

    /// Load buffers for GLB.
    fn load_buffers_glb(&self, gltf: &Gltf, bin_data: Option<&[u8]>) -> Result<Vec<Vec<u8>>> {
        let mut buffers = Vec::new();

        for (i, buffer) in gltf.buffers.iter().enumerate() {
            if i == 0 && buffer.uri.is_none() {
                // First buffer uses GLB binary chunk
                if let Some(bin) = bin_data {
                    buffers.push(bin[..buffer.byte_length.min(bin.len())].to_vec());
                } else {
                    return Err(IoError::InvalidData("GLB buffer 0 missing binary data".into()));
                }
            } else if let Some(uri) = &buffer.uri {
                // Load from data URI
                let data = self.load_buffer_uri(uri)?;
                buffers.push(data);
            } else {
                return Err(IoError::InvalidData(format!("buffer {} has no data", i)));
            }
        }

        Ok(buffers)
    }

    /// Load buffers for JSON glTF.
    fn load_buffers_json(&self, gltf: &Gltf) -> Result<Vec<Vec<u8>>> {
        let mut buffers = Vec::new();

        for (i, buffer) in gltf.buffers.iter().enumerate() {
            if let Some(uri) = &buffer.uri {
                let data = self.load_buffer_uri(uri)?;
                buffers.push(data);
            } else {
                return Err(IoError::InvalidData(format!("buffer {} has no URI", i)));
            }
        }

        Ok(buffers)
    }

    /// Load buffer data from a URI.
    fn load_buffer_uri(&self, uri: &str) -> Result<Vec<u8>> {
        if uri.starts_with("data:") {
            // Data URI
            self.decode_data_uri(uri)
        } else {
            // External file - not supported in WASM
            Err(IoError::Unsupported(format!(
                "external buffer URIs not supported: {}",
                uri
            )))
        }
    }

    /// Decode a data URI.
    fn decode_data_uri(&self, uri: &str) -> Result<Vec<u8>> {
        // Format: data:[<mediatype>][;base64],<data>
        let parts: Vec<&str> = uri.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(IoError::InvalidData("invalid data URI".into()));
        }

        let header = parts[0];
        let data = parts[1];

        if header.contains(";base64") {
            // Base64 encoded
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|e| IoError::InvalidData(format!("base64 decode error: {}", e)))
        } else {
            // URL encoded (less common)
            Err(IoError::Unsupported("URL-encoded data URIs not supported".into()))
        }
    }

    /// Convert glTF to UnifiedScene.
    fn convert_gltf(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        options: &ReadOptions,
    ) -> Result<UnifiedScene> {
        let mut scene = UnifiedScene::new();

        // Set metadata
        scene.metadata.source_format = Some("glTF".to_string());
        scene.metadata.generator = gltf.asset.generator.clone();
        scene.metadata.copyright = gltf.asset.copyright.clone();

        // Convert materials
        let material_map = self.convert_materials(gltf, &mut scene)?;

        // Convert textures
        self.convert_textures(gltf, buffers, &mut scene)?;

        // Convert meshes
        let mesh_map = self.convert_meshes(gltf, buffers, &material_map, options, &mut scene)?;

        // Convert scene hierarchy
        self.convert_scene_hierarchy(gltf, &mesh_map, &mut scene)?;

        Ok(scene)
    }

    /// Convert glTF materials.
    fn convert_materials(
        &self,
        gltf: &Gltf,
        scene: &mut UnifiedScene,
    ) -> Result<Vec<Option<usize>>> {
        let mut material_map = Vec::new();

        for mat in &gltf.materials {
            let mut material = Material::new(mat.name.clone().unwrap_or_default());

            if let Some(pbr) = &mat.pbr_metallic_roughness {
                material.base_color = Vec4::from(pbr.base_color_factor);
                material.metallic = pbr.metallic_factor;
                material.roughness = pbr.roughness_factor;

                if let Some(tex) = &pbr.base_color_texture {
                    material.base_color_texture = Some(TextureRef {
                        texture_index: tex.index,
                        texcoord: tex.tex_coord,
                    });
                }

                if let Some(tex) = &pbr.metallic_roughness_texture {
                    material.metallic_roughness_texture = Some(TextureRef {
                        texture_index: tex.index,
                        texcoord: tex.tex_coord,
                    });
                }
            }

            if let Some(tex) = &mat.normal_texture {
                material.normal_texture = Some(TextureRef {
                    texture_index: tex.index,
                    texcoord: tex.tex_coord,
                });
                material.normal_scale = tex.scale;
            }

            if let Some(tex) = &mat.occlusion_texture {
                material.occlusion_texture = Some(TextureRef {
                    texture_index: tex.index,
                    texcoord: tex.tex_coord,
                });
                material.occlusion_strength = tex.strength;
            }

            if let Some(tex) = &mat.emissive_texture {
                material.emissive_texture = Some(TextureRef {
                    texture_index: tex.index,
                    texcoord: tex.tex_coord,
                });
            }

            material.emissive = mat.emissive_factor;

            material.alpha_mode = match mat.alpha_mode.as_str() {
                "MASK" => AlphaMode::Mask,
                "BLEND" => AlphaMode::Blend,
                _ => AlphaMode::Opaque,
            };
            material.alpha_cutoff = mat.alpha_cutoff;
            material.double_sided = mat.double_sided;

            let idx = scene.add_material(material);
            material_map.push(Some(idx));
        }

        Ok(material_map)
    }

    /// Convert glTF textures.
    fn convert_textures(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        scene: &mut UnifiedScene,
    ) -> Result<()> {
        for tex in &gltf.textures {
            let source = if let Some(source_idx) = tex.source {
                if let Some(image) = gltf.images.get(source_idx) {
                    if let Some(uri) = &image.uri {
                        if uri.starts_with("data:") {
                            // Embedded base64 image
                            let data = self.decode_data_uri(uri)?;
                            let mime_type = image.mime_type.clone().unwrap_or_else(|| {
                                if data.starts_with(b"\x89PNG") {
                                    "image/png".to_string()
                                } else if data.starts_with(b"\xFF\xD8") {
                                    "image/jpeg".to_string()
                                } else {
                                    "application/octet-stream".to_string()
                                }
                            });
                            ImageSource::Embedded { mime_type, data }
                        } else {
                            ImageSource::External { uri: uri.clone() }
                        }
                    } else if let Some(buffer_view_idx) = image.buffer_view {
                        // Image from buffer view (GLB)
                        let data = self.read_buffer_view_raw(gltf, buffers, buffer_view_idx)?;
                        let mime_type = image.mime_type.clone().unwrap_or_else(|| {
                            if data.starts_with(b"\x89PNG") {
                                "image/png".to_string()
                            } else if data.starts_with(b"\xFF\xD8") {
                                "image/jpeg".to_string()
                            } else {
                                "application/octet-stream".to_string()
                            }
                        });
                        ImageSource::Embedded { mime_type, data }
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            };

            let mut sampler = crate::scene::Sampler::default();
            if let Some(sampler_idx) = tex.sampler {
                if let Some(s) = gltf.samplers.get(sampler_idx) {
                    sampler.mag_filter = match s.mag_filter {
                        Some(9728) => Filter::Nearest,
                        _ => Filter::Linear,
                    };
                    sampler.min_filter = match s.min_filter {
                        Some(9728) | Some(9984) | Some(9986) => Filter::Nearest,
                        _ => Filter::Linear,
                    };
                    sampler.wrap_u = match s.wrap_s {
                        33071 => Wrap::ClampToEdge,
                        33648 => Wrap::MirroredRepeat,
                        _ => Wrap::Repeat,
                    };
                    sampler.wrap_v = match s.wrap_t {
                        33071 => Wrap::ClampToEdge,
                        33648 => Wrap::MirroredRepeat,
                        _ => Wrap::Repeat,
                    };
                }
            }

            scene.add_texture(SceneTexture {
                name: tex.name.clone().unwrap_or_default(),
                source,
                sampler,
            });
        }

        Ok(())
    }

    /// Convert glTF meshes.
    fn convert_meshes(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        material_map: &[Option<usize>],
        options: &ReadOptions,
        scene: &mut UnifiedScene,
    ) -> Result<Vec<Vec<(usize, Option<usize>)>>> {
        let mut mesh_map = Vec::new();

        for mesh in &gltf.meshes {
            let mut primitives = Vec::new();

            for prim in &mesh.primitives {
                // Only support triangles
                if prim.mode != 4 {
                    continue;
                }

                let mut tri_mesh = TriangleMesh::new();

                // Read positions (required)
                if let Some(&pos_accessor) = prim.attributes.get("POSITION") {
                    tri_mesh.positions = self.read_accessor_vec3(gltf, buffers, pos_accessor)?;
                } else {
                    continue; // Skip primitives without positions
                }

                // Read normals (optional)
                if let Some(&normal_accessor) = prim.attributes.get("NORMAL") {
                    tri_mesh.normals = Some(self.read_accessor_vec3(gltf, buffers, normal_accessor)?);
                } else if options.compute_normals {
                    // Will compute after indices are read
                }

                // Read texture coordinates (optional)
                if let Some(&texcoord_accessor) = prim.attributes.get("TEXCOORD_0") {
                    tri_mesh.texcoords = Some(self.read_accessor_vec2(gltf, buffers, texcoord_accessor)?);
                }

                // Read vertex colors (optional)
                if let Some(&color_accessor) = prim.attributes.get("COLOR_0") {
                    tri_mesh.colors = Some(self.read_accessor_vec4(gltf, buffers, color_accessor)?);
                }

                // Read indices
                if let Some(indices_accessor) = prim.indices {
                    tri_mesh.indices = self.read_accessor_indices(gltf, buffers, indices_accessor)?;
                } else {
                    // Non-indexed geometry - generate sequential indices
                    tri_mesh.indices = (0..tri_mesh.positions.len() as u32).collect();
                }

                // Compute normals if missing and requested
                if tri_mesh.normals.is_none() && options.compute_normals {
                    tri_mesh.compute_normals();
                }

                // Add geometry to scene
                let geom_idx = scene.add_geometry(Geometry::Mesh(tri_mesh));

                // Get material index
                let mat_idx = prim
                    .material
                    .and_then(|m| material_map.get(m).copied().flatten());

                primitives.push((geom_idx, mat_idx));
            }

            mesh_map.push(primitives);
        }

        Ok(mesh_map)
    }

    /// Read raw buffer view data.
    fn read_buffer_view_raw(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        buffer_view_idx: usize,
    ) -> Result<Vec<u8>> {
        let view = gltf
            .buffer_views
            .get(buffer_view_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer view {}", buffer_view_idx)))?;

        let buffer = buffers
            .get(view.buffer)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer {}", view.buffer)))?;

        let start = view.byte_offset;
        let end = start + view.byte_length;

        if end > buffer.len() {
            return Err(IoError::InvalidData("buffer view out of bounds".into()));
        }

        Ok(buffer[start..end].to_vec())
    }

    /// Read accessor data as Vec3 array.
    fn read_accessor_vec3(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        accessor_idx: usize,
    ) -> Result<Vec<Vec3>> {
        let accessor = gltf
            .accessors
            .get(accessor_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid accessor {}", accessor_idx)))?;

        if accessor.accessor_type != "VEC3" {
            return Err(IoError::InvalidData(format!(
                "expected VEC3, got {}",
                accessor.accessor_type
            )));
        }

        let buffer_view_idx = accessor
            .buffer_view
            .ok_or_else(|| IoError::InvalidData("accessor missing buffer view".into()))?;

        let view = gltf
            .buffer_views
            .get(buffer_view_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer view {}", buffer_view_idx)))?;

        let buffer = buffers
            .get(view.buffer)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer {}", view.buffer)))?;

        let start = view.byte_offset + accessor.byte_offset;
        let stride = view.byte_stride.unwrap_or(12); // 3 * 4 bytes for float32

        let mut result = Vec::with_capacity(accessor.count);

        for i in 0..accessor.count {
            let offset = start + i * stride;
            if offset + 12 > buffer.len() {
                return Err(IoError::InvalidData("accessor data out of bounds".into()));
            }

            let x = f32::from_le_bytes([
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                buffer[offset + 4],
                buffer[offset + 5],
                buffer[offset + 6],
                buffer[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                buffer[offset + 8],
                buffer[offset + 9],
                buffer[offset + 10],
                buffer[offset + 11],
            ]);

            result.push(Vec3::new(x, y, z));
        }

        Ok(result)
    }

    /// Read accessor data as Vec2 array.
    fn read_accessor_vec2(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        accessor_idx: usize,
    ) -> Result<Vec<Vec2>> {
        let accessor = gltf
            .accessors
            .get(accessor_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid accessor {}", accessor_idx)))?;

        if accessor.accessor_type != "VEC2" {
            return Err(IoError::InvalidData(format!(
                "expected VEC2, got {}",
                accessor.accessor_type
            )));
        }

        let buffer_view_idx = accessor
            .buffer_view
            .ok_or_else(|| IoError::InvalidData("accessor missing buffer view".into()))?;

        let view = gltf
            .buffer_views
            .get(buffer_view_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer view {}", buffer_view_idx)))?;

        let buffer = buffers
            .get(view.buffer)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer {}", view.buffer)))?;

        let start = view.byte_offset + accessor.byte_offset;
        let stride = view.byte_stride.unwrap_or(8); // 2 * 4 bytes for float32

        let mut result = Vec::with_capacity(accessor.count);

        for i in 0..accessor.count {
            let offset = start + i * stride;
            if offset + 8 > buffer.len() {
                return Err(IoError::InvalidData("accessor data out of bounds".into()));
            }

            let x = f32::from_le_bytes([
                buffer[offset],
                buffer[offset + 1],
                buffer[offset + 2],
                buffer[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                buffer[offset + 4],
                buffer[offset + 5],
                buffer[offset + 6],
                buffer[offset + 7],
            ]);

            result.push(Vec2::new(x, y));
        }

        Ok(result)
    }

    /// Read accessor data as Vec4 array.
    fn read_accessor_vec4(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        accessor_idx: usize,
    ) -> Result<Vec<Vec4>> {
        let accessor = gltf
            .accessors
            .get(accessor_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid accessor {}", accessor_idx)))?;

        if accessor.accessor_type != "VEC4" {
            return Err(IoError::InvalidData(format!(
                "expected VEC4, got {}",
                accessor.accessor_type
            )));
        }

        let buffer_view_idx = accessor
            .buffer_view
            .ok_or_else(|| IoError::InvalidData("accessor missing buffer view".into()))?;

        let view = gltf
            .buffer_views
            .get(buffer_view_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer view {}", buffer_view_idx)))?;

        let buffer = buffers
            .get(view.buffer)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer {}", view.buffer)))?;

        let start = view.byte_offset + accessor.byte_offset;

        // Handle different component types for colors
        let (stride, read_fn): (usize, Box<dyn Fn(&[u8], usize) -> Vec4>) =
            match accessor.component_type {
                COMPONENT_FLOAT => {
                    let s = view.byte_stride.unwrap_or(16);
                    (s, Box::new(|buf: &[u8], off: usize| {
                        Vec4::new(
                            f32::from_le_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3]]),
                            f32::from_le_bytes([buf[off+4], buf[off+5], buf[off+6], buf[off+7]]),
                            f32::from_le_bytes([buf[off+8], buf[off+9], buf[off+10], buf[off+11]]),
                            f32::from_le_bytes([buf[off+12], buf[off+13], buf[off+14], buf[off+15]]),
                        )
                    }))
                }
                schema::COMPONENT_UNSIGNED_BYTE => {
                    let s = view.byte_stride.unwrap_or(4);
                    let normalized = accessor.normalized;
                    (s, Box::new(move |buf: &[u8], off: usize| {
                        let scale = if normalized { 255.0 } else { 1.0 };
                        Vec4::new(
                            buf[off] as f32 / scale,
                            buf[off+1] as f32 / scale,
                            buf[off+2] as f32 / scale,
                            buf[off+3] as f32 / scale,
                        )
                    }))
                }
                schema::COMPONENT_UNSIGNED_SHORT => {
                    let s = view.byte_stride.unwrap_or(8);
                    let normalized = accessor.normalized;
                    (s, Box::new(move |buf: &[u8], off: usize| {
                        let scale = if normalized { 65535.0 } else { 1.0 };
                        Vec4::new(
                            u16::from_le_bytes([buf[off], buf[off+1]]) as f32 / scale,
                            u16::from_le_bytes([buf[off+2], buf[off+3]]) as f32 / scale,
                            u16::from_le_bytes([buf[off+4], buf[off+5]]) as f32 / scale,
                            u16::from_le_bytes([buf[off+6], buf[off+7]]) as f32 / scale,
                        )
                    }))
                }
                _ => {
                    let s = view.byte_stride.unwrap_or(16);
                    (s, Box::new(|buf: &[u8], off: usize| {
                        Vec4::new(
                            f32::from_le_bytes([buf[off], buf[off+1], buf[off+2], buf[off+3]]),
                            f32::from_le_bytes([buf[off+4], buf[off+5], buf[off+6], buf[off+7]]),
                            f32::from_le_bytes([buf[off+8], buf[off+9], buf[off+10], buf[off+11]]),
                            f32::from_le_bytes([buf[off+12], buf[off+13], buf[off+14], buf[off+15]]),
                        )
                    }))
                }
            };

        let mut result = Vec::with_capacity(accessor.count);

        for i in 0..accessor.count {
            let offset = start + i * stride;
            result.push(read_fn(buffer, offset));
        }

        Ok(result)
    }

    /// Read accessor data as index array.
    fn read_accessor_indices(
        &self,
        gltf: &Gltf,
        buffers: &[Vec<u8>],
        accessor_idx: usize,
    ) -> Result<Vec<u32>> {
        let accessor = gltf
            .accessors
            .get(accessor_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid accessor {}", accessor_idx)))?;

        if accessor.accessor_type != "SCALAR" {
            return Err(IoError::InvalidData(format!(
                "expected SCALAR for indices, got {}",
                accessor.accessor_type
            )));
        }

        let buffer_view_idx = accessor
            .buffer_view
            .ok_or_else(|| IoError::InvalidData("accessor missing buffer view".into()))?;

        let view = gltf
            .buffer_views
            .get(buffer_view_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer view {}", buffer_view_idx)))?;

        let buffer = buffers
            .get(view.buffer)
            .ok_or_else(|| IoError::InvalidData(format!("invalid buffer {}", view.buffer)))?;

        let start = view.byte_offset + accessor.byte_offset;

        let mut result = Vec::with_capacity(accessor.count);

        match accessor.component_type {
            COMPONENT_UNSIGNED_SHORT => {
                let stride = view.byte_stride.unwrap_or(2);
                for i in 0..accessor.count {
                    let offset = start + i * stride;
                    if offset + 2 > buffer.len() {
                        return Err(IoError::InvalidData("index data out of bounds".into()));
                    }
                    let idx = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]);
                    result.push(idx as u32);
                }
            }
            COMPONENT_UNSIGNED_INT => {
                let stride = view.byte_stride.unwrap_or(4);
                for i in 0..accessor.count {
                    let offset = start + i * stride;
                    if offset + 4 > buffer.len() {
                        return Err(IoError::InvalidData("index data out of bounds".into()));
                    }
                    let idx = u32::from_le_bytes([
                        buffer[offset],
                        buffer[offset + 1],
                        buffer[offset + 2],
                        buffer[offset + 3],
                    ]);
                    result.push(idx);
                }
            }
            schema::COMPONENT_UNSIGNED_BYTE => {
                let stride = view.byte_stride.unwrap_or(1);
                for i in 0..accessor.count {
                    let offset = start + i * stride;
                    if offset >= buffer.len() {
                        return Err(IoError::InvalidData("index data out of bounds".into()));
                    }
                    result.push(buffer[offset] as u32);
                }
            }
            _ => {
                return Err(IoError::InvalidData(format!(
                    "unsupported index component type: {}",
                    accessor.component_type
                )));
            }
        }

        Ok(result)
    }

    /// Convert glTF scene hierarchy.
    fn convert_scene_hierarchy(
        &self,
        gltf: &Gltf,
        mesh_map: &[Vec<(usize, Option<usize>)>],
        scene: &mut UnifiedScene,
    ) -> Result<()> {
        // Get the default scene or first scene
        let scene_idx = gltf.scene.unwrap_or(0);
        let gltf_scene = gltf.scenes.get(scene_idx);

        if let Some(gltf_scene) = gltf_scene {
            // Convert root nodes
            for &root_node_idx in &gltf_scene.nodes {
                self.convert_node(gltf, mesh_map, root_node_idx, None, scene)?;
            }
        } else if !gltf.nodes.is_empty() {
            // No scene defined, use all nodes without parents as roots
            let mut has_parent = vec![false; gltf.nodes.len()];
            for node in &gltf.nodes {
                for &child in &node.children {
                    if child < has_parent.len() {
                        has_parent[child] = true;
                    }
                }
            }

            for (i, is_child) in has_parent.iter().enumerate() {
                if !is_child {
                    self.convert_node(gltf, mesh_map, i, None, scene)?;
                }
            }
        }

        Ok(())
    }

    /// Convert a single glTF node.
    fn convert_node(
        &self,
        gltf: &Gltf,
        mesh_map: &[Vec<(usize, Option<usize>)>],
        node_idx: usize,
        parent: Option<usize>,
        scene: &mut UnifiedScene,
    ) -> Result<()> {
        let gltf_node = gltf
            .nodes
            .get(node_idx)
            .ok_or_else(|| IoError::InvalidData(format!("invalid node {}", node_idx)))?;

        // Compute transform
        let transform = if let Some(matrix) = &gltf_node.matrix {
            Mat4::from_cols_array(matrix)
        } else {
            let translation = gltf_node
                .translation
                .map(Vec3::from)
                .unwrap_or(Vec3::ZERO);
            let rotation = gltf_node
                .rotation
                .map(|r| Quat::from_xyzw(r[0], r[1], r[2], r[3]))
                .unwrap_or(Quat::IDENTITY);
            let scale = gltf_node.scale.map(Vec3::from).unwrap_or(Vec3::ONE);

            Mat4::from_scale_rotation_translation(scale, rotation, translation)
        };

        let name = gltf_node
            .name
            .clone()
            .unwrap_or_else(|| format!("node_{}", node_idx));

        // If node has a mesh, create nodes for each primitive
        if let Some(mesh_idx) = gltf_node.mesh {
            if let Some(primitives) = mesh_map.get(mesh_idx) {
                if primitives.len() == 1 {
                    // Single primitive - create one node
                    let (geom_idx, mat_idx) = primitives[0];
                    let mut node = SceneNode::with_geometry(&name, geom_idx);
                    node.transform = transform;
                    node.material = mat_idx;

                    let node_idx = if let Some(parent_idx) = parent {
                        scene.add_child(parent_idx, node)
                    } else {
                        scene.add_root(node)
                    };

                    // Convert children
                    for &child_idx in &gltf_node.children {
                        self.convert_node(gltf, mesh_map, child_idx, Some(node_idx), scene)?;
                    }
                } else {
                    // Multiple primitives - create parent node with children
                    let mut parent_node = SceneNode::new(&name);
                    parent_node.transform = transform;

                    let parent_node_idx = if let Some(parent_idx) = parent {
                        scene.add_child(parent_idx, parent_node)
                    } else {
                        scene.add_root(parent_node)
                    };

                    for (i, &(geom_idx, mat_idx)) in primitives.iter().enumerate() {
                        let prim_name = format!("{}_primitive_{}", name, i);
                        let mut prim_node = SceneNode::with_geometry(&prim_name, geom_idx);
                        prim_node.material = mat_idx;
                        scene.add_child(parent_node_idx, prim_node);
                    }

                    // Convert children
                    for &child_idx in &gltf_node.children {
                        self.convert_node(gltf, mesh_map, child_idx, Some(parent_node_idx), scene)?;
                    }
                }
            } else {
                // Mesh not found, create empty node
                let mut node = SceneNode::new(&name);
                node.transform = transform;

                let node_idx = if let Some(parent_idx) = parent {
                    scene.add_child(parent_idx, node)
                } else {
                    scene.add_root(node)
                };

                for &child_idx in &gltf_node.children {
                    self.convert_node(gltf, mesh_map, child_idx, Some(node_idx), scene)?;
                }
            }
        } else {
            // No mesh - create empty node (transform only)
            let mut node = SceneNode::new(&name);
            node.transform = transform;

            let node_idx = if let Some(parent_idx) = parent {
                scene.add_child(parent_idx, node)
            } else {
                scene.add_root(node)
            };

            // Convert children
            for &child_idx in &gltf_node.children {
                self.convert_node(gltf, mesh_map, child_idx, Some(node_idx), scene)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_read_glb() {
        let reader = GltfReader::new();

        // GLB magic number
        let glb_header = b"glTF\x02\x00\x00\x00";
        assert!(reader.can_read(glb_header));

        // JSON glTF
        let json_gltf = br#"{"asset": {"version": "2.0"}}"#;
        assert!(reader.can_read(json_gltf));

        // Random data
        assert!(!reader.can_read(b"random"));
    }

    #[test]
    fn test_decode_data_uri() {
        let reader = GltfReader::new();

        // Simple base64 data URI
        let uri = "data:application/octet-stream;base64,SGVsbG8=";
        let result = reader.decode_data_uri(uri).unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn test_minimal_gltf() {
        let reader = GltfReader::new();

        // Minimal valid glTF
        let json = br#"{
            "asset": {"version": "2.0"},
            "scenes": [{"nodes": [0]}],
            "nodes": [{"name": "TestNode"}]
        }"#;

        let result = reader.read(json, &ReadOptions::default());
        assert!(result.is_ok());

        let scene = result.unwrap();
        assert_eq!(scene.node_count(), 1);
        assert_eq!(scene.nodes[0].name, "TestNode");
    }

    #[test]
    fn test_gltf_with_mesh() {
        let reader = GltfReader::new();

        // glTF with embedded mesh data
        // A simple triangle with 3 vertices
        let positions: [f32; 9] = [
            0.0, 0.0, 0.0,
            1.0, 0.0, 0.0,
            0.5, 1.0, 0.0,
        ];
        let indices: [u16; 3] = [0, 1, 2];

        let pos_bytes: Vec<u8> = positions.iter().flat_map(|f| f.to_le_bytes()).collect();
        let idx_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();

        let mut buffer_data = pos_bytes.clone();
        buffer_data.extend(&idx_bytes);

        let buffer_base64 = base64::engine::general_purpose::STANDARD.encode(&buffer_data);

        let json = format!(r#"{{
            "asset": {{"version": "2.0"}},
            "scene": 0,
            "scenes": [{{"nodes": [0]}}],
            "nodes": [{{"mesh": 0, "name": "Triangle"}}],
            "meshes": [{{
                "primitives": [{{
                    "attributes": {{"POSITION": 0}},
                    "indices": 1
                }}]
            }}],
            "accessors": [
                {{
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 3,
                    "type": "VEC3"
                }},
                {{
                    "bufferView": 1,
                    "componentType": 5123,
                    "count": 3,
                    "type": "SCALAR"
                }}
            ],
            "bufferViews": [
                {{"buffer": 0, "byteOffset": 0, "byteLength": 36}},
                {{"buffer": 0, "byteOffset": 36, "byteLength": 6}}
            ],
            "buffers": [{{
                "byteLength": 42,
                "uri": "data:application/octet-stream;base64,{}"
            }}]
        }}"#, buffer_base64);

        let result = reader.read(json.as_bytes(), &ReadOptions::default());
        assert!(result.is_ok(), "Failed: {:?}", result);

        let scene = result.unwrap();
        assert_eq!(scene.node_count(), 1);
        assert_eq!(scene.geometry_count(), 1);

        // Check the mesh
        if let Geometry::Mesh(mesh) = &scene.geometries[0] {
            assert_eq!(mesh.positions.len(), 3);
            assert_eq!(mesh.indices.len(), 3);
        } else {
            panic!("Expected mesh geometry");
        }
    }
}
