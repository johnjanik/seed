//! STEP reader implementation.

use crate::error::{IoError, Result};
use crate::registry::{FormatReader, ReadOptions};
use crate::scene::{Geometry, Material, SceneNode, TriangleMesh, UnifiedScene};

use super::entities::{AdvancedFace, BSplineSurface, EntityGraph, StepEntity};
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

/// Triangulate a 2D polygon using ear clipping algorithm.
/// Returns list of triangle indices into the input vertex array.
fn triangulate_polygon(vertices: &[[f32; 2]]) -> Vec<[usize; 3]> {
    let n = vertices.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }

    let mut triangles = Vec::new();
    let mut indices: Vec<usize> = (0..n).collect();

    // Determine winding order
    let area = polygon_signed_area(vertices);
    let ccw = area > 0.0;

    let mut iterations = 0;
    let max_iterations = n * n; // Safety limit

    while indices.len() > 3 && iterations < max_iterations {
        iterations += 1;
        let mut ear_found = false;

        for i in 0..indices.len() {
            let prev = if i == 0 { indices.len() - 1 } else { i - 1 };
            let next = if i == indices.len() - 1 { 0 } else { i + 1 };

            let a = indices[prev];
            let b = indices[i];
            let c = indices[next];

            // Check if this is a convex vertex (potential ear)
            if is_convex(vertices[a], vertices[b], vertices[c], ccw) {
                // Check if any other vertex is inside this triangle
                let mut is_ear = true;
                for j in 0..indices.len() {
                    if j == prev || j == i || j == next {
                        continue;
                    }
                    let p = indices[j];
                    if point_in_triangle(vertices[p], vertices[a], vertices[b], vertices[c]) {
                        is_ear = false;
                        break;
                    }
                }

                if is_ear {
                    triangles.push([a, b, c]);
                    indices.remove(i);
                    ear_found = true;
                    break;
                }
            }
        }

        if !ear_found {
            break; // No ear found, polygon may be degenerate
        }
    }

    // Add final triangle
    if indices.len() == 3 {
        triangles.push([indices[0], indices[1], indices[2]]);
    }

    triangles
}

/// Compute signed area of a 2D polygon (positive if CCW).
fn polygon_signed_area(vertices: &[[f32; 2]]) -> f32 {
    let mut area = 0.0;
    let n = vertices.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += vertices[i][0] * vertices[j][1];
        area -= vertices[j][0] * vertices[i][1];
    }
    area / 2.0
}

/// Check if vertex b is convex (forms a left turn from a to c).
fn is_convex(a: [f32; 2], b: [f32; 2], c: [f32; 2], ccw: bool) -> bool {
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if ccw { cross > 0.0 } else { cross < 0.0 }
}

/// Check if point p is inside triangle abc.
fn point_in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let sign = |p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]| -> f32 {
        (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1])
    };

    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);

    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

    !(has_neg && has_pos)
}

/// Check if segment AB intersects segment CD.
fn segments_intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let sign = |p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]| -> f32 {
        (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1])
    };

    let d1 = sign(c, d, a);
    let d2 = sign(c, d, b);
    let d3 = sign(a, b, c);
    let d4 = sign(a, b, d);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }

    false
}

/// Merge a polygon with holes by creating bridge edges.
/// Returns merged 2D and 3D vertex arrays.
fn merge_polygon_with_holes(
    outer_2d: &[[f32; 2]],
    outer_3d: &[Vec3],
    holes_2d: &[Vec<[f32; 2]>],
    holes_3d: &[Vec<Vec3>],
) -> (Vec<[f32; 2]>, Vec<Vec3>) {
    if holes_2d.is_empty() {
        return (outer_2d.to_vec(), outer_3d.to_vec());
    }

    let mut result_2d = outer_2d.to_vec();
    let mut result_3d = outer_3d.to_vec();

    // Process each hole
    for (hole_2d, hole_3d) in holes_2d.iter().zip(holes_3d.iter()) {
        if hole_2d.len() < 3 {
            continue;
        }

        // Find the rightmost vertex in the hole
        let mut rightmost_idx = 0;
        let mut rightmost_x = hole_2d[0][0];
        for (i, v) in hole_2d.iter().enumerate() {
            if v[0] > rightmost_x {
                rightmost_x = v[0];
                rightmost_idx = i;
            }
        }

        let hole_point = hole_2d[rightmost_idx];

        // Find the best connection point on the outer polygon
        // Look for a vertex visible from the hole point
        let mut best_idx = 0;
        let mut best_dist = f32::INFINITY;

        for (i, outer_point) in result_2d.iter().enumerate() {
            let dist = (outer_point[0] - hole_point[0]).powi(2)
                + (outer_point[1] - hole_point[1]).powi(2);

            if dist < best_dist {
                // Check if the connection is valid (doesn't cross edges)
                let mut valid = true;

                // Check against outer polygon edges
                for j in 0..result_2d.len() {
                    let k = (j + 1) % result_2d.len();
                    if j == i || k == i {
                        continue;
                    }
                    if segments_intersect(hole_point, *outer_point, result_2d[j], result_2d[k]) {
                        valid = false;
                        break;
                    }
                }

                // Check against hole edges
                if valid {
                    for j in 0..hole_2d.len() {
                        let k = (j + 1) % hole_2d.len();
                        if j == rightmost_idx || k == rightmost_idx {
                            continue;
                        }
                        if segments_intersect(hole_point, *outer_point, hole_2d[j], hole_2d[k]) {
                            valid = false;
                            break;
                        }
                    }
                }

                if valid {
                    best_dist = dist;
                    best_idx = i;
                }
            }
        }

        // Create merged polygon: outer[0..best_idx] + hole (rotated) + bridge back
        let mut new_2d = Vec::new();
        let mut new_3d = Vec::new();

        // Add outer vertices up to and including connection point
        for i in 0..=best_idx {
            new_2d.push(result_2d[i]);
            new_3d.push(result_3d[i]);
        }

        // Add hole vertices starting from rightmost, going around
        for i in 0..hole_2d.len() {
            let idx = (rightmost_idx + i) % hole_2d.len();
            new_2d.push(hole_2d[idx]);
            new_3d.push(hole_3d[idx]);
        }

        // Add bridge back (duplicate hole start and outer connection point)
        new_2d.push(hole_2d[rightmost_idx]);
        new_3d.push(hole_3d[rightmost_idx]);

        new_2d.push(result_2d[best_idx]);
        new_3d.push(result_3d[best_idx]);

        // Add remaining outer vertices
        for i in (best_idx + 1)..result_2d.len() {
            new_2d.push(result_2d[i]);
            new_3d.push(result_3d[i]);
        }

        result_2d = new_2d;
        result_3d = new_3d;
    }

    (result_2d, result_3d)
}

/// Project a 3D point onto a cylinder's UV space.
/// Returns (theta, z) where theta is angle and z is height.
fn project_to_cylinder_uv(point: Vec3, inverse_transform: glam::Mat4, radius: f32) -> [f32; 2] {
    let local = inverse_transform.transform_point3(point);
    let theta = local.y.atan2(local.x);
    [theta, local.z]
}

/// Project a 3D point onto a sphere's UV space.
/// Returns (theta, phi) where theta is azimuthal and phi is polar angle.
fn project_to_sphere_uv(point: Vec3, inverse_transform: glam::Mat4) -> [f32; 2] {
    let local = inverse_transform.transform_point3(point).normalize();
    let theta = local.y.atan2(local.x);
    let phi = local.z.acos();
    [theta, phi]
}

/// Project a 3D point onto a cone's UV space.
/// Returns (theta, height) normalized parameters.
fn project_to_cone_uv(point: Vec3, inverse_transform: glam::Mat4) -> [f32; 2] {
    let local = inverse_transform.transform_point3(point);
    let theta = local.y.atan2(local.x);
    [theta, local.z]
}

/// Project a 3D point onto a torus's UV space.
/// Returns (major_angle, minor_angle).
fn project_to_torus_uv(point: Vec3, inverse_transform: glam::Mat4, major_radius: f32) -> [f32; 2] {
    let local = inverse_transform.transform_point3(point);
    let major_angle = local.y.atan2(local.x);

    // Project onto the major circle to find the minor circle center
    let dist_xy = (local.x * local.x + local.y * local.y).sqrt();
    let minor_angle = local.z.atan2(dist_xy - major_radius);

    [major_angle, minor_angle]
}

/// Check if a UV point is inside a UV polygon using ray casting.
fn point_in_uv_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;

    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];

        if ((pi[1] > point[1]) != (pj[1] > point[1]))
            && (point[0] < (pj[0] - pi[0]) * (point[1] - pi[1]) / (pj[1] - pi[1]) + pi[0])
        {
            inside = !inside;
        }
        j = i;
    }

    inside
}

/// Normalize an angle to [-π, π] range.
fn normalize_angle(angle: f32) -> f32 {
    let mut a = angle;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

/// Unwrap angles in a polygon to handle ±π discontinuity.
fn unwrap_angles(polygon: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if polygon.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(polygon.len());
    result.push(polygon[0]);

    for i in 1..polygon.len() {
        let prev = result[i - 1];
        let curr = polygon[i];

        let mut theta = curr[0];
        // Unwrap angle relative to previous
        while theta - prev[0] > PI {
            theta -= 2.0 * PI;
        }
        while theta - prev[0] < -PI {
            theta += 2.0 * PI;
        }

        result.push([theta, curr[1]]);
    }

    result
}

/// Expand knots with multiplicities into full knot vector.
fn expand_knots(knots: &[f64], multiplicities: &[u32]) -> Vec<f64> {
    let mut result = Vec::new();
    for (knot, &mult) in knots.iter().zip(multiplicities.iter()) {
        for _ in 0..mult {
            result.push(*knot);
        }
    }
    result
}

/// Evaluate B-spline surface at parameter (u, v) using de Boor's algorithm.
fn evaluate_bspline_surface(
    control_points: &[Vec<Vec3>],
    u_knots: &[f64],
    v_knots: &[f64],
    u_degree: usize,
    v_degree: usize,
    u: f64,
    v: f64,
) -> Vec3 {
    let n_v = control_points.len();

    // First, evaluate B-spline curves in V direction for each U column
    let mut u_points = Vec::with_capacity(control_points[0].len());

    for i in 0..control_points[0].len() {
        // Collect control points for this V curve
        let v_controls: Vec<Vec3> = (0..n_v).map(|j| control_points[j][i]).collect();
        let point = de_boor(&v_controls, v_knots, v_degree, v);
        u_points.push(point);
    }

    // Then evaluate the resulting curve in U direction
    de_boor(&u_points, u_knots, u_degree, u)
}

/// De Boor's algorithm for B-spline curve evaluation.
/// Returns the point on the curve at parameter t.
fn de_boor(control_points: &[Vec3], knots: &[f64], degree: usize, t: f64) -> Vec3 {
    let n = control_points.len();
    if n == 0 {
        return Vec3::ZERO;
    }
    if n == 1 {
        return control_points[0];
    }

    // Find the knot span index k such that knots[k] <= t < knots[k+1]
    let mut k = degree;
    for i in degree..knots.len() - degree - 1 {
        if t >= knots[i] && t < knots[i + 1] {
            k = i;
            break;
        }
    }

    // Handle edge case at end of parameter range
    if t >= knots[knots.len() - degree - 1] {
        k = knots.len() - degree - 2;
    }

    // Copy the affected control points
    let mut d: Vec<Vec3> = Vec::with_capacity(degree + 1);
    for j in 0..=degree {
        let idx = k.saturating_sub(degree) + j;
        if idx < n {
            d.push(control_points[idx]);
        } else {
            d.push(control_points[n - 1]);
        }
    }

    // De Boor recursion
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = k.saturating_sub(degree) + j;
            let denom = knots.get(i + degree + 1 - r).unwrap_or(&1.0)
                - knots.get(i).unwrap_or(&0.0);

            let alpha = if denom.abs() > 1e-10 {
                ((t - knots.get(i).unwrap_or(&0.0)) / denom) as f32
            } else {
                0.0
            };

            d[j] = d[j - 1] * (1.0 - alpha) + d[j] * alpha;
        }
    }

    d[degree]
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
            Some(StepEntity::BSplineSurface(bspline)) => {
                self.tessellate_bspline_face(face, bspline, mesh)?;
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
        let transform = self.get_axis_transform(position_id);
        let inverse_transform = transform.inverse();

        // Collect vertices from the outer bound, sampling curves
        let mut outer_vertices: Vec<Vec3> = Vec::new();
        let mut inner_loops: Vec<Vec<Vec3>> = Vec::new();

        for bound_id in &face.bounds {
            let bound = self.graph.get(*bound_id);
            let (loop_id, is_outer) = match bound {
                Some(StepEntity::FaceOuterBound(b)) => (b.bound, true),
                Some(StepEntity::FaceBound(b)) => (b.bound, false),
                _ => continue,
            };

            let mut loop_vertices = Vec::new();

            if let Some(StepEntity::EdgeLoop(loop_entity)) = self.graph.get(loop_id) {
                for edge_id in &loop_entity.edges {
                    // Only sample curved edges; for lines, just add the start point
                    if self.is_curved_edge(*edge_id) {
                        // Sample curved edge for smoother boundaries
                        let edge_points = self.sample_edge_curve(*edge_id, 8);
                        for point in edge_points {
                            // Avoid duplicate consecutive points
                            if loop_vertices.last().map_or(true, |last: &Vec3| (*last - point).length() > 1e-6) {
                                loop_vertices.push(point);
                            }
                        }
                    } else {
                        // Linear edge - just add start point (end point will be next edge's start)
                        if let Some(point) = self.get_edge_start_point(*edge_id) {
                            if loop_vertices.last().map_or(true, |last: &Vec3| (*last - point).length() > 1e-6) {
                                loop_vertices.push(point);
                            }
                        }
                    }
                }
            }

            if is_outer {
                outer_vertices = loop_vertices;
            } else if !loop_vertices.is_empty() {
                inner_loops.push(loop_vertices);
            }
        }

        if outer_vertices.len() < 3 {
            return Ok(());
        }

        // Project outer boundary to 2D for triangulation (use plane's local XY)
        let outer_2d: Vec<[f32; 2]> = outer_vertices
            .iter()
            .map(|v| {
                let local = inverse_transform.transform_point3(*v);
                [local.x, local.y]
            })
            .collect();

        // Project inner loops (holes) to 2D
        let inner_2d: Vec<Vec<[f32; 2]>> = inner_loops
            .iter()
            .map(|hole| {
                hole.iter()
                    .map(|v| {
                        let local = inverse_transform.transform_point3(*v);
                        [local.x, local.y]
                    })
                    .collect()
            })
            .collect();

        // Merge holes into outer boundary using bridge edges
        let (merged_2d, merged_3d) = if inner_2d.is_empty() {
            (outer_2d, outer_vertices)
        } else {
            merge_polygon_with_holes(&outer_2d, &outer_vertices, &inner_2d, &inner_loops)
        };

        // Triangulate using ear clipping
        let triangles = triangulate_polygon(&merged_2d);

        if triangles.is_empty() {
            // Fallback to fan triangulation if ear clipping fails
            let base_idx = mesh.positions.len() as u32;
            mesh.positions.extend(merged_3d.iter().cloned());

            for i in 1..(merged_3d.len() - 1) {
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
        } else {
            let base_idx = mesh.positions.len() as u32;
            mesh.positions.extend(merged_3d.iter().cloned());

            for tri in triangles {
                if face.same_sense {
                    mesh.indices.push(base_idx + tri[0] as u32);
                    mesh.indices.push(base_idx + tri[1] as u32);
                    mesh.indices.push(base_idx + tri[2] as u32);
                } else {
                    mesh.indices.push(base_idx + tri[0] as u32);
                    mesh.indices.push(base_idx + tri[2] as u32);
                    mesh.indices.push(base_idx + tri[1] as u32);
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

        // Collect UV boundary polygon from edge loops
        let (outer_uv, inner_uvs) = self.collect_cylinder_uv_boundary(face, inverse_transform);

        if outer_uv.len() < 3 {
            return Ok(());
        }

        // Merge holes into outer boundary if needed
        let (merged_uv, _) = if inner_uvs.is_empty() {
            (outer_uv.clone(), Vec::<Vec3>::new())
        } else {
            // Convert UV arrays for hole merging
            let outer_3d: Vec<Vec3> = outer_uv.iter().map(|uv| Vec3::new(uv[0], uv[1], 0.0)).collect();
            let inner_3d: Vec<Vec<Vec3>> = inner_uvs
                .iter()
                .map(|hole| hole.iter().map(|uv| Vec3::new(uv[0], uv[1], 0.0)).collect())
                .collect();
            let inner_2d_ref: Vec<Vec<[f32; 2]>> = inner_uvs.clone();

            let (merged_2d, _merged_3d) = merge_polygon_with_holes(
                &outer_uv,
                &outer_3d,
                &inner_2d_ref,
                &inner_3d,
            );
            (merged_2d, Vec::<Vec3>::new())
        };

        if merged_uv.len() < 3 {
            return Ok(());
        }

        // Triangulate the UV polygon using ear clipping
        let triangles = triangulate_polygon(&merged_uv);

        if triangles.is_empty() {
            // Fallback: compute UV bounding box and use grid tessellation
            let mut min_theta = f32::MAX;
            let mut max_theta = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;

            for uv in &merged_uv {
                min_theta = min_theta.min(uv[0]);
                max_theta = max_theta.max(uv[0]);
                min_z = min_z.min(uv[1]);
                max_z = max_z.max(uv[1]);
            }

            let angular_range = max_theta - min_theta;
            let height = max_z - min_z;

            if angular_range <= 1e-6 || height <= 1e-6 {
                return Ok(());
            }

            let u_segments = ((angular_range.abs() / (PI / 12.0)).ceil() as u32).max(4).min(48);
            let v_segments = ((height / (radius * 0.2)).ceil() as u32).max(2).min(24);

            let base_idx = mesh.positions.len() as u32;

            // Generate cylinder vertices
            for j in 0..=v_segments {
                let z = min_z + (j as f32 / v_segments as f32) * height;
                for i in 0..=u_segments {
                    let theta = min_theta + (i as f32 / u_segments as f32) * angular_range;
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
        } else {
            // For proper curvature, use grid tessellation within UV bounds
            // Compute UV bounding box
            let mut min_theta = f32::MAX;
            let mut max_theta = f32::MIN;
            let mut min_z = f32::MAX;
            let mut max_z = f32::MIN;

            for uv in &merged_uv {
                min_theta = min_theta.min(uv[0]);
                max_theta = max_theta.max(uv[0]);
                min_z = min_z.min(uv[1]);
                max_z = max_z.max(uv[1]);
            }

            let angular_range = max_theta - min_theta;
            let height = max_z - min_z;

            if angular_range <= 1e-6 || height <= 1e-6 {
                return Ok(());
            }

            // Use finer grid for proper curvature
            let u_segments = ((angular_range.abs() / (PI / 16.0)).ceil() as u32).max(4).min(48);
            let v_segments = ((height / (radius * 0.15)).ceil() as u32).max(2).min(32);

            let base_idx = mesh.positions.len() as u32;

            // Generate grid vertices, filtering by UV boundary
            let mut vertex_indices: Vec<Vec<Option<u32>>> = Vec::new();
            let mut current_idx = base_idx;

            for j in 0..=v_segments {
                let z = min_z + (j as f32 / v_segments as f32) * height;
                let mut row = Vec::new();

                for i in 0..=u_segments {
                    let theta = min_theta + (i as f32 / u_segments as f32) * angular_range;
                    let uv_point = [theta, z];

                    // Check if point is inside UV boundary (with margin for edges)
                    let inside = point_in_uv_polygon(uv_point, &merged_uv)
                        || i == 0 || i == u_segments || j == 0 || j == v_segments;

                    if inside {
                        let x = radius * theta.cos();
                        let y = radius * theta.sin();
                        let local = Vec3::new(x, y, z);
                        let world = transform.transform_point3(local);
                        mesh.positions.push(world);
                        row.push(Some(current_idx));
                        current_idx += 1;
                    } else {
                        row.push(None);
                    }
                }
                vertex_indices.push(row);
            }

            // Generate triangles only for quads where all 4 vertices exist
            for j in 0..v_segments as usize {
                for i in 0..u_segments as usize {
                    let i0 = vertex_indices[j][i];
                    let i1 = vertex_indices[j][i + 1];
                    let i2 = vertex_indices[j + 1][i + 1];
                    let i3 = vertex_indices[j + 1][i];

                    if let (Some(v0), Some(v1), Some(v2), Some(v3)) = (i0, i1, i2, i3) {
                        if face.same_sense {
                            mesh.indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
                        } else {
                            mesh.indices.extend_from_slice(&[v0, v2, v1, v0, v3, v2]);
                        }
                    }
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

    /// Collect the outer boundary of a face as a UV polygon for a cylindrical surface.
    /// Returns (outer_uv, inner_uvs) where each is a Vec of [theta, z] coordinates.
    fn collect_cylinder_uv_boundary(
        &self,
        face: &AdvancedFace,
        inverse_transform: glam::Mat4,
    ) -> (Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>) {
        let mut outer_uv: Vec<[f32; 2]> = Vec::new();
        let mut inner_uvs: Vec<Vec<[f32; 2]>> = Vec::new();

        for bound_id in &face.bounds {
            let bound = self.graph.get(*bound_id);
            let (loop_id, is_outer) = match bound {
                Some(StepEntity::FaceOuterBound(b)) => (b.bound, true),
                Some(StepEntity::FaceBound(b)) => (b.bound, false),
                _ => continue,
            };

            let mut loop_uv = Vec::new();

            if let Some(StepEntity::EdgeLoop(loop_entity)) = self.graph.get(loop_id) {
                for edge_id in &loop_entity.edges {
                    // Sample edge points and project to UV
                    let edge_points = if self.is_curved_edge(*edge_id) {
                        self.sample_edge_curve(*edge_id, 16)
                    } else {
                        // For lines, sample a few points
                        self.sample_edge_curve(*edge_id, 4)
                    };

                    for point in edge_points {
                        let uv = project_to_cylinder_uv(point, inverse_transform, 1.0);
                        // Avoid duplicate consecutive points
                        if loop_uv.last().map_or(true, |last: &[f32; 2]| {
                            (last[0] - uv[0]).abs() > 1e-6 || (last[1] - uv[1]).abs() > 1e-6
                        }) {
                            loop_uv.push(uv);
                        }
                    }
                }
            }

            if !loop_uv.is_empty() {
                // Unwrap angles to handle ±π discontinuity
                let unwrapped = unwrap_angles(&loop_uv);

                if is_outer {
                    outer_uv = unwrapped;
                } else {
                    inner_uvs.push(unwrapped);
                }
            }
        }

        (outer_uv, inner_uvs)
    }

    /// Check if an edge is a curved edge (circle, ellipse, spline) vs a straight line.
    fn is_curved_edge(&self, edge_id: u64) -> bool {
        let edge = match self.graph.get(edge_id) {
            Some(StepEntity::OrientedEdge(oe)) => self.graph.get(oe.edge),
            _ => return false,
        };

        let curve_id = match edge {
            Some(StepEntity::EdgeCurve(ec)) => ec.curve,
            _ => return false,
        };

        matches!(
            self.graph.get(curve_id),
            Some(StepEntity::Circle(_)) | Some(StepEntity::Ellipse(_))
        )
    }

    /// Sample points along an edge curve for smoother boundary tessellation.
    /// Returns a sequence of points from start to end of the edge.
    fn sample_edge_curve(&self, edge_id: u64, segments: usize) -> Vec<Vec3> {
        let segments = segments.max(1);

        // Get the oriented edge
        let edge = match self.graph.get(edge_id) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let (edge_curve_id, orientation) = match edge {
            StepEntity::OrientedEdge(oe) => (oe.edge, oe.orientation),
            _ => return Vec::new(),
        };

        // Get the edge curve
        let edge_curve = match self.graph.get(edge_curve_id) {
            Some(StepEntity::EdgeCurve(ec)) => ec,
            _ => return Vec::new(),
        };

        // Get start and end points
        let start_point = self.graph.get_vertex_coords(edge_curve.start_vertex);
        let end_point = self.graph.get_vertex_coords(edge_curve.end_vertex);

        let (start, end) = match (start_point, end_point) {
            (Some(s), Some(e)) => {
                if orientation { (s, e) } else { (e, s) }
            }
            _ => return Vec::new(),
        };

        // Get the underlying curve geometry
        let curve = match self.graph.get(edge_curve.curve) {
            Some(c) => c,
            None => {
                // No curve geometry, just return endpoints
                return vec![start, end];
            }
        };

        match curve {
            StepEntity::Line(_) => {
                // Linear edge - sample along straight line
                let mut points = Vec::with_capacity(segments + 1);
                for i in 0..=segments {
                    let t = i as f32 / segments as f32;
                    points.push(start + (end - start) * t);
                }
                points
            }
            StepEntity::Circle(circle) => {
                // Circular edge - sample along arc
                let transform = self.get_axis_transform(circle.position);
                let inverse = transform.inverse();
                let radius = circle.radius as f32;

                // Convert endpoints to local coordinates
                let local_start = inverse.transform_point3(start);
                let local_end = inverse.transform_point3(end);

                // Compute angles in local XY plane
                let mut start_angle = local_start.y.atan2(local_start.x);
                let mut end_angle = local_end.y.atan2(local_end.x);

                // Handle angle wrapping - assume we take the shorter arc
                // unless the angle difference is very small (full circle edge)
                let angle_diff = end_angle - start_angle;
                if angle_diff.abs() < 1e-6 {
                    // Full circle - use 2π
                    end_angle = start_angle + std::f32::consts::TAU;
                } else if angle_diff > std::f32::consts::PI {
                    start_angle += std::f32::consts::TAU;
                } else if angle_diff < -std::f32::consts::PI {
                    end_angle += std::f32::consts::TAU;
                }

                // Sample points along the arc
                let mut points = Vec::with_capacity(segments + 1);
                let z = local_start.z; // Use start point's Z (should be constant for circle)

                for i in 0..=segments {
                    let t = i as f32 / segments as f32;
                    let angle = start_angle + (end_angle - start_angle) * t;
                    let local_point = Vec3::new(
                        radius * angle.cos(),
                        radius * angle.sin(),
                        z,
                    );
                    points.push(transform.transform_point3(local_point));
                }
                points
            }
            StepEntity::Ellipse(ellipse) => {
                // Elliptical edge - sample along arc
                let transform = self.get_axis_transform(ellipse.position);
                let inverse = transform.inverse();
                let a = ellipse.semi_axis_1 as f32;
                let b = ellipse.semi_axis_2 as f32;

                // Convert endpoints to local coordinates
                let local_start = inverse.transform_point3(start);
                let local_end = inverse.transform_point3(end);

                // Compute parametric angles (not geometric angles)
                let start_angle = (local_start.y / b).atan2(local_start.x / a);
                let mut end_angle = (local_end.y / b).atan2(local_end.x / a);

                // Handle angle wrapping
                let angle_diff = end_angle - start_angle;
                if angle_diff > std::f32::consts::PI {
                    end_angle -= std::f32::consts::TAU;
                } else if angle_diff < -std::f32::consts::PI {
                    end_angle += std::f32::consts::TAU;
                }

                // Sample points along the arc
                let mut points = Vec::with_capacity(segments + 1);
                let z = local_start.z;

                for i in 0..=segments {
                    let t = i as f32 / segments as f32;
                    let angle = start_angle + (end_angle - start_angle) * t;
                    let local_point = Vec3::new(
                        a * angle.cos(),
                        b * angle.sin(),
                        z,
                    );
                    points.push(transform.transform_point3(local_point));
                }
                points
            }
            _ => {
                // Unknown curve type - just return endpoints
                vec![start, end]
            }
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
        let inverse_transform = transform.inverse();

        // Collect boundary points in local coordinates
        let mut local_points: Vec<Vec3> = Vec::new();

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

        // Compute spherical coordinate bounds (theta = azimuth, phi = polar)
        let mut theta_min = f32::MAX;
        let mut theta_max = f32::MIN;
        let mut phi_min = f32::MAX;
        let mut phi_max = f32::MIN;

        for p in &local_points {
            let r = p.length();
            if r > 1e-6 {
                let theta = p.y.atan2(p.x);
                let phi = (p.z / r).acos();

                theta_min = theta_min.min(theta);
                theta_max = theta_max.max(theta);
                phi_min = phi_min.min(phi);
                phi_max = phi_max.max(phi);
            }
        }

        // Handle full sphere case
        let theta_range = theta_max - theta_min;
        let (theta_min, theta_max) = if theta_range < 0.1 || theta_range > 2.0 * PI - 0.1 {
            (-PI, PI)
        } else {
            (theta_min, theta_max)
        };

        let phi_range = phi_max - phi_min;
        let (phi_min, phi_max) = if phi_range < 0.1 || phi_range > PI - 0.1 {
            (0.0, PI)
        } else {
            (phi_min, phi_max)
        };

        // Adaptive segments
        let u_segments = ((theta_max - theta_min).abs() / (PI / 8.0)).ceil() as u32;
        let u_segments = u_segments.max(4).min(32);
        let v_segments = ((phi_max - phi_min).abs() / (PI / 8.0)).ceil() as u32;
        let v_segments = v_segments.max(2).min(16);

        let base_idx = mesh.positions.len() as u32;

        // Generate sphere vertices within bounds
        for j in 0..=v_segments {
            let phi = phi_min + (j as f32 / v_segments as f32) * (phi_max - phi_min);
            for i in 0..=u_segments {
                let theta = theta_min + (i as f32 / u_segments as f32) * (theta_max - theta_min);
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

    fn tessellate_conical_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        base_radius: f32,
        semi_angle: f32,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        let transform = self.get_axis_transform(position_id);
        let inverse_transform = transform.inverse();

        // Collect boundary points in local coordinates
        let mut local_points: Vec<Vec3> = Vec::new();

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

        // Compute z bounds and angular bounds
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        let mut theta_min = f32::MAX;
        let mut theta_max = f32::MIN;

        for p in &local_points {
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
            let theta = p.y.atan2(p.x);
            theta_min = theta_min.min(theta);
            theta_max = theta_max.max(theta);
        }

        let height = max_z - min_z;
        if height <= 1e-6 {
            return Ok(());
        }

        // Handle full cone case
        let theta_range = theta_max - theta_min;
        let (theta_min, theta_max) = if theta_range < 0.1 || theta_range > 2.0 * PI - 0.1 {
            (-PI, PI)
        } else {
            (theta_min, theta_max)
        };

        // Adaptive segments
        let angular_range = theta_max - theta_min;
        let u_segments = ((angular_range.abs() / (PI / 12.0)).ceil() as u32).max(4).min(48);
        let v_segments = ((height / (base_radius.abs().max(0.1) * 0.2)).ceil() as u32).max(2).min(24);

        let base_idx = mesh.positions.len() as u32;

        // Generate cone vertices
        for j in 0..=v_segments {
            let z = min_z + (j as f32 / v_segments as f32) * height;
            let r = (base_radius + z * semi_angle.tan()).abs();
            for i in 0..=u_segments {
                let theta = theta_min + (i as f32 / u_segments as f32) * angular_range;
                let x = r * theta.cos();
                let y = r * theta.sin();
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

    fn tessellate_toroidal_face(
        &self,
        face: &AdvancedFace,
        position_id: u64,
        major_radius: f32,
        minor_radius: f32,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        let transform = self.get_axis_transform(position_id);
        let inverse_transform = transform.inverse();

        // Collect boundary points in local coordinates
        let mut local_points: Vec<Vec3> = Vec::new();

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

        // Compute toroidal coordinate bounds
        // u = major angle (around the torus center), v = minor angle (around the tube)
        let mut u_min = f32::MAX;
        let mut u_max = f32::MIN;
        let mut v_min = f32::MAX;
        let mut v_max = f32::MIN;

        for p in &local_points {
            // u is the angle in the XY plane from the torus center
            let u = p.y.atan2(p.x);
            u_min = u_min.min(u);
            u_max = u_max.max(u);

            // v is the angle around the tube cross-section
            // Project point onto the tube cross-section plane
            let dist_from_axis = (p.x * p.x + p.y * p.y).sqrt();
            let tube_x = dist_from_axis - major_radius;
            let tube_y = p.z;
            let v = tube_y.atan2(tube_x);
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }

        // Handle full torus case
        let u_range = u_max - u_min;
        let (u_min, u_max) = if u_range < 0.1 || u_range > 2.0 * PI - 0.1 {
            (-PI, PI)
        } else {
            (u_min, u_max)
        };

        let v_range = v_max - v_min;
        let (v_min, v_max) = if v_range < 0.1 || v_range > 2.0 * PI - 0.1 {
            (-PI, PI)
        } else {
            (v_min, v_max)
        };

        // Adaptive segments
        let u_segments = (((u_max - u_min).abs() / (PI / 12.0)).ceil() as u32).max(4).min(48);
        let v_segments = (((v_max - v_min).abs() / (PI / 6.0)).ceil() as u32).max(4).min(24);

        let base_idx = mesh.positions.len() as u32;

        // Generate torus vertices within bounds
        for j in 0..=v_segments {
            let v = v_min + (j as f32 / v_segments as f32) * (v_max - v_min);
            for i in 0..=u_segments {
                let u = u_min + (i as f32 / u_segments as f32) * (u_max - u_min);
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

    /// Tessellate a B-spline surface face using de Boor's algorithm.
    fn tessellate_bspline_face(
        &self,
        face: &AdvancedFace,
        bspline: &BSplineSurface,
        mesh: &mut TriangleMesh,
    ) -> Result<()> {
        // Get control points as Vec3
        let mut control_points: Vec<Vec<Vec3>> = Vec::new();
        for row in &bspline.control_points {
            let mut row_points = Vec::new();
            for &point_id in row {
                if let Some(point) = self.graph.get_point(point_id) {
                    row_points.push(point);
                } else {
                    // Missing control point - abort
                    return self.tessellate_face_from_edges(face, mesh);
                }
            }
            control_points.push(row_points);
        }

        if control_points.is_empty() || control_points[0].is_empty() {
            return self.tessellate_face_from_edges(face, mesh);
        }

        // Build full knot vectors from multiplicities
        let u_knots = expand_knots(&bspline.u_knots, &bspline.u_multiplicities);
        let v_knots = expand_knots(&bspline.v_knots, &bspline.v_multiplicities);

        if u_knots.len() < 2 || v_knots.len() < 2 {
            return self.tessellate_face_from_edges(face, mesh);
        }

        let u_degree = bspline.u_degree as usize;
        let v_degree = bspline.v_degree as usize;

        // Determine parameter range
        let u_min = u_knots[u_degree] as f32;
        let u_max = u_knots[u_knots.len() - u_degree - 1] as f32;
        let v_min = v_knots[v_degree] as f32;
        let v_max = v_knots[v_knots.len() - v_degree - 1] as f32;

        // Adaptive tessellation based on control point grid size
        let n_u = control_points[0].len();
        let n_v = control_points.len();
        let u_segments = (n_u * 2).clamp(8, 64) as u32;
        let v_segments = (n_v * 2).clamp(8, 64) as u32;

        let base_idx = mesh.positions.len() as u32;

        // Sample the surface
        for j in 0..=v_segments {
            let v = v_min + (j as f32 / v_segments as f32) * (v_max - v_min);
            for i in 0..=u_segments {
                let u = u_min + (i as f32 / u_segments as f32) * (u_max - u_min);

                // Evaluate B-spline surface at (u, v) using de Boor
                let point = evaluate_bspline_surface(
                    &control_points,
                    &u_knots,
                    &v_knots,
                    u_degree,
                    v_degree,
                    u as f64,
                    v as f64,
                );
                mesh.positions.push(point);
            }
        }

        // Generate triangle indices
        let row_size = u_segments + 1;
        for j in 0..v_segments {
            for i in 0..u_segments {
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
