//! 3MF (3D Manufacturing Format) export.
//!
//! 3MF is an XML-based format for 3D printing, developed by the 3MF Consortium.
//! It supports colors, materials, multiple objects, and is packaged as a ZIP file.

use seed_core::{Document, ExportError, ast::Element};
use seed_render_3d::{create_shape, Mesh, TessellationOptions};
use std::io::{Write, Cursor};
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

/// 3MF export options.
#[derive(Debug, Clone)]
pub struct ThreeMfOptions {
    /// Model title/name.
    pub title: String,
    /// Model designer/author.
    pub designer: String,
    /// Unit of measurement (millimeter, meter, inch, etc.).
    pub unit: ThreeMfUnit,
    /// Tessellation options for mesh generation.
    pub tessellation: TessellationOptions,
}

impl Default for ThreeMfOptions {
    fn default() -> Self {
        Self {
            title: "Seed Model".to_string(),
            designer: "Seed Engine".to_string(),
            unit: ThreeMfUnit::Millimeter,
            tessellation: TessellationOptions::default(),
        }
    }
}

/// Unit of measurement for 3MF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreeMfUnit {
    Micron,
    Millimeter,
    Centimeter,
    Inch,
    Foot,
    Meter,
}

impl ThreeMfUnit {
    fn as_str(&self) -> &'static str {
        match self {
            ThreeMfUnit::Micron => "micron",
            ThreeMfUnit::Millimeter => "millimeter",
            ThreeMfUnit::Centimeter => "centimeter",
            ThreeMfUnit::Inch => "inch",
            ThreeMfUnit::Foot => "foot",
            ThreeMfUnit::Meter => "meter",
        }
    }
}

/// Export a document to 3MF format.
pub fn export(doc: &Document) -> Result<Vec<u8>, ExportError> {
    export_with_options(doc, &ThreeMfOptions::default())
}

/// Export a document to 3MF with custom options.
pub fn export_with_options(doc: &Document, options: &ThreeMfOptions) -> Result<Vec<u8>, ExportError> {
    let mut builder = ThreeMfBuilder::new(options);

    // Collect all Part elements
    for element in &doc.elements {
        if let Element::Part(part) = element {
            let name = part.name.as_ref()
                .map(|n| n.0.clone())
                .unwrap_or_else(|| format!("Part_{}", builder.object_count() + 1));

            let shape = create_shape(&part.geometry)
                .map_err(|e| ExportError::GeometryError { reason: e.to_string() })?;
            let mut mesh = seed_render_3d::tessellate_with_options(&shape, &options.tessellation);
            mesh.transform(shape.transform());

            builder.add_object(&name, &mesh)?;
        }
    }

    if builder.object_count() == 0 {
        return Err(ExportError::NoGeometry);
    }

    builder.finish()
}

/// Builder for 3MF package.
struct ThreeMfBuilder<'a> {
    options: &'a ThreeMfOptions,
    objects: Vec<ObjectData>,
}

struct ObjectData {
    id: u32,
    name: String,
    mesh: MeshData,
}

struct MeshData {
    vertices: Vec<(f32, f32, f32)>,
    triangles: Vec<(u32, u32, u32)>,
}

impl<'a> ThreeMfBuilder<'a> {
    fn new(options: &'a ThreeMfOptions) -> Self {
        Self {
            options,
            objects: Vec::new(),
        }
    }

    fn object_count(&self) -> usize {
        self.objects.len()
    }

    fn add_object(&mut self, name: &str, mesh: &Mesh) -> Result<(), ExportError> {
        if mesh.triangle_count() == 0 {
            return Ok(()); // Skip empty meshes
        }

        let id = (self.objects.len() + 1) as u32;

        // Convert vertices
        let vertices: Vec<(f32, f32, f32)> = mesh.vertices.iter()
            .map(|v| (v.x, v.y, v.z))
            .collect();

        // Convert triangles
        let triangles: Vec<(u32, u32, u32)> = mesh.indices.chunks(3)
            .filter(|c| c.len() == 3)
            .map(|c| (c[0], c[1], c[2]))
            .collect();

        self.objects.push(ObjectData {
            id,
            name: name.to_string(),
            mesh: MeshData { vertices, triangles },
        });

        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>, ExportError> {
        let buffer = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buffer);

        let file_options: FileOptions<'_, ()> = FileOptions::default()
            .compression_method(CompressionMethod::Deflated);

        // Write [Content_Types].xml
        zip.start_file("[Content_Types].xml", file_options.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        zip.write_all(self.content_types_xml().as_bytes())?;

        // Write _rels/.rels
        zip.add_directory("_rels", file_options.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        zip.start_file("_rels/.rels", file_options.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        zip.write_all(self.rels_xml().as_bytes())?;

        // Write 3D/3dmodel.model
        zip.add_directory("3D", file_options.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        zip.start_file("3D/3dmodel.model", file_options.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        zip.write_all(self.model_xml().as_bytes())?;

        let result = zip.finish()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Ok(result.into_inner())
    }

    fn content_types_xml(&self) -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>
"#.to_string()
    }

    fn rels_xml(&self) -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#.to_string()
    }

    fn model_xml(&self) -> String {
        let mut xml = String::new();

        // XML declaration and model element
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit=""#);
        xml.push_str(self.options.unit.as_str());
        xml.push_str(r#"" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">"#);
        xml.push('\n');

        // Metadata
        xml.push_str("  <metadata name=\"Title\">");
        xml.push_str(&escape_xml(&self.options.title));
        xml.push_str("</metadata>\n");
        xml.push_str("  <metadata name=\"Designer\">");
        xml.push_str(&escape_xml(&self.options.designer));
        xml.push_str("</metadata>\n");
        xml.push_str("  <metadata name=\"Application\">Seed Engine</metadata>\n");

        // Resources section
        xml.push_str("  <resources>\n");

        for obj in &self.objects {
            xml.push_str(&format!("    <object id=\"{}\" type=\"model\" name=\"{}\">\n",
                obj.id, escape_xml(&obj.name)));
            xml.push_str("      <mesh>\n");

            // Vertices
            xml.push_str("        <vertices>\n");
            for (x, y, z) in &obj.mesh.vertices {
                xml.push_str(&format!("          <vertex x=\"{:.6}\" y=\"{:.6}\" z=\"{:.6}\"/>\n",
                    x, y, z));
            }
            xml.push_str("        </vertices>\n");

            // Triangles
            xml.push_str("        <triangles>\n");
            for (v1, v2, v3) in &obj.mesh.triangles {
                xml.push_str(&format!("          <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
                    v1, v2, v3));
            }
            xml.push_str("        </triangles>\n");

            xml.push_str("      </mesh>\n");
            xml.push_str("    </object>\n");
        }

        xml.push_str("  </resources>\n");

        // Build section - reference all objects
        xml.push_str("  <build>\n");
        for obj in &self.objects {
            xml.push_str(&format!("    <item objectid=\"{}\"/>\n", obj.id));
        }
        xml.push_str("  </build>\n");

        xml.push_str("</model>\n");

        xml
    }
}

/// Escape special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::{Document, Element, PartElement, Span};
    use seed_core::{Geometry, Primitive};
    use seed_core::types::{Length, Identifier};

    #[test]
    fn test_threemf_options_default() {
        let opts = ThreeMfOptions::default();
        assert_eq!(opts.title, "Seed Model");
        assert_eq!(opts.unit, ThreeMfUnit::Millimeter);
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
        assert!(matches!(result, Err(ExportError::NoGeometry)));
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

        let data = result.unwrap();
        // Check for ZIP signature (PK)
        assert!(data.len() > 4);
        assert_eq!(&data[0..2], b"PK");
    }

    #[test]
    fn test_export_multiple_objects() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Part(PartElement {
                    name: Some(Identifier("Box1".to_string())),
                    geometry: Geometry::Primitive(Primitive::Box {
                        width: Length::mm(10.0),
                        height: Length::mm(10.0),
                        depth: Length::mm(10.0),
                    }),
                    properties: vec![],
                    constraints: vec![],
                    span: Span::default(),
                }),
                Element::Part(PartElement {
                    name: Some(Identifier("Sphere1".to_string())),
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
    }

    #[test]
    fn test_threemf_unit_strings() {
        assert_eq!(ThreeMfUnit::Micron.as_str(), "micron");
        assert_eq!(ThreeMfUnit::Millimeter.as_str(), "millimeter");
        assert_eq!(ThreeMfUnit::Centimeter.as_str(), "centimeter");
        assert_eq!(ThreeMfUnit::Inch.as_str(), "inch");
        assert_eq!(ThreeMfUnit::Foot.as_str(), "foot");
        assert_eq!(ThreeMfUnit::Meter.as_str(), "meter");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("Hello & World"), "Hello &amp; World");
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_content_types_xml() {
        let options = ThreeMfOptions::default();
        let builder = ThreeMfBuilder::new(&options);
        let xml = builder.content_types_xml();

        assert!(xml.contains("Types xmlns"));
        assert!(xml.contains("Extension=\"rels\""));
        assert!(xml.contains("Extension=\"model\""));
    }

    #[test]
    fn test_rels_xml() {
        let options = ThreeMfOptions::default();
        let builder = ThreeMfBuilder::new(&options);
        let xml = builder.rels_xml();

        assert!(xml.contains("Relationships xmlns"));
        assert!(xml.contains("3D/3dmodel.model"));
    }

    #[test]
    fn test_model_xml_structure() {
        let options = ThreeMfOptions::default();
        let mut builder = ThreeMfBuilder::new(&options);

        // Add a simple mesh
        let mesh = seed_render_3d::Shape::box_shape(1.0, 1.0, 1.0);
        let tessellated = seed_render_3d::tessellate(&mesh, 0.1);
        builder.add_object("TestObject", &tessellated).unwrap();

        let xml = builder.model_xml();

        assert!(xml.contains("<model unit=\"millimeter\""));
        assert!(xml.contains("<metadata name=\"Title\">"));
        assert!(xml.contains("<resources>"));
        assert!(xml.contains("<object id=\"1\""));
        assert!(xml.contains("<mesh>"));
        assert!(xml.contains("<vertices>"));
        assert!(xml.contains("<triangles>"));
        assert!(xml.contains("<build>"));
        assert!(xml.contains("<item objectid=\"1\""));
    }
}
