//! GPU renderer using wgpu.

use crate::primitives::{RenderCommand, Scene};
use crate::shapes::{Mesh, Tessellator, Vertex};
use seed_core::RenderError;
use std::borrow::Cow;
use wgpu::util::DeviceExt;

use super::shaders::SHADER_SOURCE;

/// GPU vertex type matching the software renderer's Vertex.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuVertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl From<&Vertex> for GpuVertex {
    fn from(v: &Vertex) -> Self {
        Self {
            position: v.position,
            color: v.color,
        }
    }
}

/// Uniform buffer data.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    projection: [[f32; 4]; 4],
}

/// GPU-accelerated 2D renderer.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    tessellator: Tessellator,
}

impl GpuRenderer {
    /// Create a new GPU renderer with the specified dimensions.
    pub fn new(width: u32, height: u32) -> Result<Self, RenderError> {
        let (device, queue) = pollster::block_on(Self::create_device())?;

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("2D Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SHADER_SOURCE)),
        });

        // Create bind group layout for uniforms
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("2D Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // Color
                        wgpu::VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create uniform buffer with orthographic projection
        let projection = orthographic_projection(width as f32, height as f32);
        let uniforms = Uniforms { projection };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            uniform_buffer,
            bind_group,
            width,
            height,
            tessellator: Tessellator::new(),
        })
    }

    async fn create_device() -> Result<(wgpu::Device, wgpu::Queue), RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| RenderError::GpuError {
                reason: "Failed to find a suitable GPU adapter".to_string(),
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("2D Renderer Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| RenderError::GpuError {
                reason: format!("Failed to create device: {}", e),
            })?;

        Ok((device, queue))
    }

    /// Render a scene to an RGBA pixel buffer.
    pub fn render(&mut self, scene: &Scene) -> Result<Vec<u8>, RenderError> {
        // Create render target texture
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render Target"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Begin render pass with white background
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);

            // Render each command
            for command in &scene.commands {
                self.render_command(&mut render_pass, command);
            }
        }

        // Create staging buffer for reading back pixels
        let bytes_per_row = self.width * 4;
        // Align to 256 bytes as required by wgpu
        let padded_bytes_per_row = (bytes_per_row + 255) & !255;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (padded_bytes_per_row * self.height) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to staging buffer
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &staging_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back pixels
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .recv()
            .map_err(|_| RenderError::GpuError {
                reason: "Failed to receive buffer mapping result".to_string(),
            })?
            .map_err(|e| RenderError::GpuError {
                reason: format!("Failed to map buffer: {:?}", e),
            })?;

        // Copy data, handling row padding
        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);

        for row in 0..self.height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + (self.width * 4) as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging_buffer.unmap();

        Ok(pixels)
    }

    fn render_command<'a>(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'a>,
        command: &RenderCommand,
    ) {
        let mut mesh = Mesh::new();

        match command {
            RenderCommand::Rect(rect) => {
                self.tessellator.tessellate_rect(rect, &mut mesh);
            }
            RenderCommand::RoundedRect(rect) => {
                self.tessellator.tessellate_rounded_rect(rect, &mut mesh);
            }
            RenderCommand::Ellipse(ellipse) => {
                self.tessellator.tessellate_ellipse(ellipse, &mut mesh);
            }
            RenderCommand::Path(path) => {
                self.tessellator.tessellate_path(path, &mut mesh);
            }
            RenderCommand::Text(text) => {
                // Text rendering via tessellation
                // For now, skip text in GPU renderer
                // TODO: Implement glyph atlas or SDF text rendering
                let _ = text;
                return;
            }
            RenderCommand::Shadow(_shadow) => {
                // Shadow rendering requires multi-pass blur
                // TODO: Implement shadow pass
                return;
            }
            RenderCommand::PushClip(_) | RenderCommand::PopClip => {
                // Clipping requires stencil buffer
                // TODO: Implement clipping
                return;
            }
            RenderCommand::SetOpacity(_) => {
                // Opacity requires render state management
                // TODO: Implement opacity
                return;
            }
        }

        if mesh.is_empty() {
            return;
        }

        // Convert to GPU vertices
        let gpu_vertices: Vec<GpuVertex> = mesh.vertices.iter().map(GpuVertex::from).collect();

        // Create vertex buffer
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Create index buffer
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        // Issue draw call
        // Note: We need to use raw pointers here because render_pass borrows self
        // and we need to set buffers that outlive the borrow
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
    }

    /// Update the renderer dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;

        // Update projection matrix
        let projection = orthographic_projection(width as f32, height as f32);
        let uniforms = Uniforms { projection };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }
}

/// Create an orthographic projection matrix for 2D rendering.
///
/// Maps screen coordinates (0,0)-(width,height) to NDC (-1,-1)-(1,1).
/// Y is flipped so that (0,0) is top-left.
fn orthographic_projection(width: f32, height: f32) -> [[f32; 4]; 4] {
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, -2.0 / height, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0, 1.0],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orthographic_projection() {
        let proj = orthographic_projection(800.0, 600.0);

        // Check scale factors
        assert!((proj[0][0] - 2.0 / 800.0).abs() < 0.0001);
        assert!((proj[1][1] - (-2.0 / 600.0)).abs() < 0.0001);

        // Check translation
        assert!((proj[3][0] - (-1.0)).abs() < 0.0001);
        assert!((proj[3][1] - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_gpu_vertex_size() {
        // Ensure GPU vertex matches expected size
        assert_eq!(std::mem::size_of::<GpuVertex>(), 24);
    }

    #[test]
    fn test_uniforms_size() {
        // 4x4 matrix of f32 = 64 bytes
        assert_eq!(std::mem::size_of::<Uniforms>(), 64);
    }
}
