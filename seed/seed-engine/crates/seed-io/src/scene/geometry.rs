//! Geometry types for UnifiedScene.

use glam::{Mat4, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

/// A geometry representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Geometry {
    /// Triangle mesh (universal interchange format).
    Mesh(TriangleMesh),
    /// Line mesh for edges/wireframes.
    Lines(LineMesh),
    /// Boundary representation (CAD).
    Brep(BrepGeometry),
    /// Parametric primitive.
    Primitive(PrimitiveGeometry),
    /// NURBS surface (CAD).
    Nurbs(NurbsGeometry),
}

impl Geometry {
    /// Get approximate bounding box.
    pub fn bounds(&self) -> BoundingBox {
        match self {
            Geometry::Mesh(mesh) => mesh.compute_bounds(),
            Geometry::Lines(lines) => lines.compute_bounds(),
            Geometry::Brep(brep) => brep.bounds.clone(),
            Geometry::Primitive(prim) => prim.compute_bounds(),
            Geometry::Nurbs(nurbs) => nurbs.bounds.clone(),
        }
    }
}

/// A triangle mesh.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriangleMesh {
    /// Vertex positions (3 floats per vertex).
    pub positions: Vec<Vec3>,
    /// Vertex normals (3 floats per vertex, optional).
    pub normals: Option<Vec<Vec3>>,
    /// Texture coordinates (2 floats per vertex, optional).
    pub texcoords: Option<Vec<Vec2>>,
    /// Vertex colors (4 floats per vertex, optional).
    pub colors: Option<Vec<Vec4>>,
    /// Triangle indices (3 indices per triangle).
    pub indices: Vec<u32>,
    /// Cached bounding box (for when positions aren't available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_bounds: Option<BoundingBox>,
}

impl TriangleMesh {
    /// Create a new empty mesh.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Get the number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Compute the bounding box.
    pub fn compute_bounds(&self) -> BoundingBox {
        if self.positions.is_empty() {
            return BoundingBox::default();
        }

        let mut min = self.positions[0];
        let mut max = self.positions[0];

        for pos in &self.positions[1..] {
            min = min.min(*pos);
            max = max.max(*pos);
        }

        BoundingBox { min, max }
    }

    /// Compute normals if not present.
    pub fn compute_normals(&mut self) {
        if self.normals.is_some() {
            return;
        }

        let mut normals = vec![Vec3::ZERO; self.positions.len()];

        for i in (0..self.indices.len()).step_by(3) {
            let i0 = self.indices[i] as usize;
            let i1 = self.indices[i + 1] as usize;
            let i2 = self.indices[i + 2] as usize;

            let v0 = self.positions[i0];
            let v1 = self.positions[i1];
            let v2 = self.positions[i2];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(edge2);

            normals[i0] += normal;
            normals[i1] += normal;
            normals[i2] += normal;
        }

        for normal in &mut normals {
            *normal = normal.normalize_or_zero();
        }

        self.normals = Some(normals);
    }
}

/// A line mesh for edges and wireframes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LineMesh {
    /// Vertex positions (3 floats per vertex).
    pub positions: Vec<Vec3>,
    /// Vertex colors (4 floats per vertex, optional).
    pub colors: Option<Vec<Vec4>>,
    /// Line segment indices (2 indices per line segment).
    pub indices: Vec<u32>,
    /// Line width in pixels (for rendering hints).
    pub line_width: f32,
}

impl LineMesh {
    /// Create a new empty line mesh.
    pub fn new() -> Self {
        Self {
            line_width: 1.0,
            ..Default::default()
        }
    }

    /// Create a line mesh with a specific line width.
    pub fn with_width(line_width: f32) -> Self {
        Self {
            line_width,
            ..Default::default()
        }
    }

    /// Get the number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Get the number of line segments.
    pub fn line_count(&self) -> usize {
        self.indices.len() / 2
    }

    /// Add a line segment from two points.
    pub fn add_line(&mut self, start: Vec3, end: Vec3) {
        let start_idx = self.positions.len() as u32;
        self.positions.push(start);
        self.positions.push(end);
        self.indices.push(start_idx);
        self.indices.push(start_idx + 1);
    }

    /// Add a polyline (connected line segments).
    pub fn add_polyline(&mut self, points: &[Vec3]) {
        if points.len() < 2 {
            return;
        }
        let start_idx = self.positions.len() as u32;
        for point in points {
            self.positions.push(*point);
        }
        for i in 0..(points.len() - 1) {
            self.indices.push(start_idx + i as u32);
            self.indices.push(start_idx + i as u32 + 1);
        }
    }

    /// Compute the bounding box.
    pub fn compute_bounds(&self) -> BoundingBox {
        BoundingBox::from_points(&self.positions)
    }
}

/// Boundary representation geometry (CAD).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrepGeometry {
    /// Faces of the B-rep.
    pub faces: Vec<BrepFace>,
    /// Edges of the B-rep.
    pub edges: Vec<BrepEdge>,
    /// Vertices of the B-rep.
    pub vertices: Vec<Vec3>,
    /// Cached bounding box.
    pub bounds: BoundingBox,
}

impl Default for BrepGeometry {
    fn default() -> Self {
        Self::new()
    }
}

impl BrepGeometry {
    /// Create a new empty B-rep.
    pub fn new() -> Self {
        Self {
            faces: Vec::new(),
            edges: Vec::new(),
            vertices: Vec::new(),
            bounds: BoundingBox::default(),
        }
    }
}

/// A face in a B-rep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrepFace {
    /// Surface type and parameters.
    pub surface: Surface,
    /// Outer wire (loop of edge indices).
    pub outer_wire: Vec<usize>,
    /// Inner wires (holes).
    pub inner_wires: Vec<Vec<usize>>,
    /// Whether the face normal is reversed.
    pub reversed: bool,
}

/// A surface definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Surface {
    /// Flat plane.
    Plane {
        origin: Vec3,
        normal: Vec3,
        u_axis: Vec3,
        v_axis: Vec3,
    },
    /// Cylindrical surface.
    Cylinder {
        origin: Vec3,
        axis: Vec3,
        radius: f32,
    },
    /// Conical surface.
    Cone {
        origin: Vec3,
        axis: Vec3,
        radius: f32,
        half_angle: f32,
    },
    /// Spherical surface.
    Sphere { center: Vec3, radius: f32 },
    /// Toroidal surface.
    Torus {
        center: Vec3,
        axis: Vec3,
        major_radius: f32,
        minor_radius: f32,
    },
    /// NURBS surface.
    Nurbs(Box<NurbsSurface>),
}

/// An edge in a B-rep.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrepEdge {
    /// Curve definition.
    pub curve: Curve,
    /// Start vertex index.
    pub start_vertex: usize,
    /// End vertex index.
    pub end_vertex: usize,
}

/// A curve definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Curve {
    /// Straight line.
    Line { start: Vec3, end: Vec3 },
    /// Circular arc.
    Circle {
        center: Vec3,
        axis: Vec3,
        radius: f32,
        start_angle: f32,
        end_angle: f32,
    },
    /// Elliptical arc.
    Ellipse {
        center: Vec3,
        axis: Vec3,
        major_axis: Vec3,
        major_radius: f32,
        minor_radius: f32,
        start_angle: f32,
        end_angle: f32,
    },
    /// NURBS curve.
    Nurbs(Box<NurbsCurve>),
}

/// NURBS surface definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NurbsSurface {
    /// Degree in U direction.
    pub degree_u: u32,
    /// Degree in V direction.
    pub degree_v: u32,
    /// Control points (row-major).
    pub control_points: Vec<Vec4>, // w is weight
    /// Number of control points in U.
    pub num_u: usize,
    /// Number of control points in V.
    pub num_v: usize,
    /// Knot vector in U.
    pub knots_u: Vec<f32>,
    /// Knot vector in V.
    pub knots_v: Vec<f32>,
}

/// NURBS curve definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NurbsCurve {
    /// Curve degree.
    pub degree: u32,
    /// Control points (w is weight).
    pub control_points: Vec<Vec4>,
    /// Knot vector.
    pub knots: Vec<f32>,
}

/// NURBS geometry (collection of surfaces).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NurbsGeometry {
    /// NURBS surfaces.
    pub surfaces: Vec<NurbsSurface>,
    /// NURBS curves.
    pub curves: Vec<NurbsCurve>,
    /// Cached bounding box.
    pub bounds: BoundingBox,
}

/// Parametric primitive geometry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrimitiveGeometry {
    /// Box/cuboid.
    Box {
        half_extents: Vec3,
        transform: Mat4,
    },
    /// Sphere.
    Sphere { radius: f32, transform: Mat4 },
    /// Cylinder.
    Cylinder {
        radius: f32,
        height: f32,
        transform: Mat4,
    },
    /// Cone.
    Cone {
        radius: f32,
        height: f32,
        transform: Mat4,
    },
    /// Torus.
    Torus {
        major_radius: f32,
        minor_radius: f32,
        transform: Mat4,
    },
    /// Capsule.
    Capsule {
        radius: f32,
        height: f32,
        transform: Mat4,
    },
}

impl PrimitiveGeometry {
    /// Compute the bounding box.
    pub fn compute_bounds(&self) -> BoundingBox {
        match self {
            PrimitiveGeometry::Box {
                half_extents,
                transform,
            } => {
                let corners = [
                    Vec3::new(-half_extents.x, -half_extents.y, -half_extents.z),
                    Vec3::new(half_extents.x, -half_extents.y, -half_extents.z),
                    Vec3::new(-half_extents.x, half_extents.y, -half_extents.z),
                    Vec3::new(half_extents.x, half_extents.y, -half_extents.z),
                    Vec3::new(-half_extents.x, -half_extents.y, half_extents.z),
                    Vec3::new(half_extents.x, -half_extents.y, half_extents.z),
                    Vec3::new(-half_extents.x, half_extents.y, half_extents.z),
                    Vec3::new(half_extents.x, half_extents.y, half_extents.z),
                ];

                let transformed: Vec<Vec3> =
                    corners.iter().map(|c| transform.transform_point3(*c)).collect();
                BoundingBox::from_points(&transformed)
            }
            PrimitiveGeometry::Sphere { radius, transform } => {
                let center = transform.transform_point3(Vec3::ZERO);
                let scale = transform.to_scale_rotation_translation().0;
                let max_scale = scale.x.max(scale.y).max(scale.z);
                let r = radius * max_scale;
                BoundingBox {
                    min: center - Vec3::splat(r),
                    max: center + Vec3::splat(r),
                }
            }
            PrimitiveGeometry::Cylinder {
                radius,
                height,
                transform,
            } => {
                let half_h = height / 2.0;
                let corners = [
                    Vec3::new(-*radius, -half_h, -*radius),
                    Vec3::new(*radius, -half_h, -*radius),
                    Vec3::new(-*radius, -half_h, *radius),
                    Vec3::new(*radius, -half_h, *radius),
                    Vec3::new(-*radius, half_h, -*radius),
                    Vec3::new(*radius, half_h, -*radius),
                    Vec3::new(-*radius, half_h, *radius),
                    Vec3::new(*radius, half_h, *radius),
                ];
                let transformed: Vec<Vec3> =
                    corners.iter().map(|c| transform.transform_point3(*c)).collect();
                BoundingBox::from_points(&transformed)
            }
            PrimitiveGeometry::Cone {
                radius,
                height,
                transform,
            } => {
                let corners = [
                    Vec3::new(-*radius, 0.0, -*radius),
                    Vec3::new(*radius, 0.0, -*radius),
                    Vec3::new(-*radius, 0.0, *radius),
                    Vec3::new(*radius, 0.0, *radius),
                    Vec3::new(0.0, *height, 0.0),
                ];
                let transformed: Vec<Vec3> =
                    corners.iter().map(|c| transform.transform_point3(*c)).collect();
                BoundingBox::from_points(&transformed)
            }
            PrimitiveGeometry::Torus {
                major_radius,
                minor_radius,
                transform,
            } => {
                let r = major_radius + minor_radius;
                let corners = [
                    Vec3::new(-r, -*minor_radius, -r),
                    Vec3::new(r, -*minor_radius, -r),
                    Vec3::new(-r, -*minor_radius, r),
                    Vec3::new(r, -*minor_radius, r),
                    Vec3::new(-r, *minor_radius, -r),
                    Vec3::new(r, *minor_radius, -r),
                    Vec3::new(-r, *minor_radius, r),
                    Vec3::new(r, *minor_radius, r),
                ];
                let transformed: Vec<Vec3> =
                    corners.iter().map(|c| transform.transform_point3(*c)).collect();
                BoundingBox::from_points(&transformed)
            }
            PrimitiveGeometry::Capsule {
                radius,
                height,
                transform,
            } => {
                let half_h = height / 2.0 + radius;
                let corners = [
                    Vec3::new(-*radius, -half_h, -*radius),
                    Vec3::new(*radius, -half_h, -*radius),
                    Vec3::new(-*radius, -half_h, *radius),
                    Vec3::new(*radius, -half_h, *radius),
                    Vec3::new(-*radius, half_h, -*radius),
                    Vec3::new(*radius, half_h, -*radius),
                    Vec3::new(-*radius, half_h, *radius),
                    Vec3::new(*radius, half_h, *radius),
                ];
                let transformed: Vec<Vec3> =
                    corners.iter().map(|c| transform.transform_point3(*c)).collect();
                BoundingBox::from_points(&transformed)
            }
        }
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl BoundingBox {
    /// Create from a set of points.
    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self::default();
        }

        let mut min = points[0];
        let mut max = points[0];

        for p in &points[1..] {
            min = min.min(*p);
            max = max.max(*p);
        }

        Self { min, max }
    }

    /// Get the center of the bounding box.
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) / 2.0
    }

    /// Get the size of the bounding box.
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Expand to include another bounding box.
    pub fn expand(&mut self, other: &BoundingBox) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// Expand to include a point.
    pub fn expand_point(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
}
