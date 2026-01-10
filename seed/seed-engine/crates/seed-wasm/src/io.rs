//! WebAssembly bindings for seed-io file format support.
//!
//! Provides JavaScript API for reading and writing various 3D file formats:
//! - Seed (.seed) - Native format
//! - glTF 2.0 (.gltf, .glb) - Web-friendly 3D format
//! - STEP (.step, .stp) - CAD interchange format
//! - USD (.usda, .usdc) - VFX/animation format

use wasm_bindgen::prelude::*;
use seed_io::{
    FormatRegistry,
    UnifiedScene,
    ReadOptions,
    WriteOptions,
};
use glam;

/// File format converter for 3D assets.
///
/// Supports reading and writing multiple 3D file formats with automatic
/// format detection and conversion between formats.
///
/// ## Example (JavaScript)
/// ```js
/// import { FileConverter } from 'seed-engine';
///
/// const converter = new FileConverter();
///
/// // Read a glTF file
/// const scene = converter.read(gltfBytes);
///
/// // Convert to STEP format
/// const stepBytes = converter.write(scene, 'step');
///
/// // Or use the convenience method
/// const stepBytes = converter.convert(gltfBytes, 'step');
/// ```
#[wasm_bindgen]
pub struct FileConverter {
    registry: FormatRegistry,
}

#[wasm_bindgen]
impl FileConverter {
    /// Create a new file converter with all supported formats.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            registry: FormatRegistry::with_defaults(),
        }
    }

    /// Get list of supported read formats.
    #[wasm_bindgen(js_name = supportedReadFormats)]
    pub fn supported_read_formats(&self) -> Vec<String> {
        self.registry.reader_formats().map(|s| s.to_string()).collect()
    }

    /// Get list of supported write formats.
    #[wasm_bindgen(js_name = supportedWriteFormats)]
    pub fn supported_write_formats(&self) -> Vec<String> {
        self.registry.writer_formats().map(|s| s.to_string()).collect()
    }

    /// Detect the format of file data.
    ///
    /// Returns the format name if detected, or null if unknown.
    #[wasm_bindgen(js_name = detectFormat)]
    pub fn detect_format(&self, data: &[u8]) -> Option<String> {
        // Try to detect format by checking each reader
        for format in self.registry.reader_formats() {
            if let Some(reader) = self.registry.get_reader(format) {
                if reader.can_read(data) {
                    return Some(format.to_string());
                }
            }
        }
        None
    }

    /// Read a 3D file and return scene data as JSON.
    ///
    /// The format is auto-detected from the file contents.
    #[wasm_bindgen]
    pub fn read(&self, data: &[u8]) -> Result<JsValue, JsError> {
        let options = ReadOptions::default();
        let scene = self.registry.read(data, &options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        scene_to_js(&scene)
    }

    /// Read a 3D file with filename for metadata.
    ///
    /// The filename is stored in scene metadata for use when converting to Seed format.
    #[wasm_bindgen(js_name = readWithFilename)]
    pub fn read_with_filename(&self, data: &[u8], filename: &str) -> Result<JsValue, JsError> {
        let options = ReadOptions::default();
        let mut scene = self.registry.read(data, &options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        // Store the filename in scene metadata
        scene.metadata.source_path = Some(filename.to_string());

        scene_to_js(&scene)
    }

    /// Read a 3D file with explicit format hint.
    #[wasm_bindgen(js_name = readAs)]
    pub fn read_as(&self, data: &[u8], format: &str) -> Result<JsValue, JsError> {
        let options = ReadOptions::default();
        let scene = self.registry.read_as(data, format, &options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        scene_to_js(&scene)
    }

    /// Write scene data to a specific format.
    ///
    /// Takes scene JSON (from read()) and returns file bytes.
    #[wasm_bindgen]
    pub fn write(&self, scene_js: JsValue, format: &str) -> Result<Vec<u8>, JsError> {
        let scene = scene_from_js(scene_js)?;
        let options = WriteOptions::default();

        self.registry.write(&scene, format, &options)
            .map_err(|e| JsError::new(&format!("Write error: {}", e)))
    }

    /// Write scene data with custom options.
    #[wasm_bindgen(js_name = writeWithOptions)]
    pub fn write_with_options(
        &self,
        scene_js: JsValue,
        format: &str,
        options_js: JsValue,
    ) -> Result<Vec<u8>, JsError> {
        let scene = scene_from_js(scene_js)?;
        let options = write_options_from_js(options_js)?;

        self.registry.write(&scene, format, &options)
            .map_err(|e| JsError::new(&format!("Write error: {}", e)))
    }

    /// Convert file data from one format to another.
    ///
    /// This is a convenience method that combines read() and write().
    #[wasm_bindgen]
    pub fn convert(&self, data: &[u8], target_format: &str) -> Result<Vec<u8>, JsError> {
        let read_options = ReadOptions::default();
        let write_options = WriteOptions::default();

        let scene = self.registry.read(data, &read_options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        self.registry.write(&scene, target_format, &write_options)
            .map_err(|e| JsError::new(&format!("Write error: {}", e)))
    }

    /// Convert with explicit source format.
    #[wasm_bindgen(js_name = convertFrom)]
    pub fn convert_from(
        &self,
        data: &[u8],
        source_format: &str,
        target_format: &str,
    ) -> Result<Vec<u8>, JsError> {
        let read_options = ReadOptions::default();
        let write_options = WriteOptions::default();

        let scene = self.registry.read_as(data, source_format, &read_options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        self.registry.write(&scene, target_format, &write_options)
            .map_err(|e| JsError::new(&format!("Write error: {}", e)))
    }

    /// Get scene statistics (node count, geometry count, etc.).
    #[wasm_bindgen(js_name = getSceneStats)]
    pub fn get_scene_stats(&self, data: &[u8]) -> Result<JsValue, JsError> {
        let options = ReadOptions::default();
        let scene = self.registry.read(data, &options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        let stats = SceneStats {
            node_count: scene.nodes.len(),
            geometry_count: scene.geometries.len(),
            material_count: scene.materials.len(),
            root_count: scene.roots.len(),
        };

        serde_wasm_bindgen::to_value(&stats)
            .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
    }

    /// Get mesh data for rendering (positions and indices).
    #[wasm_bindgen(js_name = getMeshData)]
    pub fn get_mesh_data(&self, data: &[u8]) -> Result<JsValue, JsError> {
        let options = ReadOptions::default();
        let scene = self.registry.read(data, &options)
            .map_err(|e| JsError::new(&format!("Read error: {}", e)))?;

        // Collect all mesh data
        let mut all_positions: Vec<f32> = Vec::new();
        let mut all_indices: Vec<u32> = Vec::new();
        let mut index_offset: u32 = 0;

        for geom in &scene.geometries {
            if let seed_io::scene::Geometry::Mesh(mesh) = geom {
                // Add positions (convert Vec3 to flat array)
                for pos in &mesh.positions {
                    all_positions.push(pos.x);
                    all_positions.push(pos.y);
                    all_positions.push(pos.z);
                }

                // Add indices with offset
                for idx in &mesh.indices {
                    all_indices.push(*idx + index_offset);
                }

                index_offset += mesh.positions.len() as u32;
            }
        }

        // Compute bounding box for camera positioning
        let (min, max) = compute_bounds(&all_positions);

        let mesh_data = MeshDataJs {
            positions: all_positions,
            indices: all_indices,
            bounds_min: min,
            bounds_max: max,
        };

        serde_wasm_bindgen::to_value(&mesh_data)
            .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
    }
}

impl Default for FileConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Scene statistics returned by getSceneStats().
#[derive(serde::Serialize)]
struct SceneStats {
    node_count: usize,
    geometry_count: usize,
    material_count: usize,
    root_count: usize,
}

/// Mesh data for JavaScript rendering.
#[derive(serde::Serialize)]
struct MeshDataJs {
    positions: Vec<f32>,
    indices: Vec<u32>,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

/// Compute bounding box from flat position array.
fn compute_bounds(positions: &[f32]) -> ([f32; 3], [f32; 3]) {
    if positions.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    for chunk in positions.chunks(3) {
        if chunk.len() == 3 {
            for i in 0..3 {
                min[i] = min[i].min(chunk[i]);
                max[i] = max[i].max(chunk[i]);
            }
        }
    }

    (min, max)
}

/// Scene node for JavaScript.
#[derive(serde::Serialize, serde::Deserialize)]
struct SceneNodeJs {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    geometry: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<usize>,
    children: Vec<usize>,
    transform: [f32; 16],
}

/// Geometry for JavaScript.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
enum GeometryJs {
    Mesh {
        vertex_count: usize,
        triangle_count: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        bounds_min: Option<[f32; 3]>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bounds_max: Option<[f32; 3]>,
    },
    Primitive {
        primitive_type: String,
    },
    Brep {
        face_count: usize,
    },
    Nurbs,
}

/// Material for JavaScript.
#[derive(serde::Serialize, serde::Deserialize)]
struct MaterialJs {
    name: String,
    base_color: [f32; 4],
    metallic: f32,
    roughness: f32,
}

/// Complete scene for JavaScript.
#[derive(serde::Serialize, serde::Deserialize)]
struct UnifiedSceneJs {
    nodes: Vec<SceneNodeJs>,
    roots: Vec<usize>,
    geometries: Vec<GeometryJs>,
    materials: Vec<MaterialJs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_format: Option<String>,
}

/// Convert UnifiedScene to JavaScript-friendly format.
fn scene_to_js(scene: &UnifiedScene) -> Result<JsValue, JsError> {
    let nodes: Vec<SceneNodeJs> = scene.nodes.iter().map(|node| {
        SceneNodeJs {
            name: node.name.clone(),
            geometry: node.geometry,
            material: node.material,
            children: node.children.clone(),
            transform: node.transform.to_cols_array(),
        }
    }).collect();

    let geometries: Vec<GeometryJs> = scene.geometries.iter().map(|geom| {
        use seed_io::scene::Geometry;
        match geom {
            Geometry::Mesh(mesh) => {
                // Compute bounds from positions
                let (bounds_min, bounds_max) = if !mesh.positions.is_empty() {
                    let mut min = [f32::MAX; 3];
                    let mut max = [f32::MIN; 3];
                    for pos in &mesh.positions {
                        min[0] = min[0].min(pos.x);
                        min[1] = min[1].min(pos.y);
                        min[2] = min[2].min(pos.z);
                        max[0] = max[0].max(pos.x);
                        max[1] = max[1].max(pos.y);
                        max[2] = max[2].max(pos.z);
                    }
                    (Some(min), Some(max))
                } else {
                    (None, None)
                };

                GeometryJs::Mesh {
                    vertex_count: mesh.positions.len(),
                    triangle_count: mesh.indices.len() / 3,
                    bounds_min,
                    bounds_max,
                }
            }
            Geometry::Primitive(prim) => {
                use seed_io::scene::PrimitiveGeometry;
                let ptype = match prim {
                    PrimitiveGeometry::Box { .. } => "box",
                    PrimitiveGeometry::Sphere { .. } => "sphere",
                    PrimitiveGeometry::Cylinder { .. } => "cylinder",
                    PrimitiveGeometry::Cone { .. } => "cone",
                    PrimitiveGeometry::Torus { .. } => "torus",
                    PrimitiveGeometry::Capsule { .. } => "capsule",
                };
                GeometryJs::Primitive {
                    primitive_type: ptype.to_string(),
                }
            }
            Geometry::Brep(brep) => GeometryJs::Brep {
                face_count: brep.faces.len(),
            },
            Geometry::Nurbs(_) => GeometryJs::Nurbs,
        }
    }).collect();

    let materials: Vec<MaterialJs> = scene.materials.iter().map(|mat| {
        MaterialJs {
            name: mat.name.clone(),
            base_color: mat.base_color.to_array(),
            metallic: mat.metallic,
            roughness: mat.roughness,
        }
    }).collect();

    let scene_js = UnifiedSceneJs {
        nodes,
        roots: scene.roots.clone(),
        geometries,
        materials,
        source_path: scene.metadata.source_path.clone(),
        source_format: scene.metadata.source_format.clone(),
    };

    serde_wasm_bindgen::to_value(&scene_js)
        .map_err(|e| JsError::new(&format!("Serialization error: {}", e)))
}

/// Convert JavaScript scene object to UnifiedScene.
fn scene_from_js(js: JsValue) -> Result<UnifiedScene, JsError> {
    let scene_js: UnifiedSceneJs = serde_wasm_bindgen::from_value(js)
        .map_err(|e| JsError::new(&format!("Invalid scene data: {}", e)))?;

    let mut scene = UnifiedScene::new();

    // Add materials first
    for mat_js in &scene_js.materials {
        let mut mat = seed_io::scene::Material::new(&mat_js.name);
        mat.base_color = glam::Vec4::from_array(mat_js.base_color);
        mat.metallic = mat_js.metallic;
        mat.roughness = mat_js.roughness;
        scene.add_material(mat);
    }

    // Add geometries
    for geom_js in &scene_js.geometries {
        let geom = match geom_js {
            GeometryJs::Primitive { primitive_type } => {
                use seed_io::scene::{Geometry, PrimitiveGeometry};
                match primitive_type.as_str() {
                    "box" => Geometry::Primitive(PrimitiveGeometry::Box {
                        half_extents: glam::Vec3::splat(0.5),
                        transform: glam::Mat4::IDENTITY,
                    }),
                    "sphere" => Geometry::Primitive(PrimitiveGeometry::Sphere {
                        radius: 0.5,
                        transform: glam::Mat4::IDENTITY,
                    }),
                    "cylinder" => Geometry::Primitive(PrimitiveGeometry::Cylinder {
                        radius: 0.5,
                        height: 1.0,
                        transform: glam::Mat4::IDENTITY,
                    }),
                    _ => Geometry::Primitive(PrimitiveGeometry::Box {
                        half_extents: glam::Vec3::splat(0.5),
                        transform: glam::Mat4::IDENTITY,
                    }),
                }
            }
            GeometryJs::Mesh { bounds_min, bounds_max, .. } => {
                // Create mesh with cached bounds (positions are not serialized to JS)
                let mut mesh = seed_io::scene::TriangleMesh::default();
                if let (Some(min), Some(max)) = (bounds_min, bounds_max) {
                    mesh.cached_bounds = Some(seed_io::scene::BoundingBox {
                        min: glam::Vec3::from_array(*min),
                        max: glam::Vec3::from_array(*max),
                    });
                }
                seed_io::scene::Geometry::Mesh(mesh)
            }
            GeometryJs::Brep { .. } => {
                seed_io::scene::Geometry::Brep(seed_io::scene::BrepGeometry::default())
            }
            GeometryJs::Nurbs => {
                seed_io::scene::Geometry::Nurbs(seed_io::scene::NurbsGeometry::default())
            }
        };
        scene.add_geometry(geom);
    }

    // Add nodes
    for node_js in &scene_js.nodes {
        let mut node = seed_io::scene::SceneNode::new(&node_js.name);
        node.geometry = node_js.geometry;
        node.material = node_js.material;
        node.transform = glam::Mat4::from_cols_array(&node_js.transform);
        scene.nodes.push(node);
    }

    // Set up parent-child relationships
    for (idx, node_js) in scene_js.nodes.iter().enumerate() {
        scene.nodes[idx].children = node_js.children.clone();
    }

    scene.roots = scene_js.roots;

    // Restore metadata
    scene.metadata.source_path = scene_js.source_path;
    scene.metadata.source_format = scene_js.source_format;

    Ok(scene)
}

/// Convert JavaScript options to WriteOptions.
fn write_options_from_js(js: JsValue) -> Result<WriteOptions, JsError> {
    #[derive(serde::Deserialize)]
    struct WriteOptionsJs {
        #[serde(default)]
        pretty: bool,
        #[serde(default)]
        binary: bool,
    }

    let opts: WriteOptionsJs = serde_wasm_bindgen::from_value(js)
        .map_err(|e| JsError::new(&format!("Invalid options: {}", e)))?;

    Ok(WriteOptions {
        pretty: opts.pretty,
        binary: opts.binary,
        ..Default::default()
    })
}

// Standalone functions for simpler usage

/// Read a 3D file and return scene data.
///
/// Auto-detects format from file contents.
#[wasm_bindgen(js_name = readFile)]
pub fn read_file(data: &[u8]) -> Result<JsValue, JsError> {
    let converter = FileConverter::new();
    converter.read(data)
}

/// Convert a 3D file to another format.
///
/// Auto-detects source format from file contents.
#[wasm_bindgen(js_name = convertFile)]
pub fn convert_file(data: &[u8], target_format: &str) -> Result<Vec<u8>, JsError> {
    let converter = FileConverter::new();
    converter.convert(data, target_format)
}

/// Detect the format of a 3D file.
#[wasm_bindgen(js_name = detectFileFormat)]
pub fn detect_file_format(data: &[u8]) -> Option<String> {
    let converter = FileConverter::new();
    converter.detect_format(data)
}

/// Get list of all supported formats for reading.
#[wasm_bindgen(js_name = getSupportedReadFormats)]
pub fn get_supported_read_formats() -> Vec<String> {
    let converter = FileConverter::new();
    converter.supported_read_formats()
}

/// Get list of all supported formats for writing.
#[wasm_bindgen(js_name = getSupportedWriteFormats)]
pub fn get_supported_write_formats() -> Vec<String> {
    let converter = FileConverter::new();
    converter.supported_write_formats()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_converter_new() {
        let converter = FileConverter::new();
        let read_formats = converter.supported_read_formats();
        let write_formats = converter.supported_write_formats();

        assert!(read_formats.contains(&"seed".to_string()));
        assert!(read_formats.contains(&"gltf".to_string()));
        assert!(read_formats.contains(&"step".to_string()));
        assert!(read_formats.contains(&"usd".to_string()));

        assert!(write_formats.contains(&"seed".to_string()));
        assert!(write_formats.contains(&"gltf".to_string()));
        assert!(write_formats.contains(&"step".to_string()));
        assert!(write_formats.contains(&"usd".to_string()));
    }

    #[test]
    fn test_detect_format() {
        let converter = FileConverter::new();

        // glTF JSON
        let gltf_data = br#"{"asset":{"version":"2.0"}}"#;
        assert_eq!(converter.detect_format(gltf_data), Some("gltf".to_string()));

        // STEP
        let step_data = b"ISO-10303-21;\nHEADER;";
        assert_eq!(converter.detect_format(step_data), Some("step".to_string()));

        // USD ASCII
        let usda_data = b"#usda 1.0\n";
        assert_eq!(converter.detect_format(usda_data), Some("usd".to_string()));

        // Seed
        let seed_data = b"@meta:\n  profile: Seed/3D";
        assert_eq!(converter.detect_format(seed_data), Some("seed".to_string()));
    }
}
