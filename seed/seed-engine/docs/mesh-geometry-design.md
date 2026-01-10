# Mesh Geometry in Seed Format: Design Options

## Context

Seed aims to be a universal design language spanning 2D UI/graphics and 3D manufacturing. The current AST supports primitives (Box, Sphere, Cylinder) and CSG operations, but lacks representation for arbitrary mesh geometry. When importing STEP/glTF files, mesh data is lost in translation.

**Goal**: Enable Seed to represent anything from mobile apps to space stations while remaining human-readable, AI-friendly, and version-controllable.

---

## Option 1: Inline Mesh Primitive

Add a `Mesh` geometry type that embeds vertex/triangle data directly in the document.

### Syntax Example
```seed
Part GearTooth:
    geometry: Mesh
        vertices:
            [0.0, 0.0, 0.0]
            [1.0, 0.0, 0.0]
            [0.5, 1.0, 0.0]
        triangles:
            [0, 1, 2]
    color: #4080ff
```

### Benefits
- **Self-contained**: Single file contains all geometry - no external dependencies
- **Version control**: Geometry changes tracked in git diffs
- **Offline**: Works without network or filesystem access (ideal for WASM)
- **AI generation**: LLMs can generate complete documents including geometry
- **Validation**: Parser can validate mesh integrity at parse time

### Drawbacks
- **File size**: Large meshes = massive documents (gear.STEP has 4,626 vertices)
- **Human readability**: Thousands of vertex lines overwhelm the design intent
- **Edit difficulty**: Hard to manually modify mesh coordinates
- **Diff noise**: Mesh changes create huge, unreadable diffs
- **Duplication**: Same mesh in multiple documents = redundant data

### Recommendation
Best for: Small meshes (<100 vertices), procedural geometry, prototyping
Not ideal for: CAD imports, complex manufacturing parts

---

## Option 2: External File Reference

Add import syntax to reference external mesh files.

### Syntax Example
```seed
Part Gear:
    geometry: Import("./meshes/gear.stl")
    # or with format hint:
    geometry: Import("./gear.step", format: "step")
    # or URL:
    geometry: Import("https://cdn.example.com/parts/gear.glb")
    color: #4080ff

# With transformation
Part ScaledGear:
    geometry: Import("./gear.stl")
    scale: 0.5
    rotate: [0, 45deg, 0]
```

### Benefits
- **Separation of concerns**: Design intent in .seed, geometry in native formats
- **Industry standard**: Engineers keep CAD files, designers reference them
- **File size**: .seed stays small and readable
- **Existing tooling**: CAD software continues to own geometry editing
- **Caching**: Runtime can cache parsed geometry separately
- **Version control**: Track geometry files independently or in LFS

### Drawbacks
- **External dependencies**: Document requires additional files to render
- **Path management**: Relative vs absolute paths, missing file errors
- **Format proliferation**: Must support many mesh formats (.stl, .obj, .step, .glb)
- **AI limitations**: LLMs can't generate/modify the external geometry
- **Offline complexity**: Need to bundle referenced files for distribution

### Recommendation
Best for: Production workflows, CAD/manufacturing integration, team collaboration
Not ideal for: Self-contained examples, AI-generated content

---

## Option 3: Bounding Box + Metadata

Output a fitted bounding box with mesh statistics as metadata/comments.

### Syntax Example
```seed
Part Gear:
    # Imported from: gear.STEP
    # Original: 4,626 vertices, 1,542 triangles
    # Bounds: 40mm x 184mm x 184mm
    geometry: Box(40mm, 184mm, 184mm)
    color: #4080ff
```

### Benefits
- **Backwards compatible**: Works with current parser
- **Quick visualization**: Bounding boxes render instantly
- **Size estimation**: Accurate dimensions for layout/collision
- **Human readable**: Clear what the part represents
- **Minimal diff**: Clean version control

### Drawbacks
- **Lossy**: Actual geometry is completely lost
- **Visual fidelity**: Box doesn't look like a gear
- **Manufacturing**: Cannot export back to valid CAD format
- **Limited utility**: Only useful for early prototyping

### Recommendation
Best for: Placeholder geometry, layout planning, documentation
Not ideal for: Any production use case

---

## Option 4: Primitive Fitting

Analyze meshes to detect if they're actually boxes/cylinders/spheres.

### Syntax Example
```seed
# Detected: Cylinder from mesh analysis
Part Shaft:
    geometry: Cylinder(10mm, 50mm)
    # confidence: 0.97
    # original_vertices: 1,024

# Falls back to bounding box if no fit
Part ComplexPart:
    geometry: Box(40mm, 184mm, 184mm)  # Could not fit primitive
```

### Benefits
- **Lossless for CAD primitives**: Extruded shapes roundtrip perfectly
- **Compact representation**: Primitive params vs vertex soup
- **Editable**: Users can tweak dimensions directly
- **Semantic preservation**: "This is a cylinder" vs "these are triangles"

### Drawbacks
- **Complex geometry lost**: Gears, organic shapes, assemblies can't fit primitives
- **Algorithm complexity**: Fitting is non-trivial and error-prone
- **False positives**: Might incorrectly classify geometry
- **Limited scope**: Only helps with a small subset of real-world parts

### Recommendation
Best for: Parametric CAD parts, simple mechanical components
Not ideal for: Organic shapes, assemblies, detailed manufactured parts

---

## Hybrid Approach: Recommended Solution

For a universal render engine, a **layered approach** best serves both humans and AI:

### Proposed Syntax
```seed
@meta:
    profile: Seed/3D
    version: 1.0

Part Gear:
    # Level 1: Semantic description (AI-friendly, human-readable)
    description: "Spur gear with 20 teeth, module 2"

    # Level 2: Parametric geometry when possible
    geometry: Gear(teeth: 20, module: 2mm, width: 40mm)
    # OR: geometry: Import("./gear.step")
    # OR: geometry: Mesh(vertices: [...], triangles: [...])

    # Level 3: Bounds for quick operations
    bounds: Box(40mm, 184mm, 184mm)

    # Level 4: Material properties
    material: Steel
    color: #4080ff
```

### Implementation Strategy

1. **Extend AST** with three geometry representations:
   - `Primitive` (existing): Box, Sphere, Cylinder + new parametric types
   - `MeshRef`: External file reference with format hint
   - `MeshInline`: Embedded vertices/triangles for small meshes

2. **Smart conversion**:
   - When writing: Use primitive if detected, else MeshRef if file exists, else MeshInline
   - Threshold: Inline if <500 vertices, else external reference

3. **Semantic layer**:
   - Add optional `description` field for AI understanding
   - Add optional `bounds` for fast spatial queries

### Benefits of Hybrid
- **Scales up**: Simple parts stay simple, complex parts use references
- **AI compatible**: Descriptions and primitives are LLM-friendly
- **Version control friendly**: Large geometry in separate files
- **Forward compatible**: New primitive types can be added (Gear, Thread, Chamfer)
- **Manufacturing ready**: Preserves parametric intent when possible

---

## Next Steps

1. **Phase 1**: Add `Import()` syntax for external file references
2. **Phase 2**: Add `MeshInline` for small embedded meshes
3. **Phase 3**: Add parametric primitives (Gear, Extrude, Revolve)
4. **Phase 4**: Add AI-friendly `description` and `bounds` fields

This approach lets Seed grow from UI mockups to space station CAD while maintaining readability and AI compatibility.
