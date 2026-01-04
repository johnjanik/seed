//! Constraint solving for Seed documents.
//!
//! This crate implements:
//! - Cassowary simplex algorithm for 2D constraints
//! - Geometric constraints for 3D parts
//! - Priority handling

mod cassowary;
mod solver;

pub use cassowary::Variable;
pub use solver::{ConstraintSystem, Solution};

use seed_core::{Document, ConstraintError};

/// Solve all constraints in a document.
pub fn solve_constraints(doc: &Document) -> Result<Solution, ConstraintError> {
    let mut system = ConstraintSystem::new();
    system.add_document(doc)?;
    system.solve()
}
