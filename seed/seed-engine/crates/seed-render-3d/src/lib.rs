//! 3D geometry and rendering for Seed documents.
//!
//! This crate provides:
//! - 3D shape primitives (box, cylinder, sphere)
//! - CSG operations (union, difference, intersection)
//! - Tessellation to triangle meshes
//! - Materials and lighting
//! - Software 3D renderer with depth buffer
//! - Scene building from documents

mod geometry;
mod tessellation;
mod material;
mod scene;
mod renderer;

pub use geometry::{Shape, ShapeKind, Mesh, BoundingBox};
pub use tessellation::{tessellate, tessellate_with_options, TessellationOptions};
pub use material::{Material, Light};
pub use scene::{Scene3D, SceneObject, Camera, build_scene};
pub use renderer::SoftwareRenderer3D;

use seed_core::{RenderError, Geometry, GeometryImport, Primitive};

/// Create a 3D shape from geometry definition.
pub fn create_shape(geometry: &Geometry) -> Result<Shape, RenderError> {
    match geometry {
        Geometry::Primitive(prim) => create_primitive(prim),
        Geometry::Csg(op) => create_csg(op),
        Geometry::Import(import) => create_import_placeholder(import),
    }
}

/// Create a placeholder box for imported geometry.
fn create_import_placeholder(import: &GeometryImport) -> Result<Shape, RenderError> {
    if let Some(bounds) = &import.bounds {
        // Create a box matching the bounding box dimensions
        let w = bounds.max[0] - bounds.min[0];
        let h = bounds.max[1] - bounds.min[1];
        let d = bounds.max[2] - bounds.min[2];
        Ok(Shape::box_shape(w.max(1.0), h.max(1.0), d.max(1.0)))
    } else {
        // Default 100mm cube
        Ok(Shape::box_shape(100.0, 100.0, 100.0))
    }
}

fn create_primitive(prim: &Primitive) -> Result<Shape, RenderError> {
    match prim {
        Primitive::Box { width, height, depth } => {
            let w = width.to_mm().unwrap_or(10.0);
            let h = height.to_mm().unwrap_or(10.0);
            let d = depth.to_mm().unwrap_or(10.0);
            Ok(Shape::box_shape(w, h, d))
        }
        Primitive::Cylinder { radius, height } => {
            let r = radius.to_mm().unwrap_or(5.0);
            let h = height.to_mm().unwrap_or(10.0);
            Ok(Shape::cylinder(r, h))
        }
        Primitive::Sphere { radius } => {
            let r = radius.to_mm().unwrap_or(5.0);
            Ok(Shape::sphere(r))
        }
    }
}

fn create_csg(op: &seed_core::ast::CsgOperation) -> Result<Shape, RenderError> {
    use seed_core::ast::CsgOperation;

    match op {
        CsgOperation::Union(geometries) => {
            let shapes: Result<Vec<Shape>, RenderError> = geometries.iter()
                .map(|g| create_shape(g))
                .collect();
            Ok(Shape::compound(shapes?))
        }
        CsgOperation::Difference { base, subtract } => {
            let mut result = create_shape(base)?;
            for sub in subtract {
                let sub_shape = create_shape(sub)?;
                result = result.difference(&sub_shape);
            }
            Ok(result)
        }
        CsgOperation::Intersection(geometries) => {
            let mut shapes = geometries.iter();
            let first = shapes.next()
                .ok_or_else(|| RenderError::GpuInitFailed {
                    reason: "Intersection requires at least one geometry".to_string(),
                })?;
            let mut result = create_shape(first)?;
            for g in shapes {
                let shape = create_shape(g)?;
                result = result.intersection(&shape);
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::types::Length;

    #[test]
    fn test_create_box() {
        let prim = Primitive::Box {
            width: Length::mm(10.0),
            height: Length::mm(20.0),
            depth: Length::mm(30.0),
        };
        let shape = create_primitive(&prim).unwrap();
        let bounds = shape.bounding_box();

        assert!((bounds.size().x - 10.0).abs() < 0.001);
        assert!((bounds.size().y - 20.0).abs() < 0.001);
        assert!((bounds.size().z - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_create_cylinder() {
        let prim = Primitive::Cylinder {
            radius: Length::mm(5.0),
            height: Length::mm(10.0),
        };
        let shape = create_primitive(&prim).unwrap();
        let bounds = shape.bounding_box();

        assert!((bounds.size().y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_create_sphere() {
        let prim = Primitive::Sphere {
            radius: Length::mm(5.0),
        };
        let shape = create_primitive(&prim).unwrap();
        let bounds = shape.bounding_box();

        assert!((bounds.size().x - 10.0).abs() < 0.001);
    }
}
