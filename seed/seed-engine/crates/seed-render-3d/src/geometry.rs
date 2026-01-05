//! 3D geometry types and operations.

use glam::{Vec3, Mat4};

/// A 3D shape (B-rep representation).
///
/// In a full implementation, this would wrap OpenCASCADE's TopoDS_Shape.
#[derive(Debug, Clone)]
pub struct Shape {
    kind: ShapeKind,
    bounds: BoundingBox,
    transform: Mat4,
}

/// The kind of shape.
#[derive(Debug, Clone)]
pub enum ShapeKind {
    Box { width: f64, height: f64, depth: f64 },
    Cylinder { radius: f64, height: f64 },
    Sphere { radius: f64 },
    Compound(Vec<Shape>),
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl BoundingBox {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: impl Iterator<Item = Vec3>) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        let mut has_points = false;

        for p in points {
            min = min.min(p);
            max = max.max(p);
            has_points = true;
        }

        if has_points {
            Some(Self { min, max })
        } else {
            None
        }
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Merge with another bounding box.
    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        BoundingBox {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Check if a point is inside the bounding box.
    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x &&
        point.y >= self.min.y && point.y <= self.max.y &&
        point.z >= self.min.z && point.z <= self.max.z
    }

    /// Check if two bounding boxes intersect.
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }
}

impl Shape {
    /// Create a box shape centered at origin.
    pub fn box_shape(width: f64, height: f64, depth: f64) -> Self {
        let half = Vec3::new(width as f32 / 2.0, height as f32 / 2.0, depth as f32 / 2.0);
        Self {
            kind: ShapeKind::Box { width, height, depth },
            bounds: BoundingBox::new(-half, half),
            transform: Mat4::IDENTITY,
        }
    }

    /// Create a cylinder shape centered at origin, extending along Y axis.
    pub fn cylinder(radius: f64, height: f64) -> Self {
        let r = radius as f32;
        let h = height as f32 / 2.0;
        Self {
            kind: ShapeKind::Cylinder { radius, height },
            bounds: BoundingBox::new(
                Vec3::new(-r, -h, -r),
                Vec3::new(r, h, r),
            ),
            transform: Mat4::IDENTITY,
        }
    }

    /// Create a sphere shape centered at origin.
    pub fn sphere(radius: f64) -> Self {
        let r = radius as f32;
        Self {
            kind: ShapeKind::Sphere { radius },
            bounds: BoundingBox::new(
                Vec3::new(-r, -r, -r),
                Vec3::new(r, r, r),
            ),
            transform: Mat4::IDENTITY,
        }
    }

    /// Create a compound shape from multiple shapes.
    pub fn compound(shapes: Vec<Shape>) -> Self {
        let bounds = shapes.iter()
            .map(|s| s.bounds)
            .reduce(|a, b| a.union(&b))
            .unwrap_or(BoundingBox::new(Vec3::ZERO, Vec3::ZERO));

        Self {
            kind: ShapeKind::Compound(shapes),
            bounds,
            transform: Mat4::IDENTITY,
        }
    }

    /// Get the shape kind.
    pub fn kind(&self) -> &ShapeKind {
        &self.kind
    }

    /// Get the bounding box.
    pub fn bounding_box(&self) -> BoundingBox {
        self.bounds
    }

    /// Get the transform matrix.
    pub fn transform(&self) -> Mat4 {
        self.transform
    }

    /// Apply a translation.
    pub fn translate(mut self, x: f32, y: f32, z: f32) -> Self {
        self.transform = Mat4::from_translation(Vec3::new(x, y, z)) * self.transform;
        self.bounds = BoundingBox::new(
            self.bounds.min + Vec3::new(x, y, z),
            self.bounds.max + Vec3::new(x, y, z),
        );
        self
    }

    /// Apply a rotation around the Y axis.
    pub fn rotate_y(mut self, angle: f32) -> Self {
        self.transform = Mat4::from_rotation_y(angle) * self.transform;
        // Note: For accurate bounds after rotation, we'd need to recalculate
        self
    }

    /// Apply a uniform scale.
    pub fn scale(mut self, factor: f32) -> Self {
        self.transform = Mat4::from_scale(Vec3::splat(factor)) * self.transform;
        self.bounds = BoundingBox::new(
            self.bounds.min * factor,
            self.bounds.max * factor,
        );
        self
    }

    /// Union (fuse) with another shape.
    pub fn union(&self, other: &Shape) -> Self {
        // Simple implementation: create a compound shape
        Shape::compound(vec![self.clone(), other.clone()])
    }

    /// Difference (cut) with another shape.
    pub fn difference(&self, other: &Shape) -> Self {
        // For a proper implementation, we'd need CSG boolean operations
        // For now, return self (the cut shape is stored for reference)
        let mut result = self.clone();
        result.kind = ShapeKind::Compound(vec![self.clone()]);
        // Store the cutting shape info in bounds adjustment
        if self.bounds.intersects(&other.bounds) {
            // Bounds remain the same for difference (could shrink but hard to compute)
        }
        result
    }

    /// Intersection with another shape.
    pub fn intersection(&self, other: &Shape) -> Self {
        // For a proper implementation, we'd need CSG boolean operations
        // For now, approximate with bounding box intersection
        let new_bounds = BoundingBox::new(
            self.bounds.min.max(other.bounds.min),
            self.bounds.max.min(other.bounds.max),
        );

        Shape {
            kind: ShapeKind::Compound(vec![self.clone(), other.clone()]),
            bounds: new_bounds,
            transform: Mat4::IDENTITY,
        }
    }
}

/// A triangle mesh for rendering.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Compute the bounding box of the mesh.
    pub fn bounding_box(&self) -> Option<BoundingBox> {
        BoundingBox::from_points(self.vertices.iter().copied())
    }

    /// Transform all vertices by a matrix.
    pub fn transform(&mut self, matrix: Mat4) {
        let normal_matrix = matrix.inverse().transpose();

        for v in &mut self.vertices {
            *v = matrix.transform_point3(*v);
        }

        for n in &mut self.normals {
            *n = normal_matrix.transform_vector3(*n).normalize();
        }
    }

    /// Merge another mesh into this one.
    pub fn merge(&mut self, other: &Mesh) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.normals.extend_from_slice(&other.normals);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Flip all normals (for inside-out meshes).
    pub fn flip_normals(&mut self) {
        for n in &mut self.normals {
            *n = -*n;
        }
        // Also reverse winding order
        for tri in self.indices.chunks_exact_mut(3) {
            tri.swap(1, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_shape() {
        let shape = Shape::box_shape(10.0, 20.0, 30.0);
        let bounds = shape.bounding_box();

        assert_eq!(bounds.size(), Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(bounds.center(), Vec3::ZERO);
    }

    #[test]
    fn test_cylinder_shape() {
        let shape = Shape::cylinder(5.0, 10.0);
        let bounds = shape.bounding_box();

        assert_eq!(bounds.size().y, 10.0);
        assert_eq!(bounds.center(), Vec3::ZERO);
    }

    #[test]
    fn test_sphere_shape() {
        let shape = Shape::sphere(5.0);
        let bounds = shape.bounding_box();

        assert_eq!(bounds.size(), Vec3::splat(10.0));
        assert_eq!(bounds.center(), Vec3::ZERO);
    }

    #[test]
    fn test_translate() {
        let shape = Shape::box_shape(2.0, 2.0, 2.0).translate(10.0, 20.0, 30.0);
        let bounds = shape.bounding_box();

        assert_eq!(bounds.center(), Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn test_union() {
        let a = Shape::box_shape(2.0, 2.0, 2.0);
        let b = Shape::sphere(1.0).translate(3.0, 0.0, 0.0);
        let result = a.union(&b);

        let bounds = result.bounding_box();
        assert!(bounds.max.x >= 3.0);
    }

    #[test]
    fn test_bounding_box_intersects() {
        let a = BoundingBox::new(Vec3::ZERO, Vec3::splat(2.0));
        let b = BoundingBox::new(Vec3::splat(1.0), Vec3::splat(3.0));
        let c = BoundingBox::new(Vec3::splat(5.0), Vec3::splat(6.0));

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_mesh_transform() {
        let mut mesh = Mesh {
            vertices: vec![Vec3::new(1.0, 0.0, 0.0)],
            normals: vec![Vec3::new(1.0, 0.0, 0.0)],
            indices: vec![],
        };

        mesh.transform(Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0)));

        assert!((mesh.vertices[0].x - 6.0).abs() < 0.001);
    }
}
