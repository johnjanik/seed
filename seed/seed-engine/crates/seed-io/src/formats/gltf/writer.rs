//! glTF writer implementation.

use crate::error::{IoError, Result};
use crate::registry::{FormatWriter, WriteOptions};
use crate::scene::{
    AlphaMode, Filter, Geometry, ImageSource, Material, Texture as SceneTexture, TextureRef,
    TriangleMesh, UnifiedScene, Wrap,
};

use super::schema::{
    self, Accessor, Asset, Buffer, BufferView, GltfMaterial, Gltf, Image, Mesh, Node,
    NormalTextureInfo, OcclusionTextureInfo, PbrMetallicRoughness, Primitive, Sampler, Scene,
    Texture, TextureInfo,
};

use glam::{Mat4, Vec3};
use std::collections::HashMap;

/// GLB magic number "glTF".
const GLB_MAGIC: u32 = 0x46546C67;
/// GLB version.
const GLB_VERSION: u32 = 2;
/// JSON chunk type.
const CHUNK_JSON: u32 = 0x4E4F534A;
/// Binary chunk type.
const CHUNK_BIN: u32 = 0x004E4942;

/// Buffer view target: ARRAY_BUFFER (vertex data).
const TARGET_ARRAY_BUFFER: u32 = 34962;
/// Buffer view target: ELEMENT_ARRAY_BUFFER (index data).
const TARGET_ELEMENT_ARRAY_BUFFER: u32 = 34963;

/// Writer for glTF 2.0 files.
pub struct GltfWriter;

impl GltfWriter {
    /// Create a new glTF writer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GltfWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatWriter for GltfWriter {
    fn name(&self) -> &'static str {
        "gltf"
    }

    fn extension(&self) -> &'static str {
        "glb"
    }

    fn write(&self, scene: &UnifiedScene, options: &WriteOptions) -> Result<Vec<u8>> {
        if options.binary {
            self.write_glb(scene)
        } else {
            self.write_json(scene, options.pretty)
        }
    }
}

/// Internal state for building glTF data.
struct GltfBuilder {
    gltf: Gltf,
    buffer_data: Vec<u8>,
    /// Maps scene geometry index to glTF mesh index.
    geometry_to_mesh: HashMap<usize, usize>,
    /// Maps scene material index to glTF material index.
    material_to_gltf: HashMap<usize, usize>,
    /// Maps scene texture index to glTF texture index.
    texture_to_gltf: HashMap<usize, usize>,
}

impl GltfBuilder {
    fn new() -> Self {
        Self {
            gltf: Gltf {
                asset: Asset {
                    version: "2.0".to_string(),
                    generator: Some("seed-io".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
            buffer_data: Vec::new(),
            geometry_to_mesh: HashMap::new(),
            material_to_gltf: HashMap::new(),
            texture_to_gltf: HashMap::new(),
        }
    }

    /// Add an accessor for Vec3 data (positions, normals).
    fn add_accessor_vec3(&mut self, data: &[Vec3], target: u32) -> usize {
        let byte_offset = self.buffer_data.len();

        // Compute min/max
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for v in data {
            min = min.min(*v);
            max = max.max(*v);
        }

        // Write data
        for v in data {
            self.buffer_data.extend_from_slice(&v.x.to_le_bytes());
            self.buffer_data.extend_from_slice(&v.y.to_le_bytes());
            self.buffer_data.extend_from_slice(&v.z.to_le_bytes());
        }

        let byte_length = self.buffer_data.len() - byte_offset;

        // Add buffer view
        let buffer_view_idx = self.gltf.buffer_views.len();
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset,
            byte_length,
            byte_stride: None,
            target: Some(target),
            name: None,
        });

        // Add accessor
        let accessor_idx = self.gltf.accessors.len();
        self.gltf.accessors.push(Accessor {
            buffer_view: Some(buffer_view_idx),
            byte_offset: 0,
            component_type: schema::COMPONENT_FLOAT,
            count: data.len(),
            accessor_type: "VEC3".to_string(),
            normalized: false,
            min: Some(vec![min.x as f64, min.y as f64, min.z as f64]),
            max: Some(vec![max.x as f64, max.y as f64, max.z as f64]),
            sparse: None,
            name: None,
        });

        accessor_idx
    }

    /// Add an accessor for Vec2 data (texcoords).
    fn add_accessor_vec2(&mut self, data: &[glam::Vec2]) -> usize {
        let byte_offset = self.buffer_data.len();

        // Write data
        for v in data {
            self.buffer_data.extend_from_slice(&v.x.to_le_bytes());
            self.buffer_data.extend_from_slice(&v.y.to_le_bytes());
        }

        let byte_length = self.buffer_data.len() - byte_offset;

        // Add buffer view
        let buffer_view_idx = self.gltf.buffer_views.len();
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset,
            byte_length,
            byte_stride: None,
            target: Some(TARGET_ARRAY_BUFFER),
            name: None,
        });

        // Add accessor
        let accessor_idx = self.gltf.accessors.len();
        self.gltf.accessors.push(Accessor {
            buffer_view: Some(buffer_view_idx),
            byte_offset: 0,
            component_type: schema::COMPONENT_FLOAT,
            count: data.len(),
            accessor_type: "VEC2".to_string(),
            normalized: false,
            min: None,
            max: None,
            sparse: None,
            name: None,
        });

        accessor_idx
    }

    /// Add an accessor for Vec4 data (colors).
    fn add_accessor_vec4(&mut self, data: &[glam::Vec4]) -> usize {
        let byte_offset = self.buffer_data.len();

        // Write data
        for v in data {
            self.buffer_data.extend_from_slice(&v.x.to_le_bytes());
            self.buffer_data.extend_from_slice(&v.y.to_le_bytes());
            self.buffer_data.extend_from_slice(&v.z.to_le_bytes());
            self.buffer_data.extend_from_slice(&v.w.to_le_bytes());
        }

        let byte_length = self.buffer_data.len() - byte_offset;

        // Add buffer view
        let buffer_view_idx = self.gltf.buffer_views.len();
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset,
            byte_length,
            byte_stride: None,
            target: Some(TARGET_ARRAY_BUFFER),
            name: None,
        });

        // Add accessor
        let accessor_idx = self.gltf.accessors.len();
        self.gltf.accessors.push(Accessor {
            buffer_view: Some(buffer_view_idx),
            byte_offset: 0,
            component_type: schema::COMPONENT_FLOAT,
            count: data.len(),
            accessor_type: "VEC4".to_string(),
            normalized: false,
            min: None,
            max: None,
            sparse: None,
            name: None,
        });

        accessor_idx
    }

    /// Add an accessor for index data.
    fn add_accessor_indices(&mut self, indices: &[u32]) -> usize {
        let byte_offset = self.buffer_data.len();

        // Determine the best component type based on max index
        let max_index = indices.iter().copied().max().unwrap_or(0);

        let (component_type, byte_length) = if max_index <= u16::MAX as u32 {
            // Use u16 indices
            for &idx in indices {
                self.buffer_data
                    .extend_from_slice(&(idx as u16).to_le_bytes());
            }
            (schema::COMPONENT_UNSIGNED_SHORT, indices.len() * 2)
        } else {
            // Use u32 indices
            for &idx in indices {
                self.buffer_data.extend_from_slice(&idx.to_le_bytes());
            }
            (schema::COMPONENT_UNSIGNED_INT, indices.len() * 4)
        };

        // Add buffer view
        let buffer_view_idx = self.gltf.buffer_views.len();
        self.gltf.buffer_views.push(BufferView {
            buffer: 0,
            byte_offset,
            byte_length,
            byte_stride: None,
            target: Some(TARGET_ELEMENT_ARRAY_BUFFER),
            name: None,
        });

        // Add accessor
        let accessor_idx = self.gltf.accessors.len();
        self.gltf.accessors.push(Accessor {
            buffer_view: Some(buffer_view_idx),
            byte_offset: 0,
            component_type,
            count: indices.len(),
            accessor_type: "SCALAR".to_string(),
            normalized: false,
            min: None,
            max: None,
            sparse: None,
            name: None,
        });

        accessor_idx
    }

    /// Convert a TriangleMesh to a glTF Mesh.
    fn add_mesh(&mut self, mesh: &TriangleMesh, material_idx: Option<usize>) -> usize {
        let mut attributes = HashMap::new();

        // Positions (required)
        let position_accessor = self.add_accessor_vec3(&mesh.positions, TARGET_ARRAY_BUFFER);
        attributes.insert("POSITION".to_string(), position_accessor);

        // Normals (optional)
        if let Some(ref normals) = mesh.normals {
            let normal_accessor = self.add_accessor_vec3(normals, TARGET_ARRAY_BUFFER);
            attributes.insert("NORMAL".to_string(), normal_accessor);
        }

        // Texcoords (optional)
        if let Some(ref texcoords) = mesh.texcoords {
            let texcoord_accessor = self.add_accessor_vec2(texcoords);
            attributes.insert("TEXCOORD_0".to_string(), texcoord_accessor);
        }

        // Vertex colors (optional)
        if let Some(ref colors) = mesh.colors {
            let color_accessor = self.add_accessor_vec4(colors);
            attributes.insert("COLOR_0".to_string(), color_accessor);
        }

        // Indices
        let indices_accessor = self.add_accessor_indices(&mesh.indices);

        // Create primitive
        let primitive = Primitive {
            attributes,
            indices: Some(indices_accessor),
            material: material_idx,
            mode: 4, // TRIANGLES
            targets: None,
        };

        // Add mesh
        let mesh_idx = self.gltf.meshes.len();
        self.gltf.meshes.push(Mesh {
            name: None,
            primitives: vec![primitive],
            weights: None,
        });

        mesh_idx
    }

    /// Convert a LineMesh to a glTF Mesh with LINES mode.
    fn add_line_mesh(&mut self, lines: &crate::scene::LineMesh, material_idx: Option<usize>) -> usize {
        let mut attributes = HashMap::new();

        // Positions (required)
        let position_accessor = self.add_accessor_vec3(&lines.positions, TARGET_ARRAY_BUFFER);
        attributes.insert("POSITION".to_string(), position_accessor);

        // Vertex colors (optional)
        if let Some(ref colors) = lines.colors {
            let color_accessor = self.add_accessor_vec4(colors);
            attributes.insert("COLOR_0".to_string(), color_accessor);
        }

        // Indices
        let indices_accessor = self.add_accessor_indices(&lines.indices);

        // Create primitive with LINES mode
        let primitive = Primitive {
            attributes,
            indices: Some(indices_accessor),
            material: material_idx,
            mode: 1, // LINES
            targets: None,
        };

        // Add mesh
        let mesh_idx = self.gltf.meshes.len();
        self.gltf.meshes.push(Mesh {
            name: None,
            primitives: vec![primitive],
            weights: None,
        });

        mesh_idx
    }

    /// Convert a scene Material to a glTF Material.
    fn add_material(&mut self, material: &Material, scene: &UnifiedScene) -> usize {
        // Convert textures first
        let base_color_texture = material
            .base_color_texture
            .as_ref()
            .map(|t| self.convert_texture_ref(t, scene));

        let metallic_roughness_texture = material
            .metallic_roughness_texture
            .as_ref()
            .map(|t| self.convert_texture_ref(t, scene));

        let normal_texture = material.normal_texture.as_ref().map(|t| {
            let tex_info = self.convert_texture_ref(t, scene);
            NormalTextureInfo {
                index: tex_info.index,
                tex_coord: tex_info.tex_coord,
                scale: material.normal_scale,
            }
        });

        let occlusion_texture = material.occlusion_texture.as_ref().map(|t| {
            let tex_info = self.convert_texture_ref(t, scene);
            OcclusionTextureInfo {
                index: tex_info.index,
                tex_coord: tex_info.tex_coord,
                strength: material.occlusion_strength,
            }
        });

        let emissive_texture = material
            .emissive_texture
            .as_ref()
            .map(|t| self.convert_texture_ref(t, scene));

        let gltf_material = GltfMaterial {
            name: if material.name.is_empty() {
                None
            } else {
                Some(material.name.clone())
            },
            pbr_metallic_roughness: Some(PbrMetallicRoughness {
                base_color_factor: [
                    material.base_color.x,
                    material.base_color.y,
                    material.base_color.z,
                    material.base_color.w,
                ],
                base_color_texture,
                metallic_factor: material.metallic,
                roughness_factor: material.roughness,
                metallic_roughness_texture,
            }),
            normal_texture,
            occlusion_texture,
            emissive_texture,
            emissive_factor: material.emissive,
            alpha_mode: match material.alpha_mode {
                AlphaMode::Opaque => "OPAQUE".to_string(),
                AlphaMode::Mask => "MASK".to_string(),
                AlphaMode::Blend => "BLEND".to_string(),
            },
            alpha_cutoff: material.alpha_cutoff,
            double_sided: material.double_sided,
        };

        let material_idx = self.gltf.materials.len();
        self.gltf.materials.push(gltf_material);
        material_idx
    }

    /// Convert a TextureRef to glTF TextureInfo.
    fn convert_texture_ref(&mut self, tex_ref: &TextureRef, scene: &UnifiedScene) -> TextureInfo {
        let gltf_texture_idx = self.ensure_texture(tex_ref.texture_index, scene);
        TextureInfo {
            index: gltf_texture_idx,
            tex_coord: tex_ref.texcoord,
        }
    }

    /// Ensure a scene texture is in the glTF and return its index.
    fn ensure_texture(&mut self, scene_tex_idx: usize, scene: &UnifiedScene) -> usize {
        if let Some(&idx) = self.texture_to_gltf.get(&scene_tex_idx) {
            return idx;
        }

        let scene_texture = &scene.textures[scene_tex_idx];
        let gltf_texture_idx = self.add_texture(scene_texture);
        self.texture_to_gltf.insert(scene_tex_idx, gltf_texture_idx);
        gltf_texture_idx
    }

    /// Add a texture to glTF.
    fn add_texture(&mut self, texture: &SceneTexture) -> usize {
        // Add image
        let image_idx = self.add_image(&texture.source);

        // Add sampler
        let sampler_idx = self.add_sampler(&texture.sampler);

        // Add texture
        let texture_idx = self.gltf.textures.len();
        self.gltf.textures.push(Texture {
            sampler: Some(sampler_idx),
            source: Some(image_idx),
            name: if texture.name.is_empty() {
                None
            } else {
                Some(texture.name.clone())
            },
        });

        texture_idx
    }

    /// Add an image to glTF.
    fn add_image(&mut self, source: &ImageSource) -> usize {
        let image = match source {
            ImageSource::Embedded { mime_type, data } => {
                // Store in buffer
                let byte_offset = self.buffer_data.len();
                self.buffer_data.extend_from_slice(data);
                let byte_length = data.len();

                // Add buffer view
                let buffer_view_idx = self.gltf.buffer_views.len();
                self.gltf.buffer_views.push(BufferView {
                    buffer: 0,
                    byte_offset,
                    byte_length,
                    byte_stride: None,
                    target: None, // Images don't have a target
                    name: None,
                });

                Image {
                    uri: None,
                    mime_type: Some(mime_type.clone()),
                    buffer_view: Some(buffer_view_idx),
                    name: None,
                }
            }
            ImageSource::External { uri } => Image {
                uri: Some(uri.clone()),
                mime_type: None,
                buffer_view: None,
                name: None,
            },
        };

        let image_idx = self.gltf.images.len();
        self.gltf.images.push(image);
        image_idx
    }

    /// Add a sampler to glTF.
    fn add_sampler(&mut self, sampler: &crate::scene::Sampler) -> usize {
        let gltf_sampler = Sampler {
            mag_filter: Some(match sampler.mag_filter {
                Filter::Nearest => 9728,
                Filter::Linear => 9729,
            }),
            min_filter: Some(match sampler.min_filter {
                Filter::Nearest => 9728,
                Filter::Linear => 9729,
            }),
            wrap_s: match sampler.wrap_u {
                Wrap::ClampToEdge => 33071,
                Wrap::Repeat => 10497,
                Wrap::MirroredRepeat => 33648,
            },
            wrap_t: match sampler.wrap_v {
                Wrap::ClampToEdge => 33071,
                Wrap::Repeat => 10497,
                Wrap::MirroredRepeat => 33648,
            },
            name: None,
        };

        let sampler_idx = self.gltf.samplers.len();
        self.gltf.samplers.push(gltf_sampler);
        sampler_idx
    }

    /// Convert a scene node to a glTF node.
    fn convert_node(
        &mut self,
        node_idx: usize,
        scene: &UnifiedScene,
        node_map: &mut HashMap<usize, usize>,
    ) -> usize {
        // Check if already converted
        if let Some(&gltf_idx) = node_map.get(&node_idx) {
            return gltf_idx;
        }

        let scene_node = &scene.nodes[node_idx];

        // Convert children first
        let children: Vec<usize> = scene_node
            .children
            .iter()
            .map(|&child_idx| self.convert_node(child_idx, scene, node_map))
            .collect();

        // Convert geometry if present
        let mesh = scene_node.geometry.map(|geom_idx| {
            if let Some(&mesh_idx) = self.geometry_to_mesh.get(&geom_idx) {
                mesh_idx
            } else {
                // Convert geometry to mesh
                let geometry = &scene.geometries[geom_idx];
                let material_idx = scene_node.material.map(|mat_idx| {
                    if let Some(&gltf_mat_idx) = self.material_to_gltf.get(&mat_idx) {
                        gltf_mat_idx
                    } else {
                        let gltf_mat_idx = self.add_material(&scene.materials[mat_idx], scene);
                        self.material_to_gltf.insert(mat_idx, gltf_mat_idx);
                        gltf_mat_idx
                    }
                });

                let mesh_idx = match geometry {
                    Geometry::Mesh(mesh) => self.add_mesh(mesh, material_idx),
                    Geometry::Lines(lines) => {
                        // Convert line mesh to glTF lines primitive
                        self.add_line_mesh(lines, material_idx)
                    }
                    Geometry::Brep(_) | Geometry::Nurbs(_) => {
                        // TODO: Tessellate B-rep/NURBS to mesh
                        // For now, create an empty mesh
                        let empty_mesh = TriangleMesh::default();
                        self.add_mesh(&empty_mesh, material_idx)
                    }
                    Geometry::Primitive(prim) => {
                        // Generate mesh from primitive with default subdivisions
                        let mesh = crate::convert::primitives::generate_primitive_mesh(prim, 16);
                        self.add_mesh(&mesh, material_idx)
                    }
                };

                self.geometry_to_mesh.insert(geom_idx, mesh_idx);
                mesh_idx
            }
        });

        // Decompose transform to TRS
        let (translation, rotation, scale) = decompose_transform(&scene_node.transform);

        let gltf_node = Node {
            name: if scene_node.name.is_empty() {
                None
            } else {
                Some(scene_node.name.clone())
            },
            children: if children.is_empty() {
                vec![]
            } else {
                children
            },
            mesh,
            camera: None,
            skin: None,
            matrix: None, // Use TRS instead
            translation: if translation != [0.0, 0.0, 0.0] {
                Some(translation)
            } else {
                None
            },
            rotation: if rotation != [0.0, 0.0, 0.0, 1.0] {
                Some(rotation)
            } else {
                None
            },
            scale: if scale != [1.0, 1.0, 1.0] {
                Some(scale)
            } else {
                None
            },
            weights: None,
        };

        let gltf_node_idx = self.gltf.nodes.len();
        self.gltf.nodes.push(gltf_node);
        node_map.insert(node_idx, gltf_node_idx);
        gltf_node_idx
    }

    /// Build the glTF structure from a UnifiedScene.
    fn build(&mut self, scene: &UnifiedScene) {
        // Convert all nodes
        let mut node_map = HashMap::new();

        // Convert root nodes and collect their glTF indices
        let root_indices: Vec<usize> = scene
            .roots
            .iter()
            .map(|&root_idx| self.convert_node(root_idx, scene, &mut node_map))
            .collect();

        // Create scene
        if !root_indices.is_empty() {
            self.gltf.scenes.push(Scene {
                name: scene.metadata.name.clone(),
                nodes: root_indices,
            });
            self.gltf.scene = Some(0);
        }

        // Add buffer if we have data
        if !self.buffer_data.is_empty() {
            self.gltf.buffers.push(Buffer {
                byte_length: self.buffer_data.len(),
                uri: None, // For GLB, no URI needed
                name: None,
            });
        }
    }
}

/// Decompose a Mat4 into translation, rotation (quaternion), and scale.
fn decompose_transform(transform: &Mat4) -> ([f32; 3], [f32; 4], [f32; 3]) {
    let (scale, rotation, translation) = transform.to_scale_rotation_translation();

    (
        [translation.x, translation.y, translation.z],
        [rotation.x, rotation.y, rotation.z, rotation.w],
        [scale.x, scale.y, scale.z],
    )
}

impl GltfWriter {
    fn write_glb(&self, scene: &UnifiedScene) -> Result<Vec<u8>> {
        let mut builder = GltfBuilder::new();
        builder.build(scene);

        // Serialize JSON
        let json_bytes = serde_json::to_vec(&builder.gltf)
            .map_err(|e| IoError::Internal(format!("Failed to serialize glTF JSON: {}", e)))?;

        // Pad JSON to 4-byte alignment
        let json_padding = (4 - (json_bytes.len() % 4)) % 4;
        let padded_json_len = json_bytes.len() + json_padding;

        // Pad binary to 4-byte alignment
        let bin_padding = (4 - (builder.buffer_data.len() % 4)) % 4;
        let padded_bin_len = builder.buffer_data.len() + bin_padding;

        // Calculate total size
        let has_bin = !builder.buffer_data.is_empty();
        let total_size = 12  // GLB header
            + 8 + padded_json_len  // JSON chunk
            + if has_bin { 8 + padded_bin_len } else { 0 }; // BIN chunk (optional)

        let mut output = Vec::with_capacity(total_size);

        // GLB header
        output.extend_from_slice(&GLB_MAGIC.to_le_bytes());
        output.extend_from_slice(&GLB_VERSION.to_le_bytes());
        output.extend_from_slice(&(total_size as u32).to_le_bytes());

        // JSON chunk
        output.extend_from_slice(&(padded_json_len as u32).to_le_bytes());
        output.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        output.extend_from_slice(&json_bytes);
        output.extend(std::iter::repeat(0x20u8).take(json_padding)); // Space padding

        // BIN chunk (if we have data)
        if has_bin {
            output.extend_from_slice(&(padded_bin_len as u32).to_le_bytes());
            output.extend_from_slice(&CHUNK_BIN.to_le_bytes());
            output.extend_from_slice(&builder.buffer_data);
            output.extend(std::iter::repeat(0u8).take(bin_padding)); // Zero padding
        }

        Ok(output)
    }

    fn write_json(&self, scene: &UnifiedScene, pretty: bool) -> Result<Vec<u8>> {
        let mut builder = GltfBuilder::new();
        builder.build(scene);

        // For JSON output, embed buffer as base64 data URI
        if !builder.buffer_data.is_empty() {
            use base64::{engine::general_purpose::STANDARD, Engine};
            let base64_data = STANDARD.encode(&builder.buffer_data);
            let data_uri = format!("data:application/octet-stream;base64,{}", base64_data);

            if let Some(buffer) = builder.gltf.buffers.first_mut() {
                buffer.uri = Some(data_uri);
            }
        }

        // Serialize JSON
        let json_bytes = if pretty {
            serde_json::to_vec_pretty(&builder.gltf)
        } else {
            serde_json::to_vec(&builder.gltf)
        }
        .map_err(|e| IoError::Internal(format!("Failed to serialize glTF JSON: {}", e)))?;

        Ok(json_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::FormatReader;
    use crate::scene::{Geometry, SceneNode};
    use glam::{Vec2, Vec3, Vec4};

    #[test]
    fn test_write_empty_scene() {
        let writer = GltfWriter::new();
        let scene = UnifiedScene::new();

        let options = WriteOptions {
            binary: true,
            ..Default::default()
        };
        let result = writer.write(&scene, &options);
        assert!(result.is_ok());

        let glb = result.unwrap();
        // Check GLB magic
        assert_eq!(&glb[0..4], &GLB_MAGIC.to_le_bytes());
    }

    #[test]
    fn test_write_simple_mesh() {
        let writer = GltfWriter::new();
        let mut scene = UnifiedScene::new();

        // Create a simple triangle
        let mesh = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 1.0, 0.0),
            ],
            normals: Some(vec![
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 1.0),
            ]),
            texcoords: Some(vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.5, 1.0),
            ]),
            colors: None,
            indices: vec![0, 1, 2],
            cached_bounds: None,
        };

        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));
        scene.add_root(SceneNode::with_geometry("Triangle", geom_idx));

        let options = WriteOptions {
            binary: true,
            ..Default::default()
        };
        let result = writer.write(&scene, &options);
        assert!(result.is_ok());

        let glb = result.unwrap();

        // Verify we can read it back
        let reader = super::super::reader::GltfReader::new();
        assert!(reader.can_read(&glb));

        let read_result = reader.read(&glb, &crate::registry::ReadOptions::default());
        assert!(read_result.is_ok());

        let read_scene = read_result.unwrap();
        assert_eq!(read_scene.nodes.len(), 1);
        assert_eq!(read_scene.geometries.len(), 1);
    }

    #[test]
    fn test_write_with_material() {
        let writer = GltfWriter::new();
        let mut scene = UnifiedScene::new();

        // Create a triangle
        let mesh = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 1.0, 0.0),
            ],
            normals: None,
            texcoords: None,
            colors: None,
            indices: vec![0, 1, 2],
            cached_bounds: None,
        };

        // Create a red material
        let material = Material {
            name: "Red".to_string(),
            base_color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            metallic: 0.5,
            roughness: 0.3,
            ..Default::default()
        };

        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));
        let mat_idx = scene.add_material(material);
        scene.add_root(
            SceneNode::with_geometry("Triangle", geom_idx).with_material(mat_idx),
        );

        let options = WriteOptions {
            binary: true,
            ..Default::default()
        };
        let result = writer.write(&scene, &options);
        assert!(result.is_ok());

        // Read it back and verify material
        let reader = super::super::reader::GltfReader::new();
        let read_scene = reader
            .read(&result.unwrap(), &crate::registry::ReadOptions::default())
            .unwrap();

        assert_eq!(read_scene.materials.len(), 1);
        let mat = &read_scene.materials[0];
        assert_eq!(mat.name, "Red");
        assert!((mat.base_color.x - 1.0).abs() < 0.001);
        assert!((mat.metallic - 0.5).abs() < 0.001);
        assert!((mat.roughness - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_write_json_format() {
        let writer = GltfWriter::new();
        let mut scene = UnifiedScene::new();

        let mesh = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 1.0, 0.0),
            ],
            normals: None,
            texcoords: None,
            colors: None,
            indices: vec![0, 1, 2],
            cached_bounds: None,
        };

        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));
        scene.add_root(SceneNode::with_geometry("Triangle", geom_idx));

        let options = WriteOptions {
            binary: false,
            pretty: true,
            ..Default::default()
        };

        let result = writer.write(&scene, &options);
        assert!(result.is_ok());

        let json = String::from_utf8(result.unwrap()).unwrap();
        assert!(json.contains("\"asset\""));
        assert!(json.contains("\"version\": \"2.0\""));
        assert!(json.contains("data:application/octet-stream;base64,"));
    }

    #[test]
    fn test_roundtrip_hierarchy() {
        let writer = GltfWriter::new();
        let mut scene = UnifiedScene::new();

        // Create a hierarchy: root -> child1, child2
        let root_idx = scene.add_root(SceneNode::new("Root"));
        scene.add_child(root_idx, SceneNode::new("Child1"));
        scene.add_child(root_idx, SceneNode::new("Child2"));

        let options = WriteOptions {
            binary: true,
            ..Default::default()
        };
        let glb = writer.write(&scene, &options).unwrap();

        let reader = super::super::reader::GltfReader::new();
        let read_scene = reader
            .read(&glb, &crate::registry::ReadOptions::default())
            .unwrap();

        assert_eq!(read_scene.roots.len(), 1);
        let root_node = &read_scene.nodes[read_scene.roots[0]];
        assert_eq!(root_node.name, "Root");
        assert_eq!(root_node.children.len(), 2);
    }

    #[test]
    fn test_transform_decomposition() {
        // Test identity
        let (t, r, s) = decompose_transform(&Mat4::IDENTITY);
        assert_eq!(t, [0.0, 0.0, 0.0]);
        assert_eq!(r, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(s, [1.0, 1.0, 1.0]);

        // Test translation
        let transform = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let (t, _, _) = decompose_transform(&transform);
        assert!((t[0] - 1.0).abs() < 0.001);
        assert!((t[1] - 2.0).abs() < 0.001);
        assert!((t[2] - 3.0).abs() < 0.001);

        // Test scale
        let transform = Mat4::from_scale(Vec3::new(2.0, 3.0, 4.0));
        let (_, _, s) = decompose_transform(&transform);
        assert!((s[0] - 2.0).abs() < 0.001);
        assert!((s[1] - 3.0).abs() < 0.001);
        assert!((s[2] - 4.0).abs() < 0.001);
    }
}
