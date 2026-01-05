# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This repository contains the specification and technical design documents for **Seed**, a unified semantic design language for expressing design intent across 2D graphics and 3D manufacturing.

Seed is:
- A human-readable, machine-parseable markup language
- AI-native (structured for LLM comprehension and generation)
- A bridge between 2D UI/graphics and 3D CAD/manufacturing
- Version-controllable with meaningful diffs

## Repository Structure

- `seed-engine/` - Rust implementation of the rendering engine
  - `crates/seed-core/` - Core types, AST, errors
  - `crates/seed-parser/` - Parser (nom-based)
  - `crates/seed-resolver/` - Token and reference resolution
  - `crates/seed-expander/` - Component expansion
  - `crates/seed-constraint/` - Cassowary constraint solver
  - `crates/seed-layout/` - Layout computation
  - `crates/seed-render-2d/` - 2D GPU rendering (wgpu)
  - `crates/seed-render-3d/` - 3D geometry (OpenCASCADE)
  - `crates/seed-export/` - Export: SVG, PDF, STEP, STL
  - `crates/seed-wasm/` - WebAssembly bindings
- `seed-engine-tds.md` - Technical Design Specification for the engine
- `seed_spec_20260104/` - LaTeX specification document (v1.5)
  - `seed-specification.tex` - Main document assembling all parts
  - `parts/` - Specification sections (foundations, 2D, 3D, AI protocol, implementation)

## Build Commands

### Rust Engine
```bash
cd seed-engine
cargo build              # Build all crates
cargo test               # Run tests
cargo bench              # Run benchmarks
cargo check              # Fast type checking
```

### WASM Build
```bash
cd seed-engine/crates/seed-wasm
wasm-pack build --target web
```

### LaTeX Specification
```bash
cd seed_spec_20260104
pdflatex seed-specification.tex && pdflatex seed-specification.tex
```

## Key Technical Decisions

The Seed Rendering Engine uses:
- **Rust** for memory safety + performance + WASM support
- **nom** parser combinators for parsing
- **Cassowary algorithm** for 2D constraint solving
- **OpenCASCADE** (via FFI) for 3D B-rep geometry
- **wgpu** for cross-platform GPU rendering
- Functional core, imperative shell architecture

## Seed Language Concepts

- **Elements**: Frame, Text, Part (3D) - semantic building blocks
- **Constraints**: Relationship-based positioning (not coordinates)
- **Tokens**: Design tokens for colors, spacing, typography
- **Components**: Reusable, parameterized design patterns
- **Profiles**: Seed/2D (screens) and Seed/3D (manufacturing)

## Current Work In Progress (2026-01-05)

### WASM Demo Status
The WASM demo at `seed-engine/crates/seed-wasm/demo/index.html`:
- **Working**: Simple single-frame rendering
- **Working**: Nested frames (fixed 2026-01-05)
- **Untested**: Gradients - may have separate issues

### Recent Fixes Applied
1. **Parser syntax**: Changed from `Frame` to `Frame:` (with colon) - parser requires colon
2. **Properties to constraints**: Modified `seed-constraint/src/solver.rs` to convert layout properties (`width`, `height`, `x`, `y`) to implicit constraints
3. **Name matching**: Fixed element naming in constraint system to match layout system (`frame_1` not `_frame_0`)
4. **CRC32 table**: Fixed 2 typos in PNG CRC32 lookup table at indices 111 and 245 in `seed-export/src/png.rs`
5. **Nested loop bug (2026-01-05)**: Fixed O(n²) nested loop in `scene.rs:156-159` that caused stack overflow - was iterating all AST children for each layout child instead of zipping them
6. **Canvas child ID bug (2026-01-05)**: Fixed `canvas.rs:129-131` to use actual layout tree child IDs instead of computing incorrect IDs

### How to Test
```bash
cd seed-engine/crates/seed-wasm
wasm-pack build --target web
# Then open demo/index.html via HTTP server (python -m http.server 8081)
```

### Next Steps
1. Test gradient rendering in WASM demo
2. Add more comprehensive unit tests for nested elements
3. Clean up dead code warnings
