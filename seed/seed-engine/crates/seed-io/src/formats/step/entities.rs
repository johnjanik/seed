//! STEP entity type definitions and conversion.
//!
//! Defines the entity types used in AP203/AP214/AP242.
//! Many fields are defined for spec completeness but not yet used.

#![allow(dead_code)]

use glam::Vec3;
use std::collections::HashMap;

use super::p21::{EntityInstance, StepValue};

/// A processed STEP entity.
#[derive(Debug, Clone)]
pub enum StepEntity {
    // Geometry entities
    CartesianPoint(CartesianPoint),
    Direction(Direction),
    Vector(Vector),
    Line(Line),
    Circle(Circle),
    Ellipse(Ellipse),
    BSplineCurve(BSplineCurve),
    Plane(Plane),
    CylindricalSurface(CylindricalSurface),
    ConicalSurface(ConicalSurface),
    SphericalSurface(SphericalSurface),
    ToroidalSurface(ToroidalSurface),
    BSplineSurface(BSplineSurface),

    // Topology entities
    VertexPoint(VertexPoint),
    EdgeCurve(EdgeCurve),
    OrientedEdge(OrientedEdge),
    EdgeLoop(EdgeLoop),
    FaceBound(FaceBound),
    FaceOuterBound(FaceOuterBound),
    AdvancedFace(AdvancedFace),
    ClosedShell(ClosedShell),
    OpenShell(OpenShell),
    ManifoldSolidBrep(ManifoldSolidBrep),

    // Axis placement
    Axis1Placement(Axis1Placement),
    Axis2Placement3D(Axis2Placement3D),

    // Representation
    ShapeRepresentation(ShapeRepresentation),
    AdvancedBrepShapeRepresentation(AdvancedBrepShapeRepresentation),
    GeometricallyBoundedSurfaceShapeRepresentation(ShapeRepresentation),
    FacetedBrep(FacetedBrep),
    FacetedBrepShapeRepresentation(ShapeRepresentation),
    ShellBasedSurfaceModel(ShellBasedSurfaceModel),

    // Product structure
    Product(Product),
    ProductDefinition(ProductDefinition),
    ProductDefinitionShape(ProductDefinitionShape),
    ShapeDefinitionRepresentation(ShapeDefinitionRepresentation),
    NextAssemblyUsageOccurrence(NextAssemblyUsageOccurrence),

    // Assembly/transform entities
    ItemDefinedTransformation(ItemDefinedTransformation),
    ContextDependentShapeRepresentation(ContextDependentShapeRepresentation),
    RepresentationRelationshipWithTransformation(RepresentationRelationshipWithTransformation),

    // Context
    RepresentationContext(RepresentationContext),
    GeometricRepresentationContext(GeometricRepresentationContext),

    // Unknown/unsupported
    Unknown { type_name: String, id: u64 },
}

// ============================================================================
// Geometry Entities
// ============================================================================

#[derive(Debug, Clone)]
pub struct CartesianPoint {
    pub id: u64,
    pub name: String,
    pub coords: Vec3,
}

#[derive(Debug, Clone)]
pub struct Direction {
    pub id: u64,
    pub name: String,
    pub ratios: Vec3,
}

#[derive(Debug, Clone)]
pub struct Vector {
    pub id: u64,
    pub name: String,
    pub orientation: u64,
    pub magnitude: f64,
}

#[derive(Debug, Clone)]
pub struct Axis1Placement {
    pub id: u64,
    pub name: String,
    pub location: u64,
    pub axis: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Axis2Placement3D {
    pub id: u64,
    pub name: String,
    pub location: u64,
    pub axis: Option<u64>,
    pub ref_direction: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub id: u64,
    pub name: String,
    pub point: u64,
    pub direction: u64,
}

#[derive(Debug, Clone)]
pub struct Circle {
    pub id: u64,
    pub name: String,
    pub position: u64,
    pub radius: f64,
}

#[derive(Debug, Clone)]
pub struct Ellipse {
    pub id: u64,
    pub name: String,
    pub position: u64,
    pub semi_axis_1: f64,
    pub semi_axis_2: f64,
}

#[derive(Debug, Clone)]
pub struct BSplineCurve {
    pub id: u64,
    pub name: String,
    pub degree: u32,
    pub control_points: Vec<u64>,
    pub curve_form: String,
    pub closed: bool,
    pub self_intersect: bool,
    pub knots: Vec<f64>,
    pub knot_multiplicities: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct Plane {
    pub id: u64,
    pub name: String,
    pub position: u64,
}

#[derive(Debug, Clone)]
pub struct CylindricalSurface {
    pub id: u64,
    pub name: String,
    pub position: u64,
    pub radius: f64,
}

#[derive(Debug, Clone)]
pub struct ConicalSurface {
    pub id: u64,
    pub name: String,
    pub position: u64,
    pub radius: f64,
    pub semi_angle: f64,
}

#[derive(Debug, Clone)]
pub struct SphericalSurface {
    pub id: u64,
    pub name: String,
    pub position: u64,
    pub radius: f64,
}

#[derive(Debug, Clone)]
pub struct ToroidalSurface {
    pub id: u64,
    pub name: String,
    pub position: u64,
    pub major_radius: f64,
    pub minor_radius: f64,
}

#[derive(Debug, Clone)]
pub struct BSplineSurface {
    pub id: u64,
    pub name: String,
    pub u_degree: u32,
    pub v_degree: u32,
    pub control_points: Vec<Vec<u64>>,
    pub surface_form: String,
    pub u_closed: bool,
    pub v_closed: bool,
    pub self_intersect: bool,
    pub u_knots: Vec<f64>,
    pub v_knots: Vec<f64>,
    pub u_multiplicities: Vec<u32>,
    pub v_multiplicities: Vec<u32>,
}

// ============================================================================
// Topology Entities
// ============================================================================

#[derive(Debug, Clone)]
pub struct VertexPoint {
    pub id: u64,
    pub name: String,
    pub point: u64,
}

#[derive(Debug, Clone)]
pub struct EdgeCurve {
    pub id: u64,
    pub name: String,
    pub start_vertex: u64,
    pub end_vertex: u64,
    pub curve: u64,
    pub same_sense: bool,
}

#[derive(Debug, Clone)]
pub struct OrientedEdge {
    pub id: u64,
    pub name: String,
    pub edge: u64,
    pub orientation: bool,
}

#[derive(Debug, Clone)]
pub struct EdgeLoop {
    pub id: u64,
    pub name: String,
    pub edges: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct FaceBound {
    pub id: u64,
    pub name: String,
    pub bound: u64,
    pub orientation: bool,
}

#[derive(Debug, Clone)]
pub struct FaceOuterBound {
    pub id: u64,
    pub name: String,
    pub bound: u64,
    pub orientation: bool,
}

#[derive(Debug, Clone)]
pub struct AdvancedFace {
    pub id: u64,
    pub name: String,
    pub bounds: Vec<u64>,
    pub surface: u64,
    pub same_sense: bool,
}

#[derive(Debug, Clone)]
pub struct ClosedShell {
    pub id: u64,
    pub name: String,
    pub faces: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct OpenShell {
    pub id: u64,
    pub name: String,
    pub faces: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct ManifoldSolidBrep {
    pub id: u64,
    pub name: String,
    pub outer: u64,
}

#[derive(Debug, Clone)]
pub struct FacetedBrep {
    pub id: u64,
    pub name: String,
    pub outer: u64,
}

#[derive(Debug, Clone)]
pub struct ShellBasedSurfaceModel {
    pub id: u64,
    pub name: String,
    pub shells: Vec<u64>,
}

// ============================================================================
// Representation Entities
// ============================================================================

#[derive(Debug, Clone)]
pub struct ShapeRepresentation {
    pub id: u64,
    pub name: String,
    pub items: Vec<u64>,
    pub context: u64,
}

#[derive(Debug, Clone)]
pub struct AdvancedBrepShapeRepresentation {
    pub id: u64,
    pub name: String,
    pub items: Vec<u64>,
    pub context: u64,
}

#[derive(Debug, Clone)]
pub struct RepresentationContext {
    pub id: u64,
    pub identifier: String,
    pub context_type: String,
}

#[derive(Debug, Clone)]
pub struct GeometricRepresentationContext {
    pub id: u64,
    pub identifier: String,
    pub context_type: String,
    pub dimension: u32,
}

// ============================================================================
// Product Structure Entities
// ============================================================================

#[derive(Debug, Clone)]
pub struct Product {
    pub id: u64,
    pub product_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ProductDefinition {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub formation: u64,
}

#[derive(Debug, Clone)]
pub struct ProductDefinitionShape {
    pub id: u64,
    pub name: String,
    pub description: String,
    pub definition: u64,
}

#[derive(Debug, Clone)]
pub struct ShapeDefinitionRepresentation {
    pub id: u64,
    pub definition: u64,
    pub used_representation: u64,
}

#[derive(Debug, Clone)]
pub struct NextAssemblyUsageOccurrence {
    pub id: u64,
    pub nauo_id: String,
    pub name: String,
    pub relating_product: u64,
    pub related_product: u64,
}

#[derive(Debug, Clone)]
pub struct ItemDefinedTransformation {
    pub id: u64,
    pub name: String,
    pub transform_item_1: u64, // Source axis placement
    pub transform_item_2: u64, // Target axis placement
}

#[derive(Debug, Clone)]
pub struct ContextDependentShapeRepresentation {
    pub id: u64,
    pub representation_relation: u64,
    pub represented_product_relation: u64,
}

#[derive(Debug, Clone)]
pub struct RepresentationRelationshipWithTransformation {
    pub id: u64,
    pub name: String,
    pub rep1: u64,                   // First shape representation
    pub rep2: u64,                   // Second shape representation
    pub transformation_operator: u64, // ITEM_DEFINED_TRANSFORMATION ref
}

// ============================================================================
// Value Extraction Helpers
// ============================================================================

/// Extract a string from a StepValue.
pub fn extract_string(value: &StepValue) -> String {
    match value {
        StepValue::String(s) => s.clone(),
        _ => String::new(),
    }
}

/// Extract a real from a StepValue.
pub fn extract_real(value: &StepValue) -> f64 {
    match value {
        StepValue::Real(r) => *r,
        StepValue::Integer(i) => *i as f64,
        _ => 0.0,
    }
}

/// Extract an integer from a StepValue.
pub fn extract_int(value: &StepValue) -> i64 {
    match value {
        StepValue::Integer(i) => *i,
        StepValue::Real(r) => *r as i64,
        _ => 0,
    }
}

/// Extract a reference from a StepValue.
pub fn extract_ref(value: &StepValue) -> Option<u64> {
    match value {
        StepValue::Reference(r) => Some(*r),
        _ => None,
    }
}

/// Extract a boolean from a StepValue (STEP uses .T. and .F. enums).
pub fn extract_bool(value: &StepValue) -> bool {
    match value {
        StepValue::Enum(s) => s == "T" || s == "TRUE",
        _ => false,
    }
}

/// Extract an enum value as string.
pub fn extract_enum(value: &StepValue) -> String {
    match value {
        StepValue::Enum(s) => s.clone(),
        _ => String::new(),
    }
}

/// Extract a list of references from a StepValue.
pub fn extract_ref_list(value: &StepValue) -> Vec<u64> {
    match value {
        StepValue::List(items) => items.iter().filter_map(extract_ref).collect(),
        _ => Vec::new(),
    }
}

/// Extract a list of reals from a StepValue.
pub fn extract_real_list(value: &StepValue) -> Vec<f64> {
    match value {
        StepValue::List(items) => items.iter().map(extract_real).collect(),
        _ => Vec::new(),
    }
}

/// Extract a list of integers from a StepValue.
pub fn extract_int_list(value: &StepValue) -> Vec<u32> {
    match value {
        StepValue::List(items) => items.iter().map(|v| extract_int(v) as u32).collect(),
        _ => Vec::new(),
    }
}

/// Extract coordinates from a list StepValue.
pub fn extract_coords(value: &StepValue) -> Vec3 {
    match value {
        StepValue::List(items) => {
            let x = items.first().map(extract_real).unwrap_or(0.0);
            let y = items.get(1).map(extract_real).unwrap_or(0.0);
            let z = items.get(2).map(extract_real).unwrap_or(0.0);
            Vec3::new(x as f32, y as f32, z as f32)
        }
        _ => Vec3::ZERO,
    }
}

/// Extract 2D list of references (for B-spline control point grids).
pub fn extract_ref_grid(value: &StepValue) -> Vec<Vec<u64>> {
    match value {
        StepValue::List(rows) => {
            rows.iter().map(extract_ref_list).collect()
        }
        _ => Vec::new(),
    }
}

// ============================================================================
// Entity Conversion
// ============================================================================

/// Convert a raw entity instance to a typed entity.
pub fn convert_entity(instance: &EntityInstance) -> StepEntity {
    let params = &instance.params;
    let id = instance.id;

    match instance.type_name.as_str() {
        // Geometry
        "CARTESIAN_POINT" => {
            StepEntity::CartesianPoint(CartesianPoint {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                coords: params.get(1).map(extract_coords).unwrap_or(Vec3::ZERO),
            })
        }
        "DIRECTION" => {
            StepEntity::Direction(Direction {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                ratios: params.get(1).map(extract_coords).unwrap_or(Vec3::Z),
            })
        }
        "VECTOR" => {
            StepEntity::Vector(Vector {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                orientation: params.get(1).and_then(extract_ref).unwrap_or(0),
                magnitude: params.get(2).map(extract_real).unwrap_or(1.0),
            })
        }
        "AXIS1_PLACEMENT" => {
            StepEntity::Axis1Placement(Axis1Placement {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                location: params.get(1).and_then(extract_ref).unwrap_or(0),
                axis: params.get(2).and_then(extract_ref),
            })
        }
        "AXIS2_PLACEMENT_3D" => {
            StepEntity::Axis2Placement3D(Axis2Placement3D {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                location: params.get(1).and_then(extract_ref).unwrap_or(0),
                axis: params.get(2).and_then(extract_ref),
                ref_direction: params.get(3).and_then(extract_ref),
            })
        }
        "LINE" => {
            StepEntity::Line(Line {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                point: params.get(1).and_then(extract_ref).unwrap_or(0),
                direction: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }
        "CIRCLE" => {
            StepEntity::Circle(Circle {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
                radius: params.get(2).map(extract_real).unwrap_or(1.0),
            })
        }
        "ELLIPSE" => {
            StepEntity::Ellipse(Ellipse {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
                semi_axis_1: params.get(2).map(extract_real).unwrap_or(1.0),
                semi_axis_2: params.get(3).map(extract_real).unwrap_or(1.0),
            })
        }
        "PLANE" => {
            StepEntity::Plane(Plane {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
            })
        }
        "CYLINDRICAL_SURFACE" => {
            StepEntity::CylindricalSurface(CylindricalSurface {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
                radius: params.get(2).map(extract_real).unwrap_or(1.0),
            })
        }
        "CONICAL_SURFACE" => {
            StepEntity::ConicalSurface(ConicalSurface {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
                radius: params.get(2).map(extract_real).unwrap_or(1.0),
                semi_angle: params.get(3).map(extract_real).unwrap_or(0.0),
            })
        }
        "SPHERICAL_SURFACE" => {
            StepEntity::SphericalSurface(SphericalSurface {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
                radius: params.get(2).map(extract_real).unwrap_or(1.0),
            })
        }
        "TOROIDAL_SURFACE" => {
            StepEntity::ToroidalSurface(ToroidalSurface {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                position: params.get(1).and_then(extract_ref).unwrap_or(0),
                major_radius: params.get(2).map(extract_real).unwrap_or(1.0),
                minor_radius: params.get(3).map(extract_real).unwrap_or(0.5),
            })
        }
        "B_SPLINE_SURFACE_WITH_KNOTS" => {
            // Parse 2D control point grid
            let control_points = params
                .get(3)
                .map(|v| match v {
                    StepValue::List(rows) => rows
                        .iter()
                        .map(|row| extract_ref_list(row))
                        .collect(),
                    _ => Vec::new(),
                })
                .unwrap_or_default();

            StepEntity::BSplineSurface(BSplineSurface {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                u_degree: params.get(1).map(|v| extract_int(v) as u32).unwrap_or(3),
                v_degree: params.get(2).map(|v| extract_int(v) as u32).unwrap_or(3),
                control_points,
                surface_form: params.get(4).map(extract_string).unwrap_or_default(),
                u_closed: params.get(5).map(extract_bool).unwrap_or(false),
                v_closed: params.get(6).map(extract_bool).unwrap_or(false),
                self_intersect: params.get(7).map(extract_bool).unwrap_or(false),
                u_multiplicities: params.get(8).map(extract_int_list).unwrap_or_default(),
                v_multiplicities: params.get(9).map(extract_int_list).unwrap_or_default(),
                u_knots: params.get(10).map(extract_real_list).unwrap_or_default(),
                v_knots: params.get(11).map(extract_real_list).unwrap_or_default(),
            })
        }

        // Topology
        "VERTEX_POINT" => {
            StepEntity::VertexPoint(VertexPoint {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                point: params.get(1).and_then(extract_ref).unwrap_or(0),
            })
        }
        "EDGE_CURVE" => {
            StepEntity::EdgeCurve(EdgeCurve {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                start_vertex: params.get(1).and_then(extract_ref).unwrap_or(0),
                end_vertex: params.get(2).and_then(extract_ref).unwrap_or(0),
                curve: params.get(3).and_then(extract_ref).unwrap_or(0),
                same_sense: params.get(4).map(extract_bool).unwrap_or(true),
            })
        }
        "ORIENTED_EDGE" => {
            StepEntity::OrientedEdge(OrientedEdge {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                // ORIENTED_EDGE has vertex_1, vertex_2, edge_element, orientation
                edge: params.get(3).and_then(extract_ref).unwrap_or(0),
                orientation: params.get(4).map(extract_bool).unwrap_or(true),
            })
        }
        "EDGE_LOOP" => {
            StepEntity::EdgeLoop(EdgeLoop {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                edges: params.get(1).map(extract_ref_list).unwrap_or_default(),
            })
        }
        "FACE_BOUND" => {
            StepEntity::FaceBound(FaceBound {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                bound: params.get(1).and_then(extract_ref).unwrap_or(0),
                orientation: params.get(2).map(extract_bool).unwrap_or(true),
            })
        }
        "FACE_OUTER_BOUND" => {
            StepEntity::FaceOuterBound(FaceOuterBound {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                bound: params.get(1).and_then(extract_ref).unwrap_or(0),
                orientation: params.get(2).map(extract_bool).unwrap_or(true),
            })
        }
        "ADVANCED_FACE" => {
            StepEntity::AdvancedFace(AdvancedFace {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                bounds: params.get(1).map(extract_ref_list).unwrap_or_default(),
                surface: params.get(2).and_then(extract_ref).unwrap_or(0),
                same_sense: params.get(3).map(extract_bool).unwrap_or(true),
            })
        }
        "CLOSED_SHELL" => {
            StepEntity::ClosedShell(ClosedShell {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                faces: params.get(1).map(extract_ref_list).unwrap_or_default(),
            })
        }
        "OPEN_SHELL" => {
            StepEntity::OpenShell(OpenShell {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                faces: params.get(1).map(extract_ref_list).unwrap_or_default(),
            })
        }
        "MANIFOLD_SOLID_BREP" => {
            StepEntity::ManifoldSolidBrep(ManifoldSolidBrep {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                outer: params.get(1).and_then(extract_ref).unwrap_or(0),
            })
        }
        "FACETED_BREP" => {
            StepEntity::FacetedBrep(FacetedBrep {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                outer: params.get(1).and_then(extract_ref).unwrap_or(0),
            })
        }
        "SHELL_BASED_SURFACE_MODEL" => {
            StepEntity::ShellBasedSurfaceModel(ShellBasedSurfaceModel {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                shells: params.get(1).map(extract_ref_list).unwrap_or_default(),
            })
        }

        // Representation
        "SHAPE_REPRESENTATION" | "GEOMETRICALLY_BOUNDED_WIREFRAME_SHAPE_REPRESENTATION" => {
            StepEntity::ShapeRepresentation(ShapeRepresentation {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                items: params.get(1).map(extract_ref_list).unwrap_or_default(),
                context: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }
        "ADVANCED_BREP_SHAPE_REPRESENTATION" => {
            StepEntity::AdvancedBrepShapeRepresentation(AdvancedBrepShapeRepresentation {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                items: params.get(1).map(extract_ref_list).unwrap_or_default(),
                context: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }
        "GEOMETRICALLY_BOUNDED_SURFACE_SHAPE_REPRESENTATION" => {
            StepEntity::GeometricallyBoundedSurfaceShapeRepresentation(ShapeRepresentation {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                items: params.get(1).map(extract_ref_list).unwrap_or_default(),
                context: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }
        "FACETED_BREP_SHAPE_REPRESENTATION" => {
            StepEntity::FacetedBrepShapeRepresentation(ShapeRepresentation {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                items: params.get(1).map(extract_ref_list).unwrap_or_default(),
                context: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }

        // Product structure
        "PRODUCT" => {
            StepEntity::Product(Product {
                id,
                product_id: params.first().map(extract_string).unwrap_or_default(),
                name: params.get(1).map(extract_string).unwrap_or_default(),
                description: params.get(2).map(extract_string).unwrap_or_default(),
            })
        }
        "PRODUCT_DEFINITION" => {
            StepEntity::ProductDefinition(ProductDefinition {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                description: params.get(1).map(extract_string).unwrap_or_default(),
                formation: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }
        "PRODUCT_DEFINITION_SHAPE" => {
            StepEntity::ProductDefinitionShape(ProductDefinitionShape {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                description: params.get(1).map(extract_string).unwrap_or_default(),
                definition: params.get(2).and_then(extract_ref).unwrap_or(0),
            })
        }
        "SHAPE_DEFINITION_REPRESENTATION" => {
            StepEntity::ShapeDefinitionRepresentation(ShapeDefinitionRepresentation {
                id,
                definition: params.first().and_then(extract_ref).unwrap_or(0),
                used_representation: params.get(1).and_then(extract_ref).unwrap_or(0),
            })
        }
        "NEXT_ASSEMBLY_USAGE_OCCURRENCE" => {
            // NAUO structure: (id_string, name, description, relating_product, related_product, ref_designator)
            StepEntity::NextAssemblyUsageOccurrence(NextAssemblyUsageOccurrence {
                id,
                nauo_id: params.first().map(extract_string).unwrap_or_default(),
                name: params.get(1).map(extract_string).unwrap_or_default(),
                relating_product: params.get(3).and_then(extract_ref).unwrap_or(0),
                related_product: params.get(4).and_then(extract_ref).unwrap_or(0),
            })
        }
        "ITEM_DEFINED_TRANSFORMATION" => {
            StepEntity::ItemDefinedTransformation(ItemDefinedTransformation {
                id,
                name: params.first().map(extract_string).unwrap_or_default(),
                transform_item_1: params.get(2).and_then(extract_ref).unwrap_or(0),
                transform_item_2: params.get(3).and_then(extract_ref).unwrap_or(0),
            })
        }
        "CONTEXT_DEPENDENT_SHAPE_REPRESENTATION" => {
            StepEntity::ContextDependentShapeRepresentation(ContextDependentShapeRepresentation {
                id,
                representation_relation: params.first().and_then(extract_ref).unwrap_or(0),
                represented_product_relation: params.get(1).and_then(extract_ref).unwrap_or(0),
            })
        }
        // Complex entity: REPRESENTATION_RELATIONSHIP combined with
        // REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION + SHAPE_REPRESENTATION_RELATIONSHIP
        // Flattened params: (name, desc, rep1, rep2, transform_ref)
        "REPRESENTATION_RELATIONSHIP" => {
            // Check if this has a transform reference (5th param from complex entity)
            let transform_op = params.get(4).and_then(extract_ref).unwrap_or(0);
            if transform_op != 0 {
                StepEntity::RepresentationRelationshipWithTransformation(
                    RepresentationRelationshipWithTransformation {
                        id,
                        name: params.first().map(extract_string).unwrap_or_default(),
                        rep1: params.get(2).and_then(extract_ref).unwrap_or(0),
                        rep2: params.get(3).and_then(extract_ref).unwrap_or(0),
                        transformation_operator: transform_op,
                    },
                )
            } else {
                StepEntity::Unknown {
                    type_name: "REPRESENTATION_RELATIONSHIP".to_string(),
                    id,
                }
            }
        }

        _ => StepEntity::Unknown {
            type_name: instance.type_name.clone(),
            id,
        },
    }
}

/// Build an entity map from raw instances.
pub fn build_entity_map(instances: &[EntityInstance]) -> HashMap<u64, StepEntity> {
    instances.iter().map(|inst| (inst.id, convert_entity(inst))).collect()
}

/// Entity map wrapper with helper methods.
#[derive(Debug)]
pub struct EntityGraph {
    pub entities: HashMap<u64, StepEntity>,
    pub raw: HashMap<u64, EntityInstance>,
}

impl EntityGraph {
    /// Build an entity graph from raw instances.
    pub fn new(instances: &[EntityInstance]) -> Self {
        let raw: HashMap<u64, EntityInstance> = instances
            .iter()
            .map(|inst| (inst.id, inst.clone()))
            .collect();
        let entities = build_entity_map(instances);
        Self { entities, raw }
    }

    /// Get an entity by ID.
    pub fn get(&self, id: u64) -> Option<&StepEntity> {
        self.entities.get(&id)
    }

    /// Get a cartesian point by ID.
    pub fn get_point(&self, id: u64) -> Option<Vec3> {
        match self.get(id)? {
            StepEntity::CartesianPoint(p) => Some(p.coords),
            _ => None,
        }
    }

    /// Get a direction by ID.
    pub fn get_direction(&self, id: u64) -> Option<Vec3> {
        match self.get(id)? {
            StepEntity::Direction(d) => Some(d.ratios.normalize_or_zero()),
            _ => None,
        }
    }

    /// Get vertex point coordinates.
    pub fn get_vertex_coords(&self, id: u64) -> Option<Vec3> {
        match self.get(id)? {
            StepEntity::VertexPoint(v) => self.get_point(v.point),
            _ => None,
        }
    }

    /// Find all entities of a given type.
    pub fn find_by_type(&self, type_name: &str) -> Vec<u64> {
        self.raw
            .iter()
            .filter(|(_, inst)| inst.type_name == type_name)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Find shape representations.
    pub fn find_shape_representations(&self) -> Vec<u64> {
        let mut reps = Vec::new();
        for (id, entity) in &self.entities {
            match entity {
                StepEntity::ShapeRepresentation(_)
                | StepEntity::AdvancedBrepShapeRepresentation(_)
                | StepEntity::GeometricallyBoundedSurfaceShapeRepresentation(_)
                | StepEntity::FacetedBrepShapeRepresentation(_) => {
                    reps.push(*id);
                }
                _ => {}
            }
        }
        reps
    }

    /// Find manifold solid breps.
    pub fn find_solids(&self) -> Vec<u64> {
        let mut solids = Vec::new();
        for (id, entity) in &self.entities {
            match entity {
                StepEntity::ManifoldSolidBrep(_) | StepEntity::FacetedBrep(_) => {
                    solids.push(*id);
                }
                _ => {}
            }
        }
        solids
    }

    /// Find closed/open shells.
    pub fn find_shells(&self) -> Vec<u64> {
        let mut shells = Vec::new();
        for (id, entity) in &self.entities {
            match entity {
                StepEntity::ClosedShell(_) | StepEntity::OpenShell(_) => {
                    shells.push(*id);
                }
                _ => {}
            }
        }
        shells
    }

    /// Find all NEXT_ASSEMBLY_USAGE_OCCURRENCE entities.
    pub fn find_assembly_occurrences(&self) -> Vec<&NextAssemblyUsageOccurrence> {
        let mut nauos = Vec::new();
        for entity in self.entities.values() {
            if let StepEntity::NextAssemblyUsageOccurrence(nauo) = entity {
                nauos.push(nauo);
            }
        }
        nauos
    }

    /// Find ItemDefinedTransformation by ID.
    pub fn get_item_defined_transformation(&self, id: u64) -> Option<&ItemDefinedTransformation> {
        match self.get(id)? {
            StepEntity::ItemDefinedTransformation(t) => Some(t),
            _ => None,
        }
    }

    /// Find RepresentationRelationshipWithTransformation entities.
    pub fn find_representation_relationships_with_transform(
        &self,
    ) -> Vec<&RepresentationRelationshipWithTransformation> {
        let mut rels = Vec::new();
        for entity in self.entities.values() {
            if let StepEntity::RepresentationRelationshipWithTransformation(rel) = entity {
                rels.push(rel);
            }
        }
        rels
    }

    /// Find ContextDependentShapeRepresentation entities.
    pub fn find_context_dependent_shape_representations(
        &self,
    ) -> Vec<&ContextDependentShapeRepresentation> {
        let mut cdsrs = Vec::new();
        for entity in self.entities.values() {
            if let StepEntity::ContextDependentShapeRepresentation(cdsr) = entity {
                cdsrs.push(cdsr);
            }
        }
        cdsrs
    }

    /// Compute a 4x4 transform matrix from an axis placement.
    /// Returns Mat4 where the Z axis is the placement axis, X is ref_direction.
    pub fn get_axis_placement_transform(&self, placement_id: u64) -> Option<glam::Mat4> {
        let placement = match self.get(placement_id)? {
            StepEntity::Axis2Placement3D(p) => p,
            _ => return None,
        };

        let origin = self.get_point(placement.location)?;
        let z_axis = placement.axis.and_then(|id| self.get_direction(id)).unwrap_or(Vec3::Z);
        let x_axis = placement
            .ref_direction
            .and_then(|id| self.get_direction(id))
            .unwrap_or_else(|| {
                // Compute X from Z using Gram-Schmidt
                let arbitrary = if z_axis.x.abs() < 0.9 {
                    Vec3::X
                } else {
                    Vec3::Y
                };
                (arbitrary - z_axis * z_axis.dot(arbitrary)).normalize()
            });
        let y_axis = z_axis.cross(x_axis).normalize();

        Some(glam::Mat4::from_cols(
            x_axis.extend(0.0),
            y_axis.extend(0.0),
            z_axis.extend(0.0),
            origin.extend(1.0),
        ))
    }

    /// Compute the relative transform from ItemDefinedTransformation.
    /// Returns the transform that maps from source to target coordinate system.
    pub fn get_relative_transform(&self, transform_id: u64) -> Option<glam::Mat4> {
        let transform = self.get_item_defined_transformation(transform_id)?;
        let source_mat = self.get_axis_placement_transform(transform.transform_item_1)?;
        let target_mat = self.get_axis_placement_transform(transform.transform_item_2)?;

        // Transform = target * source^(-1)
        // This maps points from source coordinate system to target
        Some(target_mat * source_mat.inverse())
    }
}
