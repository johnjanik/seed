//! Primitive mesh generation.
//!
//! Generates triangle meshes from parametric primitives.

use crate::scene::{PrimitiveGeometry, TriangleMesh};
use glam::{Mat4, Vec3};

/// Generate a triangle mesh from a primitive.
pub fn generate_primitive_mesh(primitive: &PrimitiveGeometry, subdivisions: u32) -> TriangleMesh {
    match primitive {
        PrimitiveGeometry::Box {
            half_extents,
            transform,
        } => generate_box(half_extents, transform),
        PrimitiveGeometry::Sphere { radius, transform } => {
            generate_sphere(*radius, transform, subdivisions)
        }
        PrimitiveGeometry::Cylinder {
            radius,
            height,
            transform,
        } => generate_cylinder(*radius, *height, transform, subdivisions),
        PrimitiveGeometry::Cone {
            radius,
            height,
            transform,
        } => generate_cone(*radius, *height, transform, subdivisions),
        PrimitiveGeometry::Torus {
            major_radius,
            minor_radius,
            transform,
        } => generate_torus(*major_radius, *minor_radius, transform, subdivisions),
        PrimitiveGeometry::Capsule {
            radius,
            height,
            transform,
        } => generate_capsule(*radius, *height, transform, subdivisions),
    }
}

/// Generate a box mesh.
fn generate_box(half_extents: &Vec3, transform: &Mat4) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let hx = half_extents.x;
    let hy = half_extents.y;
    let hz = half_extents.z;

    // 8 corners
    let corners = [
        Vec3::new(-hx, -hy, -hz), // 0: back-bottom-left
        Vec3::new(hx, -hy, -hz),  // 1: back-bottom-right
        Vec3::new(hx, hy, -hz),   // 2: back-top-right
        Vec3::new(-hx, hy, -hz),  // 3: back-top-left
        Vec3::new(-hx, -hy, hz),  // 4: front-bottom-left
        Vec3::new(hx, -hy, hz),   // 5: front-bottom-right
        Vec3::new(hx, hy, hz),    // 6: front-top-right
        Vec3::new(-hx, hy, hz),   // 7: front-top-left
    ];

    // Transform corners
    let corners: Vec<Vec3> = corners
        .iter()
        .map(|c| transform.transform_point3(*c))
        .collect();

    // Each face has 4 vertices (for proper normals)
    let faces = [
        // Front (+Z)
        ([4, 5, 6, 7], Vec3::Z),
        // Back (-Z)
        ([1, 0, 3, 2], Vec3::NEG_Z),
        // Right (+X)
        ([5, 1, 2, 6], Vec3::X),
        // Left (-X)
        ([0, 4, 7, 3], Vec3::NEG_X),
        // Top (+Y)
        ([7, 6, 2, 3], Vec3::Y),
        // Bottom (-Y)
        ([0, 1, 5, 4], Vec3::NEG_Y),
    ];

    let mut normals = Vec::new();

    for (indices, normal) in &faces {
        let base = mesh.positions.len() as u32;

        // Add 4 vertices for this face
        for &i in indices {
            mesh.positions.push(corners[i]);
            normals.push(transform.transform_vector3(*normal).normalize());
        }

        // Two triangles
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
        mesh.indices
            .extend_from_slice(&[base, base + 2, base + 3]);
    }

    mesh.normals = Some(normals);
    mesh
}

/// Generate a UV sphere mesh.
fn generate_sphere(radius: f32, transform: &Mat4, subdivisions: u32) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let stacks = subdivisions.max(4);
    let slices = subdivisions.max(4) * 2;

    // Generate vertices
    for i in 0..=stacks {
        let phi = std::f32::consts::PI * i as f32 / stacks as f32;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();

        for j in 0..=slices {
            let theta = 2.0 * std::f32::consts::PI * j as f32 / slices as f32;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();

            let x = sin_phi * cos_theta;
            let y = cos_phi;
            let z = sin_phi * sin_theta;

            let pos = Vec3::new(x * radius, y * radius, z * radius);
            mesh.positions.push(transform.transform_point3(pos));
        }
    }

    // Generate indices
    for i in 0..stacks {
        for j in 0..slices {
            let first = i * (slices + 1) + j;
            let second = first + slices + 1;

            mesh.indices
                .extend_from_slice(&[first, second, first + 1]);
            mesh.indices
                .extend_from_slice(&[second, second + 1, first + 1]);
        }
    }

    mesh.compute_normals();
    mesh
}

/// Generate a cylinder mesh.
fn generate_cylinder(radius: f32, height: f32, transform: &Mat4, subdivisions: u32) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let slices = subdivisions.max(8);
    let half_h = height / 2.0;

    // Generate side vertices
    for i in 0..=slices {
        let theta = 2.0 * std::f32::consts::PI * i as f32 / slices as f32;
        let x = theta.cos() * radius;
        let z = theta.sin() * radius;

        // Bottom vertex
        mesh.positions
            .push(transform.transform_point3(Vec3::new(x, -half_h, z)));
        // Top vertex
        mesh.positions
            .push(transform.transform_point3(Vec3::new(x, half_h, z)));
    }

    // Side indices
    for i in 0..slices {
        let base = i * 2;
        mesh.indices
            .extend_from_slice(&[base, base + 2, base + 1]);
        mesh.indices
            .extend_from_slice(&[base + 1, base + 2, base + 3]);
    }

    // Top and bottom caps
    let bottom_center = mesh.positions.len() as u32;
    mesh.positions
        .push(transform.transform_point3(Vec3::new(0.0, -half_h, 0.0)));
    let top_center = mesh.positions.len() as u32;
    mesh.positions
        .push(transform.transform_point3(Vec3::new(0.0, half_h, 0.0)));

    for i in 0..slices {
        let base = i * 2;
        // Bottom cap (CCW when viewed from below)
        mesh.indices
            .extend_from_slice(&[bottom_center, base + 2, base]);
        // Top cap (CCW when viewed from above)
        mesh.indices
            .extend_from_slice(&[top_center, base + 1, base + 3]);
    }

    mesh.compute_normals();
    mesh
}

/// Generate a cone mesh.
fn generate_cone(radius: f32, height: f32, transform: &Mat4, subdivisions: u32) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let slices = subdivisions.max(8);

    // Apex
    let apex_idx = 0u32;
    mesh.positions
        .push(transform.transform_point3(Vec3::new(0.0, height, 0.0)));

    // Base vertices
    for i in 0..=slices {
        let theta = 2.0 * std::f32::consts::PI * i as f32 / slices as f32;
        let x = theta.cos() * radius;
        let z = theta.sin() * radius;
        mesh.positions
            .push(transform.transform_point3(Vec3::new(x, 0.0, z)));
    }

    // Side triangles
    for i in 0..slices {
        let base = i + 1;
        mesh.indices
            .extend_from_slice(&[apex_idx, base, base + 1]);
    }

    // Base cap
    let center_idx = mesh.positions.len() as u32;
    mesh.positions
        .push(transform.transform_point3(Vec3::new(0.0, 0.0, 0.0)));

    for i in 0..slices {
        let base = i + 1;
        mesh.indices
            .extend_from_slice(&[center_idx, base + 1, base]);
    }

    mesh.compute_normals();
    mesh
}

/// Generate a torus mesh.
fn generate_torus(
    major_radius: f32,
    minor_radius: f32,
    transform: &Mat4,
    subdivisions: u32,
) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let major_segments = subdivisions.max(8);
    let minor_segments = subdivisions.max(8);

    // Generate vertices
    for i in 0..=major_segments {
        let u = 2.0 * std::f32::consts::PI * i as f32 / major_segments as f32;
        let cos_u = u.cos();
        let sin_u = u.sin();

        for j in 0..=minor_segments {
            let v = 2.0 * std::f32::consts::PI * j as f32 / minor_segments as f32;
            let cos_v = v.cos();
            let sin_v = v.sin();

            let x = (major_radius + minor_radius * cos_v) * cos_u;
            let y = minor_radius * sin_v;
            let z = (major_radius + minor_radius * cos_v) * sin_u;

            mesh.positions
                .push(transform.transform_point3(Vec3::new(x, y, z)));
        }
    }

    // Generate indices
    for i in 0..major_segments {
        for j in 0..minor_segments {
            let first = i * (minor_segments + 1) + j;
            let second = first + minor_segments + 1;

            mesh.indices
                .extend_from_slice(&[first, second, first + 1]);
            mesh.indices
                .extend_from_slice(&[second, second + 1, first + 1]);
        }
    }

    mesh.compute_normals();
    mesh
}

/// Generate a capsule mesh (cylinder with hemisphere caps).
fn generate_capsule(
    radius: f32,
    height: f32,
    transform: &Mat4,
    subdivisions: u32,
) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    let slices = subdivisions.max(8);
    let stacks = subdivisions.max(4) / 2;
    let half_h = height / 2.0;

    // Bottom hemisphere
    for i in 0..=stacks {
        let phi = std::f32::consts::PI / 2.0 + std::f32::consts::PI / 2.0 * i as f32 / stacks as f32;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();

        for j in 0..=slices {
            let theta = 2.0 * std::f32::consts::PI * j as f32 / slices as f32;
            let x = sin_phi * theta.cos() * radius;
            let y = cos_phi * radius - half_h;
            let z = sin_phi * theta.sin() * radius;
            mesh.positions
                .push(transform.transform_point3(Vec3::new(x, y, z)));
        }
    }

    // Cylinder body
    for i in 0..=1 {
        let y = if i == 0 { -half_h } else { half_h };
        for j in 0..=slices {
            let theta = 2.0 * std::f32::consts::PI * j as f32 / slices as f32;
            let x = theta.cos() * radius;
            let z = theta.sin() * radius;
            mesh.positions
                .push(transform.transform_point3(Vec3::new(x, y, z)));
        }
    }

    // Top hemisphere
    for i in 0..=stacks {
        let phi = std::f32::consts::PI / 2.0 * i as f32 / stacks as f32;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();

        for j in 0..=slices {
            let theta = 2.0 * std::f32::consts::PI * j as f32 / slices as f32;
            let x = sin_phi * theta.cos() * radius;
            let y = cos_phi * radius + half_h;
            let z = sin_phi * theta.sin() * radius;
            mesh.positions
                .push(transform.transform_point3(Vec3::new(x, y, z)));
        }
    }

    // Generate indices for all sections
    let total_rows = stacks * 2 + 2;
    for i in 0..total_rows {
        for j in 0..slices {
            let first = i * (slices + 1) + j;
            let second = first + slices + 1;

            mesh.indices
                .extend_from_slice(&[first, second, first + 1]);
            mesh.indices
                .extend_from_slice(&[second, second + 1, first + 1]);
        }
    }

    mesh.compute_normals();
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_box() {
        let prim = PrimitiveGeometry::Box {
            half_extents: Vec3::ONE,
            transform: Mat4::IDENTITY,
        };
        let mesh = generate_primitive_mesh(&prim, 1);

        assert_eq!(mesh.positions.len(), 24); // 6 faces * 4 vertices
        assert_eq!(mesh.indices.len(), 36); // 6 faces * 2 triangles * 3 indices
    }

    #[test]
    fn test_generate_sphere() {
        let prim = PrimitiveGeometry::Sphere {
            radius: 1.0,
            transform: Mat4::IDENTITY,
        };
        let mesh = generate_primitive_mesh(&prim, 8);

        assert!(mesh.positions.len() > 0);
        assert!(mesh.indices.len() > 0);
    }
}
