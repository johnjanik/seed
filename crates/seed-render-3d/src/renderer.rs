//! 3D software renderer with depth buffer.

use glam::{Vec3, Vec4, Mat4};
use seed_core::types::Color;

use crate::geometry::Mesh;
use crate::material::{Material, Light, fresnel_schlick, gamma_correct};
use crate::scene::{Scene3D, Camera};

/// 3D software renderer.
pub struct SoftwareRenderer3D {
    width: u32,
    height: u32,
    color_buffer: Vec<u8>,
    depth_buffer: Vec<f32>,
}

impl SoftwareRenderer3D {
    /// Create a new software renderer.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            color_buffer: vec![0; (width * height * 4) as usize],
            depth_buffer: vec![f32::INFINITY; (width * height) as usize],
        }
    }

    /// Clear the buffers.
    pub fn clear(&mut self, color: Color) {
        let r = (color.r * 255.0) as u8;
        let g = (color.g * 255.0) as u8;
        let b = (color.b * 255.0) as u8;

        for chunk in self.color_buffer.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }

        for d in &mut self.depth_buffer {
            *d = f32::INFINITY;
        }
    }

    /// Render a scene.
    pub fn render(&mut self, scene: &Scene3D) -> &[u8] {
        self.clear(scene.background);

        let aspect = self.width as f32 / self.height as f32;
        let view = scene.camera.view_matrix();
        let proj = scene.camera.projection_matrix(aspect);
        let view_proj = proj * view;

        // Render each object
        for obj in &scene.objects {
            let mvp = view_proj * obj.transform;
            self.render_mesh(&obj.mesh, &obj.material, &mvp, &scene.lights, &scene.camera);
        }

        &self.color_buffer
    }

    /// Render a mesh.
    fn render_mesh(
        &mut self,
        mesh: &Mesh,
        material: &Material,
        mvp: &Mat4,
        lights: &[Light],
        camera: &Camera,
    ) {
        if mesh.indices.is_empty() {
            return;
        }

        let camera_pos = camera.position();

        // Process each triangle
        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }

            let i0 = tri[0] as usize;
            let i1 = tri[1] as usize;
            let i2 = tri[2] as usize;

            // Get vertices and normals
            let v0 = mesh.vertices[i0];
            let v1 = mesh.vertices[i1];
            let v2 = mesh.vertices[i2];

            let n0 = mesh.normals.get(i0).copied().unwrap_or(Vec3::Y);
            let n1 = mesh.normals.get(i1).copied().unwrap_or(Vec3::Y);
            let n2 = mesh.normals.get(i2).copied().unwrap_or(Vec3::Y);

            // Transform to clip space
            let clip0 = *mvp * Vec4::new(v0.x, v0.y, v0.z, 1.0);
            let clip1 = *mvp * Vec4::new(v1.x, v1.y, v1.z, 1.0);
            let clip2 = *mvp * Vec4::new(v2.x, v2.y, v2.z, 1.0);

            // Simple near-plane clipping
            if clip0.w <= 0.0 || clip1.w <= 0.0 || clip2.w <= 0.0 {
                continue;
            }

            // Perspective divide to NDC
            let ndc0 = Vec3::new(clip0.x / clip0.w, clip0.y / clip0.w, clip0.z / clip0.w);
            let ndc1 = Vec3::new(clip1.x / clip1.w, clip1.y / clip1.w, clip1.z / clip1.w);
            let ndc2 = Vec3::new(clip2.x / clip2.w, clip2.y / clip2.w, clip2.z / clip2.w);

            // Convert to screen space
            let screen0 = self.ndc_to_screen(ndc0);
            let screen1 = self.ndc_to_screen(ndc1);
            let screen2 = self.ndc_to_screen(ndc2);

            // Rasterize the triangle
            self.rasterize_triangle(
                screen0, screen1, screen2,
                ndc0.z, ndc1.z, ndc2.z,
                v0, v1, v2,
                n0, n1, n2,
                material,
                lights,
                camera_pos,
            );
        }
    }

    /// Convert NDC coordinates to screen coordinates.
    fn ndc_to_screen(&self, ndc: Vec3) -> Vec3 {
        Vec3::new(
            (ndc.x + 1.0) * 0.5 * self.width as f32,
            (1.0 - ndc.y) * 0.5 * self.height as f32, // Flip Y
            ndc.z,
        )
    }

    /// Rasterize a single triangle.
    #[allow(clippy::too_many_arguments)]
    fn rasterize_triangle(
        &mut self,
        s0: Vec3, s1: Vec3, s2: Vec3,
        z0: f32, z1: f32, z2: f32,
        v0: Vec3, v1: Vec3, v2: Vec3,
        n0: Vec3, n1: Vec3, n2: Vec3,
        material: &Material,
        lights: &[Light],
        camera_pos: Vec3,
    ) {
        // Get bounding box
        let min_x = s0.x.min(s1.x).min(s2.x).max(0.0) as i32;
        let max_x = s0.x.max(s1.x).max(s2.x).min(self.width as f32 - 1.0) as i32;
        let min_y = s0.y.min(s1.y).min(s2.y).max(0.0) as i32;
        let max_y = s0.y.max(s1.y).max(s2.y).min(self.height as f32 - 1.0) as i32;

        // Compute edge function denominator
        let area = edge_function([s0.x, s0.y], [s1.x, s1.y], [s2.x, s2.y]);
        if area.abs() < 0.0001 {
            return; // Degenerate triangle
        }
        let inv_area = 1.0 / area;

        // Scan through bounding box
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];

                // Compute barycentric coordinates
                let w0 = edge_function([s1.x, s1.y], [s2.x, s2.y], p);
                let w1 = edge_function([s2.x, s2.y], [s0.x, s0.y], p);
                let w2 = edge_function([s0.x, s0.y], [s1.x, s1.y], p);

                // Check if inside triangle
                if (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0) {
                    let b0 = w0 * inv_area;
                    let b1 = w1 * inv_area;
                    let b2 = w2 * inv_area;

                    // Interpolate depth
                    let depth = z0 * b0 + z1 * b1 + z2 * b2;

                    // Depth test
                    let idx = (y as u32 * self.width + x as u32) as usize;
                    if depth < self.depth_buffer[idx] {
                        self.depth_buffer[idx] = depth;

                        // Interpolate world position and normal
                        let world_pos = v0 * b0 + v1 * b1 + v2 * b2;
                        let normal = (n0 * b0 + n1 * b1 + n2 * b2).normalize();

                        // Compute lighting
                        let color = compute_lighting(
                            world_pos,
                            normal,
                            material,
                            lights,
                            camera_pos,
                        );

                        // Write to color buffer
                        let pixel_idx = idx * 4;
                        self.color_buffer[pixel_idx] = (color.r * 255.0) as u8;
                        self.color_buffer[pixel_idx + 1] = (color.g * 255.0) as u8;
                        self.color_buffer[pixel_idx + 2] = (color.b * 255.0) as u8;
                        self.color_buffer[pixel_idx + 3] = 255;
                    }
                }
            }
        }
    }

    /// Get the width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the color buffer.
    pub fn buffer(&self) -> &[u8] {
        &self.color_buffer
    }

    /// Get the depth buffer.
    pub fn depth_buffer(&self) -> &[f32] {
        &self.depth_buffer
    }
}

/// Edge function for triangle rasterization.
#[inline]
fn edge_function(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
}

/// Compute PBR-inspired lighting with Fresnel and gamma correction.
fn compute_lighting(
    position: Vec3,
    normal: Vec3,
    material: &Material,
    lights: &[Light],
    camera_pos: Vec3,
) -> Color {
    let view_dir = (camera_pos - position).normalize();
    let n_dot_v = normal.dot(view_dir).max(0.0);

    // F0 for Fresnel (dielectric = 0.04, metal uses albedo)
    let f0 = if material.metallic > 0.5 {
        [material.color.r, material.color.g, material.color.b]
    } else {
        [0.04, 0.04, 0.04]
    };

    let mut result = Color::rgb(0.0, 0.0, 0.0);

    for light in lights {
        match light {
            Light::Directional { direction, color, intensity } => {
                let light_dir = Vec3::new(-direction[0], -direction[1], -direction[2]).normalize();
                add_light_contribution(
                    &mut result, normal, view_dir, light_dir, n_dot_v,
                    color, *intensity, material, &f0,
                );
            }
            Light::Ambient { color, intensity } => {
                // Ambient with AO
                let ao = material.ambient_occlusion;
                result.r += material.color.r * color.r * intensity * ao;
                result.g += material.color.g * color.g * intensity * ao;
                result.b += material.color.b * color.b * intensity * ao;
            }
            Light::Point { position: light_pos, color, intensity, range } => {
                let to_light = Vec3::new(light_pos[0], light_pos[1], light_pos[2]) - position;
                let distance = to_light.length();

                if distance < *range {
                    let light_dir = to_light / distance;
                    // Quadratic attenuation for more realistic falloff
                    let attenuation = ((1.0 - (distance / range).powi(4)).max(0.0)).powi(2)
                        / (distance * distance + 1.0);
                    let effective_intensity = intensity * attenuation;

                    add_light_contribution(
                        &mut result, normal, view_dir, light_dir, n_dot_v,
                        color, effective_intensity, material, &f0,
                    );
                }
            }
            Light::Spot { position: light_pos, direction, color, intensity, range, inner_angle, outer_angle } => {
                let to_light = Vec3::new(light_pos[0], light_pos[1], light_pos[2]) - position;
                let distance = to_light.length();

                if distance < *range {
                    let light_dir = to_light / distance;
                    let spot_dir = Vec3::new(-direction[0], -direction[1], -direction[2]).normalize();

                    // Spot light cone attenuation
                    let cos_angle = light_dir.dot(spot_dir);
                    let cos_inner = inner_angle.cos();
                    let cos_outer = outer_angle.cos();

                    if cos_angle > cos_outer {
                        let spot_factor = if cos_angle > cos_inner {
                            1.0
                        } else {
                            // Smooth falloff between inner and outer cone
                            let t = (cos_angle - cos_outer) / (cos_inner - cos_outer);
                            t * t * (3.0 - 2.0 * t) // Smoothstep
                        };

                        let distance_attenuation = (1.0 - distance / range).max(0.0);
                        let effective_intensity = intensity * spot_factor * distance_attenuation;

                        add_light_contribution(
                            &mut result, normal, view_dir, light_dir, n_dot_v,
                            color, effective_intensity, material, &f0,
                        );
                    }
                }
            }
        }
    }

    // Add emissive
    if let Some(ref emissive) = material.emissive {
        result.r += emissive.r;
        result.g += emissive.g;
        result.b += emissive.b;
    }

    // Apply gamma correction for proper display
    let corrected = gamma_correct(Color {
        r: result.r.clamp(0.0, 1.0),
        g: result.g.clamp(0.0, 1.0),
        b: result.b.clamp(0.0, 1.0),
        a: 1.0,
    });

    corrected
}

/// Add light contribution with Fresnel and specular.
#[inline]
fn add_light_contribution(
    result: &mut Color,
    normal: Vec3,
    view_dir: Vec3,
    light_dir: Vec3,
    _n_dot_v: f32,
    light_color: &Color,
    intensity: f32,
    material: &Material,
    f0: &[f32; 3],
) {
    // Diffuse
    let n_dot_l = normal.dot(light_dir).max(0.0);
    if n_dot_l <= 0.0 {
        return;
    }

    // Half vector for specular
    let half_vec = (light_dir + view_dir).normalize();
    let n_dot_h = normal.dot(half_vec).max(0.0);
    let v_dot_h = view_dir.dot(half_vec).max(0.0);

    // Fresnel
    let fresnel = fresnel_schlick(v_dot_h, f0[0]);

    // Roughness-based shininess
    let shininess = (1.0 - material.roughness) * 256.0 + 4.0;
    let specular_strength = n_dot_h.powf(shininess) * (1.0 - material.roughness);

    // Combine: for metals, specular uses albedo color; for dielectrics, white specular
    let diffuse = n_dot_l * intensity;
    let specular = specular_strength * intensity * fresnel;

    if material.metallic > 0.5 {
        // Metallic: albedo affects specular, reduced diffuse
        let metal_diffuse = diffuse * (1.0 - fresnel) * 0.1;
        result.r += (material.color.r * metal_diffuse + material.color.r * specular) * light_color.r;
        result.g += (material.color.g * metal_diffuse + material.color.g * specular) * light_color.g;
        result.b += (material.color.b * metal_diffuse + material.color.b * specular) * light_color.b;
    } else {
        // Dielectric: normal diffuse + white specular
        result.r += (material.color.r * diffuse * (1.0 - fresnel) + specular) * light_color.r;
        result.g += (material.color.g * diffuse * (1.0 - fresnel) + specular) * light_color.g;
        result.b += (material.color.b * diffuse * (1.0 - fresnel) + specular) * light_color.b;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Shape;
    use crate::scene::SceneObject;

    #[test]
    fn test_renderer_new() {
        let renderer = SoftwareRenderer3D::new(100, 100);
        assert_eq!(renderer.width(), 100);
        assert_eq!(renderer.height(), 100);
        assert_eq!(renderer.buffer().len(), 100 * 100 * 4);
        assert_eq!(renderer.depth_buffer().len(), 100 * 100);
    }

    #[test]
    fn test_renderer_clear() {
        let mut renderer = SoftwareRenderer3D::new(10, 10);
        renderer.clear(Color::rgb(1.0, 0.0, 0.0)); // Red

        assert_eq!(renderer.buffer()[0], 255); // R
        assert_eq!(renderer.buffer()[1], 0);   // G
        assert_eq!(renderer.buffer()[2], 0);   // B
    }

    #[test]
    fn test_render_empty_scene() {
        let mut renderer = SoftwareRenderer3D::new(100, 100);
        let scene = Scene3D::new();

        let buffer = renderer.render(&scene);
        assert_eq!(buffer.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_render_box() {
        let mut renderer = SoftwareRenderer3D::new(100, 100);
        let mut scene = Scene3D::new();

        scene.add_object(SceneObject::from_shape(Shape::box_shape(5.0, 5.0, 5.0)));
        scene.fit_camera();

        let buffer = renderer.render(&scene);

        // Should have rendered something (not all background color)
        let bg = scene.background;
        let bg_r = (bg.r * 255.0) as u8;
        let bg_g = (bg.g * 255.0) as u8;
        let bg_b = (bg.b * 255.0) as u8;

        let mut has_non_bg = false;
        for chunk in buffer.chunks(4) {
            if chunk[0] != bg_r || chunk[1] != bg_g || chunk[2] != bg_b {
                has_non_bg = true;
                break;
            }
        }

        assert!(has_non_bg, "Scene should have rendered visible geometry");
    }

    #[test]
    fn test_depth_buffer_works() {
        let mut renderer = SoftwareRenderer3D::new(100, 100);
        let mut scene = Scene3D::new();

        // Add two overlapping boxes
        scene.add_object(
            SceneObject::from_shape(Shape::box_shape(5.0, 5.0, 5.0))
                .with_material(Material::new(Color::rgb(1.0, 0.0, 0.0)))
        );
        scene.add_object(
            SceneObject::from_shape(Shape::box_shape(3.0, 3.0, 3.0).translate(0.0, 0.0, 3.0))
                .with_material(Material::new(Color::rgb(0.0, 1.0, 0.0)))
        );
        scene.fit_camera();

        renderer.render(&scene);

        // Depth buffer should have varying values
        let depth_buf = renderer.depth_buffer();
        let has_finite = depth_buf.iter().any(|d| d.is_finite());
        assert!(has_finite, "Depth buffer should have finite values where geometry was rendered");
    }
}
