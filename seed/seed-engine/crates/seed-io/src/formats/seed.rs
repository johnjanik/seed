//! Seed format reader and writer.
//!
//! Converts between seed_core::Document and UnifiedScene.

use crate::error::{IoError, Result};
use crate::registry::{FormatReader, FormatWriter, ReadOptions, WriteOptions};
use crate::scene::{Geometry, Material, PrimitiveGeometry, SceneNode, UnifiedScene};
use glam::{Mat4, Vec4};
use seed_core::ast::{
    CsgOperation, Document, Element, FrameElement, Geometry as SeedGeometry, PartElement,
    Primitive, Property, PropertyValue,
};
use seed_core::types::{Identifier, Length, LengthUnit};

/// Reader for Seed documents.
pub struct SeedReader;

impl SeedReader {
    /// Create a new Seed reader.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SeedReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatReader for SeedReader {
    fn name(&self) -> &'static str {
        "seed"
    }

    fn extensions(&self) -> &[&'static str] {
        &["seed"]
    }

    fn can_read(&self, data: &[u8]) -> bool {
        // Check for common Seed document markers
        let text = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // Look for Seed indicators
        text.contains("@meta") || text.contains("Frame:") || text.contains("Part:")
    }

    fn read(&self, data: &[u8], _options: &ReadOptions) -> Result<UnifiedScene> {
        // Parse the Seed document
        let text =
            std::str::from_utf8(data).map_err(|e| IoError::parse(format!("invalid UTF-8: {}", e)))?;

        let document = seed_parser::parse(text)
            .map_err(|e| IoError::parse(format!("seed parse error: {:?}", e)))?;

        convert_document_to_scene(&document)
    }
}

/// Writer for Seed documents.
pub struct SeedWriter;

impl SeedWriter {
    /// Create a new Seed writer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for SeedWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatWriter for SeedWriter {
    fn name(&self) -> &'static str {
        "seed"
    }

    fn extension(&self) -> &'static str {
        "seed"
    }

    fn write(&self, scene: &UnifiedScene, options: &WriteOptions) -> Result<Vec<u8>> {
        let document = convert_scene_to_document(scene)?;
        let text = serialize_document(&document, options.pretty);
        Ok(text.into_bytes())
    }
}

/// Convert a Seed document to a UnifiedScene.
fn convert_document_to_scene(doc: &Document) -> Result<UnifiedScene> {
    let mut scene = UnifiedScene::new();

    // Set metadata
    if let Some(meta) = &doc.meta {
        scene.metadata.source_format = Some("seed".to_string());
        if let Some(version) = &meta.version {
            scene.metadata.generator = Some(format!("Seed {}", version));
        }
    }

    // Convert elements
    for element in &doc.elements {
        convert_element(&mut scene, element, None)?;
    }

    Ok(scene)
}

/// Convert a single element to scene nodes.
fn convert_element(
    scene: &mut UnifiedScene,
    element: &Element,
    parent: Option<usize>,
) -> Result<Option<usize>> {
    match element {
        Element::Frame(frame) => convert_frame(scene, frame, parent),
        Element::Part(part) => convert_part(scene, part, parent),
        Element::Text(_) | Element::Svg(_) | Element::Image(_) | Element::Icon(_) => {
            // 2D elements are skipped for 3D interchange
            Ok(None)
        }
        Element::Component(_) | Element::Slot(_) => {
            // Components should be expanded before conversion
            Ok(None)
        }
    }
}

/// Convert a Frame element to a scene node.
fn convert_frame(
    scene: &mut UnifiedScene,
    frame: &FrameElement,
    parent: Option<usize>,
) -> Result<Option<usize>> {
    let name = frame
        .name
        .as_ref()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| "frame".to_string());

    let node = SceneNode::new(name);

    let node_idx = if let Some(parent_idx) = parent {
        scene.add_child(parent_idx, node)
    } else {
        scene.add_root(node)
    };

    // Convert children
    for child in &frame.children {
        convert_element(scene, child, Some(node_idx))?;
    }

    Ok(Some(node_idx))
}

/// Convert a Part element to a scene node with geometry.
fn convert_part(
    scene: &mut UnifiedScene,
    part: &PartElement,
    parent: Option<usize>,
) -> Result<Option<usize>> {
    let name = part
        .name
        .as_ref()
        .map(|id| id.0.clone())
        .unwrap_or_else(|| "part".to_string());

    // Convert geometry
    let geometry = convert_geometry(&part.geometry)?;
    let geom_idx = scene.add_geometry(geometry);

    // Extract material from properties
    let material = extract_material(&part.properties);
    let mat_idx = if material.base_color != Vec4::ONE {
        Some(scene.add_material(material))
    } else {
        None
    };

    // Create node
    let mut node = SceneNode::with_geometry(&name, geom_idx);
    node.material = mat_idx;
    node.transform = Mat4::IDENTITY; // 2D transforms in Seed don't map to 3D

    let node_idx = if let Some(parent_idx) = parent {
        scene.add_child(parent_idx, node)
    } else {
        scene.add_root(node)
    };

    Ok(Some(node_idx))
}

/// Convert Seed geometry to UnifiedScene geometry.
fn convert_geometry(geom: &SeedGeometry) -> Result<Geometry> {
    match geom {
        SeedGeometry::Primitive(prim) => convert_primitive(prim),
        SeedGeometry::Csg(csg) => convert_csg(csg),
    }
}

/// Convert a Seed primitive to UnifiedScene geometry.
fn convert_primitive(prim: &Primitive) -> Result<Geometry> {
    let geom = match prim {
        Primitive::Box {
            width,
            height,
            depth,
        } => PrimitiveGeometry::Box {
            half_extents: glam::Vec3::new(
                length_to_meters(width) / 2.0,
                length_to_meters(height) / 2.0,
                length_to_meters(depth) / 2.0,
            ),
            transform: Mat4::IDENTITY,
        },
        Primitive::Cylinder { radius, height } => PrimitiveGeometry::Cylinder {
            radius: length_to_meters(radius),
            height: length_to_meters(height),
            transform: Mat4::IDENTITY,
        },
        Primitive::Sphere { radius } => PrimitiveGeometry::Sphere {
            radius: length_to_meters(radius),
            transform: Mat4::IDENTITY,
        },
    };
    Ok(Geometry::Primitive(geom))
}

/// Convert CSG operations.
fn convert_csg(csg: &CsgOperation) -> Result<Geometry> {
    match csg {
        CsgOperation::Union(geoms) => {
            if let Some(first) = geoms.first() {
                convert_geometry(first)
            } else {
                Err(IoError::InvalidData("empty CSG union".into()))
            }
        }
        CsgOperation::Difference { base, .. } => convert_geometry(base),
        CsgOperation::Intersection(geoms) => {
            if let Some(first) = geoms.first() {
                convert_geometry(first)
            } else {
                Err(IoError::InvalidData("empty CSG intersection".into()))
            }
        }
    }
}

/// Convert a Length to meters.
fn length_to_meters(len: &Length) -> f32 {
    let mm = len.to_mm().unwrap_or(len.value);
    (mm / 1000.0) as f32
}

/// Extract material from properties.
fn extract_material(properties: &[Property]) -> Material {
    let mut material = Material::new("material");

    for prop in properties {
        match prop.name.as_str() {
            "color" | "fill" => {
                if let PropertyValue::Color(color) = &prop.value {
                    material.base_color = Vec4::new(color.r, color.g, color.b, color.a);
                }
            }
            "metallic" => {
                if let PropertyValue::Number(n) = &prop.value {
                    material.metallic = *n as f32;
                }
            }
            "roughness" => {
                if let PropertyValue::Number(n) = &prop.value {
                    material.roughness = *n as f32;
                }
            }
            _ => {}
        }
    }

    material
}

/// Convert a UnifiedScene to a Seed document.
fn convert_scene_to_document(scene: &UnifiedScene) -> Result<Document> {
    use seed_core::ast::{MetaBlock, Profile, Span};

    let mut elements = Vec::new();

    // Convert root nodes
    for &root_idx in &scene.roots {
        if let Some(node) = scene.nodes.get(root_idx) {
            if let Some(element) = convert_node_to_element(scene, node, root_idx)? {
                elements.push(element);
            }
        }
    }

    Ok(Document {
        meta: Some(MetaBlock {
            profile: Profile::Seed3D,
            version: Some("1.0".to_string()),
            span: Span::default(),
        }),
        tokens: None,
        elements,
        span: Span::default(),
    })
}

/// Convert a scene node to a Seed element.
fn convert_node_to_element(
    scene: &UnifiedScene,
    node: &SceneNode,
    _node_idx: usize,
) -> Result<Option<Element>> {
    use seed_core::ast::Span;

    // If node has geometry, create a Part element
    if let Some(geom_idx) = node.geometry {
        if let Some(geom) = scene.geometries.get(geom_idx) {
            let seed_geom = convert_scene_geometry_to_seed(geom)?;
            let mut properties = Vec::new();

            // Add material properties
            if let Some(mat_idx) = node.material {
                if let Some(mat) = scene.materials.get(mat_idx) {
                    let color = seed_core::types::Color {
                        r: mat.base_color.x,
                        g: mat.base_color.y,
                        b: mat.base_color.z,
                        a: mat.base_color.w,
                    };
                    properties.push(Property {
                        name: "color".to_string(),
                        value: PropertyValue::Color(color),
                        span: Span::default(),
                    });
                }
            }

            return Ok(Some(Element::Part(PartElement {
                name: Some(Identifier(node.name.clone())),
                geometry: seed_geom,
                properties,
                constraints: Vec::new(),
                span: Span::default(),
            })));
        }
    }

    // If node has children but no geometry, create a Frame
    if !node.children.is_empty() {
        let mut children = Vec::new();
        for &child_idx in &node.children {
            if let Some(child_node) = scene.nodes.get(child_idx) {
                if let Some(child_elem) = convert_node_to_element(scene, child_node, child_idx)? {
                    children.push(child_elem);
                }
            }
        }

        if !children.is_empty() {
            return Ok(Some(Element::Frame(FrameElement {
                name: Some(Identifier(node.name.clone())),
                properties: Vec::new(),
                constraints: Vec::new(),
                children,
                span: Span::default(),
            })));
        }
    }

    Ok(None)
}

/// Convert UnifiedScene geometry to Seed geometry.
fn convert_scene_geometry_to_seed(geom: &Geometry) -> Result<SeedGeometry> {
    match geom {
        Geometry::Primitive(prim) => {
            let seed_prim = match prim {
                PrimitiveGeometry::Box {
                    half_extents,
                    transform: _,
                } => Primitive::Box {
                    width: Length::mm((half_extents.x * 2.0 * 1000.0) as f64),
                    height: Length::mm((half_extents.y * 2.0 * 1000.0) as f64),
                    depth: Length::mm((half_extents.z * 2.0 * 1000.0) as f64),
                },
                PrimitiveGeometry::Sphere {
                    radius,
                    transform: _,
                } => Primitive::Sphere {
                    radius: Length::mm((*radius * 1000.0) as f64),
                },
                PrimitiveGeometry::Cylinder {
                    radius,
                    height,
                    transform: _,
                } => Primitive::Cylinder {
                    radius: Length::mm((*radius * 1000.0) as f64),
                    height: Length::mm((*height * 1000.0) as f64),
                },
                PrimitiveGeometry::Cone {
                    radius,
                    height,
                    transform: _,
                } => Primitive::Cylinder {
                    radius: Length::mm((*radius * 1000.0) as f64),
                    height: Length::mm((*height * 1000.0) as f64),
                },
                PrimitiveGeometry::Torus { .. } | PrimitiveGeometry::Capsule { .. } => {
                    Primitive::Sphere {
                        radius: Length::mm(50.0),
                    }
                }
            };
            Ok(SeedGeometry::Primitive(seed_prim))
        }
        Geometry::Mesh(_) | Geometry::Brep(_) | Geometry::Nurbs(_) => {
            Ok(SeedGeometry::Primitive(Primitive::Box {
                width: Length::mm(100.0),
                height: Length::mm(100.0),
                depth: Length::mm(100.0),
            }))
        }
    }
}

/// Serialize a Seed document to text.
fn serialize_document(doc: &Document, pretty: bool) -> String {
    let mut output = String::new();
    let indent = if pretty { "    " } else { "  " };

    // Meta block
    if let Some(meta) = &doc.meta {
        output.push_str("@meta:\n");
        output.push_str(indent);
        match meta.profile {
            seed_core::ast::Profile::Seed2D => output.push_str("profile: Seed/2D"),
            seed_core::ast::Profile::Seed3D => output.push_str("profile: Seed/3D"),
        }
        output.push('\n');
        if let Some(version) = &meta.version {
            output.push_str(indent);
            output.push_str(&format!("version: {}", version));
            output.push('\n');
        }
        output.push('\n');
    }

    // Elements
    for element in &doc.elements {
        serialize_element(&mut output, element, 0, pretty);
        output.push('\n');
    }

    output
}

/// Serialize a single element.
fn serialize_element(output: &mut String, element: &Element, depth: usize, pretty: bool) {
    let indent_str = if pretty {
        "    ".repeat(depth)
    } else {
        "  ".repeat(depth)
    };

    match element {
        Element::Part(part) => {
            output.push_str(&indent_str);
            output.push_str("Part");
            if let Some(name) = &part.name {
                output.push_str(&format!(" {}", name.0));
            }
            output.push_str(":\n");

            let geom_indent = if pretty {
                "    ".repeat(depth + 1)
            } else {
                "  ".repeat(depth + 1)
            };
            match &part.geometry {
                SeedGeometry::Primitive(prim) => {
                    output.push_str(&geom_indent);
                    match prim {
                        Primitive::Box {
                            width,
                            height,
                            depth: d,
                        } => {
                            output.push_str(&format!(
                                "geometry: Box({}, {}, {})",
                                format_length(width),
                                format_length(height),
                                format_length(d)
                            ));
                        }
                        Primitive::Sphere { radius } => {
                            output
                                .push_str(&format!("geometry: Sphere({})", format_length(radius)));
                        }
                        Primitive::Cylinder { radius, height } => {
                            output.push_str(&format!(
                                "geometry: Cylinder({}, {})",
                                format_length(radius),
                                format_length(height)
                            ));
                        }
                    }
                    output.push('\n');
                }
                SeedGeometry::Csg(_) => {
                    output.push_str(&geom_indent);
                    output.push_str("geometry: Box(100mm, 100mm, 100mm)");
                    output.push('\n');
                }
            }

            for prop in &part.properties {
                output.push_str(&geom_indent);
                output.push_str(&format!("{}: ", prop.name));
                serialize_property_value(output, &prop.value);
                output.push('\n');
            }
        }
        Element::Frame(frame) => {
            output.push_str(&indent_str);
            output.push_str("Frame");
            if let Some(name) = &frame.name {
                output.push_str(&format!(" {}", name.0));
            }
            output.push_str(":\n");

            for child in &frame.children {
                serialize_element(output, child, depth + 1, pretty);
            }
        }
        _ => {}
    }
}

/// Serialize a property value.
fn serialize_property_value(output: &mut String, value: &PropertyValue) {
    match value {
        PropertyValue::Color(c) => {
            let r = (c.r * 255.0) as u8;
            let g = (c.g * 255.0) as u8;
            let b = (c.b * 255.0) as u8;
            output.push_str(&format!("#{:02x}{:02x}{:02x}", r, g, b));
        }
        PropertyValue::Number(n) => {
            output.push_str(&format!("{}", n));
        }
        PropertyValue::Length(l) => {
            output.push_str(&format_length(l));
        }
        PropertyValue::String(s) => {
            output.push_str(&format!("\"{}\"", s));
        }
        PropertyValue::Boolean(b) => {
            output.push_str(if *b { "true" } else { "false" });
        }
        _ => {
            output.push_str("null");
        }
    }
}

/// Format a length value.
fn format_length(len: &Length) -> String {
    match len.unit {
        LengthUnit::Px => format!("{}px", len.value),
        LengthUnit::Mm => format!("{}mm", len.value),
        LengthUnit::Cm => format!("{}cm", len.value),
        LengthUnit::In => format!("{}in", len.value),
        LengthUnit::Pt => format!("{}pt", len.value),
        LengthUnit::Em => format!("{}em", len.value),
        LengthUnit::Rem => format!("{}rem", len.value),
        LengthUnit::Percent => format!("{}%", len.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{FormatReader, FormatWriter, WriteOptions};
    use crate::scene::{Geometry, PrimitiveGeometry};
    use glam::Vec3;

    #[test]
    fn test_seed_reader_can_read() {
        let reader = SeedReader::new();

        assert!(reader.can_read(b"@meta:\n  profile: Seed/3D"));
        assert!(reader.can_read(b"Frame:\n  width: 100px"));
        assert!(reader.can_read(b"Part:\n  geometry: Box"));
        assert!(!reader.can_read(b"random text"));
        assert!(!reader.can_read(&[0xFF, 0xFE, 0x00]));
    }

    #[test]
    fn test_length_to_meters() {
        let len_mm = Length::mm(1000.0);
        assert!((length_to_meters(&len_mm) - 1.0).abs() < 0.001);

        let len_cm = Length {
            value: 100.0,
            unit: LengthUnit::Cm,
        };
        assert!((length_to_meters(&len_cm) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_seed_writer_basic() {
        let writer = SeedWriter::new();

        // Create a simple scene with a box
        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Box {
            half_extents: Vec3::new(0.05, 0.05, 0.05), // 100mm box
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);
        scene.add_root(SceneNode::with_geometry("TestBox", geom_idx));

        let options = WriteOptions::default();
        let result = writer.write(&scene, &options).unwrap();
        let text = String::from_utf8(result).unwrap();

        assert!(text.contains("@meta:"));
        assert!(text.contains("profile: Seed/3D"));
        assert!(text.contains("Part TestBox:"));
        assert!(text.contains("geometry: Box"));
    }

    #[test]
    fn test_seed_write_primitive_box() {
        let writer = SeedWriter::new();

        // Create scene with a box primitive
        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Box {
            half_extents: Vec3::new(0.05, 0.025, 0.01), // 100x50x20mm
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);

        let mut material = Material::new("RedMaterial");
        material.base_color = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let mat_idx = scene.add_material(material);

        let mut node = SceneNode::with_geometry("RedBox", geom_idx);
        node.material = Some(mat_idx);
        scene.add_root(node);

        // Write to Seed format
        let options = WriteOptions::default();
        let seed_bytes = writer.write(&scene, &options).unwrap();
        let seed_text = String::from_utf8(seed_bytes).unwrap();

        // Verify output format
        assert!(seed_text.contains("@meta:"));
        assert!(seed_text.contains("profile: Seed/3D"));
        assert!(seed_text.contains("Part RedBox:"));
        assert!(seed_text.contains("geometry: Box(100mm, 50mm, 20mm)"));
        assert!(seed_text.contains("color: #ff0000"));
    }

    #[test]
    fn test_seed_write_sphere() {
        let writer = SeedWriter::new();

        // Create scene with a sphere primitive
        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Sphere {
            radius: 0.025, // 25mm radius
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);
        scene.add_root(SceneNode::with_geometry("MySphere", geom_idx));

        let options = WriteOptions::default();
        let seed_bytes = writer.write(&scene, &options).unwrap();
        let seed_text = String::from_utf8(seed_bytes).unwrap();

        // Verify output
        assert!(seed_text.contains("Part MySphere:"));
        assert!(seed_text.contains("geometry: Sphere(25mm)"));
    }

    #[test]
    fn test_seed_write_cylinder() {
        let writer = SeedWriter::new();

        // Create scene with a cylinder primitive
        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Cylinder {
            radius: 0.01, // 10mm radius
            height: 0.05, // 50mm height
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);
        scene.add_root(SceneNode::with_geometry("MyCylinder", geom_idx));

        let options = WriteOptions::default();
        let seed_bytes = writer.write(&scene, &options).unwrap();
        let seed_text = String::from_utf8(seed_bytes).unwrap();

        assert!(seed_text.contains("Part MyCylinder:"));
        assert!(seed_text.contains("Cylinder(10mm, 50mm)"));
    }

    #[test]
    fn test_seed_write_hierarchy() {
        let writer = SeedWriter::new();

        // Create scene with hierarchy
        let mut scene = UnifiedScene::new();

        // Root frame
        let root_idx = scene.add_root(SceneNode::new("Assembly"));

        // Add child with geometry
        let geom = Geometry::Primitive(PrimitiveGeometry::Box {
            half_extents: Vec3::new(0.05, 0.05, 0.05),
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);
        scene.add_child(root_idx, SceneNode::with_geometry("Part1", geom_idx));

        // Add another child
        let geom2 = Geometry::Primitive(PrimitiveGeometry::Sphere {
            radius: 0.03,
            transform: Mat4::IDENTITY,
        });
        let geom_idx2 = scene.add_geometry(geom2);
        scene.add_child(root_idx, SceneNode::with_geometry("Part2", geom_idx2));

        let options = WriteOptions::default();
        let seed_bytes = writer.write(&scene, &options).unwrap();
        let seed_text = String::from_utf8(seed_bytes).unwrap();

        // Verify output structure
        assert!(seed_text.contains("Frame Assembly:"));
        assert!(seed_text.contains("Part Part1:"));
        assert!(seed_text.contains("Part Part2:"));
        assert!(seed_text.contains("geometry: Box(100mm, 100mm, 100mm)"));
        assert!(seed_text.contains("geometry: Sphere(30mm)"));
    }

    // Full roundtrip tests (now that Part parsing is supported)

    #[test]
    fn test_seed_roundtrip_box() {
        let reader = SeedReader::new();
        let writer = SeedWriter::new();

        // Create scene with a box primitive
        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Box {
            half_extents: glam::Vec3::new(0.05, 0.025, 0.01), // 100x50x20mm
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);

        let mut material = Material::new("RedMaterial");
        material.base_color = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let mat_idx = scene.add_material(material);

        let mut node = SceneNode::with_geometry("RedBox", geom_idx);
        node.material = Some(mat_idx);
        scene.add_root(node);

        // Write to Seed format
        let seed_bytes = writer.write(&scene, &WriteOptions::default()).unwrap();

        // Read back
        let read_scene = reader.read(&seed_bytes, &crate::registry::ReadOptions::default()).unwrap();

        // Verify
        assert_eq!(read_scene.roots.len(), 1);
        assert_eq!(read_scene.geometries.len(), 1);

        let root_node = &read_scene.nodes[read_scene.roots[0]];
        assert_eq!(root_node.name, "RedBox");
        assert!(root_node.geometry.is_some());

        // Verify geometry
        if let Geometry::Primitive(PrimitiveGeometry::Box { half_extents, .. }) = &read_scene.geometries[0] {
            // Original: 100mm x 50mm x 20mm -> half_extents: 50mm x 25mm x 10mm = 0.05 x 0.025 x 0.01 meters
            assert!((half_extents.x - 0.05).abs() < 0.001);
            assert!((half_extents.y - 0.025).abs() < 0.001);
            assert!((half_extents.z - 0.01).abs() < 0.001);
        } else {
            panic!("Expected Box geometry");
        }
    }

    #[test]
    fn test_seed_roundtrip_sphere() {
        let reader = SeedReader::new();
        let writer = SeedWriter::new();

        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Sphere {
            radius: 0.025, // 25mm
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);
        scene.add_root(SceneNode::with_geometry("MySphere", geom_idx));

        let seed_bytes = writer.write(&scene, &WriteOptions::default()).unwrap();
        let read_scene = reader.read(&seed_bytes, &crate::registry::ReadOptions::default()).unwrap();

        assert_eq!(read_scene.roots.len(), 1);
        let root_node = &read_scene.nodes[read_scene.roots[0]];
        assert_eq!(root_node.name, "MySphere");

        if let Geometry::Primitive(PrimitiveGeometry::Sphere { radius, .. }) = &read_scene.geometries[0] {
            assert!((*radius - 0.025).abs() < 0.001);
        } else {
            panic!("Expected Sphere geometry");
        }
    }

    #[test]
    fn test_seed_roundtrip_cylinder() {
        let reader = SeedReader::new();
        let writer = SeedWriter::new();

        let mut scene = UnifiedScene::new();
        let geom = Geometry::Primitive(PrimitiveGeometry::Cylinder {
            radius: 0.01, // 10mm
            height: 0.05, // 50mm
            transform: Mat4::IDENTITY,
        });
        let geom_idx = scene.add_geometry(geom);
        scene.add_root(SceneNode::with_geometry("MyCylinder", geom_idx));

        let seed_bytes = writer.write(&scene, &WriteOptions::default()).unwrap();
        let read_scene = reader.read(&seed_bytes, &crate::registry::ReadOptions::default()).unwrap();

        assert_eq!(read_scene.roots.len(), 1);

        if let Geometry::Primitive(PrimitiveGeometry::Cylinder { radius, height, .. }) = &read_scene.geometries[0] {
            assert!((*radius - 0.01).abs() < 0.001);
            assert!((*height - 0.05).abs() < 0.001);
        } else {
            panic!("Expected Cylinder geometry");
        }
    }

    #[test]
    fn test_seed_roundtrip_hierarchy() {
        let reader = SeedReader::new();
        let writer = SeedWriter::new();

        let mut scene = UnifiedScene::new();
        let root_idx = scene.add_root(SceneNode::new("Assembly"));

        let geom1 = Geometry::Primitive(PrimitiveGeometry::Box {
            half_extents: glam::Vec3::new(0.05, 0.05, 0.05),
            transform: Mat4::IDENTITY,
        });
        let geom_idx1 = scene.add_geometry(geom1);
        scene.add_child(root_idx, SceneNode::with_geometry("Part1", geom_idx1));

        let geom2 = Geometry::Primitive(PrimitiveGeometry::Sphere {
            radius: 0.03,
            transform: Mat4::IDENTITY,
        });
        let geom_idx2 = scene.add_geometry(geom2);
        scene.add_child(root_idx, SceneNode::with_geometry("Part2", geom_idx2));

        let seed_bytes = writer.write(&scene, &WriteOptions::default()).unwrap();
        let read_scene = reader.read(&seed_bytes, &crate::registry::ReadOptions::default()).unwrap();

        assert_eq!(read_scene.roots.len(), 1);
        let root = &read_scene.nodes[read_scene.roots[0]];
        assert_eq!(root.name, "Assembly");
        assert_eq!(root.children.len(), 2);

        // Verify children
        let child1 = &read_scene.nodes[root.children[0]];
        assert_eq!(child1.name, "Part1");
        assert!(child1.geometry.is_some());

        let child2 = &read_scene.nodes[root.children[1]];
        assert_eq!(child2.name, "Part2");
        assert!(child2.geometry.is_some());
    }

    #[test]
    fn test_seed_format_length() {
        assert_eq!(format_length(&Length::mm(100.0)), "100mm");
        assert_eq!(format_length(&Length::px(50.0)), "50px");
        assert_eq!(format_length(&Length {
            value: 2.5,
            unit: LengthUnit::Cm
        }), "2.5cm");
    }

    #[test]
    fn test_seed_writer_extensions() {
        let writer = SeedWriter::new();
        assert_eq!(writer.name(), "seed");
        assert_eq!(writer.extension(), "seed");
    }

    #[test]
    fn test_seed_reader_extensions() {
        let reader = SeedReader::new();
        assert_eq!(reader.name(), "seed");
        assert_eq!(reader.extensions(), &["seed"]);
    }
}
