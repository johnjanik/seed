# Seed Rendering Engine

A high-performance rendering engine for [Seed](../seed_spec_20260104/seed-specification.pdf), a unified semantic design language for 2D graphics and 3D manufacturing.

## Architecture

```
seed-engine/
├── crates/
│   ├── seed-core/        # Core types, AST, errors
│   ├── seed-parser/      # Seed document parser (nom-based)
│   ├── seed-resolver/    # Token and reference resolution
│   ├── seed-expander/    # Component expansion
│   ├── seed-constraint/  # Cassowary constraint solver
│   ├── seed-layout/      # Layout computation
│   ├── seed-render-2d/   # 2D GPU rendering (wgpu)
│   ├── seed-render-3d/   # 3D geometry (OpenCASCADE)
│   ├── seed-export/      # Export: SVG, PDF, STEP, STL
│   └── seed-wasm/        # WebAssembly bindings
├── tests/                # Integration tests
└── benches/              # Performance benchmarks
```

## Building

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Build WASM package
cd crates/seed-wasm
wasm-pack build --target web
```

## Design Principles

1. **Functional Core, Imperative Shell**: Pure transformations in the core, effects at boundaries
2. **Make Illegal States Unrepresentable**: Use the type system to prevent bugs
3. **Zero-Copy Where Possible**: Minimize allocations in hot paths
4. **Cross-Platform**: Native (Linux/macOS/Windows) and WebAssembly

## Status

🚧 **Early Development** - Core architecture in place, implementations in progress.

See [seed-engine-tds.md](../seed-engine-tds.md) for the full technical design specification.
