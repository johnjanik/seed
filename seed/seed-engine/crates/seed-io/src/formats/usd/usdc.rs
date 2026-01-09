//! USDC (USD Crate/Binary) parser.
//!
//! Implements a pure Rust parser for USD binary format.
//! Some types are defined for spec completeness.

#![allow(dead_code)]

/// USDC file magic number.
pub const USDC_MAGIC: &[u8; 8] = b"PXR-USDC";

/// USDC file version.
#[derive(Debug, Clone, Copy)]
pub struct UsdcVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

/// USDC file header.
#[derive(Debug, Clone)]
pub struct UsdcHeader {
    /// File version
    pub version: UsdcVersion,
    /// String table offset
    pub strings_offset: u64,
    /// Token table offset
    pub tokens_offset: u64,
    /// Field sets offset
    pub field_sets_offset: u64,
    /// Paths offset
    pub paths_offset: u64,
    /// Specs offset
    pub specs_offset: u64,
}

/// Check if data is a USDC file.
pub fn is_usdc(data: &[u8]) -> bool {
    data.len() >= 8 && &data[0..8] == USDC_MAGIC
}

/// Parse USDC header.
pub fn parse_header(data: &[u8]) -> Option<UsdcHeader> {
    if !is_usdc(data) || data.len() < 64 {
        return None;
    }

    // Version is at offset 8
    let version = UsdcVersion {
        major: data[8],
        minor: data[9],
        patch: data[10],
    };

    // Offsets are at various positions in the header
    // This is a simplified version - real USDC has more complex header

    Some(UsdcHeader {
        version,
        strings_offset: 0, // TODO: Parse actual offsets
        tokens_offset: 0,
        field_sets_offset: 0,
        paths_offset: 0,
        specs_offset: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_usdc() {
        assert!(is_usdc(b"PXR-USDC\x00\x00\x00\x00"));
        assert!(!is_usdc(b"not usdc"));
        assert!(!is_usdc(b"PXR")); // too short
    }
}
