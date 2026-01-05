//! Error types for the Seed engine.

use crate::ast::Span;
use thiserror::Error;

/// Top-level error type for the Seed engine.
#[derive(Debug, Error)]
pub enum SeedError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Resolve(#[from] ResolveError),

    #[error(transparent)]
    Expand(#[from] ExpandError),

    #[error(transparent)]
    Constraint(#[from] ConstraintError),

    #[error(transparent)]
    Layout(#[from] LayoutError),

    #[error(transparent)]
    Render(#[from] RenderError),

    #[error(transparent)]
    Export(#[from] ExportError),
}

/// Errors during parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unexpected token at line {line}, column {column}: expected {expected}, found {found:?}")]
    UnexpectedToken {
        found: String,
        expected: String,
        line: u32,
        column: u32,
    },

    #[error("Invalid indentation at line {line}: expected {expected} spaces, found {found}")]
    InvalidIndentation {
        line: u32,
        expected: u32,
        found: u32,
    },

    #[error("Unterminated string starting at line {line}")]
    UnterminatedString { line: u32 },

    #[error("Invalid number format: {value}")]
    InvalidNumber { value: String, span: Span },

    #[error("Invalid color format: {value}")]
    InvalidColor { value: String, span: Span },

    #[error("Unknown element type: {name}")]
    UnknownElementType { name: String, span: Span },

    #[error("Unexpected end of input")]
    UnexpectedEof,
}

/// Errors during token/reference resolution.
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("Undefined token: ${path}")]
    UndefinedToken { path: String, span: Span },

    #[error("Circular token reference: {}", .cycle.join(" -> "))]
    CircularTokenReference { cycle: Vec<String> },

    #[error("Undefined element reference: {name}")]
    UndefinedElement { name: String, span: Span },

    #[error("Cannot reference element {name} from this context")]
    InvalidElementReference { name: String, span: Span },

    #[error("Invalid reference '{reference}': {reason}")]
    InvalidReference { reference: String, reason: String, span: Span },
}

/// Errors during component expansion.
#[derive(Debug, Error)]
pub enum ExpandError {
    #[error("Undefined component: {name}")]
    UndefinedComponent { name: String, span: Span },

    #[error("Missing required prop '{prop}' for component {component}")]
    MissingRequiredProp {
        component: String,
        prop: String,
        span: Span,
    },

    #[error("Invalid prop type for '{prop}': expected {expected}, got {got}")]
    InvalidPropType {
        prop: String,
        expected: String,
        got: String,
        span: Span,
    },

    #[error("Maximum component nesting depth ({depth}) exceeded")]
    MaxDepthExceeded { depth: u32 },
}

/// Errors during constraint solving.
#[derive(Debug, Error)]
pub enum ConstraintError {
    #[error("Unsatisfiable required constraint")]
    Unsatisfiable { constraint_desc: String, span: Span },

    #[error("Constraint references unknown property: {property}")]
    UnknownProperty { property: String, span: Span },

    #[error("Conflicting required constraints")]
    ConflictingRequired {
        constraint1: String,
        constraint2: String,
    },
}

/// Errors during layout computation.
#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("Layout cycle detected involving element {element}")]
    CycleDetected { element: String },

    #[error("Invalid layout mode for element: {reason}")]
    InvalidLayoutMode { reason: String, span: Span },

    #[error("Constraint error: {0}")]
    ConstraintError(#[from] ConstraintError),
}

/// Errors during rendering.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("GPU initialization failed: {reason}")]
    GpuInitFailed { reason: String },

    #[error("Shader compilation failed: {reason}")]
    ShaderCompileFailed { reason: String },

    #[error("Texture creation failed: {reason}")]
    TextureFailed { reason: String },

    #[error("Font loading failed: {path}")]
    FontLoadFailed { path: String },

    #[error("GPU error: {reason}")]
    GpuError { reason: String },
}

/// Errors during export.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("Export format not supported for this document type: {format}")]
    UnsupportedFormat { format: String },

    #[error("I/O error during export: {0}")]
    Io(#[from] std::io::Error),

    #[error("3D geometry error: {reason}")]
    GeometryError { reason: String },

    #[error("No geometry to export")]
    NoGeometry,

    #[error("Render failed: {reason}")]
    RenderFailed { reason: String },
}
