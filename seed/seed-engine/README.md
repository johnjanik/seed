# Seed Engine

A pure Rust engine for parsing, rendering, and converting design documents. Supports 2D graphics rendering and 3D CAD file interchange.

## What This Is

Seed Engine is a **visualization and file interchange toolkit**, not a geometry kernel. It provides:

- **Seed language parser** - Parse `.seed` design documents
- **2D rendering pipeline** - Layout, constraints, GPU rendering via wgpu
- **3D file I/O** - Import/export STEP, glTF, USD, and other CAD formats
- **WebAssembly support** - Run in browsers with full functionality

## What This Is NOT

This is **not** a replacement for geometry kernels like OpenCASCADE, Parasolid, or ACIS. It cannot:

- Perform boolean operations (union, subtract, intersect)
- Create or modify B-rep geometry
- Generate fillets, chamfers, or sweeps
- Handle geometric tolerances and healing

For CAD modeling operations, use [OpenCASCADE](https://github.com/Open-Cascade-SAS/OCCT), [truck](https://github.com/ricosjp/truck), or similar kernels.

## Use Cases

- **CAD file visualization** - Display STEP/glTF/USD files in web or native apps
- **Format conversion** - Convert between CAD interchange formats
- **Design document rendering** - Render Seed markup to SVG, PDF, PNG
- **Rapid prototyping** - Quick visualization without heavy CAD dependencies

## Crate Structure

```
seed-engine/
├── crates/
│   ├── seed-core/        # Core types, AST, error definitions
│   ├── seed-parser/      # Seed document parser (nom-based)
│   ├── seed-resolver/    # Token and reference resolution
│   ├── seed-expander/    # Component expansion
│   ├── seed-constraint/  # Cassowary constraint solver for 2D layout
│   ├── seed-layout/      # Layout computation
│   ├── seed-render-2d/   # 2D GPU rendering (wgpu)
│   ├── seed-render-3d/   # 3D geometry utilities
│   ├── seed-export/      # Export: SVG, PDF, PNG, STL
│   ├── seed-io/          # Universal file I/O (STEP, glTF, USD)
│   └── seed-wasm/        # WebAssembly bindings
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
seed-io = { git = "https://github.com/johnjanik/seed", branch = "master" }
```

Or for the full engine:

```toml
[dependencies]
seed-core = { git = "https://github.com/johnjanik/seed", branch = "master" }
seed-parser = { git = "https://github.com/johnjanik/seed", branch = "master" }
seed-io = { git = "https://github.com/johnjanik/seed", branch = "master" }
```

## Usage

### Reading CAD Files

```rust
use seed_io::{FormatRegistry, ReadOptions};

// Auto-detect format and parse
let registry = FormatRegistry::with_defaults();
let data = std::fs::read("model.step")?;
let scene = registry.read(&data, &ReadOptions::default())?;

println!("Nodes: {}", scene.nodes.len());
println!("Geometries: {}", scene.geometries.len());
println!("Materials: {}", scene.materials.len());
```

### Converting Between Formats

```rust
use seed_io::{FormatRegistry, ReadOptions, WriteOptions};

let registry = FormatRegistry::with_defaults();

// Read STEP
let step_data = std::fs::read("input.step")?;
let scene = registry.read(&step_data, &ReadOptions::default())?;

// Write glTF
let gltf_data = registry.write(&scene, "gltf", &WriteOptions::default())?;
std::fs::write("output.glb", gltf_data)?;
```

### Parsing Seed Documents

```rust
use seed_parser::parse_document;

let source = r#"
Frame:
    width: 800
    height: 600

    Text:
        content: "Hello, Seed!"
        font-size: 24
"#;

let document = parse_document(source)?;
```

## Supported Formats

### Import (Read)

| Format | Extensions | Status |
|--------|------------|--------|
| STEP AP203/AP214/AP242 | `.step`, `.stp` | Tessellation only |
| glTF 2.0 | `.gltf`, `.glb` | Full support |
| USD | `.usda`, `.usdc`, `.usdz` | Basic support |
| Seed | `.seed` | Full support |

### Export (Write)

| Format | Extensions | Notes |
|--------|------------|-------|
| glTF 2.0 | `.glb` | Binary GLB |
| STEP AP203 | `.step` | Tessellated mesh |
| USD | `.usda` | ASCII format |
| SVG | `.svg` | 2D only |
| PDF | `.pdf` | 2D only |
| PNG | `.png` | 2D only |
| STL | `.stl` | Mesh export |
| 3MF | `.3mf` | 3D printing |
| G-code | `.gcode` | CNC/3D printing |

## STEP Import Details

The STEP reader supports:

- **Geometry**: Planes, cylinders, spheres, cones, tori, B-spline surfaces
- **Tessellation**: Adaptive chord-tolerance based meshing
- **Edges**: Boundary edge extraction for wireframe display
- **Assemblies**: Transform hierarchies via REPRESENTATION_RELATIONSHIP
- **Colors**: STYLED_ITEM color extraction (when present)

Limitations:
- Tessellation only (no exact B-rep preservation)
- No NURBS curve/surface evaluation beyond tessellation
- Limited AP242 semantic PMI support

## Building

```bash
# Build all crates
cargo build --release

# Run tests
cargo test

# Build for WebAssembly
cd crates/seed-wasm
wasm-pack build --target web
```

## WebAssembly

The engine compiles to WebAssembly for browser use:

```bash
cd crates/seed-wasm
wasm-pack build --target web
```

Then in JavaScript:

```javascript
import init, { parse_step, scene_to_gltf } from './pkg/seed_wasm.js';

await init();
const scene = parse_step(stepFileBytes);
const gltfBytes = scene_to_gltf(scene);
```

## Performance

Benchmarks on a typical mechanical assembly (5.8MB STEP file):

| Metric | Value |
|--------|-------|
| Parse time | ~240ms |
| Mesh vertices | 175,000 |
| Triangles | 264,000 |
| Edge segments | 127,000 |

## Design Principles

1. **Pure Rust** - No C/C++ dependencies, full WASM compatibility
2. **Functional Core** - Pure transformations, effects at boundaries
3. **Zero-Copy Where Possible** - Minimize allocations in hot paths
4. **Cross-Platform** - Native (Linux/macOS/Windows) and WebAssembly

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting PRs.

## Acknowledgments

- [nom](https://github.com/rust-bakery/nom) - Parser combinators
- [glam](https://github.com/bitshifter/glam-rs) - Linear algebra
- [wgpu](https://github.com/gfx-rs/wgpu) - GPU abstraction
- [lyon](https://github.com/nical/lyon) - 2D tessellation
