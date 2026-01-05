//! Abstract Syntax Tree types for Seed documents.

use crate::types::{Color, Length, Identifier, Gradient, Shadow, Transform};
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
    Svg(SvgElement),
    Image(ImageElement),
    Icon(IconElement),
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

/// An SVG element for vector graphics and icons.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SvgElement {
    pub name: Option<Identifier>,
    /// SVG path data (d attribute) or paths list
    pub paths: Vec<SvgPath>,
    /// ViewBox dimensions (minX, minY, width, height)
    pub view_box: Option<SvgViewBox>,
    /// Properties like fill, stroke, width, height, x, y
    pub properties: Vec<Property>,
    pub constraints: Vec<Constraint>,
    pub span: Span,
}

/// ViewBox for SVG coordinate system.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SvgViewBox {
    pub min_x: f64,
    pub min_y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for SvgViewBox {
    fn default() -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            width: 24.0,
            height: 24.0,
        }
    }
}

/// A single SVG path with its own fill/stroke.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SvgPath {
    /// Path commands
    pub commands: Vec<SvgPathCommand>,
    /// Optional fill color for this path
    pub fill: Option<crate::types::Color>,
    /// Optional stroke color for this path
    pub stroke: Option<crate::types::Color>,
    /// Optional stroke width for this path
    pub stroke_width: Option<f64>,
    /// Fill rule: nonzero or evenodd
    pub fill_rule: SvgFillRule,
}

/// SVG fill rule for determining inside/outside of paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SvgFillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// SVG path commands (subset of SVG path spec).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SvgPathCommand {
    /// M x,y - Move to absolute position
    MoveTo { x: f64, y: f64 },
    /// m dx,dy - Move to relative position
    MoveToRel { dx: f64, dy: f64 },
    /// L x,y - Line to absolute position
    LineTo { x: f64, y: f64 },
    /// l dx,dy - Line to relative position
    LineToRel { dx: f64, dy: f64 },
    /// H x - Horizontal line to absolute x
    HorizontalTo { x: f64 },
    /// h dx - Horizontal line relative
    HorizontalToRel { dx: f64 },
    /// V y - Vertical line to absolute y
    VerticalTo { y: f64 },
    /// v dy - Vertical line relative
    VerticalToRel { dy: f64 },
    /// C x1,y1 x2,y2 x,y - Cubic bezier absolute
    CubicTo { x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 },
    /// c dx1,dy1 dx2,dy2 dx,dy - Cubic bezier relative
    CubicToRel { dx1: f64, dy1: f64, dx2: f64, dy2: f64, dx: f64, dy: f64 },
    /// S x2,y2 x,y - Smooth cubic bezier absolute
    SmoothCubicTo { x2: f64, y2: f64, x: f64, y: f64 },
    /// s dx2,dy2 dx,dy - Smooth cubic bezier relative
    SmoothCubicToRel { dx2: f64, dy2: f64, dx: f64, dy: f64 },
    /// Q x1,y1 x,y - Quadratic bezier absolute
    QuadTo { x1: f64, y1: f64, x: f64, y: f64 },
    /// q dx1,dy1 dx,dy - Quadratic bezier relative
    QuadToRel { dx1: f64, dy1: f64, dx: f64, dy: f64 },
    /// T x,y - Smooth quadratic bezier absolute
    SmoothQuadTo { x: f64, y: f64 },
    /// t dx,dy - Smooth quadratic bezier relative
    SmoothQuadToRel { dx: f64, dy: f64 },
    /// A rx,ry x-axis-rotation large-arc-flag sweep-flag x,y - Arc absolute
    ArcTo {
        rx: f64,
        ry: f64,
        x_rotation: f64,
        large_arc: bool,
        sweep: bool,
        x: f64,
        y: f64,
    },
    /// a rx,ry x-axis-rotation large-arc-flag sweep-flag dx,dy - Arc relative
    ArcToRel {
        rx: f64,
        ry: f64,
        x_rotation: f64,
        large_arc: bool,
        sweep: bool,
        dx: f64,
        dy: f64,
    },
    /// Z/z - Close path
    ClosePath,
}

/// An Image element for raster images.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageElement {
    pub name: Option<Identifier>,
    /// Image source (URL, file path, or embedded data)
    pub source: ImageSource,
    /// How the image should fit within its bounds
    pub fit: ImageFit,
    /// Alt text for accessibility
    pub alt: Option<String>,
    /// Properties like width, height, opacity
    pub properties: Vec<Property>,
    pub constraints: Vec<Constraint>,
    pub span: Span,
}

/// Image source types.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImageSource {
    /// URL (http/https)
    Url(String),
    /// File path (relative or absolute)
    File(String),
    /// Base64-encoded image data with MIME type
    Data { mime_type: String, data: String },
    /// Token reference to an image asset
    TokenRef(TokenPath),
}

/// How an image should fit within its bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ImageFit {
    /// Scale to fill, cropping if necessary (CSS: object-fit: cover)
    #[default]
    Cover,
    /// Scale to fit entirely within bounds (CSS: object-fit: contain)
    Contain,
    /// Stretch to fill bounds exactly (CSS: object-fit: fill)
    Fill,
    /// No scaling, display at natural size (CSS: object-fit: none)
    None,
    /// Scale down only if larger than bounds
    ScaleDown,
}

/// An Icon element for vector icons.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IconElement {
    pub name: Option<Identifier>,
    /// Icon identifier or source
    pub icon: IconSource,
    /// Icon size (width and height are equal)
    pub size: Option<Length>,
    /// Icon color (overrides the icon's native colors)
    pub color: Option<crate::types::Color>,
    /// Properties like opacity, transform
    pub properties: Vec<Property>,
    pub constraints: Vec<Constraint>,
    pub span: Span,
}

/// Icon source types.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IconSource {
    /// Named icon from a library (e.g., "lucide:home", "material:settings")
    Named { library: Option<String>, name: String },
    /// Inline SVG path data
    Svg(Vec<SvgPath>),
    /// Token reference to an icon
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
    Gradient(Gradient),
    Shadow(Shadow),
    Transform(Transform),
    Length(Length),
    Number(f64),
    String(String),
    Boolean(bool),
    TokenRef(TokenPath),
    Enum(String),
    /// Reference to a component prop (used in templates)
    PropRef(PropRef),
    /// Grid track sizes (for columns/rows)
    GridTracks(Vec<GridTrackSize>),
    /// Grid line reference (e.g., "1 / 3" or "1 / -1")
    GridLine(GridLineValue),
}

/// Grid track size definition (for CSS Grid-like layouts).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GridTrackSize {
    /// Fixed size in pixels
    Fixed(f64),
    /// Fraction of available space (fr units)
    Fraction(f64),
    /// Content-based sizing
    Auto,
    /// Minimum content size
    MinContent,
    /// Maximum content size
    MaxContent,
    /// Bounded size: minmax(min, max)
    MinMax { min: Box<GridTrackSize>, max: Box<GridTrackSize> },
    /// Repeat pattern: repeat(count, size) or repeat(auto-fill, size)
    Repeat { count: RepeatCount, sizes: Vec<GridTrackSize> },
}

/// Repeat count for grid tracks.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RepeatCount {
    /// Fixed number of repetitions
    Count(u32),
    /// Auto-fill: fill container with as many tracks as fit
    AutoFill,
    /// Auto-fit: like auto-fill but collapses empty tracks
    AutoFit,
}

/// Grid line value for placement (grid-column, grid-row).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GridLineValue {
    /// Start line (1-indexed, negative counts from end)
    pub start: GridLine,
    /// End line (optional, defaults to span 1)
    pub end: Option<GridLine>,
}

/// A grid line reference.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GridLine {
    /// Line number (1-indexed, negative from end)
    Number(i32),
    /// Span a number of tracks
    Span(u32),
    /// Named line
    Named(String),
    /// Auto placement
    Auto,
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
