//! 2D rendering for Seed documents.
//!
//! This crate provides GPU-accelerated 2D rendering using wgpu,
//! as well as a software rasterizer for headless rendering.

mod shapes;
mod pipeline;
pub mod primitives;
pub mod scene;
pub mod text;

pub use primitives::*;
pub use scene::build_scene;
pub use shapes::{Mesh, Tessellator, Vertex};
pub use text::{TextRenderer, blend_text_onto_buffer};

use seed_core::{Document, RenderError};
use seed_layout::LayoutTree;

/// 2D renderer using wgpu.
#[cfg(feature = "gpu")]
pub struct Renderer2D {
    // GPU resources will go here
}

#[cfg(feature = "gpu")]
impl Renderer2D {
    /// Create a new renderer.
    pub fn new() -> Result<Self, RenderError> {
        Ok(Self {})
    }

    /// Render a document with computed layout.
    pub fn render(&mut self, _doc: &Document, _layout: &LayoutTree) -> Result<(), RenderError> {
        // TODO: Implement GPU rendering
        Ok(())
    }
}

/// Software rasterizer for headless rendering.
pub struct SoftwareRenderer {
    width: u32,
    height: u32,
    buffer: Vec<u8>,
    tessellator: Tessellator,
    text_renderer: TextRenderer,
}

impl SoftwareRenderer {
    /// Create a new software renderer with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            buffer: vec![255; (width * height * 4) as usize], // RGBA, white background
            tessellator: Tessellator::new(),
            text_renderer: TextRenderer::new(),
        }
    }

    /// Clear the buffer to a solid color.
    pub fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for chunk in self.buffer.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = a;
        }
    }

    /// Render a document with computed layout.
    pub fn render(&mut self, doc: &Document, layout: &LayoutTree) -> Result<&[u8], RenderError> {
        // Clear to white
        self.clear(255, 255, 255, 255);

        // Build the scene
        let scene = build_scene(doc, layout);

        // Tessellate and rasterize each command
        let mut mesh = Mesh::new();

        for command in &scene.commands {
            mesh.clear();

            match command {
                RenderCommand::Rect(rect) => {
                    self.tessellator.tessellate_rect(rect, &mut mesh);
                    self.rasterize_mesh(&mesh);
                }
                RenderCommand::RoundedRect(rect) => {
                    self.tessellator.tessellate_rounded_rect(rect, &mut mesh);
                    self.rasterize_mesh(&mesh);
                }
                RenderCommand::Ellipse(ellipse) => {
                    self.tessellator.tessellate_ellipse(ellipse, &mut mesh);
                    self.rasterize_mesh(&mesh);
                }
                RenderCommand::Path(path) => {
                    self.tessellator.tessellate_path(path, &mut mesh);
                    self.rasterize_mesh(&mesh);
                }
                RenderCommand::Text(text) => {
                    self.render_text(text);
                }
                RenderCommand::Shadow(shadow) => {
                    self.render_shadow(shadow);
                }
                RenderCommand::PushClip(_) | RenderCommand::PopClip => {
                    // Clipping requires more complex state management
                    // For now, skip clipping in software renderer
                }
                RenderCommand::SetOpacity(_) => {
                    // Opacity requires blending state
                    // For now, skip opacity in software renderer
                }
            }
        }

        Ok(&self.buffer)
    }

    /// Rasterize a tessellated mesh to the buffer.
    fn rasterize_mesh(&mut self, mesh: &Mesh) {
        if mesh.indices.is_empty() {
            return;
        }

        // Process triangles
        for triangle in mesh.indices.chunks(3) {
            if triangle.len() < 3 {
                continue;
            }

            let v0 = &mesh.vertices[triangle[0] as usize];
            let v1 = &mesh.vertices[triangle[1] as usize];
            let v2 = &mesh.vertices[triangle[2] as usize];

            self.rasterize_triangle(v0, v1, v2);
        }
    }

    /// Rasterize a single triangle using edge functions.
    fn rasterize_triangle(&mut self, v0: &Vertex, v1: &Vertex, v2: &Vertex) {
        // Get bounding box
        let min_x = v0.position[0].min(v1.position[0]).min(v2.position[0]).max(0.0) as i32;
        let max_x = v0.position[0].max(v1.position[0]).max(v2.position[0]).min(self.width as f32 - 1.0) as i32;
        let min_y = v0.position[1].min(v1.position[1]).min(v2.position[1]).max(0.0) as i32;
        let max_y = v0.position[1].max(v1.position[1]).max(v2.position[1]).min(self.height as f32 - 1.0) as i32;

        // Compute edge function denominator for barycentric coordinates
        let area = edge_function(v0.position, v1.position, v2.position);
        if area.abs() < 0.0001 {
            return; // Degenerate triangle
        }

        let inv_area = 1.0 / area;

        // Scan through bounding box
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];

                // Compute barycentric coordinates
                let w0 = edge_function(v1.position, v2.position, p);
                let w1 = edge_function(v2.position, v0.position, p);
                let w2 = edge_function(v0.position, v1.position, p);

                // Check if inside triangle (with consistent winding)
                if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                    // Normalize barycentric coordinates
                    let b0 = w0 * inv_area;
                    let b1 = w1 * inv_area;
                    let b2 = w2 * inv_area;

                    // Interpolate color
                    let r = (v0.color[0] * b0 + v1.color[0] * b1 + v2.color[0] * b2).clamp(0.0, 1.0);
                    let g = (v0.color[1] * b0 + v1.color[1] * b1 + v2.color[1] * b2).clamp(0.0, 1.0);
                    let b = (v0.color[2] * b0 + v1.color[2] * b1 + v2.color[2] * b2).clamp(0.0, 1.0);
                    let a = (v0.color[3] * b0 + v1.color[3] * b1 + v2.color[3] * b2).clamp(0.0, 1.0);

                    // Alpha blend with existing pixel
                    let idx = ((y as u32 * self.width + x as u32) * 4) as usize;
                    if idx + 3 < self.buffer.len() {
                        let dst_r = self.buffer[idx] as f32 / 255.0;
                        let dst_g = self.buffer[idx + 1] as f32 / 255.0;
                        let dst_b = self.buffer[idx + 2] as f32 / 255.0;

                        // Standard alpha blending: out = src * alpha + dst * (1 - alpha)
                        self.buffer[idx] = ((r * a + dst_r * (1.0 - a)) * 255.0) as u8;
                        self.buffer[idx + 1] = ((g * a + dst_g * (1.0 - a)) * 255.0) as u8;
                        self.buffer[idx + 2] = ((b * a + dst_b * (1.0 - a)) * 255.0) as u8;
                        self.buffer[idx + 3] = 255; // Fully opaque output
                    }
                }
            }
        }
    }

    /// Get the buffer width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the buffer height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get a reference to the raw pixel buffer.
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Render text using the built-in bitmap font.
    fn render_text(&mut self, text: &TextPrimitive) {
        // Rasterize the text to a bitmap
        let (bitmap, text_width, text_height) = self.text_renderer.rasterize(&text.text, text.font_size);

        if bitmap.is_empty() {
            return;
        }

        // Blend the text bitmap onto the buffer
        blend_text_onto_buffer(
            &mut self.buffer,
            self.width,
            self.height,
            &bitmap,
            text_width,
            text_height,
            text.x as i32,
            text.y as i32,
            text.color,
        );
    }

    /// Render a shadow.
    fn render_shadow(&mut self, shadow: &ShadowPrimitive) {
        if shadow.inset {
            // Inner shadows are more complex - skip for now
            return;
        }

        // Get shape bounds
        let (x, y, width, height) = match &shadow.shape {
            ShadowShape::Rect { x, y, width, height, .. } => (*x, *y, *width, *height),
            ShadowShape::Ellipse { center_x, center_y, radius_x, radius_y } => {
                (center_x - radius_x, center_y - radius_y, radius_x * 2.0, radius_y * 2.0)
            }
        };

        // Calculate shadow bounds (offset + spread)
        let shadow_x = x + shadow.offset_x - shadow.spread;
        let shadow_y = y + shadow.offset_y - shadow.spread;
        let shadow_width = width + shadow.spread * 2.0;
        let shadow_height = height + shadow.spread * 2.0;

        // Blur radius affects the shadow expansion
        let blur_expansion = shadow.blur * 1.5;
        let render_x = (shadow_x - blur_expansion).max(0.0) as i32;
        let render_y = (shadow_y - blur_expansion).max(0.0) as i32;
        let render_x2 = ((shadow_x + shadow_width + blur_expansion) as i32).min(self.width as i32);
        let render_y2 = ((shadow_y + shadow_height + blur_expansion) as i32).min(self.height as i32);

        // Get shadow color
        let (sr, sg, sb, sa) = shadow.color.to_rgba8();
        let sr = sr as f32 / 255.0;
        let sg = sg as f32 / 255.0;
        let sb = sb as f32 / 255.0;
        let sa = sa as f32 / 255.0;

        // Render shadow with simple distance-based falloff
        let blur_radius = shadow.blur.max(0.1);

        for py in render_y..render_y2 {
            for px in render_x..render_x2 {
                // Calculate distance to shadow rectangle
                let dx = if (px as f32) < shadow_x {
                    shadow_x - px as f32
                } else if (px as f32) > shadow_x + shadow_width {
                    px as f32 - (shadow_x + shadow_width)
                } else {
                    0.0
                };

                let dy = if (py as f32) < shadow_y {
                    shadow_y - py as f32
                } else if (py as f32) > shadow_y + shadow_height {
                    py as f32 - (shadow_y + shadow_height)
                } else {
                    0.0
                };

                let distance = (dx * dx + dy * dy).sqrt();

                // Calculate alpha based on distance and blur
                let alpha = if distance <= 0.0 {
                    sa
                } else {
                    // Gaussian-like falloff
                    let falloff = (-distance * distance / (blur_radius * blur_radius * 0.5)).exp();
                    sa * falloff
                };

                if alpha > 0.001 {
                    // Alpha blend with existing pixel
                    let idx = ((py as u32 * self.width + px as u32) * 4) as usize;
                    if idx + 3 < self.buffer.len() {
                        let dst_r = self.buffer[idx] as f32 / 255.0;
                        let dst_g = self.buffer[idx + 1] as f32 / 255.0;
                        let dst_b = self.buffer[idx + 2] as f32 / 255.0;

                        self.buffer[idx] = ((sr * alpha + dst_r * (1.0 - alpha)) * 255.0) as u8;
                        self.buffer[idx + 1] = ((sg * alpha + dst_g * (1.0 - alpha)) * 255.0) as u8;
                        self.buffer[idx + 2] = ((sb * alpha + dst_b * (1.0 - alpha)) * 255.0) as u8;
                    }
                }
            }
        }
    }
}

/// Edge function for triangle rasterization.
/// Returns positive if point p is to the left of edge (a, b).
#[inline]
fn edge_function(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::types::Color;

    #[test]
    fn test_software_renderer_new() {
        let renderer = SoftwareRenderer::new(100, 100);
        assert_eq!(renderer.width(), 100);
        assert_eq!(renderer.height(), 100);
        assert_eq!(renderer.buffer().len(), 100 * 100 * 4);
    }

    #[test]
    fn test_software_renderer_clear() {
        let mut renderer = SoftwareRenderer::new(10, 10);
        renderer.clear(255, 0, 0, 255); // Red

        // Check first pixel
        assert_eq!(renderer.buffer()[0], 255); // R
        assert_eq!(renderer.buffer()[1], 0);   // G
        assert_eq!(renderer.buffer()[2], 0);   // B
        assert_eq!(renderer.buffer()[3], 255); // A
    }

    #[test]
    fn test_tessellate_and_rasterize_rect() {
        let mut renderer = SoftwareRenderer::new(100, 100);
        renderer.clear(255, 255, 255, 255); // White

        // Create a simple red rectangle
        let rect = RectPrimitive::new(10.0, 10.0, 30.0, 20.0)
            .with_fill(Fill::Solid(Color::rgb(1.0, 0.0, 0.0)));

        let mut mesh = Mesh::new();
        renderer.tessellator.tessellate_rect(&rect, &mut mesh);
        renderer.rasterize_mesh(&mesh);

        // Check a pixel inside the rectangle (at 20, 15)
        let idx = ((15 * 100 + 20) * 4) as usize;
        assert_eq!(renderer.buffer()[idx], 255);     // R (red)
        assert_eq!(renderer.buffer()[idx + 1], 0);   // G
        assert_eq!(renderer.buffer()[idx + 2], 0);   // B

        // Check a pixel outside the rectangle (at 5, 5)
        let idx = ((5 * 100 + 5) * 4) as usize;
        assert_eq!(renderer.buffer()[idx], 255);     // R (white)
        assert_eq!(renderer.buffer()[idx + 1], 255); // G
        assert_eq!(renderer.buffer()[idx + 2], 255); // B
    }
}
