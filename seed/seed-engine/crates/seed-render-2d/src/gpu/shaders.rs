//! WGSL shader sources for GPU rendering.

/// Combined vertex and fragment shader for 2D rendering.
///
/// The vertex shader transforms 2D positions using an orthographic projection matrix.
/// The fragment shader outputs interpolated vertex colors with alpha blending support.
pub const SHADER_SOURCE: &str = r#"
// Uniform buffer containing the projection matrix
struct Uniforms {
    projection: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Vertex input: position (2D) and color (RGBA)
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

// Vertex shader: transform position and pass through color
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.projection * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    return out;
}

// Fragment shader: output the interpolated color
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;
