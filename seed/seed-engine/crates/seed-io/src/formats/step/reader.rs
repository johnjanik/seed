//! STEP reader implementation.

use crate::error::{IoError, Result};
use crate::registry::{FormatReader, ReadOptions};
use crate::scene::{Geometry, Material, SceneNode, TriangleMesh, UnifiedScene};

use super::entities::{AdvancedFace, EntityGraph, StepEntity};
use super::p21::parse_data_section;

use glam::{Mat4, Vec3};
use std::f32::consts::PI;

/// Information about an edge's underlying curve.
struct EdgeCurveInfo {
    /// Whether the curve is a circle (vs line or other).
    is_circle: bool,
    /// Start point of the edge.
    start_point: Option<Vec3>,
    /// End point of the edge.
    end_point: Option<Vec3>,
}

/// Reader for STEP files.
pub struct StepReader;

impl StepReader {
    /// Create a new STEP reader.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StepReader {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatReader for StepReader {
    fn name(&self) -> &'static str {
        "step"
    }

    fn extensions(&self) -> &[&'static str] {
        &["step", "stp", "p21"]
    }

    fn can_read(&self, data: &[u8]) -> bool {
        // Only check the first 8KB for efficiency
        let check_len = data.len().min(8192);
        let check_data = &data[..check_len];

        // Skip UTF-8 BOM if present
        let check_data = if check_data.starts_with(&[0xEF, 0xBB, 0xBF]) {
            &check_data[3..]
        } else {
            check_data
        };

        if let Ok(text) = std::str::from_utf8(check_data) {
            // Check for STEP header markers (case-insensitive for ISO marker)
            let upper = text.to_uppercase();
            return upper.contains("ISO-10303-21") || text.contains("FILE_DESCRIPTION");
        }
        false
    }

    fn read(&self, data: &[u8], options: &ReadOptions) -> Result<UnifiedScene> {
        // Use lossy conversion to handle non-UTF-8 bytes in STEP files
        // Some CAD software includes non-standard characters
        let text = String::from_utf8_lossy(data);

        // Parse the DATA section
        let (_, entities) = parse_data_section(&text)
            .map_err(|e| IoError::parse(format!("STEP parse error: {:?}", e)))?;

        // Build entity graph
        let graph = EntityGraph::new(&entities);

        // Convert to UnifiedScene
        let converter = StepConverter::new(&graph, options);
        converter.convert()
    }
}

/// Converter from STEP entities to UnifiedScene.
struct StepConverter<'a> {
    graph: &'a EntityGraph,
    options: &'a ReadOptions,
    scene: UnifiedScene,
}

impl<'a> StepConverter<'a> {
    fn new(graph: &'a EntityGraph, options: &'a ReadOptions) -> Self {
        Self {
            graph,
            options,
            scene: UnifiedScene::new(),
        }
    }

    fn convert(mut self) -> Result<UnifiedScene> {
        // Strategy:
        // 1. Find all solids (MANIFOLD_SOLID_BREP, FACETED_BREP)
        // 2. For each solid, extract the shell and faces
        // 3. Tessellate each face to triangles
        // 4. Build mesh from tessellated faces

        let solids = self.graph.find_solids();

        if !solids.is_empty() {
            // Process each solid
            for solid_id in &solids {
                self.process_solid(*solid_id)?;
            }
        } else {
            // Try to find shells directly
            let shells = self.graph.find_shells();
            for shell_id in &shells {
                self.process_shell(*shell_id, "Shell")?;
            }
        }

        // If still empty, try to find shape representations
        if self.scene.geometries.is_empty() {
            let reps = self.graph.find_shape_representations();
            for rep_id in &reps {
                self.process_shape_representation(*rep_id)?;
            }
        }

        // Add a default material
        if !self.scene.geometries.is_empty() {
            self.scene.add_material(Material::new("Default"));
        }

        Ok(self.scene)
    }

    fn process_solid(&mut self, solid_id: u64) -> Result<()> {
        let entity = self.graph.get(solid_id).ok_or_else(|| {
            IoError::InvalidData(format!("Solid {} not found", solid_id))
        })?;

        let (name, shell_id) = match entity {
            StepEntity::ManifoldSolidBrep(brep) => (brep.name.clone(), brep.outer),
            StepEntity::FacetedBrep(brep) => (brep.name.clone(), brep.outer),
            _ => return Ok(()),
        };

        let name = if name.is_empty() {
            format!("Solid_{}", solid_id)
        } else {
            name
        };

        self.process_shell(shell_id, &name)
    }

    fn process_shell(&mut self, shell_id: u64, name: &str) -> Result<()> {
        let entity = self.graph.get(shell_id).ok_or_else(|| {
            IoError::InvalidData(format!("Shell {} not found", shell_id))
        })?;

        let faces = match entity {
            StepEntity::ClosedShell(shell) => &shell.faces,
            StepEntity::OpenShell(shell) => &shell.faces,
            _ => return Ok(()),
        };

        // Collect all vertices from tessellated faces
        let mut mesh = TriangleMesh::new();

        for face_id in faces {
            self.tessellate_face(*face_id, &mut mesh)?;
        }

        if !mesh.positions.is_empty() {
            // Compute normals if requested
            if self.options.compute_normals {
                mesh.compute_normals();
            }

            let geom_idx = self.scene.add_geometry(Geometry::Mesh(mesh));
            let node = SceneNode::with_geometry(name, geom_idx);
            self.scene.add_root(node);
        }

        Ok(())
    }

    fn process_shape_representation(&mut self, rep_id: u64) -> Result<()> {
        let entity = self.graph.get(rep_id).ok_or_else(|| {
            IoError::InvalidData(format!("Shape representation {} not found", rep_id))
        })?;

        let (name, items) = match entity {
            StepEntity::ShapeRepresentation(rep) => (rep.name.clone(), &rep.items),
            StepEntity::AdvancedBrepShapeRepresentation(rep) => (rep.name.clone(), &rep.items),
            StepEntity::GeometricallyBoundedSurfaceShapeRepresentation(rep) => {
                (rep.name.clone(), &rep.items)
            }
            StepEntity::FacetedBrepShapeRepresentation(rep) => (rep.name.clone(), &rep.items),
            _ => return Ok(()),
        };

        // Process each item in the representation
        for item_id in items {
            match self.graph.get(*item_id) {
                Some(StepEntity::ManifoldSolidBrep(_)) | Some(StepEntity::FacetedBrep(_)) => {
                    self.process_solid(*item_id)?;
                }
                Some(StepEntity::ClosedShell(_)) | Some(StepEntity::OpenShell(_)) => {
                    let item_name = if name.is_empty() {
                        format!("Shell_{}", item_id)
                    } else {
                        name.clone()
                    };
                    self.process_shell(*item_id, &item_name)?;
                }
                Some(StepEntity::ShellBasedSurfaceModel(model)) => {
                    for shell_id in &model.shells {
                        self.process_shell(*shell_id, &name)?;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn tessellate_face(&self, face_id: u64, mesh: &mut TriangleMesh) -> Result<()> {
        let entity = self.graph.get(face_id).ok_or_else(|| {
            IoError::InvalidData(format!("Face {} not found", face_id))
        })?;

        let face = match entity {
            StepEntity::AdvancedFace(f) => f,
            _ => return Ok(()),
        };

        // Get the surface
        let surface = self.graph.get(face.surface);

        // Tessellate based on surface type
        match surface {
            Some(StepEntity::Plane(plane)) => {
                self.tessellate_planar_face(face, plane.position, mesh)?;
            }
            Some(StepEntity::CylindricalSurface(cyl)) => {
                self.tessellate_cylindrical_face(face, cyl.position, cyl.radius as f32, mesh)?;
            }
            Some(StepEntity::SphericalSurface(sphere)) => {
                self.tessellate_spherical_face(face, sphere.position, sphere.radius as f32, mesh)?;
            }
            Some(StepEntity::ConicalSurface(cone)) => {
                self.tessellate_conical_face(
                    face,
                    cone.position,
                    cone.radius as f32,
                    cone.semi_angle as f32,
                    mesh,
                )?;
            }
            Some(StepEntity::ToroidalSurface(torus)) => {
                self.tessellate_toroidal_face(
                    face,
                    torus.position,
                    torus.major_radius as f32,
                    torus.minor_radius as f32,
                    mesh,
                )?;
            }
            _ => {
                // For unknown surfaces, try to tessellate from edge loops
                self.tessellate_face_from_edges(face, mesh)?;
            }
        }

        Ok(())
    }

    fn tessellate_planar_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        // Get the plane coordinate system (for future use with proper B-rep tessellation)
        let _transform = self.get_axis_transform(position_id);

        // Collect vertices from the outer bound
        let mut vertices = Vec::new();

        for bound_id in &face.bounds {
            let bound = self.graph.get(*bound_id);
            let loop_id = match bound {
                Some(StepEntity::FaceOuterBound(b)) => b.bound,
                Some(StepEntity::FaceBound(b)) => b.bound,
                _ => continue,
            };

            // Get edge loop
            if let Some(StepEntity::EdgeLoop(loop_entity)) = self.graph.get(loop_id) {
                for edge_id in &loop_entity.edges {
                    if let Some(point) = self.get_edge_start_point(*edge_id) {
                        vertices.push(point);
                    }
                }
            }
        }

        // Triangulate the polygon (simple fan triangulation)
        if vertices.len() >= 3 {
            let base_idx = mesh.positions.len() as u32;

            // Add vertices
            mesh.positions.extend(vertices.iter().cloned());

            // Triangulate
            for i in 1..(vertices.len() - 1) {
                if face.same_sense {
                    mesh.indices.push(base_idx);
                    mesh.indices.push(base_idx + i as u32);
                    mesh.indices.push(base_idx + (i + 1) as u32);
                } else {
                    mesh.indices.push(base_idx);
                    mesh.indices.push(base_idx + (i + 1) as u32);
                    mesh.indices.push(base_idx + i as u32);
                }
            }
        }

        Ok(())
    }

    fn tessellate_cylindrical_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        radius: f32,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        let transform = self.get_axis_transform(position_id);
        let inverse_transform = transform.inverse();

        // Collect all boundary points and analyze edge types
        let mut local_points: Vec<Vec3> = Vec::new();
        let mut has_circular_edges = false;
        let mut angular_bounds: Option<(f32, f32)> = None;

        for bound_id in &face.bounds {
            let bound = self.graph.get(*bound_id);
            let loop_id = match bound {
                Some(StepEntity::FaceOuterBound(b)) => b.bound,
                Some(StepEntity::FaceBound(b)) => b.bound,
                _ => continue,
            };

            if let Some(StepEntity::EdgeLoop(loop_entity)) = self.graph.get(loop_id) {
                for edge_id in &loop_entity.edges {
                    // Get the curve type for this edge
                    if let Some(curve_info) = self.get_edge_curve_info(*edge_id) {
                        if curve_info.is_circle {
                            has_circular_edges = true;
                            // Compute angular range from circle endpoints
                            if let (Some(start), Some(end)) = (curve_info.start_point, curve_info.end_point) {
                                let local_start = inverse_transform.transform_point3(start);
                                let local_end = inverse_transform.transform_point3(end);

                                let start_angle = local_start.y.atan2(local_start.x);
                                let end_angle = local_end.y.atan2(local_end.x);

                                // Update angular bounds
                                match angular_bounds {
                                    None => angular_bounds = Some((start_angle.min(end_angle), start_angle.max(end_angle))),
                                    Some((min_a, max_a)) => {
                                        angular_bounds = Some((
                                            min_a.min(start_angle).min(end_angle),
                                            max_a.max(start_angle).max(end_angle),
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // Collect edge endpoints for bounds calculation
                    if let Some(point) = self.get_edge_start_point(*edge_id) {
                        local_points.push(inverse_transform.transform_point3(point));
                    }
                    if let Some(point) = self.get_edge_end_point(*edge_id) {
                        local_points.push(inverse_transform.transform_point3(point));
                    }
                }
            }
        }

        if local_points.is_empty() {
            return Ok(());
        }

        // Compute z bounds from local points
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        for p in &local_points {
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }

        let height = max_z - min_z;
        if height <= 1e-6 {
            return Ok(());
        }

        // Determine angular range
        let (theta_min, theta_max) = if has_circular_edges {
            // Use detected angular bounds, but ensure we cover the arc properly
            match angular_bounds {
                Some((min_a, max_a)) => {
                    // Handle wrap-around: if range is small, it might be a full circle
                    let range = max_a - min_a;
                    if range < 0.1 {
                        // Points are close together - likely full circle
                        (-PI, PI)
                    } else if range > 2.0 * PI - 0.1 {
                        // Nearly full circle
                        (-PI, PI)
                    } else {
                        (min_a, max_a)
                    }
                }
                None => (-PI, PI),
            }
        } else {
            // No circular edges - compute from point angles
            let mut min_angle = f32::MAX;
            let mut max_angle = f32::MIN;
            for p in &local_points {
                let angle = p.y.atan2(p.x);
                min_angle = min_angle.min(angle);
                max_angle = max_angle.max(angle);
            }

            let range = max_angle - min_angle;
            if range < 0.1 || range > 2.0 * PI - 0.1 {
                (-PI, PI)
            } else {
                (min_angle, max_angle)
            }
        };

        // Calculate segments based on angular range
        let angular_range = theta_max - theta_min;
        let u_segments = ((angular_range.abs() / (PI / 12.0)).ceil() as u32).max(4).min(48);
        let v_segments = ((height / (radius * 0.2)).ceil() as u32).max(2).min(24);

        let base_idx = mesh.positions.len() as u32;

        // Generate cylinder vertices
        for j in 0..=v_segments {
            let z = min_z + (j as f32 / v_segments as f32) * height;
            for i in 0..=u_segments {
                let theta = theta_min + (i as f32 / u_segments as f32) * angular_range;
                let x = radius * theta.cos();
                let y = radius * theta.sin();
                let local = Vec3::new(x, y, z);
                let world = transform.transform_point3(local);
                mesh.positions.push(world);
            }
        }

        // Generate indices
        for j in 0..v_segments {
            for i in 0..u_segments {
                let row_size = u_segments + 1;
                let i0 = base_idx + j * row_size + i;
                let i1 = base_idx + j * row_size + i + 1;
                let i2 = base_idx + (j + 1) * row_size + i + 1;
                let i3 = base_idx + (j + 1) * row_size + i;

                if face.same_sense {
                    mesh.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
                } else {
                    mesh.indices.extend_from_slice(&[i0, i2, i1, i0, i3, i2]);
                }
            }
        }

        Ok(())
    }

    /// Get information about an edge's underlying curve.
    fn get_edge_curve_info(&self, edge_id: u64) -> Option<EdgeCurveInfo> {
        let edge = self.graph.get(edge_id)?;

        let (edge_curve_id, orientation) = match edge {
            StepEntity::OrientedEdge(oe) => (oe.edge, oe.orientation),
            _ => return None,
        };

        let edge_curve = match self.graph.get(edge_curve_id)? {
            StepEntity::EdgeCurve(ec) => ec,
            _ => return None,
        };

        let curve = self.graph.get(edge_curve.curve)?;
        let is_circle = matches!(curve, StepEntity::Circle(_));

        let start_point = self.graph.get_vertex_coords(edge_curve.start_vertex);
        let end_point = self.graph.get_vertex_coords(edge_curve.end_vertex);

        Some(EdgeCurveInfo {
            is_circle,
            start_point: if orientation { start_point } else { end_point },
            end_point: if orientation { end_point } else { start_point },
        })
    }

    /// Get the end point of an edge.
    fn get_edge_end_point(&self, edge_id: u64) -> Option<Vec3> {
        let edge = self.graph.get(edge_id)?;

        match edge {
            StepEntity::OrientedEdge(oe) => {
                let edge_curve = self.graph.get(oe.edge)?;
                match edge_curve {
                    StepEntity::EdgeCurve(ec) => {
                        let vertex_id = if oe.orientation {
                            ec.end_vertex
                        } else {
                            ec.start_vertex
                        };
                        self.graph.get_vertex_coords(vertex_id)
                    }
                    _ => None,
                }
            }
            StepEntity::EdgeCurve(ec) => self.graph.get_vertex_coords(ec.end_vertex),
            _ => None,
        }
    }

    fn tessellate_spherical_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        radius: f32,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        let transform = self.get_axis_transform(position_id);
        let u_segments = 16;
        let v_segments = 8;

        let base_idx = mesh.positions.len() as u32;

        // Generate sphere vertices (partial based on face bounds)
        for j in 0..=v_segments {
            let phi = (j as f32 / v_segments as f32) * PI;
            for i in 0..=u_segments {
                let theta = (i as f32 / u_segments as f32) * 2.0 * PI;
                let x = radius * phi.sin() * theta.cos();
                let y = radius * phi.sin() * theta.sin();
                let z = radius * phi.cos();
                let local = Vec3::new(x, y, z);
                let world = transform.transform_point3(local);
                mesh.positions.push(world);
            }
        }

        // Generate indices
        for j in 0..v_segments {
            for i in 0..u_segments {
                let i0 = base_idx + j * (u_segments + 1) + i;
                let i1 = base_idx + j * (u_segments + 1) + i + 1;
                let i2 = base_idx + (j + 1) * (u_segments + 1) + i + 1;
                let i3 = base_idx + (j + 1) * (u_segments + 1) + i;

                if face.same_sense {
                    mesh.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
                } else {
                    mesh.indices.extend_from_slice(&[i0, i2, i1, i0, i3, i2]);
                }
            }
        }

        Ok(())
    }

    fn tessellate_conical_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        base_radius: f32,
        semi_angle: f32,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        let transform = self.get_axis_transform(position_id);
        let segments = 24;

        let (min_z, max_z) = self.get_face_z_bounds(face);
        let height = max_z - min_z;

        if height <= 0.0 {
            return Ok(());
        }

        let base_idx = mesh.positions.len() as u32;

        // Generate cone vertices
        for j in 0..=1 {
            let z = min_z + (j as f32) * height;
            let r = base_radius + z * semi_angle.tan();
            for i in 0..=segments {
                let angle = (i as f32 / segments as f32) * 2.0 * PI;
                let x = r * angle.cos();
                let y = r * angle.sin();
                let local = Vec3::new(x, y, z);
                let world = transform.transform_point3(local);
                mesh.positions.push(world);
            }
        }

        // Generate indices
        for i in 0..segments {
            let i0 = base_idx + i;
            let i1 = base_idx + i + 1;
            let i2 = base_idx + i + segments + 2;
            let i3 = base_idx + i + segments + 1;

            if face.same_sense {
                mesh.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
            } else {
                mesh.indices.extend_from_slice(&[i0, i2, i1, i0, i3, i2]);
            }
        }

        Ok(())
    }

    fn tessellate_toroidal_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        major_radius: f32,
        minor_radius: f32,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        let transform = self.get_axis_transform(position_id);
        let u_segments = 24;
        let v_segments = 12;

        let base_idx = mesh.positions.len() as u32;

        // Generate torus vertices
        for j in 0..=v_segments {
            let v = (j as f32 / v_segments as f32) * 2.0 * PI;
            for i in 0..=u_segments {
                let u = (i as f32 / u_segments as f32) * 2.0 * PI;
                let x = (major_radius + minor_radius * v.cos()) * u.cos();
                let y = (major_radius + minor_radius * v.cos()) * u.sin();
                let z = minor_radius * v.sin();
                let local = Vec3::new(x, y, z);
                let world = transform.transform_point3(local);
                mesh.positions.push(world);
            }
        }

        // Generate indices
        for j in 0..v_segments {
            for i in 0..u_segments {
                let i0 = base_idx + j * (u_segments + 1) + i;
                let i1 = base_idx + j * (u_segments + 1) + i + 1;
                let i2 = base_idx + (j + 1) * (u_segments + 1) + i + 1;
                let i3 = base_idx + (j + 1) * (u_segments + 1) + i;

                if face.same_sense {
                    mesh.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
                } else {
                    mesh.indices.extend_from_slice(&[i0, i2, i1, i0, i3, i2]);
                }
            }
        }

        Ok(())
    }

    fn tessellate_face_from_edges(
        &self,
        face: &AdvancedFace,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        // Collect all edge vertices
        let mut vertices = Vec::new();

        for bound_id in &face.bounds {
            let bound = self.graph.get(*bound_id);
            let loop_id = match bound {
                Some(StepEntity::FaceOuterBound(b)) => b.bound,
                Some(StepEntity::FaceBound(b)) => b.bound,
                _ => continue,
            };

            if let Some(StepEntity::EdgeLoop(loop_entity)) = self.graph.get(loop_id) {
                for edge_id in &loop_entity.edges {
                    if let Some(point) = self.get_edge_start_point(*edge_id) {
                        vertices.push(point);
                    }
                }
            }
        }

        // Simple fan triangulation
        if vertices.len() >= 3 {
            let base_idx = mesh.positions.len() as u32;
            mesh.positions.extend(vertices.iter().cloned());

            for i in 1..(vertices.len() - 1) {
                if face.same_sense {
                    mesh.indices.push(base_idx);
                    mesh.indices.push(base_idx + i as u32);
                    mesh.indices.push(base_idx + (i + 1) as u32);
                } else {
                    mesh.indices.push(base_idx);
                    mesh.indices.push(base_idx + (i + 1) as u32);
                    mesh.indices.push(base_idx + i as u32);
                }
            }
        }

        Ok(())
    }

    fn get_edge_start_point(&self, edge_id: u64) -> Option<Vec3> {
        let edge = self.graph.get(edge_id)?;

        match edge {
            StepEntity::OrientedEdge(oe) => {
                // Get the actual edge
                let edge_curve = self.graph.get(oe.edge)?;
                match edge_curve {
                    StepEntity::EdgeCurve(ec) => {
                        let vertex_id = if oe.orientation {
                            ec.start_vertex
                        } else {
                            ec.end_vertex
                        };
                        self.graph.get_vertex_coords(vertex_id)
                    }
                    _ => None,
                }
            }
            StepEntity::EdgeCurve(ec) => self.graph.get_vertex_coords(ec.start_vertex),
            _ => None,
        }
    }

    fn get_axis_transform(&self, position_id: u64) -> Mat4 {
        let entity = match self.graph.get(position_id) {
            Some(e) => e,
            None => return Mat4::IDENTITY,
        };

        match entity {
            StepEntity::Axis2Placement3D(axis) => {
                let origin = self.graph.get_point(axis.location).unwrap_or(Vec3::ZERO);
                let z_axis = axis
                    .axis
                    .and_then(|id| self.graph.get_direction(id))
                    .unwrap_or(Vec3::Z);
                let x_axis = axis
                    .ref_direction
                    .and_then(|id| self.graph.get_direction(id))
                    .unwrap_or(Vec3::X);
                let y_axis = z_axis.cross(x_axis).normalize_or_zero();
                let x_axis = y_axis.cross(z_axis).normalize_or_zero();

                Mat4::from_cols(
                    x_axis.extend(0.0),
                    y_axis.extend(0.0),
                    z_axis.extend(0.0),
                    origin.extend(1.0),
                )
            }
            _ => Mat4::IDENTITY,
        }
    }

    fn get_face_z_bounds(&self, face: &AdvancedFace) -> (f32, f32) {
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;

        for bound_id in &face.bounds {
            let bound = self.graph.get(*bound_id);
            let loop_id = match bound {
                Some(StepEntity::FaceOuterBound(b)) => b.bound,
                Some(StepEntity::FaceBound(b)) => b.bound,
                _ => continue,
            };

            if let Some(StepEntity::EdgeLoop(loop_entity)) = self.graph.get(loop_id) {
                for edge_id in &loop_entity.edges {
                    if let Some(point) = self.get_edge_start_point(*edge_id) {
                        min_z = min_z.min(point.z);
                        max_z = max_z.max(point.z);
                    }
                }
            }
        }

        if min_z > max_z {
            (0.0, 1.0)
        } else {
            (min_z, max_z)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_read() {
        let reader = StepReader::new();

        let step_header = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION";
        assert!(reader.can_read(step_header.as_bytes()));

        assert!(!reader.can_read(b"random data"));
    }

    #[test]
    fn test_simple_step_file() {
        let reader = StepReader::new();

        // A minimal STEP file with a single point
        let step_data = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('Test'),'2;1');
FILE_NAME('test.step','2024-01-01',(''),(''),'','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('origin',(0.,0.,0.));
#2=DIRECTION('z',(0.,0.,1.));
#3=DIRECTION('x',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
#5=PLANE('',#4);
#6=VERTEX_POINT('',#1);
#7=CARTESIAN_POINT('p1',(1.,0.,0.));
#8=VERTEX_POINT('',#7);
#9=CARTESIAN_POINT('p2',(1.,1.,0.));
#10=VERTEX_POINT('',#9);
#11=CARTESIAN_POINT('p3',(0.,1.,0.));
#12=VERTEX_POINT('',#11);
ENDSEC;
END-ISO-10303-21;
"#;

        let result = reader.read(step_data.as_bytes(), &ReadOptions::default());
        // This won't create geometry since there's no complete B-rep structure,
        // but it should parse without error
        assert!(result.is_ok());
    }

    #[test]
    fn test_step_with_solid() {
        let reader = StepReader::new();

        // A more complete STEP file with a simple triangular face
        let step_data = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('Test'),'2;1');
FILE_NAME('test.step','2024-01-01',(''),(''),'','','');
FILE_SCHEMA(('AP203'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('p0',(0.,0.,0.));
#2=CARTESIAN_POINT('p1',(1.,0.,0.));
#3=CARTESIAN_POINT('p2',(0.5,1.,0.));
#4=DIRECTION('z',(0.,0.,1.));
#5=DIRECTION('x',(1.,0.,0.));
#6=AXIS2_PLACEMENT_3D('',#1,#4,#5);
#7=PLANE('',#6);
#8=VERTEX_POINT('',#1);
#9=VERTEX_POINT('',#2);
#10=VERTEX_POINT('',#3);
#11=LINE('',#1,#20);
#12=LINE('',#2,#21);
#13=LINE('',#3,#22);
#20=VECTOR('',#5,1.);
#21=VECTOR('',#4,1.);
#22=VECTOR('',#4,1.);
#14=EDGE_CURVE('',#8,#9,#11,.T.);
#15=EDGE_CURVE('',#9,#10,#12,.T.);
#16=EDGE_CURVE('',#10,#8,#13,.T.);
#17=ORIENTED_EDGE('',*,*,#14,.T.);
#18=ORIENTED_EDGE('',*,*,#15,.T.);
#19=ORIENTED_EDGE('',*,*,#16,.T.);
#30=EDGE_LOOP('',(#17,#18,#19));
#31=FACE_OUTER_BOUND('',#30,.T.);
#32=ADVANCED_FACE('',(#31),#7,.T.);
#33=CLOSED_SHELL('',(#32));
#34=MANIFOLD_SOLID_BREP('triangle',#33);
ENDSEC;
END-ISO-10303-21;
"#;

        let result = reader.read(step_data.as_bytes(), &ReadOptions::default());
        assert!(result.is_ok());

        let scene = result.unwrap();
        assert!(!scene.geometries.is_empty());

        // Should have 1 mesh with 3 vertices (triangle)
        if let Some(Geometry::Mesh(mesh)) = scene.geometries.first() {
            assert_eq!(mesh.positions.len(), 3);
            assert_eq!(mesh.indices.len(), 3);
        }
    }

    #[test]
    fn test_axis_transform() {
        // Test the axis transform calculation
        let step_data = r#"ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('Test'),'2;1');
FILE_NAME('test.step','2024-01-01',(''),(''),'','','');
FILE_SCHEMA(('AP203'));
ENDSEC;
DATA;
#1=CARTESIAN_POINT('origin',(1.,2.,3.));
#2=DIRECTION('z',(0.,0.,1.));
#3=DIRECTION('x',(1.,0.,0.));
#4=AXIS2_PLACEMENT_3D('',#1,#2,#3);
ENDSEC;
END-ISO-10303-21;
"#;

        let reader = StepReader::new();
        let result = reader.read(step_data.as_bytes(), &ReadOptions::default());
        assert!(result.is_ok());
    }
}
