//! B-rep to mesh tessellation.
//!
//! Converts B-rep geometry to triangle meshes.

use crate::scene::{BrepGeometry, TriangleMesh};

/// Tessellation options.
#[derive(Debug, Clone)]
pub struct TessellationOptions {
    /// Linear tolerance (max deviation from surface in model units).
    pub linear_tolerance: f32,
    /// Angular tolerance (max angle between face normals in radians).
    pub angular_tolerance: f32,
    /// Maximum edge length.
    pub max_edge_length: f32,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        Self {
            linear_tolerance: 0.01,
            angular_tolerance: 0.5, // ~28 degrees
            max_edge_length: 1.0,
        }
    }
}

/// Tessellate B-rep geometry to a triangle mesh.
///
/// This is a placeholder implementation. Full B-rep tessellation requires:
/// - Surface parameterization
/// - Trim curve handling
/// - Adaptive subdivision
pub fn tessellate_brep(brep: &BrepGeometry, _options: &TessellationOptions) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    // For now, just create vertices from the B-rep vertices
    for vertex in &brep.vertices {
        mesh.positions.push(*vertex);
    }

    // TODO: Implement proper tessellation
    // This would involve:
    // 1. For each face, get the surface
    // 2. Parameterize the surface (get U,V domain)
    // 3. Sample points on the surface
    // 4. Triangulate the samples respecting trim curves
    // 5. Compute normals

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_options() {
        let opts = TessellationOptions::default();
        assert!(opts.linear_tolerance > 0.0);
        assert!(opts.angular_tolerance > 0.0);
    }
}
