//! Code generators for various UI frameworks.

#[cfg(feature = "swiftui")]
mod swiftui;
#[cfg(feature = "compose")]
mod compose;
#[cfg(feature = "react")]
mod react;

mod scaffold;
mod templates;

#[cfg(feature = "swiftui")]
pub use swiftui::SwiftUIGenerator;
#[cfg(feature = "compose")]
pub use compose::ComposeGenerator;
#[cfg(feature = "react")]
pub use react::ReactGenerator;

pub use scaffold::{ScaffoldGenerator, ScaffoldOptions};
pub use templates::TemplateEngine;

use crate::error::Result;
use crate::model::{ComponentSpec, DesignSystem, InteractionGraph, StateModel};

/// Common trait for code generators.
pub trait CodeGenerator {
    /// Target framework name.
    fn framework_name(&self) -> &'static str;

    /// Generate component code.
    fn generate_component(&self, spec: &ComponentSpec) -> Result<String>;

    /// Generate state management code.
    fn generate_state(&self, model: &StateModel) -> Result<String>;

    /// Generate navigation code.
    fn generate_navigation(&self, graph: &InteractionGraph) -> Result<String>;

    /// Generate design system code (tokens, theme).
    fn generate_design_system(&self, ds: &DesignSystem) -> Result<String>;

    /// Generate full project scaffold.
    fn generate_project(&self, options: &ProjectOptions) -> Result<GeneratedProject>;
}

/// Options for project generation.
#[derive(Debug, Clone, Default)]
pub struct ProjectOptions {
    /// Project name.
    pub name: String,
    /// Package/bundle identifier.
    pub package_id: String,
    /// Minimum platform version.
    pub min_version: Option<String>,
    /// Include example code.
    pub include_examples: bool,
    /// Include tests.
    pub include_tests: bool,
    /// Component specifications.
    pub components: Vec<ComponentSpec>,
    /// State model.
    pub state_model: Option<StateModel>,
    /// Interaction graph.
    pub interaction_graph: Option<InteractionGraph>,
    /// Design system.
    pub design_system: Option<DesignSystem>,
}

/// Generated project output.
#[derive(Debug, Clone, Default)]
pub struct GeneratedProject {
    /// Generated files (path -> content).
    pub files: Vec<GeneratedFile>,
    /// Scaffold TODOs.
    pub todos: Vec<ScaffoldTodo>,
}

/// A generated file.
#[derive(Debug, Clone)]
pub struct GeneratedFile {
    /// File path relative to project root.
    pub path: String,
    /// File content.
    pub content: String,
    /// Whether this is a scaffold file.
    pub is_scaffold: bool,
}

/// A TODO marker in scaffold code.
#[derive(Debug, Clone)]
pub struct ScaffoldTodo {
    /// File path.
    pub file: String,
    /// Line number.
    pub line: usize,
    /// TODO description.
    pub description: String,
    /// Priority (high, medium, low).
    pub priority: TodoPriority,
}

/// Priority level for scaffold TODOs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for TodoPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low => write!(f, "LOW"),
        }
    }
}
