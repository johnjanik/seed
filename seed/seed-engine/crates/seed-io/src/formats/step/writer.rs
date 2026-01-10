//! STEP writer implementation.

use crate::error::Result;
use crate::registry::{FormatWriter, WriteOptions};
use crate::scene::{Geometry, Material, TriangleMesh, UnifiedScene};

use glam::Vec3;
use std::collections::HashMap;

/// Writer for STEP files.
pub struct StepWriter;

impl StepWriter {
    /// Create a new STEP writer.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StepWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatWriter for StepWriter {
    fn name(&self) -> &'static str {
        "step"
    }

    fn extension(&self) -> &'static str {
        "step"
    }

    fn write(&self, scene: &UnifiedScene, options: &WriteOptions) -> Result<Vec<u8>> {
        let mut builder = StepBuilder::new(options);
        builder.build(scene);
        Ok(builder.finish().into_bytes())
    }
}

/// Builder for STEP file content.
struct StepBuilder<'a> {
    #[allow(dead_code)] // For future use with extended options
    options: &'a WriteOptions,
    output: String,
    next_id: u64,
    /// Maps (geometry_idx, node_idx) to product definition ID
    #[allow(dead_code)] // For future assembly support
    products: HashMap<(usize, usize), u64>,
}

impl<'a> StepBuilder<'a> {
    fn new(options: &'a WriteOptions) -> Self {
        Self {
            options,
            output: String::new(),
            next_id: 1,
            products: HashMap::new(),
        }
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn build(&mut self, scene: &UnifiedScene) {
        // Collect all entities first
        let mut entities = Vec::new();

        // Write application context and units
        let app_context = self.next_id();
        entities.push(format!(
            "#{}=APPLICATION_CONTEXT('automotive design');",
            app_context
        ));

        let app_protocol = self.next_id();
        entities.push(format!(
            "#{}=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2010,#{});",
            app_protocol, app_context
        ));

        // Geometric representation context
        let geom_context = self.next_id();
        entities.push(format!(
            "#{}=( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{},#{},#{})) REPRESENTATION_CONTEXT('Context #1','3D Context with TORTURE TOLERANCE'));",
            geom_context,
            self.next_id, // uncertainty
            self.next_id + 1, // length unit
            self.next_id + 2, // angle unit
            self.next_id + 3, // solid angle unit
        ));

        // Uncertainty
        let uncertainty = self.next_id();
        entities.push(format!(
            "#{}=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),#{},'distance_accuracy_value','confusion accuracy');",
            uncertainty, self.next_id // length unit
        ));

        // Units
        let length_unit = self.next_id();
        entities.push(format!(
            "#{}=( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) );",
            length_unit
        ));

        let angle_unit = self.next_id();
        entities.push(format!(
            "#{}=( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) );",
            angle_unit
        ));

        let solid_angle_unit = self.next_id();
        entities.push(format!(
            "#{}=( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() );",
            solid_angle_unit
        ));

        // Process each root node
        for &root_idx in &scene.roots {
            self.process_node(root_idx, scene, geom_context, app_context, &mut entities);
        }

        // Write header
        self.write_header(scene);

        // Write data section
        self.output.push_str("DATA;\n");
        for entity in entities {
            self.output.push_str(&entity);
            self.output.push('\n');
        }
        self.output.push_str("ENDSEC;\n");
        self.output.push_str("END-ISO-10303-21;\n");
    }

    fn write_header(&mut self, scene: &UnifiedScene) {
        self.output.push_str("ISO-10303-21;\n");
        self.output.push_str("HEADER;\n");

        // FILE_DESCRIPTION
        let description = scene
            .metadata
            .description
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("Exported from seed-io");
        self.output.push_str(&format!(
            "FILE_DESCRIPTION(('{}'),'2;1');\n",
            escape_step_string(description)
        ));

        // FILE_NAME
        let name = scene
            .metadata
            .name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("model.step");
        let author = scene
            .metadata
            .author
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or("");
        let timestamp = scene
            .metadata
            .modified
            .as_ref()
            .or(scene.metadata.created.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("2024-01-01T00:00:00");

        self.output.push_str(&format!(
            "FILE_NAME('{}','{}',('{}'),(''),'seed-io','seed-io','');\n",
            escape_step_string(name),
            timestamp,
            escape_step_string(author)
        ));

        // FILE_SCHEMA
        self.output
            .push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\n");

        self.output.push_str("ENDSEC;\n");
    }

    fn process_node(
        &mut self,
        node_idx: usize,
        scene: &UnifiedScene,
        geom_context: u64,
        app_context: u64,
        entities: &mut Vec<String>,
    ) {
        let node = &scene.nodes[node_idx];

        // Process geometry if present
        if let Some(geom_idx) = node.geometry {
            let geometry = &scene.geometries[geom_idx];
            let material = node
                .material
                .and_then(|idx| scene.materials.get(idx));

            match geometry {
                Geometry::Mesh(mesh) => {
                    self.write_mesh(
                        mesh,
                        &node.name,
                        material,
                        geom_context,
                        app_context,
                        entities,
                    );
                }
                Geometry::Brep(_brep) => {
                    // TODO: Write B-rep directly as STEP entities
                    // For now, skip B-rep geometry - would need tessellation
                }
                Geometry::Primitive(prim) => {
                    // Generate mesh from primitive
                    let mesh =
                        crate::convert::primitives::generate_primitive_mesh(prim, 16);
                    self.write_mesh(
                        &mesh,
                        &node.name,
                        material,
                        geom_context,
                        app_context,
                        entities,
                    );
                }
                Geometry::Nurbs(_) => {
                    // TODO: Write NURBS directly as B_SPLINE_SURFACE
                }
            }
        }

        // Process children
        for &child_idx in &node.children {
            self.process_node(child_idx, scene, geom_context, app_context, entities);
        }
    }

    fn write_mesh(
        &mut self,
        mesh: &TriangleMesh,
        name: &str,
        material: Option<&Material>,
        geom_context: u64,
        app_context: u64,
        entities: &mut Vec<String>,
    ) {
        if mesh.positions.is_empty() || mesh.indices.is_empty() {
            return;
        }

        let product_name = if name.is_empty() { "Part" } else { name };

        // Write all cartesian points
        let mut point_ids: Vec<u64> = Vec::with_capacity(mesh.positions.len());
        for pos in &mesh.positions {
            let id = self.next_id();
            entities.push(format!(
                "#{}=CARTESIAN_POINT('',({},{},{}));",
                id,
                format_real(pos.x),
                format_real(pos.y),
                format_real(pos.z)
            ));
            point_ids.push(id);
        }

        // Build faces from triangles
        let mut face_ids = Vec::new();

        for tri in mesh.indices.chunks(3) {
            if tri.len() != 3 {
                continue;
            }

            let v0 = tri[0] as usize;
            let v1 = tri[1] as usize;
            let v2 = tri[2] as usize;

            if v0 >= point_ids.len() || v1 >= point_ids.len() || v2 >= point_ids.len() {
                continue;
            }

            // Create vertices
            let vp0 = self.next_id();
            entities.push(format!("#{}=VERTEX_POINT('',#{});", vp0, point_ids[v0]));

            let vp1 = self.next_id();
            entities.push(format!("#{}=VERTEX_POINT('',#{});", vp1, point_ids[v1]));

            let vp2 = self.next_id();
            entities.push(format!("#{}=VERTEX_POINT('',#{});", vp2, point_ids[v2]));

            // Create direction for plane normal
            let p0 = mesh.positions[v0];
            let p1 = mesh.positions[v1];
            let p2 = mesh.positions[v2];
            let normal = (p1 - p0).cross(p2 - p0).normalize_or_zero();

            let dir_z = self.next_id();
            entities.push(format!(
                "#{}=DIRECTION('',({},{},{}));",
                dir_z,
                format_real(normal.x),
                format_real(normal.y),
                format_real(normal.z)
            ));

            // X direction (perpendicular to normal)
            let x_dir = if normal.x.abs() < 0.9 {
                Vec3::X.cross(normal).normalize_or_zero()
            } else {
                Vec3::Y.cross(normal).normalize_or_zero()
            };

            let dir_x = self.next_id();
            entities.push(format!(
                "#{}=DIRECTION('',({},{},{}));",
                dir_x,
                format_real(x_dir.x),
                format_real(x_dir.y),
                format_real(x_dir.z)
            ));

            // Axis placement
            let axis = self.next_id();
            entities.push(format!(
                "#{}=AXIS2_PLACEMENT_3D('',#{},#{},#{});",
                axis, point_ids[v0], dir_z, dir_x
            ));

            // Plane
            let plane = self.next_id();
            entities.push(format!("#{}=PLANE('',#{});", plane, axis));

            // Create poly loop (simpler than edge loops for triangles)
            let poly_loop = self.next_id();
            entities.push(format!(
                "#{}=POLY_LOOP('',(#{},#{},#{}));",
                poly_loop, point_ids[v0], point_ids[v1], point_ids[v2]
            ));

            // Face bound
            let face_bound = self.next_id();
            entities.push(format!(
                "#{}=FACE_OUTER_BOUND('',#{},.T.);",
                face_bound, poly_loop
            ));

            // Face
            let face = self.next_id();
            entities.push(format!(
                "#{}=FACE_SURFACE('',({}),'',#{},.T.);",
                face, format!("#{}", face_bound), plane
            ));

            face_ids.push(face);
        }

        if face_ids.is_empty() {
            return;
        }

        // Create closed shell
        let face_list: String = face_ids
            .iter()
            .map(|id| format!("#{}", id))
            .collect::<Vec<_>>()
            .join(",");

        let shell = self.next_id();
        entities.push(format!("#{}=CLOSED_SHELL('',({});", shell, face_list));

        // Manifold solid brep
        let brep = self.next_id();
        entities.push(format!(
            "#{}=MANIFOLD_SOLID_BREP('{}',#{});",
            brep,
            escape_step_string(product_name),
            shell
        ));

        // Shape representation
        let shape_rep = self.next_id();
        entities.push(format!(
            "#{}=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{},#{}),#{});",
            shape_rep, brep, self.next_id, geom_context // next_id will be axis placement
        ));

        // Origin axis
        let origin = self.next_id();
        entities.push(format!("#{}=CARTESIAN_POINT('',(0.,0.,0.));", origin));

        let dir_z = self.next_id();
        entities.push(format!("#{}=DIRECTION('',(0.,0.,1.));", dir_z));

        let dir_x = self.next_id();
        entities.push(format!("#{}=DIRECTION('',(1.,0.,0.));", dir_x));

        let axis = self.next_id();
        entities.push(format!(
            "#{}=AXIS2_PLACEMENT_3D('',#{},#{},#{});",
            axis, origin, dir_z, dir_x
        ));

        // Product
        let product = self.next_id();
        entities.push(format!(
            "#{}=PRODUCT('{}','{}','',(#{}));",
            product,
            escape_step_string(product_name),
            escape_step_string(product_name),
            app_context
        ));

        // Product definition formation
        let pdf = self.next_id();
        entities.push(format!(
            "#{}=PRODUCT_DEFINITION_FORMATION('','',#{});",
            pdf, product
        ));

        // Product definition context
        let pdc = self.next_id();
        entities.push(format!(
            "#{}=PRODUCT_DEFINITION_CONTEXT('part definition',#{},'design');",
            pdc, app_context
        ));

        // Product definition
        let pd = self.next_id();
        entities.push(format!(
            "#{}=PRODUCT_DEFINITION('design','',#{},#{});",
            pd, pdf, pdc
        ));

        // Product definition shape
        let pds = self.next_id();
        entities.push(format!(
            "#{}=PRODUCT_DEFINITION_SHAPE('','Shape of {}',#{});",
            pds,
            escape_step_string(product_name),
            pd
        ));

        // Shape definition representation
        let sdr = self.next_id();
        entities.push(format!(
            "#{}=SHAPE_DEFINITION_REPRESENTATION(#{},#{});",
            sdr, pds, shape_rep
        ));

        // Write color/material if available
        if let Some(mat) = material {
            self.write_material(mat, brep, entities);
        }
    }

    fn write_material(&mut self, material: &Material, shape_id: u64, entities: &mut Vec<String>) {
        let color = material.base_color;

        // Color RGB
        let color_rgb = self.next_id();
        entities.push(format!(
            "#{}=COLOUR_RGB('',{},{},{});",
            color_rgb,
            format_real(color.x),
            format_real(color.y),
            format_real(color.z)
        ));

        // Fill area style colour
        let fill_colour = self.next_id();
        entities.push(format!(
            "#{}=FILL_AREA_STYLE_COLOUR('',#{});",
            fill_colour, color_rgb
        ));

        // Fill area style
        let fill_style = self.next_id();
        entities.push(format!(
            "#{}=FILL_AREA_STYLE('',(#{}));",
            fill_style, fill_colour
        ));

        // Surface style fill area
        let surface_fill = self.next_id();
        entities.push(format!(
            "#{}=SURFACE_STYLE_FILL_AREA(#{});",
            surface_fill, fill_style
        ));

        // Surface side style
        let surface_style = self.next_id();
        entities.push(format!(
            "#{}=SURFACE_SIDE_STYLE('',(#{}));",
            surface_style, surface_fill
        ));

        // Surface style usage
        let style_usage = self.next_id();
        entities.push(format!(
            "#{}=SURFACE_STYLE_USAGE(.BOTH.,#{});",
            style_usage, surface_style
        ));

        // Presentation style assignment
        let psa = self.next_id();
        entities.push(format!(
            "#{}=PRESENTATION_STYLE_ASSIGNMENT((#{}));",
            psa, style_usage
        ));

        // Styled item
        let styled_item = self.next_id();
        entities.push(format!(
            "#{}=STYLED_ITEM('color',(#{}),#{});",
            styled_item, psa, shape_id
        ));
    }

    fn finish(self) -> String {
        self.output
    }
}

/// Escape a string for STEP format.
fn escape_step_string(s: &str) -> String {
    s.replace('\'', "''")
}

/// Format a real number for STEP.
fn format_real(value: f32) -> String {
    if value == 0.0 {
        "0.".to_string()
    } else if value.abs() < 1e-10 {
        "0.".to_string()
    } else if value.fract() == 0.0 {
        format!("{}.", value as i64)
    } else {
        let s = format!("{:.6}", value);
        // Remove trailing zeros but keep at least one decimal place
        let s = s.trim_end_matches('0');
        if s.ends_with('.') {
            format!("{}0", s)
        } else {
            s.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::FormatReader;
    use crate::scene::{Geometry, SceneNode};
    use glam::{Vec3, Vec4};

    #[test]
    fn test_format_real() {
        assert_eq!(format_real(0.0), "0.");
        assert_eq!(format_real(1.0), "1.");
        assert_eq!(format_real(3.14), "3.14");
        assert_eq!(format_real(-2.5), "-2.5");
    }

    #[test]
    fn test_escape_step_string() {
        assert_eq!(escape_step_string("hello"), "hello");
        assert_eq!(escape_step_string("it's"), "it''s");
        assert_eq!(escape_step_string("a'b'c"), "a''b''c");
    }

    #[test]
    fn test_write_empty_scene() {
        let writer = StepWriter::new();
        let scene = UnifiedScene::new();

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let output = String::from_utf8(result.unwrap()).unwrap();
        assert!(output.contains("ISO-10303-21"));
        assert!(output.contains("FILE_DESCRIPTION"));
        assert!(output.contains("ENDSEC"));
        assert!(output.contains("END-ISO-10303-21"));
    }

    #[test]
    fn test_write_simple_mesh() {
        let writer = StepWriter::new();
        let mut scene = UnifiedScene::new();

        // Create a simple triangle
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

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let output = String::from_utf8(result.unwrap()).unwrap();

        // Check for key entities
        assert!(output.contains("CARTESIAN_POINT"));
        assert!(output.contains("CLOSED_SHELL"));
        assert!(output.contains("MANIFOLD_SOLID_BREP"));
        assert!(output.contains("PRODUCT"));
        assert!(output.contains("Triangle"));
    }

    #[test]
    fn test_write_with_material() {
        let writer = StepWriter::new();
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

        let material = Material {
            name: "Red".to_string(),
            base_color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            ..Default::default()
        };

        let geom_idx = scene.add_geometry(Geometry::Mesh(mesh));
        let mat_idx = scene.add_material(material);
        scene.add_root(
            SceneNode::with_geometry("RedTriangle", geom_idx).with_material(mat_idx),
        );

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let output = String::from_utf8(result.unwrap()).unwrap();

        // Check for color entities
        assert!(output.contains("COLOUR_RGB"));
        assert!(output.contains("STYLED_ITEM"));
    }

    #[test]
    fn test_roundtrip_detection() {
        // Write a mesh to STEP
        let writer = StepWriter::new();
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
        scene.add_root(SceneNode::with_geometry("Test", geom_idx));

        let step_data = writer.write(&scene, &WriteOptions::default()).unwrap();

        // Verify the reader can detect it
        let reader = super::super::reader::StepReader::new();
        assert!(reader.can_read(&step_data));
    }

    #[test]
    fn test_multiple_meshes() {
        let writer = StepWriter::new();
        let mut scene = UnifiedScene::new();

        // First triangle
        let mesh1 = TriangleMesh {
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

        // Second triangle
        let mesh2 = TriangleMesh {
            positions: vec![
                Vec3::new(2.0, 0.0, 0.0),
                Vec3::new(3.0, 0.0, 0.0),
                Vec3::new(2.5, 1.0, 0.0),
            ],
            normals: None,
            texcoords: None,
            colors: None,
            indices: vec![0, 1, 2],
            cached_bounds: None,
        };

        let geom1 = scene.add_geometry(Geometry::Mesh(mesh1));
        let geom2 = scene.add_geometry(Geometry::Mesh(mesh2));
        scene.add_root(SceneNode::with_geometry("Part1", geom1));
        scene.add_root(SceneNode::with_geometry("Part2", geom2));

        let result = writer.write(&scene, &WriteOptions::default());
        assert!(result.is_ok());

        let output = String::from_utf8(result.unwrap()).unwrap();

        // Should have two products
        let product_count = output.matches("PRODUCT(").count();
        assert!(product_count >= 2);
    }
}
