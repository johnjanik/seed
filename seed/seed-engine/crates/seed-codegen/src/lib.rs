//! Code generation from Seed designs to native UI frameworks.
//!
//! This crate analyzes Seed design documents and generates native code
//! for various UI frameworks including SwiftUI, Kotlin Compose, and React.
//!
//! # Features
//!
//! - `swiftui` - Generate SwiftUI code for iOS/macOS
//! - `compose` - Generate Kotlin Compose code for Android
//! - `react` - Generate React/TypeScript code for web
//!
//! # Example
//!
//! ```ignore
//! use seed_codegen::{SwiftUIGenerator, CodeGenerator};
//!
//! let generator = SwiftUIGenerator::new();
//! let code = generator.generate_component(&spec)?;
//! println!("{}", code);
//! ```

pub mod error;
pub mod analyzer;
pub mod generators;
pub mod model;
pub mod element;

pub use element::{CodegenElement, PropertyValue as ElementPropertyValue};

pub use error::{CodegenError, Result};
pub use analyzer::{IntentAnalyzer, SemanticRole, RoleDetector, InteractionPattern, PatternRecognizer};
pub use model::{StateModel, InteractionGraph, DesignSystem, ComponentSpec};
pub use generators::{
    CodeGenerator, ProjectOptions, GeneratedProject, GeneratedFile,
    ScaffoldGenerator, ScaffoldOptions, TemplateEngine,
};

// Re-export framework-specific generators
#[cfg(feature = "swiftui")]
pub use generators::SwiftUIGenerator;

#[cfg(feature = "compose")]
pub use generators::ComposeGenerator;

#[cfg(feature = "react")]
pub use generators::ReactGenerator;
