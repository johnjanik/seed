//! USD writer implementation.

use crate::error::{IoError, Result};
use crate::registry::{FormatWriter, WriteOptions};
use crate::scene::{Axis, Geometry, Material, TriangleMesh, UnifiedScene};

use glam::{Mat4, Vec3};
use std::fmt::Write as FmtWrite;

/// Writer for USD files.
pub struct UsdWriter;

impl UsdWriter {
    /// Create a new USD writer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for UsdWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatWriter for UsdWriter {
    fn name(&self) -> &'static str {
        "usd"
    }

    fn extension(&self) -> &'static str {
        "usda"
    }

    fn write(&self, scene: &UnifiedScene, options: &WriteOptions) -> Result<Vec<u8>> {
        if options.binary {
            self.write_usdc(scene)
        } else {
            self.write_usda(scene, options.pretty)
        }
    }
}

impl UsdWriter {
    fn write_usda(&self, scene: &UnifiedScene, pretty: bool) -> Result<Vec<u8>> {
        let mut builder = UsdaBuilder::new(pretty);
        builder.build(scene)?;
        Ok(builder.output.into_bytes())
    }

    fn write_usdc(&self, _scene: &UnifiedScene) -> Result<Vec<u8>> {
        // USDC binary format is complex - requires Crate format implementation
        Err(IoError::Unsupported(
            "USDC binary writing not supported. Use USDA format (binary: false).".into(),
        ))
    }
}

/// Builder for USDA output.
struct UsdaBuilder {
    output: String,
    indent: usize,
    pretty: bool,
}

impl UsdaBuilder {
    fn new(pretty: bool) -> Self {
        Self {
            output: String::new(),
            indent: 0,
            pretty,
        }
    }

    fn build(&mut self, scene: &UnifiedScene) -> Result<()> {
        // Write header
        self.write_header(scene);

        // Write materials first (they need to be defined before being referenced)
        if !scene.materials.is_empty() {
            self.write_line("");
            self.write_line("def Scope \"Materials\"");
            self.write_line("{");
            self.indent += 1;

            for (idx, material) in scene.materials.iter().enumerate() {
                self.write_material(material, idx);
            }

            self.indent -= 1;
            self.write_line("}");
        }

        // Write scene hierarchy
        for &root_idx in &scene.roots {
            self.write_line("");
            self.write_node(scene, root_idx)?;
        }

        Ok(())
    }

    fn write_header(&mut self, scene: &UnifiedScene) {
        self.output.push_str("#usda 1.0\n");
        self.output.push_str("(\n");

        // Stage metadata
        if let Some(name) = &scene.metadata.name {
            writeln!(self.output, "    defaultPrim = \"{}\"", escape_string(name)).unwrap();
        }

        // Up axis
        let up_axis = match scene.metadata.up_axis {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        };
        writeln!(self.output, "    upAxis = \"{}\"", up_axis).unwrap();

        // Meters per unit from scene units
        let meters_per_unit = scene.metadata.units.to_meters_scale();
        writeln!(self.output, "    metersPerUnit = {}", format_real(meters_per_unit as f64)).unwrap();

        // Doc string
        if let Some(desc) = &scene.metadata.description {
            writeln!(self.output, "    doc = \"{}\"", escape_string(desc)).unwrap();
        }

        self.output.push_str(")\n");
    }

    fn write_material(&mut self, material: &Material, idx: usize) {
        let name = if material.name.is_empty() {
            format!("Material_{}", idx)
        } else {
            sanitize_name(&material.name)
        };

        self.write_line(&format!("def Material \"{}\"", name));
        self.write_line("{");
        self.indent += 1;

        // Write UsdPreviewSurface shader
        self.write_line("def Shader \"PreviewSurface\"");
        self.write_line("{");
        self.indent += 1;

        self.write_line("uniform token info:id = \"UsdPreviewSurface\"");

        // Diffuse color
        let [r, g, b, a] = material.base_color.to_array();
        self.write_line(&format!(
            "color3f inputs:diffuseColor = ({}, {}, {})",
            format_real(r as f64),
            format_real(g as f64),
            format_real(b as f64)
        ));

        // Metallic
        self.write_line(&format!(
            "float inputs:metallic = {}",
            format_real(material.metallic as f64)
        ));

        // Roughness
        self.write_line(&format!(
            "float inputs:roughness = {}",
            format_real(material.roughness as f64)
        ));

        // Opacity
        if a < 1.0 {
            self.write_line(&format!(
                "float inputs:opacity = {}",
                format_real(a as f64)
            ));
        }

        // Emissive
        let [er, eg, eb] = material.emissive;
        if er > 0.0 || eg > 0.0 || eb > 0.0 {
            self.write_line(&format!(
                "color3f inputs:emissiveColor = ({}, {}, {})",
                format_real(er as f64),
                format_real(eg as f64),
                format_real(eb as f64)
            ));
        }

        // Output
        self.write_line("token outputs:surface");

        self.indent -= 1;
        self.write_line("}");

        // Surface output connection
        self.write_line(&format!(
            "token outputs:surface.connect = </Materials/{}/PreviewSurface.outputs:surface>",
            name
        ));

        self.indent -= 1;
        self.write_line("}");
    }

    fn write_node(&mut self, scene: &UnifiedScene, node_idx: usize) -> Result<()> {
        let node = &scene.nodes[node_idx];
        let name = sanitize_name(&node.name);

        // Determine prim type based on geometry
        let prim_type = if node.geometry.is_some() {
            "Mesh"
        } else {
            "Xform"
        };

        self.write_line(&format!("def {} \"{}\"", prim_type, name));
        self.write_line("{");
        self.indent += 1;

        // Write transform if not identity
        if node.transform != Mat4::IDENTITY {
            self.write_transform(&node.transform);
        }

        // Write visibility
        if !node.visible {
            self.write_line("token visibility = \"invisible\"");
        }

        // Write geometry
        if let Some(geom_idx) = node.geometry {
            if let Some(geom) = scene.geometries.get(geom_idx) {
                self.write_geometry(geom)?;
            }
        }

        // Write material binding
        if let Some(mat_idx) = node.material {
            if let Some(mat) = scene.materials.get(mat_idx) {
                let mat_name = if mat.name.is_empty() {
                    format!("Material_{}", mat_idx)
                } else {
                    sanitize_name(&mat.name)
                };
                self.write_line(&format!(
                    "rel material:binding = </Materials/{}>",
                    mat_name
                ));
            }
        }

        // Write children
        for &child_idx in &node.children {
            self.write_line("");
            self.write_node(scene, child_idx)?;
        }

        self.indent -= 1;
        self.write_line("}");

        Ok(())
    }

    fn write_transform(&mut self, transform: &Mat4) {
        // Decompose transform into translate, rotate, scale
        let (scale, rotation, translation) = transform.to_scale_rotation_translation();

        // Write translation if not zero
        if translation != Vec3::ZERO {
            self.write_line(&format!(
                "double3 xformOp:translate = ({}, {}, {})",
                format_real(translation.x as f64),
                format_real(translation.y as f64),
                format_real(translation.z as f64)
            ));
        }

        // Write rotation as euler angles
        let (rx, ry, rz) = rotation.to_euler(glam::EulerRot::XYZ);
        if rx != 0.0 || ry != 0.0 || rz != 0.0 {
            self.write_line(&format!(
                "float3 xformOp:rotateXYZ = ({}, {}, {})",
                format_real(rx.to_degrees() as f64),
                format_real(ry.to_degrees() as f64),
                format_real(rz.to_degrees() as f64)
            ));
        }

        // Write scale if not uniform 1
        if scale != Vec3::ONE {
            self.write_line(&format!(
                "float3 xformOp:scale = ({}, {}, {})",
                format_real(scale.x as f64),
                format_real(scale.y as f64),
                format_real(scale.z as f64)
            ));
        }

        // Write xformOpOrder
        let mut ops = Vec::new();
        if translation != Vec3::ZERO {
            ops.push("\"xformOp:translate\"");
        }
        if rx != 0.0 || ry != 0.0 || rz != 0.0 {
            ops.push("\"xformOp:rotateXYZ\"");
        }
        if scale != Vec3::ONE {
            ops.push("\"xformOp:scale\"");
        }

        if !ops.is_empty() {
            self.write_line(&format!("uniform token[] xformOpOrder = [{}]", ops.join(", ")));
        }
    }

    fn write_geometry(&mut self, geom: &Geometry) -> Result<()> {
        match geom {
            Geometry::Mesh(mesh) => self.write_mesh(mesh),
            Geometry::Brep(_) => {
                // B-rep needs tessellation first
                self.write_line("# B-rep geometry (not yet supported in USD export)");
                Ok(())
            }
            Geometry::Primitive(prim) => {
                // Write primitive as mesh
                use crate::convert::generate_primitive_mesh;
                let mesh = generate_primitive_mesh(prim, 16);
                self.write_mesh(&mesh)
            }
            Geometry::Nurbs(_) => {
                self.write_line("# NURBS geometry (not yet supported in USD export)");
                Ok(())
            }
        }
    }

    fn write_mesh(&mut self, mesh: &TriangleMesh) -> Result<()> {
        if mesh.positions.is_empty() {
            return Ok(());
        }

        // Points
        self.write_line("point3f[] points = [");
        self.indent += 1;
        for (i, pos) in mesh.positions.iter().enumerate() {
            let comma = if i < mesh.positions.len() - 1 { "," } else { "" };
            self.write_line(&format!(
                "({}, {}, {}){}",
                format_real(pos.x as f64),
                format_real(pos.y as f64),
                format_real(pos.z as f64),
                comma
            ));
        }
        self.indent -= 1;
        self.write_line("]");

        // Normals
        if let Some(normals) = &mesh.normals {
            self.write_line("normal3f[] normals = [");
            self.indent += 1;
            for (i, normal) in normals.iter().enumerate() {
                let comma = if i < normals.len() - 1 { "," } else { "" };
                self.write_line(&format!(
                    "({}, {}, {}){}",
                    format_real(normal.x as f64),
                    format_real(normal.y as f64),
                    format_real(normal.z as f64),
                    comma
                ));
            }
            self.indent -= 1;
            self.write_line("]");
            self.write_line("uniform token normals:interpolation = \"vertex\"");
        }

        // Texture coordinates
        if let Some(texcoords) = &mesh.texcoords {
            self.write_line("texCoord2f[] primvars:st = [");
            self.indent += 1;
            for (i, tc) in texcoords.iter().enumerate() {
                let comma = if i < texcoords.len() - 1 { "," } else { "" };
                self.write_line(&format!(
                    "({}, {}){}",
                    format_real(tc.x as f64),
                    format_real(tc.y as f64),
                    comma
                ));
            }
            self.indent -= 1;
            self.write_line("]");
            self.write_line("uniform token primvars:st:interpolation = \"vertex\"");
        }

        // Face vertex counts and indices
        if !mesh.indices.is_empty() {
            // All faces are triangles
            let face_count = mesh.indices.len() / 3;
            let counts: Vec<String> = (0..face_count).map(|_| "3".to_string()).collect();
            self.write_line(&format!("int[] faceVertexCounts = [{}]", counts.join(", ")));

            let indices: Vec<String> = mesh.indices.iter().map(|i| i.to_string()).collect();
            self.write_line(&format!("int[] faceVertexIndices = [{}]", indices.join(", ")));
        }

        // Subdivision scheme (for mesh)
        self.write_line("uniform token subdivisionScheme = \"none\"");

        Ok(())
    }

    fn write_line(&mut self, line: &str) {
        if self.pretty {
            for _ in 0..self.indent {
                self.output.push_str("    ");
            }
        }
        self.output.push_str(line);
        self.output.push('\n');
    }
}

/// Format a real number for USD output.
fn format_real(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else if value.abs() < 0.0001 || value.abs() >= 1e6 {
        format!("{:e}", value)
    } else if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        let s = format!("{:.6}", value);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Sanitize a name for use as a USD identifier.
fn sanitize_name(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_alphanumeric() || c == '_' {
            if i == 0 && c.is_ascii_digit() {
                result.push('_');
            }
            result.push(c);
        } else if c == ' ' || c == '-' || c == '.' {
            result.push('_');
        }
    }
    if result.is_empty() {
        result = "unnamed".to_string();
    }
    result
}

/// Escape a string for USD output.
fn escape_string(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::WriteOptions;
    use crate::scene::{SceneNode, TriangleMesh};
    use glam::{Vec2, Vec3, Vec4};

    #[test]
    fn test_format_real() {
        assert_eq!(format_real(0.0), "0");
        assert_eq!(format_real(1.0), "1");
        assert_eq!(format_real(3.14159), "3.14159");
        assert_eq!(format_real(0.5), "0.5");
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("Hello World"), "Hello_World");
        assert_eq!(sanitize_name("mesh-01"), "mesh_01");
        assert_eq!(sanitize_name("123abc"), "_123abc");
        assert_eq!(sanitize_name(""), "unnamed");
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_string("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_write_empty_scene() {
        let writer = UsdWriter::new();
        let scene = UnifiedScene::new();

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let usda = String::from_utf8(result.unwrap()).unwrap();
        assert!(usda.starts_with("#usda 1.0"));
        assert!(usda.contains("upAxis"));
    }

    #[test]
    fn test_write_simple_mesh() {
        let writer = UsdWriter::new();
        let mut scene = UnifiedScene::new();

        // Create a triangle mesh
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
            texcoords: None,
            indices: vec![0, 1, 2],
            ..Default::default()
        };

        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));
        scene.add_root(SceneNode::with_geometry("Triangle", geom_idx));

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok(), "Write failed: {:?}", result);

        let usda = String::from_utf8(result.unwrap()).unwrap();
        assert!(usda.contains("def Mesh \"Triangle\""));
        assert!(usda.contains("point3f[] points"));
        assert!(usda.contains("normal3f[] normals"));
        assert!(usda.contains("faceVertexCounts"));
        assert!(usda.contains("faceVertexIndices"));
    }

    #[test]
    fn test_write_with_material() {
        let writer = UsdWriter::new();
        let mut scene = UnifiedScene::new();

        // Create material
        let material = Material {
            name: "RedMaterial".to_string(),
            base_color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            metallic: 0.0,
            roughness: 0.5,
            ..Default::default()
        };
        let mat_idx = scene.add_material(material);

        // Create mesh with material
        let mesh = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 1.0, 0.0),
            ],
            indices: vec![0, 1, 2],
            ..Default::default()
        };

        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));
        let mut node = SceneNode::with_geometry("Triangle", geom_idx);
        node.material = Some(mat_idx);
        scene.add_root(node);

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let usda = String::from_utf8(result.unwrap()).unwrap();
        assert!(usda.contains("def Material \"RedMaterial\""));
        assert!(usda.contains("UsdPreviewSurface"));
        assert!(usda.contains("inputs:diffuseColor"));
        assert!(usda.contains("material:binding"));
    }

    #[test]
    fn test_write_hierarchy() {
        let writer = UsdWriter::new();
        let mut scene = UnifiedScene::new();

        let root = scene.add_root(SceneNode::new("Root"));
        scene.add_child(root, SceneNode::new("Child1"));
        scene.add_child(root, SceneNode::new("Child2"));

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let usda = String::from_utf8(result.unwrap()).unwrap();
        assert!(usda.contains("def Xform \"Root\""));
        assert!(usda.contains("def Xform \"Child1\""));
        assert!(usda.contains("def Xform \"Child2\""));
    }

    #[test]
    fn test_write_transform() {
        let writer = UsdWriter::new();
        let mut scene = UnifiedScene::new();

        let mut node = SceneNode::new("Transformed");
        node.transform = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        scene.add_root(node);

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let usda = String::from_utf8(result.unwrap()).unwrap();
        assert!(usda.contains("xformOp:translate"));
        assert!(usda.contains("xformOpOrder"));
    }

    #[test]
    fn test_roundtrip_detection() {
        let writer = UsdWriter::new();
        let mut scene = UnifiedScene::new();
        scene.add_root(SceneNode::new("Test"));

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let usda = result.unwrap();

        // Verify the output can be detected as USDA
        use super::super::reader::UsdReader;
        use crate::registry::FormatReader;

        let reader = UsdReader::new();
        assert!(reader.can_read(&usda));
    }

    #[test]
    fn test_full_roundtrip() {
        use super::super::reader::UsdReader;
        use crate::registry::{FormatReader, ReadOptions};

        // Create a complex scene
        let mut scene = UnifiedScene::new();

        // Set metadata
        scene.metadata.name = Some("TestScene".to_string());
        scene.metadata.up_axis = crate::scene::Axis::Y;

        // Create materials
        let red_material = Material {
            name: "RedMetal".to_string(),
            base_color: Vec4::new(0.8, 0.1, 0.1, 1.0),
            metallic: 0.9,
            roughness: 0.2,
            emissive: [0.1, 0.0, 0.0],
            ..Default::default()
        };
        let mat_idx = scene.add_material(red_material);

        // Create mesh geometry
        let mesh = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            normals: Some(vec![
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(0.0, 0.0, 1.0),
            ]),
            texcoords: Some(vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ]),
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Default::default()
        };
        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));

        // Create hierarchy: Root -> Child1 (with mesh) -> Grandchild
        //                       -> Child2 (transformed)
        let root_idx = scene.add_root(SceneNode::new("Root"));

        let mut child1 = SceneNode::with_geometry("MeshNode", geom_idx);
        child1.material = Some(mat_idx);
        let child1_idx = scene.add_child(root_idx, child1);

        scene.add_child(child1_idx, SceneNode::new("Grandchild"));

        let mut child2 = SceneNode::new("TransformedNode");
        child2.transform = Mat4::from_translation(Vec3::new(2.0, 3.0, 4.0))
            * Mat4::from_scale(Vec3::new(0.5, 0.5, 0.5));
        scene.add_child(root_idx, child2);

        // Write to USDA
        let writer = UsdWriter::new();
        let usda_bytes = writer.write(&scene, &WriteOptions::default())
            .expect("Failed to write USDA");

        // Print USDA for debugging
        let usda_str = String::from_utf8(usda_bytes.clone()).unwrap();
        println!("Generated USDA:\n{}", usda_str);

        // Read back
        let reader = UsdReader::new();
        let read_scene = reader.read(&usda_bytes, &ReadOptions::default())
            .expect("Failed to read USDA");

        // Verify metadata
        assert_eq!(read_scene.metadata.up_axis, crate::scene::Axis::Y);

        // Verify hierarchy structure
        assert_eq!(read_scene.roots.len(), 1, "Should have 1 root (Materials scope is separate)");

        // Find the Root node (skip Materials scope)
        let root_node = read_scene.nodes.iter()
            .find(|n| n.name == "Root")
            .expect("Should find Root node");
        assert_eq!(root_node.children.len(), 2, "Root should have 2 children");

        // Find MeshNode
        let mesh_node = read_scene.nodes.iter()
            .find(|n| n.name == "MeshNode")
            .expect("Should find MeshNode");
        assert!(mesh_node.geometry.is_some(), "MeshNode should have geometry");

        // Verify geometry was preserved
        assert!(!read_scene.geometries.is_empty(), "Should have geometries");
        if let Geometry::Mesh(read_mesh) = &read_scene.geometries[0] {
            assert_eq!(read_mesh.positions.len(), 4, "Should have 4 positions");
            assert!(read_mesh.normals.is_some(), "Should have normals");
            assert_eq!(read_mesh.indices.len(), 6, "Should have 6 indices (2 triangles)");

            // Verify positions are approximately correct
            let pos0 = read_mesh.positions[0];
            assert!((pos0.x - 0.0).abs() < 0.001);
            assert!((pos0.y - 0.0).abs() < 0.001);
            assert!((pos0.z - 0.0).abs() < 0.001);

            let pos2 = read_mesh.positions[2];
            assert!((pos2.x - 1.0).abs() < 0.001);
            assert!((pos2.y - 1.0).abs() < 0.001);
            assert!((pos2.z - 0.0).abs() < 0.001);
        } else {
            panic!("Expected mesh geometry");
        }

        // Find TransformedNode and verify transform
        let transformed_node = read_scene.nodes.iter()
            .find(|n| n.name == "TransformedNode")
            .expect("Should find TransformedNode");

        let (scale, _, translation) = transformed_node.transform.to_scale_rotation_translation();
        assert!((translation.x - 2.0).abs() < 0.01, "Translation X should be 2.0, got {}", translation.x);
        assert!((translation.y - 3.0).abs() < 0.01, "Translation Y should be 3.0, got {}", translation.y);
        assert!((translation.z - 4.0).abs() < 0.01, "Translation Z should be 4.0, got {}", translation.z);
        assert!((scale.x - 0.5).abs() < 0.01, "Scale X should be 0.5, got {}", scale.x);

        // Verify grandchild exists
        let grandchild = read_scene.nodes.iter()
            .find(|n| n.name == "Grandchild")
            .expect("Should find Grandchild node");
        assert!(grandchild.geometry.is_none(), "Grandchild should have no geometry");

        println!("Full roundtrip test passed!");
    }

    #[test]
    fn test_roundtrip_multiple_meshes() {
        use super::super::reader::UsdReader;
        use crate::registry::{FormatReader, ReadOptions};

        let mut scene = UnifiedScene::new();

        // Create two different meshes
        let triangle = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.5, 1.0, 0.0),
            ],
            indices: vec![0, 1, 2],
            ..Default::default()
        };

        let quad = TriangleMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(2.0, 2.0, 0.0),
                Vec3::new(0.0, 2.0, 0.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            ..Default::default()
        };

        let tri_idx = scene.add_geometry(Geometry::Mesh(triangle));
        let quad_idx = scene.add_geometry(Geometry::Mesh(quad));

        scene.add_root(SceneNode::with_geometry("Triangle", tri_idx));
        scene.add_root(SceneNode::with_geometry("Quad", quad_idx));

        // Write and read back
        let writer = UsdWriter::new();
        let usda = writer.write(&scene, &WriteOptions::default()).unwrap();

        let reader = UsdReader::new();
        let read_scene = reader.read(&usda, &ReadOptions::default()).unwrap();

        // Verify both meshes exist
        assert_eq!(read_scene.geometries.len(), 2, "Should have 2 geometries");

        // Find triangle node
        let tri_node = read_scene.nodes.iter()
            .find(|n| n.name == "Triangle")
            .expect("Should find Triangle");
        assert!(tri_node.geometry.is_some());

        // Find quad node
        let quad_node = read_scene.nodes.iter()
            .find(|n| n.name == "Quad")
            .expect("Should find Quad");
        assert!(quad_node.geometry.is_some());

        // Verify geometry sizes
        let mut found_tri = false;
        let mut found_quad = false;
        for geom in &read_scene.geometries {
            if let Geometry::Mesh(mesh) = geom {
                if mesh.positions.len() == 3 {
                    found_tri = true;
                    assert_eq!(mesh.indices.len(), 3);
                } else if mesh.positions.len() == 4 {
                    found_quad = true;
                    assert_eq!(mesh.indices.len(), 6);
                }
            }
        }
        assert!(found_tri, "Should find triangle mesh");
        assert!(found_quad, "Should find quad mesh");
    }
}
