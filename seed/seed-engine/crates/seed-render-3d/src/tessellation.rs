//! Tessellation of 3D shapes to triangle meshes.

use super::geometry::{Shape, ShapeKind, Mesh};
use glam::Vec3;
use std::f32::consts::PI;

/// Tessellation quality settings.
#[derive(Debug, Clone, Copy)]
pub struct TessellationOptions {
    /// Maximum edge length for tessellation.
    pub max_edge_length: f32,
    /// Number of segments for curved surfaces.
    pub curve_segments: u32,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        Self {
            max_edge_length: 1.0,
            curve_segments: 32,
        }
    }
}

/// Tessellate a shape to a triangle mesh.
///
/// `deflection` controls the quality (smaller = more triangles).
pub fn tessellate(shape: &Shape, deflection: f64) -> Mesh {
    let options = TessellationOptions {
        curve_segments: ((1.0 / deflection) as u32).max(8).min(128),
        ..Default::default()
    };
    tessellate_with_options(shape, &options)
}

/// Tessellate a shape with specific options.
pub fn tessellate_with_options(shape: &Shape, options: &TessellationOptions) -> Mesh {
    match shape.kind() {
        ShapeKind::Box { width, height, depth } => {
            tessellate_box(*width as f32, *height as f32, *depth as f32)
        }
        ShapeKind::Cylinder { radius, height } => {
            tessellate_cylinder(*radius as f32, *height as f32, options.curve_segments)
        }
        ShapeKind::Sphere { radius } => {
            tessellate_sphere(*radius as f32, options.curve_segments)
        }
        ShapeKind::Compound(shapes) => {
            // Merge all child meshes
            let mut result = Mesh::new();
            for child in shapes {
                let child_mesh = tessellate_with_options(child, options);
                merge_mesh(&mut result, &child_mesh);
            }
            result
        }
    }
}

/// Tessellate a box centered at origin.
fn tessellate_box(width: f32, height: f32, depth: f32) -> Mesh {
    let hw = width / 2.0;
    let hh = height / 2.0;
    let hd = depth / 2.0;

    // 8 corners of the box
    let corners = [
        Vec3::new(-hw, -hh, -hd), // 0: left-bottom-back
        Vec3::new( hw, -hh, -hd), // 1: right-bottom-back
        Vec3::new( hw,  hh, -hd), // 2: right-top-back
        Vec3::new(-hw,  hh, -hd), // 3: left-top-back
        Vec3::new(-hw, -hh,  hd), // 4: left-bottom-front
        Vec3::new( hw, -hh,  hd), // 5: right-bottom-front
        Vec3::new( hw,  hh,  hd), // 6: right-top-front
        Vec3::new(-hw,  hh,  hd), // 7: left-top-front
    ];

    // Face normals
    let normals = [
        Vec3::new( 0.0,  0.0, -1.0), // back
        Vec3::new( 0.0,  0.0,  1.0), // front
        Vec3::new(-1.0,  0.0,  0.0), // left
        Vec3::new( 1.0,  0.0,  0.0), // right
        Vec3::new( 0.0, -1.0,  0.0), // bottom
        Vec3::new( 0.0,  1.0,  0.0), // top
    ];

    // Each face has 4 vertices with the same normal
    let faces = [
        ([0, 1, 2, 3], 0), // back
        ([4, 5, 6, 7], 1), // front (reverse winding)
        ([0, 4, 7, 3], 2), // left
        ([1, 5, 6, 2], 3), // right (reverse winding)
        ([0, 1, 5, 4], 4), // bottom
        ([3, 2, 6, 7], 5), // top (reverse winding)
    ];

    let mut vertices = Vec::with_capacity(24);
    let mut vertex_normals = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);

    for (corner_indices, normal_idx) in &faces {
        let base = vertices.len() as u32;
        let normal = normals[*normal_idx];

        for &ci in corner_indices {
            vertices.push(corners[ci]);
            vertex_normals.push(normal);
        }

        // Two triangles per face
        indices.extend_from_slice(&[
            base, base + 2, base + 1,
            base, base + 3, base + 2,
        ]);
    }

    Mesh {
        vertices,
        normals: vertex_normals,
        indices,
    }
}

/// Tessellate a cylinder centered at origin, extending along Y axis.
fn tessellate_cylinder(radius: f32, height: f32, segments: u32) -> Mesh {
    let half_height = height / 2.0;
    let segments = segments.max(8);

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate side vertices
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * PI;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let normal = Vec3::new(angle.cos(), 0.0, angle.sin());

        // Bottom vertex
        vertices.push(Vec3::new(x, -half_height, z));
        normals.push(normal);

        // Top vertex
        vertices.push(Vec3::new(x, half_height, z));
        normals.push(normal);
    }

    // Side triangles
    for i in 0..segments {
        let base = i * 2;
        indices.extend_from_slice(&[
            base, base + 2, base + 1,
            base + 1, base + 2, base + 3,
        ]);
    }

    // Top cap
    let top_center_idx = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, half_height, 0.0));
    normals.push(Vec3::new(0.0, 1.0, 0.0));

    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * PI;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        vertices.push(Vec3::new(x, half_height, z));
        normals.push(Vec3::new(0.0, 1.0, 0.0));
    }

    for i in 0..segments {
        let base = top_center_idx + 1 + i;
        indices.extend_from_slice(&[top_center_idx, base, base + 1]);
    }

    // Bottom cap
    let bottom_center_idx = vertices.len() as u32;
    vertices.push(Vec3::new(0.0, -half_height, 0.0));
    normals.push(Vec3::new(0.0, -1.0, 0.0));

    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * PI;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        vertices.push(Vec3::new(x, -half_height, z));
        normals.push(Vec3::new(0.0, -1.0, 0.0));
    }

    for i in 0..segments {
        let base = bottom_center_idx + 1 + i;
        indices.extend_from_slice(&[bottom_center_idx, base + 1, base]);
    }

    Mesh { vertices, normals, indices }
}

/// Tessellate a sphere centered at origin.
fn tessellate_sphere(radius: f32, segments: u32) -> Mesh {
    let segments = segments.max(8);
    let rings = segments / 2;

    let mut vertices = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices in a grid pattern
    for ring in 0..=rings {
        let phi = (ring as f32 / rings as f32) * PI; // 0 to PI
        let y = phi.cos();
        let ring_radius = phi.sin();

        for seg in 0..=segments {
            let theta = (seg as f32 / segments as f32) * 2.0 * PI;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();

            let normal = Vec3::new(x, y, z).normalize();
            vertices.push(normal * radius);
            normals.push(normal);
        }
    }

    // Generate indices
    let verts_per_ring = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let current = ring * verts_per_ring + seg;
            let next = current + verts_per_ring;

            indices.extend_from_slice(&[
                current, next, current + 1,
                current + 1, next, next + 1,
            ]);
        }
    }

    Mesh { vertices, normals, indices }
}

/// Merge another mesh into this one.
fn merge_mesh(target: &mut Mesh, source: &Mesh) {
    let base = target.vertices.len() as u32;
    target.vertices.extend_from_slice(&source.vertices);
    target.normals.extend_from_slice(&source.normals);
    target.indices.extend(source.indices.iter().map(|i| i + base));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tessellate_box() {
        let mesh = tessellate_box(2.0, 3.0, 4.0);

        // Box has 6 faces * 4 vertices = 24 vertices
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.normals.len(), 24);

        // Box has 6 faces * 2 triangles * 3 indices = 36 indices
        assert_eq!(mesh.indices.len(), 36);
        assert_eq!(mesh.triangle_count(), 12);
    }

    #[test]
    fn test_tessellate_cylinder() {
        let mesh = tessellate_cylinder(1.0, 2.0, 16);

        // Should have reasonable number of triangles
        assert!(mesh.triangle_count() >= 32); // At least 16 for sides + 16 for caps
        assert_eq!(mesh.vertices.len(), mesh.normals.len());
    }

    #[test]
    fn test_tessellate_sphere() {
        let mesh = tessellate_sphere(1.0, 16);

        // Should have reasonable number of triangles
        assert!(mesh.triangle_count() >= 64);
        assert_eq!(mesh.vertices.len(), mesh.normals.len());

        // All vertices should be on the sphere surface
        for v in &mesh.vertices {
            let dist = v.length();
            assert!((dist - 1.0).abs() < 0.001, "Vertex distance: {}", dist);
        }
    }

    #[test]
    fn test_normals_normalized() {
        let mesh = tessellate_sphere(2.0, 16);

        for n in &mesh.normals {
            let len = n.length();
            assert!((len - 1.0).abs() < 0.001, "Normal length: {}", len);
        }
    }
}
