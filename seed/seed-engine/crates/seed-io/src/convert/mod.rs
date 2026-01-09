//! Geometry conversion utilities.

pub mod tessellate;
pub mod primitives;

pub use tessellate::tessellate_brep;
pub use primitives::generate_primitive_mesh;
