//! USD reader implementation.

use crate::error::{IoError, Result};
use crate::registry::{FormatReader, ReadOptions};
use crate::scene::{Axis, Geometry, Material, SceneNode, TriangleMesh, UnifiedScene};

use super::usda::{
    array_to_int_array, array_to_vec2f_array, array_to_vec3f_array, parse_stage, UsdPrim, UsdStage,
    UsdValue,
};
use super::usdc;

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;

/// Reader for USD files.
pub struct UsdReader;

impl UsdReader {
    /// Create a new USD reader.
    pub fn new() -> Self {
        Self
    }
}

impl Default for UsdReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatReader for UsdReader {
    fn name(&self) -> &'static str {
        "usd"
    }

    fn extensions(&self) -> &[&'static str] {
        &["usd", "usda", "usdc", "usdz"]
    }

    fn can_read(&self, data: &[u8]) -> bool {
        // Check for USDC binary magic
        if usdc::is_usdc(data) {
            return true;
        }

        // Check for USDZ (ZIP archive with USD files) - before USDA since ZIP is valid UTF-8
        if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
            return true;
        }

        // Check for USDA text format
        if let Ok(text) = std::str::from_utf8(data) {
            let trimmed = text.trim_start();
            if trimmed.starts_with("#usda") {
                return true;
            }
        }

        false
    }

    fn read(&self, data: &[u8], _options: &ReadOptions) -> Result<UnifiedScene> {
        // Detect format
        if usdc::is_usdc(data) {
            self.read_usdc(data)
        } else if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
            self.read_usdz(data)
        } else {
            self.read_usda(data)
        }
    }
}

impl UsdReader {
    fn read_usda(&self, data: &[u8]) -> Result<UnifiedScene> {
        let text = std::str::from_utf8(data)
            .map_err(|e| IoError::parse(format!("Invalid UTF-8: {}", e)))?;

        let (_, stage) = parse_stage(text)
            .map_err(|e| IoError::parse(format!("USDA parse error: {:?}", e)))?;

        self.convert_stage(&stage)
    }

    fn read_usdc(&self, data: &[u8]) -> Result<UnifiedScene> {
        // Parse USDC header to verify it's valid
        let _header = usdc::parse_header(data)
            .ok_or_else(|| IoError::parse("Invalid USDC header"))?;

        // USDC binary parsing is complex - for now return error
        // A full implementation would parse the Crate format tables
        Err(IoError::Unsupported(
            "USDC binary parsing not yet fully implemented. Use USDA format.".into(),
        ))
    }

    fn read_usdz(&self, data: &[u8]) -> Result<UnifiedScene> {
        // USDZ is a ZIP archive containing USD files
        // Find the first .usda or .usdc file in the archive

        // Simple ZIP parsing - look for local file headers
        let mut offset = 0;
        while offset + 30 < data.len() {
            // Check local file header signature
            if &data[offset..offset + 4] != b"PK\x03\x04" {
                break;
            }

            // Parse local file header
            let compressed_size =
                u32::from_le_bytes([data[offset + 18], data[offset + 19], data[offset + 20], data[offset + 21]])
                    as usize;
            let uncompressed_size =
                u32::from_le_bytes([data[offset + 22], data[offset + 23], data[offset + 24], data[offset + 25]])
                    as usize;
            let name_len =
                u16::from_le_bytes([data[offset + 26], data[offset + 27]]) as usize;
            let extra_len =
                u16::from_le_bytes([data[offset + 28], data[offset + 29]]) as usize;

            let name_start = offset + 30;
            let name_end = name_start + name_len;

            if name_end > data.len() {
                break;
            }

            let file_name = std::str::from_utf8(&data[name_start..name_end]).unwrap_or("");
            let data_start = name_end + extra_len;
            let data_end = data_start + compressed_size;

            // Check if this is a USD file
            if file_name.ends_with(".usda") || file_name.ends_with(".usd") {
                if data_end <= data.len() && compressed_size == uncompressed_size {
                    // Uncompressed - read directly
                    return self.read_usda(&data[data_start..data_end]);
                }
            }

            // Move to next file
            offset = data_end;
        }

        Err(IoError::parse("No readable USD file found in USDZ archive"))
    }

    /// Convert a parsed USD stage to UnifiedScene.
    fn convert_stage(&self, stage: &UsdStage) -> Result<UnifiedScene> {
        let mut scene = UnifiedScene::new();

        // Set metadata before borrowing scene for conversion
        if let Some(name) = &stage.default_prim {
            scene.metadata.name = Some(name.clone());
        }

        // Handle up axis
        if let Some(up) = &stage.up_axis {
            scene.metadata.up_axis = match up.to_uppercase().as_str() {
                "X" => Axis::X,
                "Y" => Axis::Y,
                "Z" => Axis::Z,
                _ => Axis::Y, // Default
            };
        }

        // Convert root prims and collect their indices
        // Skip "Materials" scope which is just a container for material definitions
        let root_indices: Vec<usize> = {
            let mut converter = UsdConverter::new(&mut scene);
            let mut indices = Vec::new();
            for prim in &stage.root_prims {
                // Skip Materials scope - it's just a container for materials
                let prim_name = prim.path.rsplit('/').next().unwrap_or(&prim.path);
                if prim_name == "Materials" && prim.type_name == "Scope" {
                    // Still convert to process materials within, but don't add to roots
                    converter.convert_prim(prim, None)?;
                    continue;
                }
                if let Some(node_idx) = converter.convert_prim(prim, None)? {
                    indices.push(node_idx);
                }
            }
            indices
        };

        // Set roots after converter is dropped
        scene.roots = root_indices;

        Ok(scene)
    }
}

/// Converter from USD prims to UnifiedScene.
struct UsdConverter<'a> {
    scene: &'a mut UnifiedScene,
    /// Material path -> material index mapping
    material_map: HashMap<String, usize>,
}

impl<'a> UsdConverter<'a> {
    fn new(scene: &'a mut UnifiedScene) -> Self {
        Self {
            scene,
            material_map: HashMap::new(),
        }
    }

    /// Convert a USD prim to a scene node.
    /// Returns None for prims that shouldn't become scene nodes (Materials, Shaders).
    fn convert_prim(&mut self, prim: &UsdPrim, parent: Option<usize>) -> Result<Option<usize>> {
        // Extract name from path
        let name = prim
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&prim.path)
            .to_string();

        // Handle Material/Shader prims specially - they don't become scene nodes
        match prim.type_name.as_str() {
            "Material" => {
                // Convert material and store in map
                if let Some(mat) = self.convert_material(prim) {
                    let mat_idx = self.scene.add_material(mat);
                    self.material_map.insert(prim.path.clone(), mat_idx);
                }
                // Process child shaders but don't create a node
                for child in &prim.children {
                    self.convert_prim(child, None)?;
                }
                return Ok(None);
            }
            "Shader" => {
                // Shaders are processed as part of Material conversion
                return Ok(None);
            }
            _ => {}
        }

        let mut node = SceneNode::new(&name);

        // Extract transform from xformOp attributes
        node.transform = self.extract_transform(prim);

        // Handle different prim types
        match prim.type_name.as_str() {
            "Mesh" => {
                if let Some(mesh) = self.convert_mesh(prim)? {
                    let geom_idx = self.scene.add_geometry(Geometry::Mesh(mesh));
                    node.geometry = Some(geom_idx);
                }

                // Try to find and apply material
                if let Some(mat_idx) = self.find_material_binding(prim) {
                    node.material = Some(mat_idx);
                }
            }
            "Xform" | "Scope" | "Group" | "" => {
                // Transform/group nodes - just process children
            }
            "Camera" => {
                // Skip cameras for now
            }
            "DistantLight" | "DomeLight" | "SphereLight" | "RectLight" => {
                // Skip lights for now
            }
            "Cube" | "Sphere" | "Cylinder" | "Cone" | "Capsule" => {
                // Generate primitive mesh
                if let Some(mesh) = self.convert_primitive(prim)? {
                    let geom_idx = self.scene.add_geometry(Geometry::Mesh(mesh));
                    node.geometry = Some(geom_idx);
                }
            }
            _ => {
                // Unknown type - treat as group
            }
        }

        // Add node to scene
        let node_idx = self.scene.nodes.len();
        self.scene.nodes.push(node);

        // Update parent if exists
        if let Some(parent_idx) = parent {
            self.scene.nodes[parent_idx].children.push(node_idx);
        }

        // Process children
        for child in &prim.children {
            if let Some(child_idx) = self.convert_prim(child, Some(node_idx))? {
                // Child was added and linked via parent_idx already
                let _ = child_idx;
            }
        }

        Ok(Some(node_idx))
    }

    /// Extract transform from USD xformOp attributes.
    fn extract_transform(&self, prim: &UsdPrim) -> Mat4 {
        let mut transform = Mat4::IDENTITY;

        // Look for common transform attributes
        for attr in &prim.attributes {
            match attr.name.as_str() {
                "xformOp:transform" => {
                    if let Some(UsdValue::Array(outer)) = &attr.default {
                        // Matrix4d is 4 rows of 4 values
                        if outer.len() == 4 {
                            let mut mat = [[0.0f32; 4]; 4];
                            for (i, row) in outer.iter().enumerate() {
                                if let UsdValue::Array(row_vals) = row {
                                    for (j, val) in row_vals.iter().enumerate() {
                                        if j < 4 {
                                            mat[i][j] = val.as_f32().unwrap_or(0.0);
                                        }
                                    }
                                }
                            }
                            transform = Mat4::from_cols_array_2d(&mat).transpose();
                        }
                    }
                }
                "xformOp:translate" => {
                    if let Some(val) = &attr.default {
                        if let Some([x, y, z]) = extract_vec3f(val) {
                            transform = transform * Mat4::from_translation(Vec3::new(x, y, z));
                        }
                    }
                }
                "xformOp:rotateXYZ" | "xformOp:rotateZYX" => {
                    if let Some(val) = &attr.default {
                        if let Some([x, y, z]) = extract_vec3f(val) {
                            // Convert degrees to radians
                            let rx = x.to_radians();
                            let ry = y.to_radians();
                            let rz = z.to_radians();

                            let rot = if attr.name.contains("XYZ") {
                                Mat4::from_rotation_x(rx)
                                    * Mat4::from_rotation_y(ry)
                                    * Mat4::from_rotation_z(rz)
                            } else {
                                Mat4::from_rotation_z(rz)
                                    * Mat4::from_rotation_y(ry)
                                    * Mat4::from_rotation_x(rx)
                            };
                            transform = transform * rot;
                        }
                    }
                }
                "xformOp:scale" => {
                    if let Some(val) = &attr.default {
                        if let Some([x, y, z]) = extract_vec3f(val) {
                            transform = transform * Mat4::from_scale(Vec3::new(x, y, z));
                        }
                    }
                }
                _ => {}
            }
        }

        transform
    }

    /// Convert a USD Mesh prim to a TriangleMesh.
    fn convert_mesh(&self, prim: &UsdPrim) -> Result<Option<TriangleMesh>> {
        let mut mesh = TriangleMesh::new();

        // Find required attributes
        let mut points: Option<&UsdValue> = None;
        let mut normals: Option<&UsdValue> = None;
        let mut texcoords: Option<&UsdValue> = None;
        let mut face_vertex_counts: Option<&UsdValue> = None;
        let mut face_vertex_indices: Option<&UsdValue> = None;

        for attr in &prim.attributes {
            match attr.name.as_str() {
                "points" => points = attr.default.as_ref(),
                "normals" => normals = attr.default.as_ref(),
                "primvars:st" | "st" | "texCoord" => texcoords = attr.default.as_ref(),
                "faceVertexCounts" => face_vertex_counts = attr.default.as_ref(),
                "faceVertexIndices" => face_vertex_indices = attr.default.as_ref(),
                _ => {}
            }
        }

        // Points are required
        let points_val = match points {
            Some(p) => p,
            None => return Ok(None),
        };

        // Convert points
        let point_data = array_to_vec3f_array(points_val);
        if point_data.is_empty() {
            return Ok(None);
        }

        mesh.positions = point_data.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();

        // Convert normals if present
        if let Some(n) = normals {
            let normal_data = array_to_vec3f_array(n);
            if !normal_data.is_empty() {
                mesh.normals = Some(normal_data.iter().map(|n| Vec3::new(n[0], n[1], n[2])).collect());
            }
        }

        // Convert texcoords if present
        if let Some(tc) = texcoords {
            let tc_data = array_to_vec2f_array(tc);
            if !tc_data.is_empty() {
                mesh.texcoords = Some(tc_data.iter().map(|t| Vec2::new(t[0], t[1])).collect());
            }
        }

        // Convert faces to triangles
        match (face_vertex_counts, face_vertex_indices) {
            (Some(counts), Some(indices)) => {
                let counts_data = array_to_int_array(counts);
                let indices_data = array_to_int_array(indices);

                // Triangulate faces
                let mut idx_offset = 0;
                for count in counts_data {
                    let n = count as usize;
                    if n < 3 || idx_offset + n > indices_data.len() {
                        idx_offset += n;
                        continue;
                    }

                    // Fan triangulation
                    let base = indices_data[idx_offset] as u32;
                    for i in 1..n - 1 {
                        mesh.indices.push(base);
                        mesh.indices.push(indices_data[idx_offset + i] as u32);
                        mesh.indices.push(indices_data[idx_offset + i + 1] as u32);
                    }

                    idx_offset += n;
                }
            }
            _ => {
                // No face data - assume triangle list
                if mesh.positions.len() % 3 == 0 {
                    mesh.indices = (0..mesh.positions.len() as u32).collect();
                }
            }
        }

        // Compute normals if not present
        if mesh.normals.is_none() && !mesh.indices.is_empty() {
            mesh.compute_normals();
        }

        Ok(Some(mesh))
    }

    /// Convert a USD primitive (Cube, Sphere, etc.) to a mesh.
    fn convert_primitive(&self, prim: &UsdPrim) -> Result<Option<TriangleMesh>> {
        let subdivisions = 16;

        match prim.type_name.as_str() {
            "Cube" => {
                let size = self.get_prim_attr_f32(prim, "size").unwrap_or(2.0);
                let half = size / 2.0;
                Ok(Some(generate_box_mesh(half, half, half)))
            }
            "Sphere" => {
                let radius = self.get_prim_attr_f32(prim, "radius").unwrap_or(1.0);
                Ok(Some(generate_sphere_mesh(radius, subdivisions)))
            }
            "Cylinder" => {
                let radius = self.get_prim_attr_f32(prim, "radius").unwrap_or(1.0);
                let height = self.get_prim_attr_f32(prim, "height").unwrap_or(2.0);
                Ok(Some(generate_cylinder_mesh(radius, height, subdivisions)))
            }
            "Cone" => {
                let radius = self.get_prim_attr_f32(prim, "radius").unwrap_or(1.0);
                let height = self.get_prim_attr_f32(prim, "height").unwrap_or(2.0);
                Ok(Some(generate_cone_mesh(radius, height, subdivisions)))
            }
            "Capsule" => {
                let radius = self.get_prim_attr_f32(prim, "radius").unwrap_or(0.5);
                let height = self.get_prim_attr_f32(prim, "height").unwrap_or(2.0);
                Ok(Some(generate_capsule_mesh(radius, height, subdivisions)))
            }
            _ => Ok(None),
        }
    }

    /// Get a float attribute from a prim.
    fn get_prim_attr_f32(&self, prim: &UsdPrim, name: &str) -> Option<f32> {
        prim.attributes
            .iter()
            .find(|a| a.name == name)
            .and_then(|a| a.default.as_ref())
            .and_then(|v| v.as_f32())
    }

    /// Convert a USD Material prim.
    fn convert_material(&self, prim: &UsdPrim) -> Option<Material> {
        let name = prim.path.rsplit('/').next().unwrap_or("Material").to_string();

        let mut mat = Material {
            name,
            ..Default::default()
        };

        // Look for UsdPreviewSurface shader in children
        for child in &prim.children {
            if child.type_name == "Shader" {
                self.extract_preview_surface_params(child, &mut mat);
            }
        }

        // Also check attributes directly on Material prim
        self.extract_preview_surface_params(prim, &mut mat);

        Some(mat)
    }

    /// Extract UsdPreviewSurface parameters.
    fn extract_preview_surface_params(&self, prim: &UsdPrim, mat: &mut Material) {
        for attr in &prim.attributes {
            match attr.name.as_str() {
                "inputs:diffuseColor" | "diffuseColor" => {
                    if let Some(val) = &attr.default {
                        if let Some([r, g, b]) = extract_vec3f(val) {
                            mat.base_color = Vec4::new(r, g, b, mat.base_color.w);
                        }
                    }
                }
                "inputs:metallic" | "metallic" => {
                    if let Some(val) = &attr.default {
                        mat.metallic = val.as_f32().unwrap_or(0.0);
                    }
                }
                "inputs:roughness" | "roughness" => {
                    if let Some(val) = &attr.default {
                        mat.roughness = val.as_f32().unwrap_or(0.5);
                    }
                }
                "inputs:emissiveColor" | "emissiveColor" => {
                    if let Some(val) = &attr.default {
                        if let Some([r, g, b]) = extract_vec3f(val) {
                            mat.emissive = [r, g, b];
                        }
                    }
                }
                "inputs:opacity" | "opacity" => {
                    if let Some(val) = &attr.default {
                        let opacity = val.as_f32().unwrap_or(1.0);
                        mat.base_color.w = opacity;
                    }
                }
                _ => {}
            }
        }
    }

    /// Find material binding for a prim.
    fn find_material_binding(&self, prim: &UsdPrim) -> Option<usize> {
        // Look for material:binding relationship in metadata or attributes
        for (key, val) in &prim.metadata {
            if key == "material:binding" || key.contains("material") {
                if let UsdValue::Reference(path) | UsdValue::String(path) = val {
                    return self.material_map.get(path).copied();
                }
            }
        }

        None
    }
}

/// Extract a Vec3f from a UsdValue.
fn extract_vec3f(val: &UsdValue) -> Option<[f32; 3]> {
    match val {
        UsdValue::Vec3f(v) => Some(*v),
        UsdValue::Array(arr) if arr.len() == 3 => {
            let x = arr[0].as_f32()?;
            let y = arr[1].as_f32()?;
            let z = arr[2].as_f32()?;
            Some([x, y, z])
        }
        _ => None,
    }
}

/// Generate a box mesh.
fn generate_box_mesh(hx: f32, hy: f32, hz: f32) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    // 8 corners
    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(hx, hy, hz),
        Vec3::new(-hx, hy, hz),
    ];

    // 6 faces, each with 4 vertices (for proper normals)
    let faces = [
        ([0, 1, 2, 3], Vec3::new(0.0, 0.0, -1.0)), // -Z
        ([5, 4, 7, 6], Vec3::new(0.0, 0.0, 1.0)),  // +Z
        ([4, 0, 3, 7], Vec3::new(-1.0, 0.0, 0.0)), // -X
        ([1, 5, 6, 2], Vec3::new(1.0, 0.0, 0.0)),  // +X
        ([4, 5, 1, 0], Vec3::new(0.0, -1.0, 0.0)), // -Y
        ([3, 2, 6, 7], Vec3::new(0.0, 1.0, 0.0)),  // +Y
    ];

    for (indices, normal) in &faces {
        let base = mesh.positions.len() as u32;
        for &i in indices {
            mesh.positions.push(corners[i]);
        }
        // Two triangles per face
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
        mesh.indices.extend_from_slice(&[base, base + 2, base + 3]);

        // Add normals
        if mesh.normals.is_none() {
            mesh.normals = Some(Vec::new());
        }
        if let Some(normals) = &mut mesh.normals {
            for _ in 0..4 {
                normals.push(*normal);
            }
        }
    }

    mesh
}

/// Generate a sphere mesh.
fn generate_sphere_mesh(radius: f32, subdivisions: usize) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let lat_steps = subdivisions;
    let lon_steps = subdivisions * 2;

    // Generate vertices
    for lat in 0..=lat_steps {
        let theta = std::f32::consts::PI * (lat as f32) / (lat_steps as f32);
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=lon_steps {
            let phi = 2.0 * std::f32::consts::PI * (lon as f32) / (lon_steps as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            mesh.positions.push(Vec3::new(x * radius, y * radius, z * radius));

            if mesh.normals.is_none() {
                mesh.normals = Some(Vec::new());
            }
            if let Some(normals) = &mut mesh.normals {
                normals.push(Vec3::new(x, y, z));
            }

            if mesh.texcoords.is_none() {
                mesh.texcoords = Some(Vec::new());
            }
            if let Some(texcoords) = &mut mesh.texcoords {
                let u = lon as f32 / lon_steps as f32;
                let v = lat as f32 / lat_steps as f32;
                texcoords.push(Vec2::new(u, v));
            }
        }
    }

    // Generate indices
    for lat in 0..lat_steps {
        for lon in 0..lon_steps {
            let first = (lat * (lon_steps + 1) + lon) as u32;
            let second = first + lon_steps as u32 + 1;

            mesh.indices.push(first);
            mesh.indices.push(second);
            mesh.indices.push(first + 1);

            mesh.indices.push(second);
            mesh.indices.push(second + 1);
            mesh.indices.push(first + 1);
        }
    }

    mesh
}

/// Generate a cylinder mesh.
fn generate_cylinder_mesh(radius: f32, height: f32, subdivisions: usize) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();
    let half_height = height / 2.0;

    // Generate side vertices
    for i in 0..=subdivisions {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (subdivisions as f32);
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;

        // Bottom vertex
        mesh.positions.push(Vec3::new(x, -half_height, z));
        // Top vertex
        mesh.positions.push(Vec3::new(x, half_height, z));
    }

    // Side faces
    for i in 0..subdivisions {
        let base = (i * 2) as u32;
        mesh.indices.push(base);
        mesh.indices.push(base + 2);
        mesh.indices.push(base + 1);

        mesh.indices.push(base + 1);
        mesh.indices.push(base + 2);
        mesh.indices.push(base + 3);
    }

    // Cap centers
    let bottom_center = mesh.positions.len() as u32;
    mesh.positions.push(Vec3::new(0.0, -half_height, 0.0));
    let top_center = mesh.positions.len() as u32;
    mesh.positions.push(Vec3::new(0.0, half_height, 0.0));

    // Cap vertices
    let cap_start = mesh.positions.len() as u32;
    for i in 0..=subdivisions {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (subdivisions as f32);
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        mesh.positions.push(Vec3::new(x, -half_height, z)); // bottom cap
        mesh.positions.push(Vec3::new(x, half_height, z)); // top cap
    }

    // Bottom cap faces
    for i in 0..subdivisions {
        let idx = cap_start + (i * 2) as u32;
        mesh.indices.push(bottom_center);
        mesh.indices.push(idx + 2);
        mesh.indices.push(idx);
    }

    // Top cap faces
    for i in 0..subdivisions {
        let idx = cap_start + (i * 2) as u32 + 1;
        mesh.indices.push(top_center);
        mesh.indices.push(idx);
        mesh.indices.push(idx + 2);
    }

    mesh.compute_normals();
    mesh
}

/// Generate a cone mesh.
fn generate_cone_mesh(radius: f32, height: f32, subdivisions: usize) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    // Apex at top
    let apex = mesh.positions.len() as u32;
    mesh.positions.push(Vec3::new(0.0, height, 0.0));

    // Base center
    let base_center = mesh.positions.len() as u32;
    mesh.positions.push(Vec3::new(0.0, 0.0, 0.0));

    // Base vertices
    let base_start = mesh.positions.len() as u32;
    for i in 0..=subdivisions {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (subdivisions as f32);
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        mesh.positions.push(Vec3::new(x, 0.0, z));
    }

    // Side faces
    for i in 0..subdivisions {
        let idx = base_start + i as u32;
        mesh.indices.push(apex);
        mesh.indices.push(idx);
        mesh.indices.push(idx + 1);
    }

    // Base faces
    for i in 0..subdivisions {
        let idx = base_start + i as u32;
        mesh.indices.push(base_center);
        mesh.indices.push(idx + 1);
        mesh.indices.push(idx);
    }

    mesh.compute_normals();
    mesh
}

/// Generate a capsule mesh.
fn generate_capsule_mesh(radius: f32, height: f32, subdivisions: usize) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();
    let half_height = height / 2.0;
    let half_steps = subdivisions / 2;

    // Top hemisphere
    for lat in 0..=half_steps {
        let theta = std::f32::consts::PI * 0.5 * (lat as f32) / (half_steps as f32);
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=subdivisions {
            let phi = 2.0 * std::f32::consts::PI * (lon as f32) / (subdivisions as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            mesh.positions.push(Vec3::new(
                x * radius,
                y * radius + half_height,
                z * radius,
            ));
        }
    }

    // Cylinder section
    for i in 0..=subdivisions {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (subdivisions as f32);
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        mesh.positions.push(Vec3::new(x, half_height, z));
        mesh.positions.push(Vec3::new(x, -half_height, z));
    }

    // Bottom hemisphere
    for lat in 0..=half_steps {
        let theta = std::f32::consts::PI * 0.5 + std::f32::consts::PI * 0.5 * (lat as f32) / (half_steps as f32);
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=subdivisions {
            let phi = 2.0 * std::f32::consts::PI * (lon as f32) / (subdivisions as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            mesh.positions.push(Vec3::new(
                x * radius,
                y * radius - half_height,
                z * radius,
            ));
        }
    }

    // Generate indices for top hemisphere
    for lat in 0..half_steps {
        for lon in 0..subdivisions {
            let first = (lat * (subdivisions + 1) + lon) as u32;
            let second = first + subdivisions as u32 + 1;

            mesh.indices.push(first);
            mesh.indices.push(second);
            mesh.indices.push(first + 1);

            mesh.indices.push(second);
            mesh.indices.push(second + 1);
            mesh.indices.push(first + 1);
        }
    }

    // Cylinder indices
    let cyl_start = ((half_steps + 1) * (subdivisions + 1)) as u32;
    for i in 0..subdivisions {
        let base = cyl_start + (i * 2) as u32;
        mesh.indices.push(base);
        mesh.indices.push(base + 2);
        mesh.indices.push(base + 1);

        mesh.indices.push(base + 1);
        mesh.indices.push(base + 2);
        mesh.indices.push(base + 3);
    }

    // Bottom hemisphere indices
    let bottom_start = cyl_start + ((subdivisions + 1) * 2) as u32;
    for lat in 0..half_steps {
        for lon in 0..subdivisions {
            let first = bottom_start + (lat * (subdivisions + 1) + lon) as u32;
            let second = first + subdivisions as u32 + 1;

            mesh.indices.push(first);
            mesh.indices.push(second);
            mesh.indices.push(first + 1);

            mesh.indices.push(second);
            mesh.indices.push(second + 1);
            mesh.indices.push(first + 1);
        }
    }

    mesh.compute_normals();
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_read() {
        let reader = UsdReader::new();

        // USDA
        assert!(reader.can_read(b"#usda 1.0\n(\n)\n"));

        // USDC
        assert!(reader.can_read(b"PXR-USDC\x00\x07\x00\x00"));

        // USDZ (ZIP)
        assert!(reader.can_read(b"PK\x03\x04test"));

        // Random
        assert!(!reader.can_read(b"random data"));
    }

    #[test]
    fn test_read_simple_usda() {
        let reader = UsdReader::new();
        let input = br#"#usda 1.0
(
    defaultPrim = "World"
    upAxis = "Y"
)

def Xform "World"
{
    def Mesh "Triangle"
    {
        float3[] points = [(0, 0, 0), (1, 0, 0), (0.5, 1, 0)]
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
    }
}
"#;

        let result = reader.read(input, &ReadOptions::default());
        assert!(result.is_ok(), "Read failed: {:?}", result);

        let scene = result.unwrap();
        assert_eq!(scene.metadata.name, Some("World".to_string()));
        assert_eq!(scene.metadata.up_axis, Axis::Y);
        assert_eq!(scene.roots.len(), 1);

        // Check mesh was created
        assert_eq!(scene.geometries.len(), 1);
        if let Geometry::Mesh(mesh) = &scene.geometries[0] {
            assert_eq!(mesh.positions.len(), 3);
            assert_eq!(mesh.indices.len(), 3);
        } else {
            panic!("Expected mesh geometry");
        }
    }

    #[test]
    fn test_read_mesh_with_normals() {
        let reader = UsdReader::new();
        let input = br#"#usda 1.0

def Mesh "Quad"
{
    float3[] points = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0)]
    float3[] normals = [(0, 0, 1), (0, 0, 1), (0, 0, 1), (0, 0, 1)]
    int[] faceVertexCounts = [4]
    int[] faceVertexIndices = [0, 1, 2, 3]
}
"#;

        let result = reader.read(input, &ReadOptions::default());
        assert!(result.is_ok());

        let scene = result.unwrap();
        assert_eq!(scene.geometries.len(), 1);

        if let Geometry::Mesh(mesh) = &scene.geometries[0] {
            assert_eq!(mesh.positions.len(), 4);
            assert!(mesh.normals.is_some());
            // Quad triangulated to 2 triangles
            assert_eq!(mesh.indices.len(), 6);
        } else {
            panic!("Expected mesh geometry");
        }
    }

    #[test]
    fn test_read_primitive_cube() {
        let reader = UsdReader::new();
        let input = br#"#usda 1.0

def Cube "MyCube"
{
    float size = 2.0
}
"#;

        let result = reader.read(input, &ReadOptions::default());
        assert!(result.is_ok());

        let scene = result.unwrap();
        assert_eq!(scene.geometries.len(), 1);

        if let Geometry::Mesh(mesh) = &scene.geometries[0] {
            assert!(!mesh.positions.is_empty());
            assert!(!mesh.indices.is_empty());
        } else {
            panic!("Expected mesh geometry");
        }
    }

    #[test]
    fn test_read_hierarchy() {
        let reader = UsdReader::new();
        let input = br#"#usda 1.0

def Xform "Root"
{
    def Xform "Child1"
    {
        def Mesh "Mesh1"
        {
            float3[] points = [(0, 0, 0), (1, 0, 0), (0, 1, 0)]
            int[] faceVertexCounts = [3]
            int[] faceVertexIndices = [0, 1, 2]
        }
    }

    def Xform "Child2"
    {
    }
}
"#;

        let result = reader.read(input, &ReadOptions::default());
        assert!(result.is_ok());

        let scene = result.unwrap();
        assert_eq!(scene.roots.len(), 1);

        let root = &scene.nodes[scene.roots[0]];
        assert_eq!(root.name, "Root");
        assert_eq!(root.children.len(), 2);

        let child1 = &scene.nodes[root.children[0]];
        assert_eq!(child1.name, "Child1");
        assert_eq!(child1.children.len(), 1);
    }

    #[test]
    fn test_read_transform() {
        let reader = UsdReader::new();
        let input = br#"#usda 1.0

def Xform "Translated"
{
    float3 xformOp:translate = (1, 2, 3)
}
"#;

        let result = reader.read(input, &ReadOptions::default());
        assert!(result.is_ok());

        let scene = result.unwrap();
        let node = &scene.nodes[0];

        // Check translation was applied
        let (_, _, translation) = node.transform.to_scale_rotation_translation();
        assert!((translation.x - 1.0).abs() < 0.001);
        assert!((translation.y - 2.0).abs() < 0.001);
        assert!((translation.z - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_generate_box_mesh() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0);
        assert_eq!(mesh.positions.len(), 24); // 6 faces * 4 vertices
        assert_eq!(mesh.indices.len(), 36); // 6 faces * 2 triangles * 3 indices
    }

    #[test]
    fn test_generate_sphere_mesh() {
        let mesh = generate_sphere_mesh(1.0, 8);
        assert!(!mesh.positions.is_empty());
        assert!(!mesh.indices.is_empty());
        assert!(mesh.normals.is_some());
    }
}
