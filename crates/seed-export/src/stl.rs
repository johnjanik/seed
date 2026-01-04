//! STL export for 3D printing.

use seed_core::{Document, ExportError, ast::Element};
use seed_render_3d::{create_shape, Mesh, TessellationOptions};

/// Export a document to binary STL format.
pub fn export(doc: &Document) -> Result<Vec<u8>, ExportError> {
    export_with_options(doc, &TessellationOptions::default())
}

/// Export a document to binary STL with custom tessellation options.
pub fn export_with_options(
    doc: &Document,
    options: &TessellationOptions,
) -> Result<Vec<u8>, ExportError> {
    // Collect all meshes from Part elements
    let mut combined_mesh = Mesh::new();

    for element in &doc.elements {
        if let Element::Part(part) = element {
            let shape = create_shape(&part.geometry)
                .map_err(|e| ExportError::GeometryError { reason: e.to_string() })?;
            let mut mesh = seed_render_3d::tessellate_with_options(&shape, options);
            mesh.transform(shape.transform());
            combined_mesh.merge(&mesh);
        }
    }

    if combined_mesh.triangle_count() == 0 {
        return Err(ExportError::NoGeometry);
    }

    encode_binary_stl(&combined_mesh)
}

/// Export a document to ASCII STL format.
pub fn export_ascii(doc: &Document) -> Result<String, ExportError> {
    export_ascii_with_options(doc, &TessellationOptions::default())
}

/// Export a document to ASCII STL with custom tessellation options.
pub fn export_ascii_with_options(
    doc: &Document,
    options: &TessellationOptions,
) -> Result<String, ExportError> {
    // Collect all meshes from Part elements
    let mut combined_mesh = Mesh::new();

    for element in &doc.elements {
        if let Element::Part(part) = element {
            let shape = create_shape(&part.geometry)
                .map_err(|e| ExportError::GeometryError { reason: e.to_string() })?;
            let mut mesh = seed_render_3d::tessellate_with_options(&shape, options);
            mesh.transform(shape.transform());
            combined_mesh.merge(&mesh);
        }
    }

    if combined_mesh.triangle_count() == 0 {
        return Err(ExportError::NoGeometry);
    }

    encode_ascii_stl(&combined_mesh, "seed_model")
}

/// Encode a mesh as binary STL.
fn encode_binary_stl(mesh: &Mesh) -> Result<Vec<u8>, ExportError> {
    let triangle_count = mesh.triangle_count();
    let mut output = Vec::with_capacity(84 + triangle_count * 50);

    // 80-byte header (padded with spaces)
    let mut header = [0x20u8; 80]; // Fill with spaces
    let header_text = b"Binary STL exported by Seed Engine";
    let copy_len = header_text.len().min(80);
    header[..copy_len].copy_from_slice(&header_text[..copy_len]);
    output.extend_from_slice(&header);

    // Triangle count (u32 little-endian)
    output.extend_from_slice(&(triangle_count as u32).to_le_bytes());

    // Triangles
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }

        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        let v0 = mesh.vertices[i0];
        let v1 = mesh.vertices[i1];
        let v2 = mesh.vertices[i2];

        // Calculate face normal
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let normal = edge1.cross(edge2).normalize_or_zero();

        // Normal (3 x f32)
        output.extend_from_slice(&normal.x.to_le_bytes());
        output.extend_from_slice(&normal.y.to_le_bytes());
        output.extend_from_slice(&normal.z.to_le_bytes());

        // Vertex 1 (3 x f32)
        output.extend_from_slice(&v0.x.to_le_bytes());
        output.extend_from_slice(&v0.y.to_le_bytes());
        output.extend_from_slice(&v0.z.to_le_bytes());

        // Vertex 2 (3 x f32)
        output.extend_from_slice(&v1.x.to_le_bytes());
        output.extend_from_slice(&v1.y.to_le_bytes());
        output.extend_from_slice(&v1.z.to_le_bytes());

        // Vertex 3 (3 x f32)
        output.extend_from_slice(&v2.x.to_le_bytes());
        output.extend_from_slice(&v2.y.to_le_bytes());
        output.extend_from_slice(&v2.z.to_le_bytes());

        // Attribute byte count (u16, typically 0)
        output.extend_from_slice(&0u16.to_le_bytes());
    }

    Ok(output)
}

/// Encode a mesh as ASCII STL.
fn encode_ascii_stl(mesh: &Mesh, name: &str) -> Result<String, ExportError> {
    let mut output = String::new();

    output.push_str(&format!("solid {}\n", name));

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }

        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        let v0 = mesh.vertices[i0];
        let v1 = mesh.vertices[i1];
        let v2 = mesh.vertices[i2];

        // Calculate face normal
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let normal = edge1.cross(edge2).normalize_or_zero();

        output.push_str(&format!(
            "  facet normal {} {} {}\n",
            normal.x, normal.y, normal.z
        ));
        output.push_str("    outer loop\n");
        output.push_str(&format!("      vertex {} {} {}\n", v0.x, v0.y, v0.z));
        output.push_str(&format!("      vertex {} {} {}\n", v1.x, v1.y, v1.z));
        output.push_str(&format!("      vertex {} {} {}\n", v2.x, v2.y, v2.z));
        output.push_str("    endloop\n");
        output.push_str("  endfacet\n");
    }

    output.push_str(&format!("endsolid {}\n", name));

    Ok(output)
}

/// Export a mesh directly to binary STL.
pub fn mesh_to_stl(mesh: &Mesh) -> Result<Vec<u8>, ExportError> {
    if mesh.triangle_count() == 0 {
        return Err(ExportError::NoGeometry);
    }
    encode_binary_stl(mesh)
}

/// Export a mesh directly to ASCII STL.
pub fn mesh_to_stl_ascii(mesh: &Mesh, name: &str) -> Result<String, ExportError> {
    if mesh.triangle_count() == 0 {
        return Err(ExportError::NoGeometry);
    }
    encode_ascii_stl(mesh, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_render_3d::{Shape, tessellate};

    #[test]
    fn test_binary_stl_header() {
        let shape = Shape::box_shape(10.0, 10.0, 10.0);
        let mesh = tessellate(&shape, 0.1);
        let stl = encode_binary_stl(&mesh).unwrap();

        // Header is 80 bytes
        assert!(stl.len() >= 84);

        // Check triangle count at offset 80
        let count = u32::from_le_bytes([stl[80], stl[81], stl[82], stl[83]]);
        assert_eq!(count as usize, mesh.triangle_count());
    }

    #[test]
    fn test_binary_stl_size() {
        let shape = Shape::box_shape(10.0, 10.0, 10.0);
        let mesh = tessellate(&shape, 0.1);
        let stl = encode_binary_stl(&mesh).unwrap();

        // Binary STL: 80 header + 4 count + (50 * triangles)
        let expected_size = 84 + 50 * mesh.triangle_count();
        assert_eq!(stl.len(), expected_size);
    }

    #[test]
    fn test_ascii_stl_format() {
        let shape = Shape::box_shape(10.0, 10.0, 10.0);
        let mesh = tessellate(&shape, 0.1);
        let stl = encode_ascii_stl(&mesh, "test").unwrap();

        assert!(stl.starts_with("solid test\n"));
        assert!(stl.ends_with("endsolid test\n"));
        assert!(stl.contains("facet normal"));
        assert!(stl.contains("outer loop"));
        assert!(stl.contains("vertex"));
    }

    #[test]
    fn test_mesh_to_stl() {
        let shape = Shape::sphere(5.0);
        let mesh = tessellate(&shape, 0.1);
        let stl = mesh_to_stl(&mesh).unwrap();

        assert!(stl.len() > 84);
    }

    #[test]
    fn test_empty_mesh_error() {
        let mesh = Mesh::new();
        let result = mesh_to_stl(&mesh);
        assert!(result.is_err());
    }
}
