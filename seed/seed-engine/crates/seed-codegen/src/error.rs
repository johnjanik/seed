//! Error types for code generation.

use thiserror::Error;

/// Result type alias for codegen operations.
pub type Result<T> = std::result::Result<T, CodegenError>;

/// Errors that can occur during code generation.
#[derive(Error, Debug)]
pub enum CodegenError {
    /// Invalid Seed document structure.
    #[error("Invalid document structure: {0}")]
    InvalidDocument(String),

    /// Unknown element type.
    #[error("Unknown element type: {0}")]
    UnknownElement(String),

    /// Unsupported feature for target framework.
    #[error("Feature '{feature}' is not supported for {target}")]
    UnsupportedFeature {
        feature: String,
        target: String,
    },

    /// Template rendering error.
    #[error("Template error: {0}")]
    TemplateError(#[from] handlebars::RenderError),

    /// Template not found.
    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    /// Invalid template.
    #[error("Invalid template: {0}")]
    InvalidTemplate(#[from] handlebars::TemplateError),

    /// Invalid state model.
    #[error("Invalid state model: {0}")]
    InvalidStateModel(String),

    /// Circular dependency detected.
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    /// Missing required property.
    #[error("Missing required property '{property}' on element '{element}'")]
    MissingProperty {
        element: String,
        property: String,
    },

    /// Invalid property value.
    #[error("Invalid value for property '{property}': {message}")]
    InvalidPropertyValue {
        property: String,
        message: String,
    },

    /// Code formatting error.
    #[error("Code formatting error: {0}")]
    FormattingError(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic error.
    #[error("{0}")]
    Other(String),
}
