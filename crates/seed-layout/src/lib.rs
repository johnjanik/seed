//! Layout computation for Seed documents.
//!
//! This crate computes the final positions and sizes of elements based on
//! constraint solutions and auto-layout rules.
//!
//! # Architecture
//!
//! 1. **Constraint solving**: Uses seed-constraint to solve explicit constraints
//! 2. **Auto-layout**: Stack/flow layout for elements without explicit positioning
//! 3. **Text measurement**: Computes text bounds for proper sizing
//!
//! # Example
//!
//! ```ignore
//! use seed_layout::{compute_layout, LayoutTree, LayoutOptions};
//!
//! let doc = parse_document(source)?;
//! let layout = compute_layout(&doc, &LayoutOptions::default())?;
//!
//! for node in layout.nodes() {
//!     println!("{}: {:?}", node.name(), node.bounds());
//! }
//! ```

mod tree;
mod compute;
mod auto_layout;
mod text;

pub use tree::{LayoutTree, LayoutNode, LayoutNodeId, Bounds};
pub use compute::{compute_layout, LayoutOptions};
pub use auto_layout::{AutoLayout, Direction, Alignment, Distribution};
pub use text::{TextMetrics, measure_text};
