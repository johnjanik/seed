# STEP Rendering Parity with FreeCAD

## Overview

This document analyzes the differences between Seed Engine and FreeCAD when rendering the same STEP file (`original.STEP` - a complex watch tourbillon mechanism) and outlines the work required to achieve visual parity.

## Current State Comparison

### FreeCAD Render
- Clean white/gray shaded model with proper Phong lighting
- Edge highlighting (dark outlines on geometry edges)
- All 1,510 faces properly tessellated and rendered
- Correct depth sorting and face culling
- Full assembly visible: gears, screws, curved arms, rings, springs

### Seed Engine Render
- Blue-tinted partial rendering
- Curved surfaces rendered as wireframe circles/ellipses
- ~70% of geometry missing
- Incorrect depth sorting (transparency-like artifacts)
- Only scattered fragments visible

## Root Cause Analysis

### 1. Curved Surface Tessellation Failure

**Problem:** Cylindrical, toroidal, and spherical surfaces are not being properly tessellated.

**Evidence:** The STEP file contains:
- 796 `CYLINDRICAL_SURFACE` entities
- 216 `TOROIDAL_SURFACE` entities
- Multiple spherical surfaces

**Current Code:** In `reader.rs:241-244`:
```rust
Some(StepEntity::CylindricalSurface(_cyl)) => {
    // Uses edge-based tessellation which only extracts boundary curves
    self.tessellate_face_from_edges(face, mesh)?;
}
```

The `tessellate_face_from_edges` function only extracts boundary curve vertices, resulting in wireframe circles instead of tessellated surfaces.

**FreeCAD Approach:** Uses OpenCASCADE's `BRepMesh_IncrementalMesh` which:
1. Analyzes surface type and curvature
2. Samples the surface at appropriate UV resolution
3. Creates triangulated mesh respecting trim curves
4. Handles complex boundary topologies (holes, multiple loops)

### 2. B-Spline Surface Support Missing

**Problem:** No tessellation for NURBS/B-spline surfaces.

**Evidence:** The file contains 276 `B_SPLINE_CURVE_WITH_KNOTS` entities which may bound B-spline surfaces.

**Current Code:** Falls through to edge-based tessellation.

### 3. Trim Curve Handling

**Problem:** Faces bounded by circular arcs and complex curves are not properly trimmed.

**FreeCAD/OpenCASCADE:** Uses parametric UV mapping to:
1. Project trim curves onto surface parameter space
2. Sample surface only within trimmed region
3. Handle multiple trim loops (outer boundary + inner holes)

### 4. Assembly Hierarchy Not Preserved

**Problem:** Parts rendered in wrong positions or missing entirely.

**Evidence:** Second screenshot shows detached geometry on right side.

**Required:** Parse and apply transforms from:
- `PRODUCT_DEFINITION`
- `SHAPE_REPRESENTATION_RELATIONSHIP`
- `ITEM_DEFINED_TRANSFORMATION`
- `REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION`

## Implementation Roadmap

### Phase 1: Proper Cylindrical Surface Tessellation (Critical)

**Goal:** Render 796 cylindrical surfaces correctly.

**Implementation:**
```rust
fn tessellate_cylindrical_face(
    &self,
    face: &AdvancedFace,
    cylinder: &CylindricalSurface,
    mesh: &mut TriangleMesh,
) -> Result<()> {
    // 1. Extract boundary curves (circles, lines, arcs)
    let bounds = self.extract_face_bounds(face)?;

    // 2. Determine angular range from circular edges
    let (theta_min, theta_max) = self.compute_angular_range(&bounds, cylinder)?;

    // 3. Determine height range from linear edges
    let (z_min, z_max) = self.compute_height_range(&bounds, cylinder)?;

    // 4. Generate UV grid respecting angular and height bounds
    let u_segments = ((theta_max - theta_min).abs() / 0.1).ceil() as usize;
    let v_segments = ((z_max - z_min) / (cylinder.radius * 0.1)).ceil() as usize;

    // 5. Sample surface and triangulate
    for v in 0..=v_segments {
        for u in 0..=u_segments {
            let theta = theta_min + (u as f32 / u_segments as f32) * (theta_max - theta_min);
            let z = z_min + (v as f32 / v_segments as f32) * (z_max - z_min);

            let point = cylinder.evaluate(theta, z);
            mesh.positions.push(point);
        }
    }

    // 6. Generate triangle indices
    self.triangulate_grid(mesh, u_segments, v_segments);
}
```

### Phase 2: Toroidal Surface Tessellation

**Goal:** Render 216 toroidal surfaces (fillets, rounded edges).

**Implementation:** Similar to cylindrical but with two angular parameters (major/minor angle).

### Phase 3: B-Spline Surface Tessellation

**Goal:** Support NURBS surfaces for complex freeform geometry.

**Implementation:**
1. Parse `B_SPLINE_SURFACE_WITH_KNOTS` entities
2. Implement de Boor's algorithm for surface evaluation
3. Adaptive tessellation based on curvature

### Phase 4: Proper Trim Curve Handling

**Goal:** Correctly bound surfaces with complex trim curves.

**Implementation:**
1. Project boundary curves to UV space
2. Use polygon clipping (Sutherland-Hodgman or similar)
3. Tessellate only within trimmed region

### Phase 5: Assembly Transforms

**Goal:** Position all parts correctly in world space.

**Implementation:**
1. Parse `NEXT_ASSEMBLY_USAGE_OCCURRENCE`
2. Build transform hierarchy
3. Apply accumulated transforms to geometry

### Phase 6: Visual Enhancements

**Goal:** Match FreeCAD's visual quality.

1. **Edge highlighting:** Extract and render boundary edges as line primitives
2. **Improved lighting:** Phong shading with ambient occlusion
3. **Face coloring:** Parse `STYLED_ITEM` and `COLOUR_RGB` entities

## FreeCAD Reference Implementation

FreeCAD's STEP import uses OpenCASCADE Technology (OCCT):

**Key files in FreeCAD source:**
- `src/Mod/Part/App/ImportStep.cpp` - STEP import entry point
- `src/Mod/Part/App/TopoShape.cpp` - B-Rep to mesh conversion

**Key OCCT classes:**
- `STEPControl_Reader` - STEP file parsing
- `BRepMesh_IncrementalMesh` - Tessellation
- `BRep_Tool::Triangulation` - Access mesh data

**Tessellation parameters FreeCAD uses:**
```cpp
// Default linear deflection (chord height)
double linearDeflection = 0.1;  // mm

// Angular deflection for curves
double angularDeflection = 0.5;  // radians

BRepMesh_IncrementalMesh mesh(shape, linearDeflection,
                               false,  // relative
                               angularDeflection);
```

## Alternative: Pure Rust B-Rep Tessellation

Instead of FFI to OpenCASCADE, implement pure Rust tessellation:

### Libraries to Consider
- `truck` - Rust CAD kernel with B-Rep support (https://github.com/ricosjp/truck)
- `opencascade-rs` - Rust bindings to OCCT (requires C++ toolchain)

### Custom Implementation Strategy
1. **Analytic surfaces** (plane, cylinder, cone, sphere, torus): Direct parametric sampling
2. **B-spline surfaces**: de Boor evaluation with adaptive refinement
3. **Trim curves**: 2D polygon operations in UV space

## Estimated Effort

| Phase | Complexity | Estimate |
|-------|------------|----------|
| 1. Cylindrical tessellation | Medium | 2-3 days |
| 2. Toroidal tessellation | Medium | 1-2 days |
| 3. B-Spline surfaces | High | 1 week |
| 4. Trim curves | High | 1 week |
| 5. Assembly transforms | Medium | 2-3 days |
| 6. Visual enhancements | Low | 2-3 days |

**Total:** 3-4 weeks for full parity

## Metrics for Success

1. **Geometry coverage:** All 1,510 faces rendered (currently ~30%)
2. **Visual accuracy:** Side-by-side comparison indistinguishable
3. **Performance:** <1 second import time for this file
4. **No visual artifacts:** Correct depth sorting, no wireframe leaks

## Test Files

- `original.STEP` - Complex watch tourbillon (68,722 lines, 65,461 entities)
- Khronos glTF sample models for comparison
- CAx-IF STEP test cases (https://www.cax-if.org/)
