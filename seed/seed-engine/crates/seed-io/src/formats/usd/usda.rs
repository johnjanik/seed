//! USDA (USD ASCII) parser.
//!
//! Implements a pure Rust tokenizer and parser for USD ASCII format.
//! Some types and methods are defined for spec completeness.

#![allow(dead_code)]

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::{char, digit1, multispace0, one_of},
    combinator::{map, opt, recognize, value},
    multi::separated_list0,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

/// A USD prim (primitive).
#[derive(Debug, Clone)]
pub struct UsdPrim {
    /// Prim type (e.g., "Xform", "Mesh", "Material")
    pub type_name: String,
    /// Prim path (e.g., "/World/MyMesh")
    pub path: String,
    /// Prim specifier (def, over, class)
    pub specifier: PrimSpecifier,
    /// Attributes
    pub attributes: Vec<UsdAttribute>,
    /// Child prims
    pub children: Vec<UsdPrim>,
    /// Metadata
    pub metadata: Vec<(String, UsdValue)>,
    /// References
    pub references: Vec<String>,
    /// Inherits
    pub inherits: Vec<String>,
    /// Variants
    pub variants: Vec<(String, String)>,
}

impl Default for UsdPrim {
    fn default() -> Self {
        Self {
            type_name: String::new(),
            path: String::new(),
            specifier: PrimSpecifier::Def,
            attributes: Vec::new(),
            children: Vec::new(),
            metadata: Vec::new(),
            references: Vec::new(),
            inherits: Vec::new(),
            variants: Vec::new(),
        }
    }
}

/// Prim specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrimSpecifier {
    #[default]
    Def,
    Over,
    Class,
}

/// A USD attribute.
#[derive(Debug, Clone)]
pub struct UsdAttribute {
    /// Attribute name
    pub name: String,
    /// Attribute type
    pub type_name: String,
    /// Default value
    pub default: Option<UsdValue>,
    /// Time samples
    pub time_samples: Vec<(f64, UsdValue)>,
    /// Variability (uniform vs varying)
    pub variability: Variability,
    /// Custom attribute marker
    pub custom: bool,
}

impl Default for UsdAttribute {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_name: String::new(),
            default: None,
            time_samples: Vec::new(),
            variability: Variability::Varying,
            custom: false,
        }
    }
}

/// Attribute variability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variability {
    #[default]
    Varying,
    Uniform,
}

/// A USD value.
#[derive(Debug, Clone)]
pub enum UsdValue {
    Bool(bool),
    Int(i64),
    Float(f32),
    Double(f64),
    String(String),
    Token(String),
    Asset(String),
    Vec2f([f32; 2]),
    Vec3f([f32; 3]),
    Vec4f([f32; 4]),
    Vec2d([f64; 2]),
    Vec3d([f64; 3]),
    Vec4d([f64; 4]),
    Quath([f32; 4]),
    Quatf([f32; 4]),
    Quatd([f64; 4]),
    Matrix4d([[f64; 4]; 4]),
    Color3f([f32; 3]),
    Color4f([f32; 4]),
    Normal3f([f32; 3]),
    Point3f([f32; 3]),
    TexCoord2f([f32; 2]),
    Array(Vec<UsdValue>),
    Dictionary(Vec<(String, UsdValue)>),
    Reference(String),
    None,
}

impl UsdValue {
    /// Try to extract as Vec3f
    pub fn as_vec3f(&self) -> Option<[f32; 3]> {
        match self {
            UsdValue::Vec3f(v) | UsdValue::Color3f(v) | UsdValue::Normal3f(v) | UsdValue::Point3f(v) => Some(*v),
            UsdValue::Vec3d(v) => Some([v[0] as f32, v[1] as f32, v[2] as f32]),
            _ => None,
        }
    }

    /// Try to extract as Vec2f
    pub fn as_vec2f(&self) -> Option<[f32; 2]> {
        match self {
            UsdValue::Vec2f(v) | UsdValue::TexCoord2f(v) => Some(*v),
            UsdValue::Vec2d(v) => Some([v[0] as f32, v[1] as f32]),
            _ => None,
        }
    }

    /// Try to extract as f32
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            UsdValue::Float(v) => Some(*v),
            UsdValue::Double(v) => Some(*v as f32),
            UsdValue::Int(v) => Some(*v as f32),
            _ => None,
        }
    }

    /// Try to extract as f64
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            UsdValue::Double(v) => Some(*v),
            UsdValue::Float(v) => Some(*v as f64),
            UsdValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Try to extract as String
    pub fn as_string(&self) -> Option<&str> {
        match self {
            UsdValue::String(s) | UsdValue::Token(s) | UsdValue::Asset(s) => Some(s),
            _ => None,
        }
    }

    /// Try to extract as array
    pub fn as_array(&self) -> Option<&[UsdValue]> {
        match self {
            UsdValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Try to extract as i64
    pub fn as_int(&self) -> Option<i64> {
        match self {
            UsdValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Try to extract as bool
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            UsdValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Extract 4x4 matrix
    pub fn as_matrix4d(&self) -> Option<[[f64; 4]; 4]> {
        match self {
            UsdValue::Matrix4d(m) => Some(*m),
            _ => None,
        }
    }
}

/// USD Stage (root document).
#[derive(Debug, Clone, Default)]
pub struct UsdStage {
    /// Stage metadata
    pub metadata: Vec<(String, UsdValue)>,
    /// Root prims
    pub root_prims: Vec<UsdPrim>,
    /// Default prim name
    pub default_prim: Option<String>,
    /// Up axis
    pub up_axis: Option<String>,
    /// Meters per unit
    pub meters_per_unit: Option<f64>,
}

/// Parse whitespace and comments.
fn ws(input: &str) -> IResult<&str, ()> {
    let (mut input, _) = multispace0(input)?;

    loop {
        // Try to parse a comment
        if input.starts_with('#') {
            if let Some(end) = input.find('\n') {
                input = &input[end + 1..];
                let (rest, _) = multispace0(input)?;
                input = rest;
                continue;
            } else {
                // Comment extends to end of input
                return Ok(("", ()));
            }
        }

        if input.starts_with("/*") {
            if let Some(end) = input.find("*/") {
                input = &input[end + 2..];
                let (rest, _) = multispace0(input)?;
                input = rest;
                continue;
            }
        }

        break;
    }

    Ok((input, ()))
}

/// Parse an identifier.
fn identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)
}

/// Parse a namespaced identifier (e.g., "primvars:st").
fn namespaced_identifier(input: &str) -> IResult<&str, String> {
    let (input, parts) = separated_list0(char(':'), identifier)(input)?;
    Ok((input, parts.join(":")))
}

/// Parse a double-quoted string literal.
fn double_quoted_string(input: &str) -> IResult<&str, String> {
    let (input, _) = char('"')(input)?;
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    let mut consumed = 0;

    while let Some(c) = chars.next() {
        consumed += c.len_utf8();
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                chars.next();
                consumed += next.len_utf8();
                match next {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    'r' => result.push('\r'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    _ => {
                        result.push('\\');
                        result.push(next);
                    }
                }
            }
        } else if c == '"' {
            return Ok((&input[consumed..], result));
        } else {
            result.push(c);
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))
}

/// Parse a single-quoted string (token).
fn single_quoted_string(input: &str) -> IResult<&str, String> {
    delimited(
        char('\''),
        map(take_while(|c| c != '\''), String::from),
        char('\''),
    )(input)
}

/// Parse an asset path @...@.
fn asset_path(input: &str) -> IResult<&str, String> {
    delimited(
        char('@'),
        map(take_while(|c| c != '@'), String::from),
        char('@'),
    )(input)
}

/// Parse a string literal (double or single quoted).
fn string_literal(input: &str) -> IResult<&str, String> {
    alt((double_quoted_string, single_quoted_string))(input)
}

/// Parse a signed number (int or float).
fn number(input: &str) -> IResult<&str, UsdValue> {
    let (input, num_str) = recognize(tuple((
        opt(one_of("+-")),
        alt((
            // Handle special float formats
            recognize(tuple((tag("inf"), opt(tag("inity"))))),
            recognize(tag("nan")),
            // Normal numbers
            recognize(tuple((
                digit1,
                opt(pair(char('.'), opt(digit1))),
                opt(tuple((one_of("eE"), opt(one_of("+-")), digit1))),
            ))),
        )),
    )))(input)?;

    let num_lower = num_str.to_lowercase();
    if num_lower.contains("inf") {
        let sign = if num_str.starts_with('-') { -1.0 } else { 1.0 };
        return Ok((input, UsdValue::Double(f64::INFINITY * sign)));
    }
    if num_lower.contains("nan") {
        return Ok((input, UsdValue::Double(f64::NAN)));
    }

    if num_str.contains('.') || num_str.contains('e') || num_str.contains('E') {
        Ok((input, UsdValue::Double(num_str.parse().unwrap_or(0.0))))
    } else {
        Ok((input, UsdValue::Int(num_str.parse().unwrap_or(0))))
    }
}

/// Parse a boolean.
fn boolean(input: &str) -> IResult<&str, UsdValue> {
    alt((
        value(UsdValue::Bool(true), tag("true")),
        value(UsdValue::Bool(false), tag("false")),
        value(UsdValue::Bool(true), tag("True")),
        value(UsdValue::Bool(false), tag("False")),
    ))(input)
}

/// Parse a prim specifier.
fn prim_specifier(input: &str) -> IResult<&str, PrimSpecifier> {
    alt((
        value(PrimSpecifier::Def, tag("def")),
        value(PrimSpecifier::Over, tag("over")),
        value(PrimSpecifier::Class, tag("class")),
    ))(input)
}

/// Parse None value.
fn none_value(input: &str) -> IResult<&str, UsdValue> {
    value(UsdValue::None, tag("None"))(input)
}

/// Parse a tuple (for vectors, colors, etc.).
fn tuple_value(input: &str) -> IResult<&str, Vec<UsdValue>> {
    delimited(
        pair(char('('), ws),
        separated_list0(tuple((ws, char(','), ws)), usd_value),
        pair(ws, char(')')),
    )(input)
}

/// Parse an array [...].
fn array_value(input: &str) -> IResult<&str, UsdValue> {
    let (input, _) = char('[')(input)?;
    let (input, _) = ws(input)?;

    // Handle empty array
    if input.starts_with(']') {
        let (input, _) = char(']')(input)?;
        return Ok((input, UsdValue::Array(Vec::new())));
    }

    let (input, values) = separated_list0(
        tuple((ws, char(','), ws)),
        usd_value,
    )(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = opt(char(','))(input)?; // trailing comma
    let (input, _) = ws(input)?;
    let (input, _) = char(']')(input)?;

    Ok((input, UsdValue::Array(values)))
}

/// Parse a dictionary {...}.
fn dictionary_value(input: &str) -> IResult<&str, UsdValue> {
    let (input, _) = char('{')(input)?;
    let (input, _) = ws(input)?;

    // Handle empty dict
    if input.starts_with('}') {
        let (input, _) = char('}')(input)?;
        return Ok((input, UsdValue::Dictionary(Vec::new())));
    }

    let mut entries = Vec::new();
    let mut input = input;

    loop {
        // Parse type (optional) and key
        let (rest, _) = ws(input)?;

        if rest.starts_with('}') {
            input = rest;
            break;
        }

        // Try to parse "type key = value" or just "key = value"
        let (rest, type_or_key) = identifier(rest)?;
        let (rest, _) = ws(rest)?;

        let (rest, key) = if rest.starts_with('=') {
            // No type, just key = value
            (rest, type_or_key.to_string())
        } else {
            // Type followed by key
            let (rest, key) = identifier(rest)?;
            (rest, key.to_string())
        };

        let (rest, _) = ws(rest)?;
        let (rest, _) = char('=')(rest)?;
        let (rest, _) = ws(rest)?;
        let (rest, val) = usd_value(rest)?;

        entries.push((key, val));

        let (rest, _) = ws(rest)?;

        // Check for comma or end
        if rest.starts_with(',') {
            let (rest, _) = char(',')(rest)?;
            input = rest;
        } else {
            input = rest;
            break;
        }
    }

    let (input, _) = ws(input)?;
    let (input, _) = char('}')(input)?;

    Ok((input, UsdValue::Dictionary(entries)))
}

/// Parse a USD value.
pub fn usd_value(input: &str) -> IResult<&str, UsdValue> {
    let (input, _) = ws(input)?;
    alt((
        none_value,
        boolean,
        map(asset_path, UsdValue::Asset),
        map(double_quoted_string, UsdValue::String),
        map(single_quoted_string, UsdValue::Token),
        dictionary_value,
        array_value,
        // Tuple becomes array
        map(tuple_value, |vals| {
            // Convert tuple to appropriate type based on element count
            UsdValue::Array(vals)
        }),
        number,
    ))(input)
}

/// Parse a typed value (e.g., "float3 faceVertexCounts").
fn typed_value(input: &str) -> IResult<&str, (String, UsdValue)> {
    let (input, type_name) = type_name(input)?;
    let (input, _) = ws(input)?;
    let (input, _) = char('=')(input)?;
    let (input, _) = ws(input)?;
    let (input, val) = usd_value(input)?;
    Ok((input, (type_name, val)))
}

/// Parse a type name (e.g., "float3[]", "token", "matrix4d").
fn type_name(input: &str) -> IResult<&str, String> {
    let (input, base) = identifier(input)?;
    let (input, array_marker) = opt(tag("[]"))(input)?;

    let mut result = base.to_string();
    if array_marker.is_some() {
        result.push_str("[]");
    }

    Ok((input, result))
}

/// Parse the metadata block of a prim or stage.
fn metadata_block(input: &str) -> IResult<&str, Vec<(String, UsdValue)>> {
    let (input, _) = char('(')(input)?;
    let (input, _) = ws(input)?;

    let mut metadata = Vec::new();
    let mut input = input;

    loop {
        let (rest, _) = ws(input)?;

        if rest.starts_with(')') {
            input = rest;
            break;
        }

        // Parse key = value
        let (rest, key) = namespaced_identifier(rest)?;
        let (rest, _) = ws(rest)?;
        let (rest, _) = char('=')(rest)?;
        let (rest, _) = ws(rest)?;
        let (rest, val) = usd_value(rest)?;

        metadata.push((key, val));
        input = rest;
    }

    let (input, _) = char(')')(input)?;

    Ok((input, metadata))
}

/// Parse an attribute definition.
fn attribute_def(input: &str) -> IResult<&str, UsdAttribute> {
    let (input, _) = ws(input)?;

    // Check for custom/uniform modifiers
    let (input, custom) = opt(preceded(tag("custom"), ws))(input)?;
    let (input, uniform) = opt(preceded(tag("uniform"), ws))(input)?;

    // Parse type name
    let (input, type_name) = type_name(input)?;
    let (input, _) = ws(input)?;

    // Parse attribute name
    let (input, name) = namespaced_identifier(input)?;
    let (input, _) = ws(input)?;

    let mut attr = UsdAttribute {
        name,
        type_name,
        custom: custom.is_some(),
        variability: if uniform.is_some() { Variability::Uniform } else { Variability::Varying },
        ..Default::default()
    };

    // Parse value assignment or time samples
    if input.starts_with('=') {
        let (input, _) = char('=')(input)?;
        let (input, _) = ws(input)?;
        let (input, val) = usd_value(input)?;
        attr.default = Some(val);
        return Ok((input, attr));
    }

    if input.starts_with(".timeSamples") {
        let (input, _) = tag(".timeSamples")(input)?;
        let (input, _) = ws(input)?;
        let (input, _) = char('=')(input)?;
        let (input, _) = ws(input)?;
        let (input, _) = char('{')(input)?;
        let (input, _) = ws(input)?;

        let mut samples = Vec::new();
        let mut input = input;

        loop {
            let (rest, _) = ws(input)?;

            if rest.starts_with('}') {
                input = rest;
                break;
            }

            // Parse "time: value,"
            let (rest, time_val) = number(rest)?;
            let time = match time_val {
                UsdValue::Double(t) => t,
                UsdValue::Int(t) => t as f64,
                _ => 0.0,
            };

            let (rest, _) = ws(rest)?;
            let (rest, _) = char(':')(rest)?;
            let (rest, _) = ws(rest)?;
            let (rest, val) = usd_value(rest)?;

            samples.push((time, val));

            let (rest, _) = ws(rest)?;
            let (rest, _) = opt(char(','))(rest)?;
            input = rest;
        }

        let (input, _) = char('}')(input)?;
        attr.time_samples = samples;
        return Ok((input, attr));
    }

    Ok((input, attr))
}

/// Parse a prim relationship (e.g., "rel material:binding = </Materials/Mat>").
fn relationship_def(input: &str) -> IResult<&str, (String, Vec<String>)> {
    let (input, _) = ws(input)?;
    let (input, _) = tag("rel")(input)?;
    let (input, _) = ws(input)?;
    let (input, name) = namespaced_identifier(input)?;
    let (input, _) = ws(input)?;

    if !input.starts_with('=') {
        return Ok((input, (name, Vec::new())));
    }

    let (input, _) = char('=')(input)?;
    let (input, _) = ws(input)?;

    // Parse target(s)
    let (input, targets) = alt((
        // Array of targets
        map(
            delimited(
                pair(char('['), ws),
                separated_list0(
                    tuple((ws, char(','), ws)),
                    delimited(char('<'), map(take_while(|c| c != '>'), String::from), char('>')),
                ),
                pair(ws, char(']')),
            ),
            |v| v,
        ),
        // Single target
        map(
            delimited(char('<'), map(take_while(|c| c != '>'), String::from), char('>')),
            |s| vec![s],
        ),
    ))(input)?;

    Ok((input, (name, targets)))
}

/// Parse a prim.
fn parse_prim<'a>(input: &'a str, parent_path: &str) -> IResult<&'a str, UsdPrim> {
    let (input, _) = ws(input)?;

    // Parse specifier
    let (input, specifier) = prim_specifier(input)?;
    let (input, _) = ws(input)?;

    // Parse optional type name
    let (input, type_name) = opt(identifier)(input)?;
    let (input, _) = ws(input)?;

    // Parse prim name (quoted)
    let (input, name) = string_literal(input)?;
    let (input, _) = ws(input)?;

    let path = if parent_path.is_empty() || parent_path == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", parent_path, name)
    };

    let mut prim = UsdPrim {
        type_name: type_name.unwrap_or("").to_string(),
        path: path.clone(),
        specifier,
        ..Default::default()
    };

    // Parse optional metadata
    if input.starts_with('(') {
        let (rest, metadata) = metadata_block(input)?;
        prim.metadata = metadata;
        let (rest, _) = ws(rest)?;

        // Extract special metadata
        for (key, val) in &prim.metadata {
            match key.as_str() {
                "references" => {
                    if let UsdValue::Array(arr) = val {
                        for item in arr {
                            if let UsdValue::Asset(path) = item {
                                prim.references.push(path.clone());
                            }
                        }
                    } else if let UsdValue::Asset(p) = val {
                        prim.references.push(p.clone());
                    }
                }
                "inherits" => {
                    if let UsdValue::Array(arr) = val {
                        for item in arr {
                            if let UsdValue::Reference(path) = item {
                                prim.inherits.push(path.clone());
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let (rest, _) = ws(rest)?;
        let (input, _) = char('{')(rest)?;
        let (input, _) = ws(input)?;
        return parse_prim_body(input, prim);
    }

    // Parse body
    let (input, _) = char('{')(input)?;
    let (input, _) = ws(input)?;

    parse_prim_body(input, prim)
}

/// Parse the body of a prim (attributes, children, etc.).
fn parse_prim_body(input: &str, mut prim: UsdPrim) -> IResult<&str, UsdPrim> {
    let mut input = input;
    let path = prim.path.clone();

    loop {
        let (rest, _) = ws(input)?;

        if rest.starts_with('}') {
            let (rest, _) = char('}')(rest)?;
            return Ok((rest, prim));
        }

        // Try to parse child prim
        if rest.starts_with("def ") || rest.starts_with("over ") || rest.starts_with("class ") {
            let (rest, child) = parse_prim(rest, &path)?;
            prim.children.push(child);
            input = rest;
            continue;
        }

        // Try to parse relationship
        if rest.starts_with("rel ") {
            let (rest, (_name, _targets)) = relationship_def(rest)?;
            // Store relationships if needed
            input = rest;
            continue;
        }

        // Try to parse variantSet
        if rest.starts_with("variantSet") {
            // Skip variant set definitions for now
            let (rest, _) = tag("variantSet")(rest)?;
            let (rest, _) = ws(rest)?;
            let (rest, _) = string_literal(rest)?;
            let (rest, _) = ws(rest)?;
            let (rest, _) = char('=')(rest)?;
            let (rest, _) = ws(rest)?;
            let (rest, _) = skip_braces(rest)?;
            input = rest;
            continue;
        }

        // Try to parse attribute
        let result = attribute_def(rest);
        if let Ok((rest, attr)) = result {
            prim.attributes.push(attr);
            input = rest;
            continue;
        }

        // Skip unknown line
        if let Some(end) = rest.find('\n') {
            input = &rest[end + 1..];
        } else {
            break;
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))
}

/// Skip balanced braces.
fn skip_braces(input: &str) -> IResult<&str, ()> {
    let (input, _) = char('{')(input)?;
    let mut depth = 1;
    let mut i = 0;
    let bytes = input.as_bytes();

    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'"' => {
                // Skip string
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    Ok((&input[i..], ()))
}

/// Parse a USDA stage.
pub fn parse_stage(input: &str) -> IResult<&str, UsdStage> {
    // Skip leading whitespace BUT NOT comments (since #usda looks like a comment)
    let (input, _) = multispace0(input)?;

    // Parse header "#usda version"
    let (input, _) = tag("#usda")(input)?;
    // Skip to end of line (version number etc)
    let input = if let Some(newline_pos) = input.find('\n') {
        &input[newline_pos + 1..]
    } else {
        input
    };
    let (input, _) = ws(input)?;

    let mut stage = UsdStage::default();

    // Parse stage metadata if present
    if input.starts_with('(') {
        let (rest, metadata) = metadata_block(input)?;

        // Extract stage-level metadata
        for (key, val) in &metadata {
            match key.as_str() {
                "defaultPrim" => {
                    if let UsdValue::String(s) = val {
                        stage.default_prim = Some(s.clone());
                    }
                }
                "upAxis" => {
                    if let UsdValue::String(s) | UsdValue::Token(s) = val {
                        stage.up_axis = Some(s.clone());
                    }
                }
                "metersPerUnit" => {
                    if let Some(v) = val.as_f64() {
                        stage.meters_per_unit = Some(v);
                    }
                }
                _ => {}
            }
        }

        stage.metadata = metadata;
        let (rest, _) = ws(rest)?;

        // Parse root prims
        let mut input = rest;
        loop {
            let (rest, _) = ws(input)?;

            if rest.is_empty() {
                break;
            }

            if rest.starts_with("def ") || rest.starts_with("over ") || rest.starts_with("class ") {
                let (rest, prim) = parse_prim(rest, "")?;
                stage.root_prims.push(prim);
                input = rest;
            } else {
                // Skip unknown content
                if let Some(end) = rest.find('\n') {
                    input = &rest[end + 1..];
                } else {
                    break;
                }
            }
        }

        return Ok(("", stage));
    }

    // Parse root prims without stage metadata
    let mut input = input;
    loop {
        let (rest, _) = ws(input)?;

        if rest.is_empty() {
            break;
        }

        if rest.starts_with("def ") || rest.starts_with("over ") || rest.starts_with("class ") {
            let (rest, prim) = parse_prim(rest, "")?;
            stage.root_prims.push(prim);
            input = rest;
        } else {
            // Skip unknown content
            if let Some(end) = rest.find('\n') {
                input = &rest[end + 1..];
            } else {
                break;
            }
        }
    }

    Ok(("", stage))
}

/// Helper to convert parsed array to typed vector.
pub fn array_to_vec3f_array(val: &UsdValue) -> Vec<[f32; 3]> {
    let mut result = Vec::new();

    if let UsdValue::Array(arr) = val {
        for item in arr {
            if let Some(v) = item.as_vec3f() {
                result.push(v);
            } else if let UsdValue::Array(inner) = item {
                // Handle nested array format
                if inner.len() == 3 {
                    let x = inner[0].as_f32().unwrap_or(0.0);
                    let y = inner[1].as_f32().unwrap_or(0.0);
                    let z = inner[2].as_f32().unwrap_or(0.0);
                    result.push([x, y, z]);
                }
            }
        }
    }

    result
}

/// Helper to convert parsed array to int array.
pub fn array_to_int_array(val: &UsdValue) -> Vec<i64> {
    let mut result = Vec::new();

    if let UsdValue::Array(arr) = val {
        for item in arr {
            if let Some(v) = item.as_int() {
                result.push(v);
            }
        }
    }

    result
}

/// Helper to convert parsed array to Vec2f array.
pub fn array_to_vec2f_array(val: &UsdValue) -> Vec<[f32; 2]> {
    let mut result = Vec::new();

    if let UsdValue::Array(arr) = val {
        for item in arr {
            if let Some(v) = item.as_vec2f() {
                result.push(v);
            } else if let UsdValue::Array(inner) = item {
                if inner.len() == 2 {
                    let x = inner[0].as_f32().unwrap_or(0.0);
                    let y = inner[1].as_f32().unwrap_or(0.0);
                    result.push([x, y]);
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifier() {
        assert_eq!(identifier("MyMesh"), Ok(("", "MyMesh")));
        assert_eq!(identifier("mesh_01"), Ok(("", "mesh_01")));
    }

    #[test]
    fn test_string_literal() {
        assert_eq!(double_quoted_string("\"hello\""), Ok(("", "hello".to_string())));
        assert_eq!(single_quoted_string("'token'"), Ok(("", "token".to_string())));
    }

    #[test]
    fn test_number() {
        assert!(matches!(number("42"), Ok(("", UsdValue::Int(42)))));
        assert!(matches!(number("3.14"), Ok(("", UsdValue::Double(_)))));
        assert!(matches!(number("-5"), Ok(("", UsdValue::Int(-5)))));
    }

    #[test]
    fn test_boolean() {
        assert!(matches!(boolean("true"), Ok(("", UsdValue::Bool(true)))));
        assert!(matches!(boolean("false"), Ok(("", UsdValue::Bool(false)))));
    }

    #[test]
    fn test_prim_specifier() {
        assert_eq!(prim_specifier("def"), Ok(("", PrimSpecifier::Def)));
        assert_eq!(prim_specifier("def "), Ok((" ", PrimSpecifier::Def)));
        assert_eq!(prim_specifier("def Xform"), Ok((" Xform", PrimSpecifier::Def)));
        assert_eq!(prim_specifier("over"), Ok(("", PrimSpecifier::Over)));
        assert_eq!(prim_specifier("class"), Ok(("", PrimSpecifier::Class)));
    }

    #[test]
    fn test_parse_single_prim() {
        let input = r#"def Xform "World"
{
}
"#;
        let result = parse_prim(input, "");
        assert!(result.is_ok(), "Parse single prim failed: {:?}", result);
    }

    #[test]
    fn test_header_parsing() {
        let input = "#usda 1.0\n\ndef Xform \"World\"\n{\n}\n";

        // Simulate parse_stage header handling
        // Use multispace0 instead of ws to avoid consuming #usda as a comment
        let (rest, _) = multispace0::<_, nom::error::Error<_>>(input).unwrap();
        println!("After multispace0: {:?}", &rest[..30.min(rest.len())]);

        let (rest, _) = tag::<_, _, nom::error::Error<_>>("#usda")(rest).unwrap();
        println!("After #usda: {:?}", &rest[..30.min(rest.len())]);

        let rest = if let Some(newline_pos) = rest.find('\n') {
            &rest[newline_pos + 1..]
        } else {
            rest
        };
        println!("After skip line: {:?}", &rest[..30.min(rest.len())]);

        let (rest, _) = ws(rest).unwrap();
        println!("After ws: {:?}", &rest[..30.min(rest.len())]);

        assert!(!rest.starts_with('('), "Should not start with (");
        assert!(rest.starts_with("def "), "Should start with 'def ': actual = {:?}", &rest[..10.min(rest.len())]);
    }

    #[test]
    fn test_array_value() {
        let (_, val) = array_value("[1, 2, 3]").unwrap();
        if let UsdValue::Array(arr) = val {
            assert_eq!(arr.len(), 3);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_asset_path() {
        assert_eq!(asset_path("@./texture.png@"), Ok(("", "./texture.png".to_string())));
    }

    #[test]
    fn test_parse_simple_stage() {
        // Test without metadata block first
        let input = r#"#usda 1.0

def Xform "World"
{
}
"#;

        let result = parse_stage(input);
        assert!(result.is_ok(), "Parse without metadata failed: {:?}", result);
        let (_, stage) = result.unwrap();
        assert_eq!(stage.root_prims.len(), 1);
        assert_eq!(stage.root_prims[0].type_name, "Xform");

        // Test with metadata block
        let input2 = r#"#usda 1.0
(
    defaultPrim = "World"
    upAxis = "Y"
)

def Xform "World"
{
    def Mesh "Cube"
    {
        float3[] points = [(0, 0, 0), (1, 0, 0), (1, 1, 0)]
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
    }
}
"#;

        let result = parse_stage(input2);
        assert!(result.is_ok(), "Parse with metadata failed: {:?}", result);
        let (_, stage) = result.unwrap();
        assert_eq!(stage.default_prim, Some("World".to_string()));
        assert_eq!(stage.up_axis, Some("Y".to_string()));
        assert_eq!(stage.root_prims.len(), 1);
        assert_eq!(stage.root_prims[0].type_name, "Xform");
        assert_eq!(stage.root_prims[0].children.len(), 1);
        assert_eq!(stage.root_prims[0].children[0].type_name, "Mesh");
    }

    #[test]
    fn test_parse_mesh_attributes() {
        let input = r#"#usda 1.0

def Mesh "Triangle"
{
    float3[] points = [(0, 0, 0), (1, 0, 0), (0.5, 1, 0)]
    float3[] normals = [(0, 0, 1), (0, 0, 1), (0, 0, 1)]
    int[] faceVertexCounts = [3]
    int[] faceVertexIndices = [0, 1, 2]
}
"#;

        let result = parse_stage(input);
        assert!(result.is_ok());
        let (_, stage) = result.unwrap();

        let mesh = &stage.root_prims[0];
        assert_eq!(mesh.type_name, "Mesh");

        let points_attr = mesh.attributes.iter().find(|a| a.name == "points");
        assert!(points_attr.is_some());
    }
}
