//! Token system types and utilities.

use crate::ast::TokenPath;
use crate::types::{Color, Length};
use indexmap::IndexMap;

/// A resolved token value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResolvedToken {
    Color(Color),
    Length(Length),
    Number(f64),
    String(String),
}

/// A map of resolved tokens.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenMap {
    tokens: IndexMap<String, ResolvedToken>,
}

impl TokenMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a token with a dotted path (e.g., "color.primary").
    pub fn insert(&mut self, path: &str, value: ResolvedToken) {
        self.tokens.insert(path.to_string(), value);
    }

    /// Get a token by dotted path.
    pub fn get(&self, path: &str) -> Option<&ResolvedToken> {
        self.tokens.get(path)
    }

    /// Get a token by TokenPath.
    pub fn get_by_path(&self, path: &TokenPath) -> Option<&ResolvedToken> {
        let path_str = path.0.join(".");
        self.tokens.get(&path_str)
    }

    /// Check if a token exists.
    pub fn contains(&self, path: &str) -> bool {
        self.tokens.contains_key(path)
    }

    /// Iterate over all tokens.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ResolvedToken)> {
        self.tokens.iter()
    }

    /// Number of tokens in the map.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Check if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

impl ResolvedToken {
    /// Try to get as a color.
    pub fn as_color(&self) -> Option<Color> {
        match self {
            ResolvedToken::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// Try to get as a length.
    pub fn as_length(&self) -> Option<Length> {
        match self {
            ResolvedToken::Length(l) => Some(*l),
            _ => None,
        }
    }

    /// Try to get as a number.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            ResolvedToken::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Try to get as a string.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ResolvedToken::String(s) => Some(s),
            _ => None,
        }
    }
}
