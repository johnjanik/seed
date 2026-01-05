//! GPU-accelerated 2D rendering using wgpu.
//!
//! This module provides hardware-accelerated rendering as an alternative
//! to the software renderer. It uses the same Scene/RenderCommand pipeline
//! but executes rendering on the GPU via WGSL shaders.

pub mod renderer;
pub mod shaders;

pub use renderer::GpuRenderer;
