//! Abstract Syntax Tree types for Seed documents.

use crate::types::{Color, Length, Identifier};
use smallvec::SmallVec;

/// A complete Seed document.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Document {
    /// Document metadata
    pub meta: Option<MetaBlock>,
    /// Token definitions
    pub tokens: Option<TokenBlock>,
    /// Top-level elements
    pub elements: Vec<Element>,
    /// Source span for error reporting
    pub span: Span,
}

/// Document metadata block.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetaBlock {
    pub profile: Profile,
    pub version: Option<String>,
    pub span: Span,
}

/// Seed profile (2D or 3D).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Profile {
    Seed2D,
    Seed3D,
}

/// Token definitions block.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenBlock {
    pub definitions: Vec<TokenDefinition>,
    pub span: Span,
}

/// A single token definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenDefinition {
    pub path: TokenPath,
    pub value: TokenValue,
    pub span: Span,
}

/// A token path like `color.primary` or `spacing.md`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenPath(pub SmallVec<[String; 4]>);

/// A token value (can reference other tokens).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TokenValue {
    Color(Color),
    Length(Length),
    Number(f64),
    String(String),
    Reference(TokenPath),
}

/// An element in the Seed document.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Element {
    Frame(FrameElement),
    Text(TextElement),
    Part(PartElement),
    Component(ComponentElement),
    Slot(SlotElement),
}

/// A Frame element (2D container).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameElement {
    pub name: Option<Identifier>,
    pub properties: Vec<Property>,
    pub constraints: Vec<Constraint>,
    pub children: Vec<Element>,
    pub span: Span,
}

/// A Text element.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextElement {
    pub name: Option<Identifier>,
    pub content: TextContent,
    pub properties: Vec<Property>,
    pub constraints: Vec<Constraint>,
    pub span: Span,
}

/// Text content (literal or token reference).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextContent {
    Literal(String),
    TokenRef(TokenPath),
}

/// A Part element (3D geometry).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartElement {
    pub name: Option<Identifier>,
    pub geometry: Geometry,
    pub properties: Vec<Property>,
    pub constraints: Vec<Constraint>,
    pub span: Span,
}

/// 3D geometry definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Geometry {
    Primitive(Primitive),
    Csg(CsgOperation),
    // Future: Sketch, Extrude, Revolve, etc.
}

/// Primitive 3D shapes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Primitive {
    Box { width: Length, height: Length, depth: Length },
    Cylinder { radius: Length, height: Length },
    Sphere { radius: Length },
}

/// CSG (Constructive Solid Geometry) operations.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CsgOperation {
    Union(Vec<Geometry>),
    Difference { base: Box<Geometry>, subtract: Vec<Geometry> },
    Intersection(Vec<Geometry>),
}

/// A component instantiation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ComponentElement {
    pub component_name: Identifier,
    pub instance_name: Option<Identifier>,
    pub props: Vec<Property>,
    pub children: Vec<Element>,
    pub span: Span,
}

/// An element property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
    pub span: Span,
}

/// A property value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PropertyValue {
    Color(Color),
    Length(Length),
    Number(f64),
    String(String),
    Boolean(bool),
    TokenRef(TokenPath),
    Enum(String),
    /// Reference to a component prop (used in templates)
    PropRef(PropRef),
}

/// A constraint on an element.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Constraint {
    pub kind: ConstraintKind,
    pub priority: Option<ConstraintPriority>,
    pub span: Span,
}

/// Types of constraints.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConstraintKind {
    /// width = 100px
    Equality { property: String, value: Expression },
    /// center-x align Parent
    Alignment { edge: Edge, target: ElementRef, target_edge: Option<Edge> },
    /// below Header, gap: 24px
    Relative { relation: Relation, target: ElementRef, gap: Option<Length> },
    /// width >= 100px
    Inequality { property: String, op: InequalityOp, value: Expression },
}

/// Constraint priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConstraintPriority {
    Weak = 1,
    Low = 250,
    Medium = 500,
    High = 750,
    Required = 1000,
}

impl Default for ConstraintPriority {
    fn default() -> Self {
        Self::Required
    }
}

/// Edge of an element for alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
    CenterX,
    CenterY,
}

/// Spatial relation between elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Relation {
    Above,
    Below,
    LeftOf,
    RightOf,
}

/// Inequality operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InequalityOp {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// Reference to another element.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ElementRef {
    Parent,
    Named(Identifier),
    Previous,
    Next,
}

/// A constraint expression.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Expression {
    Literal(f64),
    Length(Length),
    PropertyRef { element: ElementRef, property: String },
    TokenRef(TokenPath),
    BinaryOp { left: Box<Expression>, op: BinaryOp, right: Box<Expression> },
    Function { name: String, args: Vec<Expression> },
}

/// Binary operators for expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Source span for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

/// A component definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ComponentDefinition {
    /// Name of the component
    pub name: Identifier,
    /// Declared props with optional defaults
    pub props: Vec<PropDefinition>,
    /// Named slots this component accepts
    pub slots: Vec<SlotDefinition>,
    /// The component's template (elements to render)
    pub template: Vec<Element>,
    /// Source span
    pub span: Span,
}

/// A prop definition in a component.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropDefinition {
    /// Prop name
    pub name: String,
    /// Expected type
    pub prop_type: PropType,
    /// Default value (if optional)
    pub default: Option<PropertyValue>,
    /// Whether this prop is required
    pub required: bool,
    /// Source span
    pub span: Span,
}

/// Type of a component prop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PropType {
    Color,
    Length,
    Number,
    String,
    Boolean,
    Any,
}

/// A slot definition in a component.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlotDefinition {
    /// Slot name (None for default slot)
    pub name: Option<String>,
    /// Source span
    pub span: Span,
}

/// A slot placeholder in a component template.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlotElement {
    /// Slot name (None for default slot)
    pub name: Option<String>,
    /// Fallback content if no children provided
    pub fallback: Vec<Element>,
    /// Source span
    pub span: Span,
}

/// A prop reference in a component template.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PropRef(pub String);
