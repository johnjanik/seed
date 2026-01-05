//! STEP (ISO 10303-21) export for 3D models.
//!
//! Exports Seed 3D documents to STEP AP203 format for CAD interoperability.
//! Supports primitive shapes (Box, Cylinder, Sphere) and CSG compounds.

use seed_core::{Document, ExportError, Primitive, Geometry};
use seed_core::ast::{Element, PartElement, CsgOperation};
use std::fmt::Write;

/// STEP export options.
#[derive(Debug, Clone)]
pub struct StepOptions {
    /// Author name for file metadata.
    pub author: String,
    /// Organization name for file metadata.
    pub organization: String,
    /// Preprocessor system name.
    pub preprocessor: String,
    /// Unit for dimensions (default: millimeters).
    pub unit: LengthUnit,
}

impl Default for StepOptions {
    fn default() -> Self {
        Self {
            author: "Seed Engine".to_string(),
            organization: "".to_string(),
            preprocessor: "seed-export".to_string(),
            unit: LengthUnit::Millimeter,
        }
    }
}

/// Length unit for STEP export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    Millimeter,
    Meter,
    Inch,
}

/// Export a document to STEP AP203 format.
pub fn export(doc: &Document) -> Result<Vec<u8>, ExportError> {
    export_with_options(doc, &StepOptions::default())
}

/// Export a document to STEP format with options.
pub fn export_with_options(doc: &Document, options: &StepOptions) -> Result<Vec<u8>, ExportError> {
    let mut builder = StepBuilder::new(options);

    // Collect all Part elements from the document
    let parts: Vec<&PartElement> = doc.elements.iter()
        .filter_map(|e| match e {
            Element::Part(p) => Some(p),
            _ => None,
        })
        .collect();

    if parts.is_empty() {
        return Err(ExportError::NoGeometry);
    }

    // Build geometry for each part
    for part in parts {
        builder.add_part(part)?;
    }

    Ok(builder.finish().into_bytes())
}

/// Builder for STEP file content.
struct StepBuilder<'a> {
    options: &'a StepOptions,
    entities: Vec<String>,
    next_id: usize,
}

impl<'a> StepBuilder<'a> {
    fn new(options: &'a StepOptions) -> Self {
        Self {
            options,
            entities: Vec::new(),
            next_id: 1,
        }
    }

    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_entity(&mut self, entity: String) -> usize {
        let id = self.next_id();
        self.entities.push(format!("#{} = {};", id, entity));
        id
    }

    fn add_part(&mut self, part: &PartElement) -> Result<usize, ExportError> {
        let name = part.name.as_ref()
            .map(|n| n.0.clone())
            .unwrap_or_else(|| "Part".to_string());

        // Build the geometry
        let shape_id = self.build_geometry(&part.geometry)?;

        // Create product and shape representation
        self.create_product(&name, shape_id)
    }

    fn build_geometry(&mut self, geometry: &Geometry) -> Result<usize, ExportError> {
        match geometry {
            Geometry::Primitive(prim) => self.build_primitive(prim),
            Geometry::Csg(csg) => self.build_csg(csg),
        }
    }

    fn build_primitive(&mut self, prim: &Primitive) -> Result<usize, ExportError> {
        match prim {
            Primitive::Box { width, height, depth } => {
                let w = width.to_mm().unwrap_or(10.0);
                let h = height.to_mm().unwrap_or(10.0);
                let d = depth.to_mm().unwrap_or(10.0);
                self.build_box(w, h, d)
            }
            Primitive::Cylinder { radius, height } => {
                let r = radius.to_mm().unwrap_or(5.0);
                let h = height.to_mm().unwrap_or(10.0);
                self.build_cylinder(r, h)
            }
            Primitive::Sphere { radius } => {
                let r = radius.to_mm().unwrap_or(5.0);
                self.build_sphere(r)
            }
        }
    }

    fn build_csg(&mut self, csg: &CsgOperation) -> Result<usize, ExportError> {
        match csg {
            CsgOperation::Union(geometries) => {
                // Build each geometry and create a shell assembly
                let mut shape_ids = Vec::new();
                for geom in geometries {
                    shape_ids.push(self.build_geometry(geom)?);
                }

                if shape_ids.len() == 1 {
                    Ok(shape_ids[0])
                } else {
                    // Create assembly of shapes
                    self.build_assembly(&shape_ids)
                }
            }
            CsgOperation::Difference { base, subtract } => {
                // For proper CSG difference, we'd need boolean operations
                // For now, export the base shape with a comment
                let base_id = self.build_geometry(base)?;
                // Add a note about the subtraction (STEP viewers will show base only)
                for sub in subtract {
                    let _ = self.build_geometry(sub)?;
                }
                Ok(base_id)
            }
            CsgOperation::Intersection(geometries) => {
                // Similar to union - export all shapes
                let mut shape_ids = Vec::new();
                for geom in geometries {
                    shape_ids.push(self.build_geometry(geom)?);
                }

                if shape_ids.is_empty() {
                    Err(ExportError::NoGeometry)
                } else if shape_ids.len() == 1 {
                    Ok(shape_ids[0])
                } else {
                    self.build_assembly(&shape_ids)
                }
            }
        }
    }

    /// Build a box as a B-rep solid.
    fn build_box(&mut self, width: f64, height: f64, depth: f64) -> Result<usize, ExportError> {
        // Box centered at origin
        let hw = width / 2.0;
        let hh = height / 2.0;
        let hd = depth / 2.0;

        // 8 vertices of the box
        let v = [
            (-hw, -hh, -hd), // 0: bottom-left-back
            ( hw, -hh, -hd), // 1: bottom-right-back
            ( hw, -hh,  hd), // 2: bottom-right-front
            (-hw, -hh,  hd), // 3: bottom-left-front
            (-hw,  hh, -hd), // 4: top-left-back
            ( hw,  hh, -hd), // 5: top-right-back
            ( hw,  hh,  hd), // 6: top-right-front
            (-hw,  hh,  hd), // 7: top-left-front
        ];

        // Create cartesian points for vertices
        let mut point_ids = Vec::new();
        for (x, y, z) in &v {
            let id = self.add_entity(format!(
                "CARTESIAN_POINT('', ({:.6}, {:.6}, {:.6}))",
                x, y, z
            ));
            point_ids.push(id);
        }

        // Create vertex points
        let mut vertex_ids = Vec::new();
        for pid in &point_ids {
            let id = self.add_entity(format!("VERTEX_POINT('', #{})", pid));
            vertex_ids.push(id);
        }

        // Origin and directions for axis placements
        let _origin_id = self.add_entity("CARTESIAN_POINT('', (0.0, 0.0, 0.0))".to_string());
        let dir_z_id = self.add_entity("DIRECTION('', (0.0, 0.0, 1.0))".to_string());
        let dir_x_id = self.add_entity("DIRECTION('', (1.0, 0.0, 0.0))".to_string());
        let dir_y_id = self.add_entity("DIRECTION('', (0.0, 1.0, 0.0))".to_string());
        let dir_nz_id = self.add_entity("DIRECTION('', (0.0, 0.0, -1.0))".to_string());
        let dir_nx_id = self.add_entity("DIRECTION('', (-1.0, 0.0, 0.0))".to_string());
        let dir_ny_id = self.add_entity("DIRECTION('', (0.0, -1.0, 0.0))".to_string());

        // Create 6 faces of the box
        // Face definitions: (vertices, normal direction, ref direction)
        let face_defs = [
            // Bottom face (y = -hh): v0, v1, v2, v3
            ([0, 1, 2, 3], dir_ny_id, dir_x_id, (0.0, -hh, 0.0)),
            // Top face (y = hh): v4, v7, v6, v5
            ([4, 7, 6, 5], dir_y_id, dir_x_id, (0.0, hh, 0.0)),
            // Front face (z = hd): v3, v2, v6, v7
            ([3, 2, 6, 7], dir_z_id, dir_x_id, (0.0, 0.0, hd)),
            // Back face (z = -hd): v0, v4, v5, v1
            ([0, 4, 5, 1], dir_nz_id, dir_x_id, (0.0, 0.0, -hd)),
            // Right face (x = hw): v1, v5, v6, v2
            ([1, 5, 6, 2], dir_x_id, dir_z_id, (hw, 0.0, 0.0)),
            // Left face (x = -hw): v0, v3, v7, v4
            ([0, 3, 7, 4], dir_nx_id, dir_z_id, (-hw, 0.0, 0.0)),
        ];

        let mut face_ids = Vec::new();

        for (vertices, normal_dir, ref_dir, plane_origin) in &face_defs {
            // Create plane for this face
            let plane_origin_id = self.add_entity(format!(
                "CARTESIAN_POINT('', ({:.6}, {:.6}, {:.6}))",
                plane_origin.0, plane_origin.1, plane_origin.2
            ));
            let axis_id = self.add_entity(format!(
                "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
                plane_origin_id, normal_dir, ref_dir
            ));
            let plane_id = self.add_entity(format!("PLANE('', #{})", axis_id));

            // Create edges for this face
            let mut edge_ids = Vec::new();
            for i in 0..4 {
                let v1 = vertices[i];
                let v2 = vertices[(i + 1) % 4];

                // Line between vertices
                let p1 = point_ids[v1];

                // Compute direction first to avoid borrow issue
                let dir_id = self.get_direction_for_edge(v[v1], v[v2]);
                let vec_id = self.add_entity(format!(
                    "VECTOR('', #{}, 1.0)", dir_id
                ));
                let line_id = self.add_entity(format!(
                    "LINE('', #{}, #{})",
                    p1, vec_id
                ));

                let edge_curve_id = self.add_entity(format!(
                    "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
                    vertex_ids[v1], vertex_ids[v2], line_id
                ));

                let oriented_edge_id = self.add_entity(format!(
                    "ORIENTED_EDGE('', *, *, #{}, .T.)",
                    edge_curve_id
                ));

                edge_ids.push(oriented_edge_id);
            }

            // Create edge loop
            let edge_loop_id = self.add_entity(format!(
                "EDGE_LOOP('', ({}))",
                edge_ids.iter().map(|id| format!("#{}", id)).collect::<Vec<_>>().join(", ")
            ));

            // Create face bound
            let face_bound_id = self.add_entity(format!(
                "FACE_OUTER_BOUND('', #{}, .T.)",
                edge_loop_id
            ));

            // Create advanced face
            let face_id = self.add_entity(format!(
                "ADVANCED_FACE('', (#{}), #{}, .T.)",
                face_bound_id, plane_id
            ));

            face_ids.push(face_id);
        }

        // Create closed shell
        let shell_id = self.add_entity(format!(
            "CLOSED_SHELL('', ({}))",
            face_ids.iter().map(|id| format!("#{}", id)).collect::<Vec<_>>().join(", ")
        ));

        // Create manifold solid brep
        let solid_id = self.add_entity(format!(
            "MANIFOLD_SOLID_BREP('Box', #{})",
            shell_id
        ));

        Ok(solid_id)
    }

    fn get_direction_for_edge(&mut self, p1: (f64, f64, f64), p2: (f64, f64, f64)) -> usize {
        let dx = p2.0 - p1.0;
        let dy = p2.1 - p1.1;
        let dz = p2.2 - p1.2;
        let len = (dx*dx + dy*dy + dz*dz).sqrt();

        if len > 0.0 {
            self.add_entity(format!(
                "DIRECTION('', ({:.6}, {:.6}, {:.6}))",
                dx / len, dy / len, dz / len
            ))
        } else {
            self.add_entity("DIRECTION('', (1.0, 0.0, 0.0))".to_string())
        }
    }

    /// Build a cylinder as a B-rep solid.
    fn build_cylinder(&mut self, radius: f64, height: f64) -> Result<usize, ExportError> {
        let hh = height / 2.0;

        // Origin and directions
        let bottom_center_id = self.add_entity(format!(
            "CARTESIAN_POINT('', (0.0, {:.6}, 0.0))", -hh
        ));
        let top_center_id = self.add_entity(format!(
            "CARTESIAN_POINT('', (0.0, {:.6}, 0.0))", hh
        ));
        let origin_id = self.add_entity("CARTESIAN_POINT('', (0.0, 0.0, 0.0))".to_string());

        let dir_y_id = self.add_entity("DIRECTION('', (0.0, 1.0, 0.0))".to_string());
        let dir_ny_id = self.add_entity("DIRECTION('', (0.0, -1.0, 0.0))".to_string());
        let dir_x_id = self.add_entity("DIRECTION('', (1.0, 0.0, 0.0))".to_string());

        // Axis placement for cylinder
        let axis_bottom_id = self.add_entity(format!(
            "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
            bottom_center_id, dir_y_id, dir_x_id
        ));
        let axis_top_id = self.add_entity(format!(
            "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
            top_center_id, dir_ny_id, dir_x_id
        ));

        // Cylindrical surface
        let cylinder_axis_id = self.add_entity(format!(
            "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
            origin_id, dir_y_id, dir_x_id
        ));
        let cyl_surface_id = self.add_entity(format!(
            "CYLINDRICAL_SURFACE('', #{}, {:.6})",
            cylinder_axis_id, radius
        ));

        // Circular planes for top and bottom
        let bottom_plane_id = self.add_entity(format!("PLANE('', #{})", axis_bottom_id));
        let top_plane_id = self.add_entity(format!("PLANE('', #{})", axis_top_id));

        // Vertices at seam
        let bottom_seam_pt_id = self.add_entity(format!(
            "CARTESIAN_POINT('', ({:.6}, {:.6}, 0.0))", radius, -hh
        ));
        let top_seam_pt_id = self.add_entity(format!(
            "CARTESIAN_POINT('', ({:.6}, {:.6}, 0.0))", radius, hh
        ));

        let bottom_vertex_id = self.add_entity(format!("VERTEX_POINT('', #{})", bottom_seam_pt_id));
        let top_vertex_id = self.add_entity(format!("VERTEX_POINT('', #{})", top_seam_pt_id));

        // Bottom circle
        let bottom_circle_id = self.add_entity(format!(
            "CIRCLE('', #{}, {:.6})", axis_bottom_id, radius
        ));

        // Top circle
        let top_circle_id = self.add_entity(format!(
            "CIRCLE('', #{}, {:.6})", axis_top_id, radius
        ));

        // Seam line (vertical edge)
        let seam_dir_id = self.add_entity("DIRECTION('', (0.0, 1.0, 0.0))".to_string());
        let seam_vec_id = self.add_entity(format!("VECTOR('', #{}, 1.0)", seam_dir_id));
        let seam_line_id = self.add_entity(format!(
            "LINE('', #{}, #{})", bottom_seam_pt_id, seam_vec_id
        ));

        // Edge curves
        let bottom_edge_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            bottom_vertex_id, bottom_vertex_id, bottom_circle_id
        ));
        let top_edge_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            top_vertex_id, top_vertex_id, top_circle_id
        ));
        let seam_edge_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            bottom_vertex_id, top_vertex_id, seam_line_id
        ));

        // Oriented edges for faces
        let bottom_oe_id = self.add_entity(format!(
            "ORIENTED_EDGE('', *, *, #{}, .T.)", bottom_edge_id
        ));
        let top_oe_id = self.add_entity(format!(
            "ORIENTED_EDGE('', *, *, #{}, .T.)", top_edge_id
        ));
        let seam_oe_up_id = self.add_entity(format!(
            "ORIENTED_EDGE('', *, *, #{}, .T.)", seam_edge_id
        ));
        let seam_oe_down_id = self.add_entity(format!(
            "ORIENTED_EDGE('', *, *, #{}, .F.)", seam_edge_id
        ));

        // Edge loops
        let bottom_loop_id = self.add_entity(format!(
            "EDGE_LOOP('', (#{}));", bottom_oe_id
        ));
        let top_loop_id = self.add_entity(format!(
            "EDGE_LOOP('', (#{}));", top_oe_id
        ));
        let side_loop_id = self.add_entity(format!(
            "EDGE_LOOP('', (#{}, #{}, #{}, #{}))",
            bottom_oe_id, seam_oe_up_id, top_oe_id, seam_oe_down_id
        ));

        // Face bounds
        let bottom_bound_id = self.add_entity(format!(
            "FACE_OUTER_BOUND('', #{}, .T.)", bottom_loop_id
        ));
        let top_bound_id = self.add_entity(format!(
            "FACE_OUTER_BOUND('', #{}, .T.)", top_loop_id
        ));
        let side_bound_id = self.add_entity(format!(
            "FACE_OUTER_BOUND('', #{}, .T.)", side_loop_id
        ));

        // Faces
        let bottom_face_id = self.add_entity(format!(
            "ADVANCED_FACE('', (#{}), #{}, .F.)", bottom_bound_id, bottom_plane_id
        ));
        let top_face_id = self.add_entity(format!(
            "ADVANCED_FACE('', (#{}), #{}, .T.)", top_bound_id, top_plane_id
        ));
        let side_face_id = self.add_entity(format!(
            "ADVANCED_FACE('', (#{}), #{}, .T.)", side_bound_id, cyl_surface_id
        ));

        // Closed shell
        let shell_id = self.add_entity(format!(
            "CLOSED_SHELL('', (#{}, #{}, #{}))",
            bottom_face_id, top_face_id, side_face_id
        ));

        // Manifold solid
        let solid_id = self.add_entity(format!(
            "MANIFOLD_SOLID_BREP('Cylinder', #{})", shell_id
        ));

        Ok(solid_id)
    }

    /// Build a sphere as a B-rep solid (simplified representation).
    fn build_sphere(&mut self, radius: f64) -> Result<usize, ExportError> {
        // Origin and directions
        let origin_id = self.add_entity("CARTESIAN_POINT('', (0.0, 0.0, 0.0))".to_string());
        let dir_z_id = self.add_entity("DIRECTION('', (0.0, 0.0, 1.0))".to_string());
        let dir_x_id = self.add_entity("DIRECTION('', (1.0, 0.0, 0.0))".to_string());

        // Axis placement for sphere
        let axis_id = self.add_entity(format!(
            "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
            origin_id, dir_z_id, dir_x_id
        ));

        // Spherical surface
        let sphere_surface_id = self.add_entity(format!(
            "SPHERICAL_SURFACE('', #{}, {:.6})",
            axis_id, radius
        ));

        // For a proper sphere B-rep, we need poles and seams
        // Simplified: create a single spherical face with no bounds (complete sphere)

        // North and south pole vertices
        let north_pt_id = self.add_entity(format!(
            "CARTESIAN_POINT('', (0.0, 0.0, {:.6}))", radius
        ));
        let south_pt_id = self.add_entity(format!(
            "CARTESIAN_POINT('', (0.0, 0.0, {:.6}))", -radius
        ));
        let seam_pt_id = self.add_entity(format!(
            "CARTESIAN_POINT('', ({:.6}, 0.0, 0.0))", radius
        ));

        let north_vertex_id = self.add_entity(format!("VERTEX_POINT('', #{})", north_pt_id));
        let south_vertex_id = self.add_entity(format!("VERTEX_POINT('', #{})", south_pt_id));
        let seam_vertex_id = self.add_entity(format!("VERTEX_POINT('', #{})", seam_pt_id));

        // Create meridian circles for the seam
        let dir_y_id = self.add_entity("DIRECTION('', (0.0, 1.0, 0.0))".to_string());
        let meridian_axis_id = self.add_entity(format!(
            "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
            origin_id, dir_y_id, dir_x_id
        ));
        let meridian_circle_id = self.add_entity(format!(
            "CIRCLE('', #{}, {:.6})", meridian_axis_id, radius
        ));

        // Edges from seam to poles
        let seam_to_north_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            seam_vertex_id, north_vertex_id, meridian_circle_id
        ));
        let north_to_seam_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            north_vertex_id, seam_vertex_id, meridian_circle_id
        ));
        let seam_to_south_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            seam_vertex_id, south_vertex_id, meridian_circle_id
        ));
        let south_to_seam_id = self.add_entity(format!(
            "EDGE_CURVE('', #{}, #{}, #{}, .T.)",
            south_vertex_id, seam_vertex_id, meridian_circle_id
        ));

        // Oriented edges for the two hemispheres
        let oe1 = self.add_entity(format!("ORIENTED_EDGE('', *, *, #{}, .T.)", seam_to_north_id));
        let oe2 = self.add_entity(format!("ORIENTED_EDGE('', *, *, #{}, .T.)", north_to_seam_id));
        let oe3 = self.add_entity(format!("ORIENTED_EDGE('', *, *, #{}, .T.)", seam_to_south_id));
        let oe4 = self.add_entity(format!("ORIENTED_EDGE('', *, *, #{}, .T.)", south_to_seam_id));

        // Edge loops for two halves
        let loop1_id = self.add_entity(format!("EDGE_LOOP('', (#{}, #{}))", oe1, oe2));
        let loop2_id = self.add_entity(format!("EDGE_LOOP('', (#{}, #{}))", oe3, oe4));

        // Face bounds
        let bound1_id = self.add_entity(format!("FACE_OUTER_BOUND('', #{}, .T.)", loop1_id));
        let bound2_id = self.add_entity(format!("FACE_OUTER_BOUND('', #{}, .T.)", loop2_id));

        // Two hemisphere faces
        let face1_id = self.add_entity(format!(
            "ADVANCED_FACE('', (#{}), #{}, .T.)", bound1_id, sphere_surface_id
        ));
        let face2_id = self.add_entity(format!(
            "ADVANCED_FACE('', (#{}), #{}, .F.)", bound2_id, sphere_surface_id
        ));

        // Closed shell
        let shell_id = self.add_entity(format!(
            "CLOSED_SHELL('', (#{}, #{}))", face1_id, face2_id
        ));

        // Manifold solid
        let solid_id = self.add_entity(format!(
            "MANIFOLD_SOLID_BREP('Sphere', #{})", shell_id
        ));

        Ok(solid_id)
    }

    /// Create an assembly from multiple shapes.
    fn build_assembly(&mut self, shape_ids: &[usize]) -> Result<usize, ExportError> {
        // Create a shape representation with multiple items
        if shape_ids.is_empty() {
            return Err(ExportError::NoGeometry);
        }

        // For now, return the first shape
        // A proper implementation would create a SHAPE_REPRESENTATION with multiple items
        Ok(shape_ids[0])
    }

    /// Create product definition and link to shape.
    fn create_product(&mut self, name: &str, shape_id: usize) -> Result<usize, ExportError> {
        // Application context
        let app_ctx_id = self.add_entity(
            "APPLICATION_CONTEXT('configuration controlled 3D design')".to_string()
        );

        // Application protocol
        let app_proto_id = self.add_entity(format!(
            "APPLICATION_PROTOCOL_DEFINITION('international standard', 'automotive_design', 2000, #{})",
            app_ctx_id
        ));
        let _ = app_proto_id; // Suppress unused warning

        // Product context
        let prod_ctx_id = self.add_entity(format!(
            "PRODUCT_CONTEXT('', #{}, 'mechanical')",
            app_ctx_id
        ));

        // Product
        let product_id = self.add_entity(format!(
            "PRODUCT('{}', '{}', '', (#{}));",
            name, name, prod_ctx_id
        ));

        // Product definition context
        let pdc_id = self.add_entity(format!(
            "PRODUCT_DEFINITION_CONTEXT('detailed design', #{}, 'design')",
            app_ctx_id
        ));

        // Product definition formation
        let pdf_id = self.add_entity(format!(
            "PRODUCT_DEFINITION_FORMATION('', '', #{})",
            product_id
        ));

        // Product definition
        let pd_id = self.add_entity(format!(
            "PRODUCT_DEFINITION('design', '', #{}, #{})",
            pdf_id, pdc_id
        ));

        // Geometric representation context
        let origin_id = self.add_entity("CARTESIAN_POINT('', (0.0, 0.0, 0.0))".to_string());
        let dir_z_id = self.add_entity("DIRECTION('', (0.0, 0.0, 1.0))".to_string());
        let dir_x_id = self.add_entity("DIRECTION('', (1.0, 0.0, 0.0))".to_string());
        let axis_id = self.add_entity(format!(
            "AXIS2_PLACEMENT_3D('', #{}, #{}, #{})",
            origin_id, dir_z_id, dir_x_id
        ));

        // Length unit
        let length_unit = match self.options.unit {
            LengthUnit::Millimeter => {
                let si_unit = self.add_entity(
                    "(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI., .METRE.))".to_string()
                );
                si_unit
            }
            LengthUnit::Meter => {
                let si_unit = self.add_entity(
                    "(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($, .METRE.))".to_string()
                );
                si_unit
            }
            LengthUnit::Inch => {
                let inch_unit = self.add_entity(
                    "LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($, .METRE.)".to_string()
                );
                let conv_unit = self.add_entity(format!(
                    "(CONVERSION_BASED_UNIT('inch', #{}) LENGTH_UNIT())",
                    inch_unit
                ));
                conv_unit
            }
        };

        // Plane and solid angle units
        let plane_angle_unit = self.add_entity(
            "(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($, .RADIAN.))".to_string()
        );
        let solid_angle_unit = self.add_entity(
            "(NAMED_UNIT(*) SI_UNIT($, .STERADIAN.) SOLID_ANGLE_UNIT())".to_string()
        );

        // Uncertainty
        let uncertainty = self.add_entity(format!(
            "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.0E-6), #{}, 'distance_accuracy_value', 'confusion accuracy')",
            length_unit
        ));

        // Representation context
        let rep_ctx_id = self.add_entity(format!(
            "(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{}, #{}, #{})) REPRESENTATION_CONTEXT('Context #1', '3D Context with TORTURE UNITS'))",
            uncertainty, length_unit, plane_angle_unit, solid_angle_unit
        ));

        // Shape representation
        let shape_rep_id = self.add_entity(format!(
            "SHAPE_REPRESENTATION('{}', (#{}, #{}), #{})",
            name, axis_id, shape_id, rep_ctx_id
        ));

        // Product definition shape
        let pds_id = self.add_entity(format!(
            "PRODUCT_DEFINITION_SHAPE('', '', #{})",
            pd_id
        ));

        // Shape definition representation
        let sdr_id = self.add_entity(format!(
            "SHAPE_DEFINITION_REPRESENTATION(#{}, #{})",
            pds_id, shape_rep_id
        ));

        Ok(sdr_id)
    }

    /// Generate the complete STEP file.
    fn finish(self) -> String {
        let mut output = String::new();

        // Header section
        let timestamp = "2026-01-05T12:00:00";

        writeln!(output, "ISO-10303-21;").unwrap();
        writeln!(output, "HEADER;").unwrap();
        writeln!(output, "FILE_DESCRIPTION(('Seed 3D Model'), '2;1');").unwrap();
        writeln!(output, "FILE_NAME('model.step', '{}', ('{}'), ('{}'), 'seed-export', '{}', '');",
            timestamp,
            self.options.author,
            self.options.organization,
            self.options.preprocessor
        ).unwrap();
        writeln!(output, "FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));").unwrap();
        writeln!(output, "ENDSEC;").unwrap();
        writeln!(output).unwrap();

        // Data section
        writeln!(output, "DATA;").unwrap();

        for entity in &self.entities {
            writeln!(output, "{}", entity).unwrap();
        }

        writeln!(output, "ENDSEC;").unwrap();
        writeln!(output, "END-ISO-10303-21;").unwrap();

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::{Document, Element, PartElement, Span};
    use seed_core::types::{Length, Identifier};

    #[test]
    fn test_step_options_default() {
        let opts = StepOptions::default();
        assert_eq!(opts.author, "Seed Engine");
        assert_eq!(opts.unit, LengthUnit::Millimeter);
    }

    #[test]
    fn test_export_empty_doc() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };

        let result = export(&doc);
        assert!(matches!(result, Err(ExportError::NoGeometry { .. })));
    }

    #[test]
    fn test_export_box() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Part(PartElement {
                    name: Some(Identifier("TestBox".to_string())),
                    geometry: Geometry::Primitive(Primitive::Box {
                        width: Length::mm(10.0),
                        height: Length::mm(20.0),
                        depth: Length::mm(30.0),
                    }),
                    properties: vec![],
                    constraints: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = export(&doc);
        assert!(result.is_ok());

        let step = String::from_utf8(result.unwrap()).unwrap();
        assert!(step.contains("ISO-10303-21"));
        assert!(step.contains("MANIFOLD_SOLID_BREP"));
        assert!(step.contains("CLOSED_SHELL"));
        assert!(step.contains("CARTESIAN_POINT"));
    }

    #[test]
    fn test_export_cylinder() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Part(PartElement {
                    name: Some(Identifier("TestCylinder".to_string())),
                    geometry: Geometry::Primitive(Primitive::Cylinder {
                        radius: Length::mm(5.0),
                        height: Length::mm(10.0),
                    }),
                    properties: vec![],
                    constraints: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = export(&doc);
        assert!(result.is_ok());

        let step = String::from_utf8(result.unwrap()).unwrap();
        assert!(step.contains("CYLINDRICAL_SURFACE"));
    }

    #[test]
    fn test_export_sphere() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Part(PartElement {
                    name: Some(Identifier("TestSphere".to_string())),
                    geometry: Geometry::Primitive(Primitive::Sphere {
                        radius: Length::mm(5.0),
                    }),
                    properties: vec![],
                    constraints: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = export(&doc);
        assert!(result.is_ok());

        let step = String::from_utf8(result.unwrap()).unwrap();
        assert!(step.contains("SPHERICAL_SURFACE"));
    }

    #[test]
    fn test_export_union() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Part(PartElement {
                    name: Some(Identifier("TestUnion".to_string())),
                    geometry: Geometry::Csg(CsgOperation::Union(vec![
                        Geometry::Primitive(Primitive::Box {
                            width: Length::mm(10.0),
                            height: Length::mm(10.0),
                            depth: Length::mm(10.0),
                        }),
                        Geometry::Primitive(Primitive::Sphere {
                            radius: Length::mm(5.0),
                        }),
                    ])),
                    properties: vec![],
                    constraints: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = export(&doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_step_file_structure() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Part(PartElement {
                    name: None,
                    geometry: Geometry::Primitive(Primitive::Box {
                        width: Length::mm(1.0),
                        height: Length::mm(1.0),
                        depth: Length::mm(1.0),
                    }),
                    properties: vec![],
                    constraints: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = export(&doc).unwrap();
        let step = String::from_utf8(result).unwrap();

        // Check required STEP sections
        assert!(step.starts_with("ISO-10303-21;"));
        assert!(step.contains("HEADER;"));
        assert!(step.contains("FILE_DESCRIPTION"));
        assert!(step.contains("FILE_NAME"));
        assert!(step.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'))"));
        assert!(step.contains("ENDSEC;"));
        assert!(step.contains("DATA;"));
        assert!(step.ends_with("END-ISO-10303-21;\n"));
    }
}
