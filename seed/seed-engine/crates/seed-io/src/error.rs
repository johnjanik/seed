//! Error types for seed-io.

use thiserror::Error;

/// Result type for seed-io operations.
pub type Result<T> = std::result::Result<T, IoError>;

/// Errors that can occur during file I/O operations.
#[derive(Debug, Error)]
pub enum IoError {
    /// Unknown or unsupported file format.
    #[error("unknown format: {0}")]
    UnknownFormat(String),

    /// No reader available for the given format.
    #[error("no reader for format: {0}")]
    NoReader(String),

    /// No writer available for the given format.
    #[error("no writer for format: {0}")]
    NoWriter(String),

    /// Parse error when reading a file.
    #[error("parse error: {message}")]
    ParseError {
        /// Error message.
        message: String,
        /// Byte offset where the error occurred.
        offset: Option<usize>,
        /// Context about what was being parsed.
        context: Option<String>,
    },

    /// Invalid data in the file.
    #[error("invalid data: {0}")]
    InvalidData(String),

    /// Missing required data.
    #[error("missing required: {0}")]
    MissingRequired(String),

    /// Unsupported feature or version.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// Base64 decoding error.
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),

    /// Geometry conversion error.
    #[error("geometry error: {0}")]
    GeometryError(String),

    /// Internal error (should not happen).
    #[error("internal error: {0}")]
    Internal(String),
}

impl IoError {
    /// Create a parse error with context.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
            offset: None,
            context: None,
        }
    }

    /// Create a parse error with offset.
    pub fn parse_at(message: impl Into<String>, offset: usize) -> Self {
        Self::ParseError {
            message: message.into(),
            offset: Some(offset),
            context: None,
        }
    }

    /// Create a parse error with context.
    pub fn parse_context(message: impl Into<String>, context: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
            offset: None,
            context: Some(context.into()),
        }
    }
}
