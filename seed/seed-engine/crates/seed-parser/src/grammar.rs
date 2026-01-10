//! Grammar rules for parsing Seed documents.
//!
//! This parser uses indentation-based nesting (like Python/YAML).
//! Lines are first split and tagged with indent levels, then parsed.

use nom::{
    sequence::pair,
    IResult,
};

use seed_core::{
    ast::*,
    types::*,
    ParseError,
};
use std::f64::consts::PI;

use crate::lexer::*;

/// Parse a complete Seed document.
pub fn parse(input: &str) -> Result<Document, ParseError> {
    let lines = split_lines(input);
    let mut parser = Parser::new(&lines);
    parser.parse_document()
}

/// Stateful parser that tracks position in the line list.
struct Parser<'a> {
    lines: &'a [Line<'a>],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(lines: &'a [Line<'a>]) -> Self {
        Self { lines, pos: 0 }
    }

    /// Get current line, if any.
    fn current(&self) -> Option<&Line<'a>> {
        self.lines.get(self.pos)
    }

    /// Advance to next line.
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// Parse the full document.
    fn parse_document(&mut self) -> Result<Document, ParseError> {
        let mut elements = Vec::new();
        let mut meta = None;

        while let Some(line) = self.current() {
            let content = line.content;
            let base_indent = line.indent;

            // Parse @meta block
            if content.starts_with("@meta") {
                meta = Some(self.parse_meta_block()?);
                continue;
            }

            // Parse @tokens block (skip for now)
            if content.starts_with("@tokens") {
                self.skip_block(base_indent);
                continue;
            }

            // Parse element
            if let Some(elem) = self.parse_element(0)? {
                elements.push(elem);
            } else {
                // Skip unrecognized lines to prevent infinite loops
                self.advance();
            }
        }

        Ok(Document {
            meta,
            tokens: None,
            elements,
            span: Span::default(),
        })
    }

    /// Parse @meta block
    fn parse_meta_block(&mut self) -> Result<MetaBlock, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        self.advance();

        let mut profile = Profile::Seed2D;
        let mut version = None;

        // Parse properties inside @meta
        while let Some(line) = self.current() {
            if line.indent <= base_indent {
                break;
            }

            let content = line.content.trim();
            if let Some(val) = content.strip_prefix("profile:") {
                let val = val.trim();
                if val == "Seed/3D" {
                    profile = Profile::Seed3D;
                } else {
                    profile = Profile::Seed2D;
                }
            } else if let Some(val) = content.strip_prefix("version:") {
                version = Some(val.trim().to_string());
            }
            self.advance();
        }

        Ok(MetaBlock {
            profile,
            version,
            span: Span::default(),
        })
    }

    /// Skip a block at or deeper than the given indent level
    fn skip_block(&mut self, base_indent: usize) {
        self.advance(); // Skip the header line
        while let Some(line) = self.current() {
            if line.indent <= base_indent {
                break;
            }
            self.advance();
        }
    }

    /// Parse an element at the given minimum indentation level.
    fn parse_element(&mut self, min_indent: usize) -> Result<Option<Element>, ParseError> {
        let line = match self.current() {
            Some(l) if l.indent >= min_indent => l,
            _ => return Ok(None),
        };

        let content = line.content;

        // Try to parse as Frame
        if content.starts_with("Frame ") || content == "Frame:" {
            return self.parse_frame_element().map(|f| Some(Element::Frame(f)));
        }

        // Try to parse as Text
        if content.starts_with("Text ") || content == "Text:" {
            return self.parse_text_element().map(|t| Some(Element::Text(t)));
        }

        // Try to parse as Svg
        if content.starts_with("Svg ") || content == "Svg:" {
            return self.parse_svg_element().map(|s| Some(Element::Svg(s)));
        }

        // Try to parse as Image
        if content.starts_with("Image ") || content == "Image:" {
            return self.parse_image_element().map(|i| Some(Element::Image(i)));
        }

        // Try to parse as Icon
        if content.starts_with("Icon ") || content == "Icon:" {
            return self.parse_icon_element().map(|i| Some(Element::Icon(i)));
        }

        // Try to parse as Part (3D geometry)
        if content.starts_with("Part ") || content == "Part:" {
            return self.parse_part_element().map(|p| Some(Element::Part(p)));
        }

        Ok(None)
    }

    /// Parse a Frame element.
    fn parse_frame_element(&mut self) -> Result<FrameElement, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        let content = line.content;
        let line_num = line.line_number;

        // Parse "Frame Name:" or "Frame:"
        let name = parse_element_header(content, "Frame")
            .map_err(|_| ParseError::UnexpectedToken {
                found: content.to_string(),
                expected: "Frame element".to_string(),
                line: line_num as u32,
                column: 1,
            })?;

        self.advance();

        // Parse body (properties, constraints, children)
        let body = self.parse_element_body(base_indent)?;

        Ok(FrameElement {
            name: name.map(|s| Identifier(s.to_string())),
            properties: body.properties,
            constraints: body.constraints,
            children: body.children,
            span: Span {
                line: line_num as u32,
                ..Default::default()
            },
        })
    }

    /// Parse a Text element.
    fn parse_text_element(&mut self) -> Result<TextElement, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        let content = line.content;
        let line_num = line.line_number;

        let name = parse_element_header(content, "Text")
            .map_err(|_| ParseError::UnexpectedToken {
                found: content.to_string(),
                expected: "Text element".to_string(),
                line: line_num as u32,
                column: 1,
            })?;

        self.advance();

        let body = self.parse_element_body(base_indent)?;

        // Extract content from properties
        let text_content = body.properties.iter()
            .find(|p| p.name == "content")
            .and_then(|p| match &p.value {
                PropertyValue::String(s) => Some(TextContent::Literal(s.clone())),
                PropertyValue::TokenRef(path) => Some(TextContent::TokenRef(path.clone())),
                _ => None,
            })
            .unwrap_or(TextContent::Literal(String::new()));

        Ok(TextElement {
            name: name.map(|s| Identifier(s.to_string())),
            content: text_content,
            properties: body.properties,
            constraints: body.constraints,
            span: Span {
                line: line_num as u32,
                ..Default::default()
            },
        })
    }

    /// Parse an SVG element.
    fn parse_svg_element(&mut self) -> Result<SvgElement, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        let content = line.content;
        let line_num = line.line_number;

        let name = parse_element_header(content, "Svg")
            .map_err(|_| ParseError::UnexpectedToken {
                found: content.to_string(),
                expected: "Svg element".to_string(),
                line: line_num as u32,
                column: 1,
            })?;

        self.advance();

        // Parse SVG body manually to handle path children
        let mut paths = Vec::new();
        let mut view_box = None;
        let mut other_properties = Vec::new();
        let mut constraints = Vec::new();

        let child_indent = base_indent + 2;

        while let Some(line) = self.current() {
            if line.indent <= base_indent {
                break;
            }

            if line.indent < child_indent {
                break;
            }

            let content = line.content;
            let current_indent = line.indent;

            // Check for constraints block
            if content == "constraints:" {
                self.advance();
                constraints = self.parse_constraints_block(current_indent)?;
                continue;
            }

            // Try to parse as property
            if let Some(prop) = self.parse_property(content)? {
                match prop.name.as_str() {
                    "d" | "path" => {
                        if let PropertyValue::String(path_str) = &prop.value {
                            if let Ok(commands) = parse_svg_path_data(path_str) {
                                let path_indent = current_indent;
                                self.advance();

                                // Look for nested path properties
                                let mut fill = None;
                                let mut stroke = None;
                                let mut stroke_width = None;
                                let mut fill_rule = SvgFillRule::default();

                                while let Some(nested_line) = self.current() {
                                    if nested_line.indent <= path_indent {
                                        break;
                                    }

                                    if let Some(nested_prop) = self.parse_property(nested_line.content)? {
                                        match nested_prop.name.as_str() {
                                            "fill" => {
                                                if let PropertyValue::Color(c) = nested_prop.value {
                                                    fill = Some(c);
                                                }
                                            }
                                            "stroke" => {
                                                if let PropertyValue::Color(c) = nested_prop.value {
                                                    stroke = Some(c);
                                                }
                                            }
                                            "stroke-width" | "strokeWidth" => {
                                                if let PropertyValue::Length(l) = nested_prop.value {
                                                    stroke_width = l.to_px(None);
                                                } else if let PropertyValue::Number(n) = nested_prop.value {
                                                    stroke_width = Some(n);
                                                }
                                            }
                                            "fill-rule" | "fillRule" => {
                                                if let PropertyValue::String(s) = &nested_prop.value {
                                                    fill_rule = match s.as_str() {
                                                        "evenodd" | "even-odd" => SvgFillRule::EvenOdd,
                                                        _ => SvgFillRule::NonZero,
                                                    };
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    self.advance();
                                }

                                paths.push(SvgPath {
                                    commands,
                                    fill,
                                    stroke,
                                    stroke_width,
                                    fill_rule,
                                });
                                continue;
                            }
                        }
                    }
                    "viewBox" | "view-box" => {
                        if let PropertyValue::String(vb_str) = &prop.value {
                            if let Some(vb) = parse_viewbox(vb_str) {
                                view_box = Some(vb);
                            }
                        }
                    }
                    _ => other_properties.push(prop),
                }
                self.advance();
                continue;
            }

            // Unknown line, skip
            self.advance();
        }

        Ok(SvgElement {
            name: name.map(|s| Identifier(s.to_string())),
            paths,
            view_box,
            properties: other_properties,
            constraints,
            span: Span {
                line: line_num as u32,
                ..Default::default()
            },
        })
    }

    /// Parse an Image element.
    fn parse_image_element(&mut self) -> Result<ImageElement, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        let content = line.content;
        let line_num = line.line_number;

        let name = parse_element_header(content, "Image")
            .map_err(|_| ParseError::UnexpectedToken {
                found: content.to_string(),
                expected: "Image element".to_string(),
                line: line_num as u32,
                column: 1,
            })?;

        self.advance();

        let body = self.parse_element_body(base_indent)?;

        // Extract source from properties
        let source = body.properties.iter()
            .find(|p| p.name == "src" || p.name == "source")
            .and_then(|p| match &p.value {
                PropertyValue::String(s) => Some(parse_image_source(s)),
                PropertyValue::TokenRef(path) => Some(ImageSource::TokenRef(path.clone())),
                _ => None,
            })
            .unwrap_or(ImageSource::File(String::new()));

        // Extract fit mode from properties
        let fit = body.properties.iter()
            .find(|p| p.name == "fit" || p.name == "object-fit")
            .and_then(|p| match &p.value {
                PropertyValue::String(s) | PropertyValue::Enum(s) => Some(parse_image_fit(s)),
                _ => None,
            })
            .unwrap_or(ImageFit::Cover);

        // Extract alt text
        let alt = body.properties.iter()
            .find(|p| p.name == "alt")
            .and_then(|p| match &p.value {
                PropertyValue::String(s) => Some(s.clone()),
                _ => None,
            });

        Ok(ImageElement {
            name: name.map(|s| Identifier(s.to_string())),
            source,
            fit,
            alt,
            properties: body.properties,
            constraints: body.constraints,
            span: Span {
                line: line_num as u32,
                ..Default::default()
            },
        })
    }

    /// Parse an Icon element.
    fn parse_icon_element(&mut self) -> Result<IconElement, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        let content = line.content;
        let line_num = line.line_number;

        let name = parse_element_header(content, "Icon")
            .map_err(|_| ParseError::UnexpectedToken {
                found: content.to_string(),
                expected: "Icon element".to_string(),
                line: line_num as u32,
                column: 1,
            })?;

        self.advance();

        let body = self.parse_element_body(base_indent)?;

        // Extract icon source from properties
        let icon = body.properties.iter()
            .find(|p| p.name == "icon" || p.name == "name")
            .and_then(|p| match &p.value {
                PropertyValue::String(s) => Some(parse_icon_source(s)),
                PropertyValue::TokenRef(path) => Some(IconSource::TokenRef(path.clone())),
                _ => None,
            })
            .unwrap_or(IconSource::Named { library: None, name: String::new() });

        // Extract size
        let size = body.properties.iter()
            .find(|p| p.name == "size")
            .and_then(|p| match &p.value {
                PropertyValue::Length(l) => Some(l.clone()),
                PropertyValue::Number(n) => Some(Length { value: *n, unit: LengthUnit::Px }),
                _ => None,
            });

        // Extract color
        let color = body.properties.iter()
            .find(|p| p.name == "color")
            .and_then(|p| match &p.value {
                PropertyValue::Color(c) => Some(c.clone()),
                _ => None,
            });

        Ok(IconElement {
            name: name.map(|s| Identifier(s.to_string())),
            icon,
            size,
            color,
            properties: body.properties,
            constraints: body.constraints,
            span: Span {
                line: line_num as u32,
                ..Default::default()
            },
        })
    }

    /// Parse a Part element (3D geometry).
    fn parse_part_element(&mut self) -> Result<PartElement, ParseError> {
        let line = self.current().ok_or(ParseError::UnexpectedEof)?;
        let base_indent = line.indent;
        let content = line.content;
        let line_num = line.line_number;

        let name = parse_element_header(content, "Part")
            .map_err(|_| ParseError::UnexpectedToken {
                found: content.to_string(),
                expected: "Part element".to_string(),
                line: line_num as u32,
                column: 1,
            })?;

        self.advance();

        let body = self.parse_element_body(base_indent)?;

        // Extract geometry from properties (required)
        let geometry = body.properties.iter()
            .find(|p| p.name == "geometry")
            .and_then(|p| match &p.value {
                PropertyValue::String(s) => parse_geometry(s).ok(),
                _ => None,
            })
            .unwrap_or_else(|| {
                // Default to a unit box if no geometry specified
                Geometry::Primitive(Primitive::Box {
                    width: Length::mm(100.0),
                    height: Length::mm(100.0),
                    depth: Length::mm(100.0),
                })
            });

        // Filter out geometry property from other properties
        let other_properties: Vec<_> = body.properties.into_iter()
            .filter(|p| p.name != "geometry")
            .collect();

        Ok(PartElement {
            name: name.map(|s| Identifier(s.to_string())),
            geometry,
            properties: other_properties,
            constraints: body.constraints,
            span: Span {
                line: line_num as u32,
                ..Default::default()
            },
        })
    }

    /// Parse the body of an element (properties, constraints, children).
    fn parse_element_body(&mut self, parent_indent: usize) -> Result<ElementBody, ParseError> {
        let mut properties = Vec::new();
        let mut constraints = Vec::new();
        let mut children = Vec::new();

        let child_indent = parent_indent + 2; // Expect 2-space indentation

        while let Some(line) = self.current() {
            // Stop if we're back at parent level or less
            if line.indent <= parent_indent {
                break;
            }

            // Must be at child indent level
            if line.indent < child_indent {
                break;
            }

            let content = line.content;

            // Check for constraints block
            if content == "constraints:" {
                let constraint_parent_indent = line.indent;
                self.advance();
                constraints = self.parse_constraints_block(constraint_parent_indent)?;
                continue;
            }

            // Check for child element
            if content.starts_with("Frame ") || content.starts_with("Text ") || content.starts_with("Svg ")
               || content.starts_with("Image ") || content.starts_with("Icon ") || content.starts_with("Part ")
               || content == "Frame:" || content == "Text:" || content == "Svg:"
               || content == "Image:" || content == "Icon:" || content == "Part:" {
                if let Some(elem) = self.parse_element(child_indent)? {
                    children.push(elem);
                }
                continue;
            }

            // Try to parse as property
            if let Some(prop) = self.parse_property(content)? {
                properties.push(prop);
                self.advance();
                continue;
            }

            // Unknown line, skip
            self.advance();
        }

        Ok(ElementBody { properties, constraints, children })
    }

    /// Parse a constraints block.
    fn parse_constraints_block(&mut self, parent_indent: usize) -> Result<Vec<Constraint>, ParseError> {
        let mut constraints = Vec::new();
        let constraint_indent = parent_indent + 2;

        while let Some(line) = self.current() {
            if line.indent <= parent_indent {
                break;
            }

            if line.indent < constraint_indent {
                break;
            }

            let content = line.content;

            // Constraint lines start with "-"
            if let Some(constraint_text) = content.strip_prefix("- ").or_else(|| content.strip_prefix("-")) {
                if let Some(constraint) = parse_constraint(constraint_text.trim())? {
                    constraints.push(constraint);
                }
            }

            self.advance();
        }

        Ok(constraints)
    }

    /// Parse a property line.
    fn parse_property(&mut self, content: &str) -> Result<Option<Property>, ParseError> {
        // Skip constraint block header
        if content == "constraints:" {
            return Ok(None);
        }

        // Property format: "name: value"
        let Some(colon_pos) = content.find(':') else {
            return Ok(None);
        };

        let name = content[..colon_pos].trim();
        let value_str = content[colon_pos + 1..].trim();

        // Skip if it looks like an element header (ends with just ":")
        if value_str.is_empty() {
            return Ok(None);
        }

        let value = parse_property_value(value_str)?;

        Ok(Some(Property {
            name: name.to_string(),
            value,
            span: Span::default(),
        }))
    }
}

struct ElementBody {
    properties: Vec<Property>,
    constraints: Vec<Constraint>,
    children: Vec<Element>,
}

/// Parse element header like "Frame Name:" or "Frame:"
fn parse_element_header<'a>(content: &'a str, keyword: &str) -> Result<Option<&'a str>, ()> {
    let content = content.strip_suffix(':').ok_or(())?;

    if content == keyword {
        return Ok(None);
    }

    let rest = content.strip_prefix(keyword).ok_or(())?;
    let name = rest.trim();

    if name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(name))
    }
}

/// Parse a property value.
fn parse_property_value(input: &str) -> Result<PropertyValue, ParseError> {
    let input = input.trim();

    // Grid track definitions: [1fr, 2fr, auto] or repeat(3, 1fr)
    if input.starts_with('[') && input.ends_with(']') {
        if let Ok(tracks) = parse_grid_tracks(input) {
            return Ok(PropertyValue::GridTracks(tracks));
        }
    }
    if input.starts_with("repeat(") {
        if let Ok(tracks) = parse_grid_repeat(input) {
            return Ok(PropertyValue::GridTracks(tracks));
        }
    }

    // Grid line placement: "1 / 3" or "1 / -1" or "span 2"
    if input.contains(" / ") || input.starts_with("span ") {
        if let Ok(line) = parse_grid_line_value(input) {
            return Ok(PropertyValue::GridLine(line));
        }
    }

    // Transform functions
    if input.starts_with("rotate(") {
        return parse_rotate_transform(input).map(PropertyValue::Transform);
    }
    if input.starts_with("scale(") {
        return parse_scale_transform(input).map(PropertyValue::Transform);
    }
    if input.starts_with("translate(") {
        return parse_translate_transform(input).map(PropertyValue::Transform);
    }
    if input.starts_with("skew(") {
        return parse_skew_transform(input).map(PropertyValue::Transform);
    }
    if input.starts_with("matrix(") {
        return parse_matrix_transform(input).map(PropertyValue::Transform);
    }

    // Shadow functions
    if input.starts_with("drop-shadow(") {
        return parse_drop_shadow(input).map(PropertyValue::Shadow);
    }
    if input.starts_with("box-shadow(") {
        return parse_box_shadow(input).map(PropertyValue::Shadow);
    }
    if input.starts_with("inset-shadow(") {
        return parse_inset_shadow(input).map(PropertyValue::Shadow);
    }

    // Gradient functions
    if input.starts_with("linear-gradient(") {
        return parse_linear_gradient(input).map(|g| PropertyValue::Gradient(Gradient::Linear(g)));
    }
    if input.starts_with("radial-gradient(") {
        return parse_radial_gradient(input).map(|g| PropertyValue::Gradient(Gradient::Radial(g)));
    }
    if input.starts_with("conic-gradient(") {
        return parse_conic_gradient(input).map(|g| PropertyValue::Gradient(Gradient::Conic(g)));
    }

    // Hex color
    if let Some(hex) = input.strip_prefix('#') {
        if let Some(color) = Color::from_hex(hex) {
            return Ok(PropertyValue::Color(color));
        }
    }

    // Token reference
    if let Some(token_path) = input.strip_prefix('$') {
        let path = TokenPath(token_path.split('.').map(String::from).collect());
        return Ok(PropertyValue::TokenRef(path));
    }

    // String literal
    if input.starts_with('"') && input.ends_with('"') && input.len() >= 2 {
        let content = &input[1..input.len()-1];
        return Ok(PropertyValue::String(content.to_string()));
    }

    // Boolean
    if input == "true" {
        return Ok(PropertyValue::Boolean(true));
    }
    if input == "false" {
        return Ok(PropertyValue::Boolean(false));
    }

    // Number with unit
    if let Ok((rest, (num, unit_str))) = parse_number_with_unit(input) {
        if rest.is_empty() {
            return Ok(PropertyValue::Length(make_length(num, unit_str)));
        }
    }

    // Plain number
    if let Ok((rest, num)) = number(input) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            return Ok(PropertyValue::Number(num));
        }
    }

    // Enum/identifier
    if let Ok((rest, ident)) = identifier(input) {
        if rest.is_empty() {
            return Ok(PropertyValue::Enum(ident.to_string()));
        }
    }

    // Fallback: treat as string
    Ok(PropertyValue::String(input.to_string()))
}

/// Parse a linear gradient: linear-gradient(90deg, #ff0000, #0000ff)
fn parse_linear_gradient(input: &str) -> Result<LinearGradient, ParseError> {
    let inner = extract_function_args(input, "linear-gradient")?;
    let parts = split_gradient_args(&inner);

    if parts.is_empty() {
        return Err(ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "gradient stops".to_string(),
            line: 0,
            column: 0,
        });
    }

    // First part might be angle or direction
    let (angle, color_start) = parse_gradient_direction(&parts[0])
        .map(|a| (a, 1))
        .unwrap_or((270.0, 0)); // Default: top to bottom (270deg)

    let stops = parse_color_stops(&parts[color_start..])?;

    Ok(LinearGradient { angle, stops })
}

/// Parse a radial gradient: radial-gradient(circle, #ff0000, #0000ff)
fn parse_radial_gradient(input: &str) -> Result<RadialGradient, ParseError> {
    let inner = extract_function_args(input, "radial-gradient")?;
    let parts = split_gradient_args(&inner);

    if parts.is_empty() {
        return Err(ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "gradient stops".to_string(),
            line: 0,
            column: 0,
        });
    }

    // First part might be shape/position
    let (center_x, center_y, radius_x, radius_y, color_start) =
        if parts[0].trim() == "circle" || parts[0].trim() == "ellipse" {
            (0.5, 0.5, 1.0, 1.0, 1)
        } else if parts[0].contains("at ") {
            // Parse "circle at 50% 50%" syntax
            let (cx, cy) = parse_position(&parts[0])?;
            (cx, cy, 1.0, 1.0, 1)
        } else {
            (0.5, 0.5, 1.0, 1.0, 0)
        };

    let stops = parse_color_stops(&parts[color_start..])?;

    Ok(RadialGradient {
        center_x,
        center_y,
        radius_x,
        radius_y,
        stops,
    })
}

/// Parse a conic gradient: conic-gradient(from 0deg, #ff0000, #0000ff)
fn parse_conic_gradient(input: &str) -> Result<ConicGradient, ParseError> {
    let inner = extract_function_args(input, "conic-gradient")?;
    let parts = split_gradient_args(&inner);

    if parts.is_empty() {
        return Err(ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "gradient stops".to_string(),
            line: 0,
            column: 0,
        });
    }

    // First part might be "from Xdeg" or "from Xdeg at X% Y%"
    let (start_angle, center_x, center_y, color_start) =
        if parts[0].trim().starts_with("from ") {
            let angle_part = parts[0].trim().strip_prefix("from ").unwrap();
            if let Some(at_pos) = angle_part.find(" at ") {
                let angle_str = &angle_part[..at_pos];
                let pos_str = &angle_part[at_pos + 4..];
                let angle = parse_angle(angle_str).unwrap_or(0.0);
                let (cx, cy) = parse_simple_position(pos_str)?;
                (angle, cx, cy, 1)
            } else {
                let angle = parse_angle(angle_part).unwrap_or(0.0);
                (angle, 0.5, 0.5, 1)
            }
        } else {
            (0.0, 0.5, 0.5, 0)
        };

    let stops = parse_color_stops(&parts[color_start..])?;

    Ok(ConicGradient {
        center_x,
        center_y,
        start_angle,
        stops,
    })
}

/// Parse a drop shadow: drop-shadow(4px 4px 10px #000000)
/// Format: drop-shadow(offset-x offset-y blur-radius color)
fn parse_drop_shadow(input: &str) -> Result<Shadow, ParseError> {
    let inner = extract_function_args(input, "drop-shadow")?;
    parse_shadow_values(inner, false)
}

/// Parse a box shadow: box-shadow(4px 4px 10px 2px #000000)
/// Format: box-shadow(offset-x offset-y blur-radius spread-radius color)
fn parse_box_shadow(input: &str) -> Result<Shadow, ParseError> {
    let inner = extract_function_args(input, "box-shadow")?;
    parse_shadow_values(inner, false)
}

/// Parse an inset shadow: inset-shadow(4px 4px 10px #000000)
fn parse_inset_shadow(input: &str) -> Result<Shadow, ParseError> {
    let inner = extract_function_args(input, "inset-shadow")?;
    parse_shadow_values(inner, true)
}

/// Parse shadow values from the inner part of a shadow function.
fn parse_shadow_values(input: &str, inset: bool) -> Result<Shadow, ParseError> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.len() < 3 {
        return Err(ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "shadow values: offset-x offset-y blur [spread] color".to_string(),
            line: 0,
            column: 0,
        });
    }

    // Parse offset-x
    let offset_x = parse_length_value(parts[0]).ok_or_else(|| ParseError::UnexpectedToken {
        found: parts[0].to_string(),
        expected: "offset-x length".to_string(),
        line: 0,
        column: 0,
    })?;

    // Parse offset-y
    let offset_y = parse_length_value(parts[1]).ok_or_else(|| ParseError::UnexpectedToken {
        found: parts[1].to_string(),
        expected: "offset-y length".to_string(),
        line: 0,
        column: 0,
    })?;

    // Parse blur
    let blur = parse_length_value(parts[2]).ok_or_else(|| ParseError::UnexpectedToken {
        found: parts[2].to_string(),
        expected: "blur length".to_string(),
        line: 0,
        column: 0,
    })?;

    // Determine if we have spread (4 values before color) or no spread (3 values before color)
    let (spread, color_str) = if parts.len() >= 5 {
        // Check if parts[3] looks like a length (has a unit or is a number)
        if let Some(spread_val) = parse_length_value(parts[3]) {
            (spread_val, parts[4..].join(" "))
        } else {
            (0.0, parts[3..].join(" "))
        }
    } else if parts.len() == 4 {
        (0.0, parts[3].to_string())
    } else {
        (0.0, "#000000".to_string())
    };

    // Parse color
    let color = parse_shadow_color(&color_str).unwrap_or(Color::rgba(0.0, 0.0, 0.0, 0.5));

    Ok(Shadow {
        offset_x,
        offset_y,
        blur,
        spread,
        color,
        inset,
    })
}

/// Parse a length value and return pixels.
fn parse_length_value(input: &str) -> Option<f64> {
    let input = input.trim();
    if let Ok((_, (num, unit))) = parse_number_with_unit(input) {
        let length = make_length(num, unit);
        length.to_px(None)
    } else if let Ok((rest, num)) = number(input) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            Some(num)
        } else {
            None
        }
    } else {
        None
    }
}

/// Parse a color value (returns Option for shadow parsing).
fn parse_shadow_color(input: &str) -> Option<Color> {
    let input = input.trim();
    if let Some(hex) = input.strip_prefix('#') {
        Color::from_hex(hex)
    } else if input.starts_with("rgb(") || input.starts_with("rgba(") {
        parse_rgb_color(input)
    } else {
        // Named colors
        match input.to_lowercase().as_str() {
            "black" => Some(Color::BLACK),
            "white" => Some(Color::WHITE),
            "transparent" => Some(Color::TRANSPARENT),
            "red" => Some(Color::rgb(1.0, 0.0, 0.0)),
            "green" => Some(Color::rgb(0.0, 0.5, 0.0)),
            "blue" => Some(Color::rgb(0.0, 0.0, 1.0)),
            "yellow" => Some(Color::rgb(1.0, 1.0, 0.0)),
            "cyan" => Some(Color::rgb(0.0, 1.0, 1.0)),
            "magenta" => Some(Color::rgb(1.0, 0.0, 1.0)),
            "gray" | "grey" => Some(Color::rgb(0.5, 0.5, 0.5)),
            _ => None,
        }
    }
}

/// Parse grid track sizes: [1fr, 2fr, auto] or [100px, 1fr, minmax(100px, 1fr)]
fn parse_grid_tracks(input: &str) -> Result<Vec<GridTrackSize>, ParseError> {
    // Strip brackets
    let inner = input
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "grid track list [...]".to_string(),
            line: 0,
            column: 0,
        })?;

    let parts = split_grid_args(inner);
    let mut tracks = Vec::new();

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        tracks.push(parse_single_track(part)?);
    }

    Ok(tracks)
}

/// Parse repeat() syntax: repeat(3, 1fr) or repeat(auto-fill, minmax(200px, 1fr))
fn parse_grid_repeat(input: &str) -> Result<Vec<GridTrackSize>, ParseError> {
    let inner = extract_function_args(input, "repeat")?;
    let parts: Vec<&str> = split_grid_args(inner);

    if parts.len() < 2 {
        return Err(ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "repeat(count, track-size)".to_string(),
            line: 0,
            column: 0,
        });
    }

    let count_str = parts[0].trim();
    let count = match count_str {
        "auto-fill" => RepeatCount::AutoFill,
        "auto-fit" => RepeatCount::AutoFit,
        _ => {
            let n = count_str.parse::<u32>().map_err(|_| ParseError::UnexpectedToken {
                found: count_str.to_string(),
                expected: "repeat count (number or auto-fill/auto-fit)".to_string(),
                line: 0,
                column: 0,
            })?;
            RepeatCount::Count(n)
        }
    };

    // Parse the track sizes to repeat
    let mut sizes = Vec::new();
    for part in &parts[1..] {
        sizes.push(parse_single_track(part.trim())?);
    }

    Ok(vec![GridTrackSize::Repeat { count, sizes }])
}

/// Parse a single grid track size: 1fr, auto, 100px, minmax(100px, 1fr), etc.
fn parse_single_track(input: &str) -> Result<GridTrackSize, ParseError> {
    let input = input.trim();

    // Keywords
    match input {
        "auto" => return Ok(GridTrackSize::Auto),
        "min-content" => return Ok(GridTrackSize::MinContent),
        "max-content" => return Ok(GridTrackSize::MaxContent),
        _ => {}
    }

    // minmax(min, max)
    if input.starts_with("minmax(") {
        let inner = extract_function_args(input, "minmax")?;
        let parts: Vec<&str> = split_grid_args(inner);
        if parts.len() != 2 {
            return Err(ParseError::UnexpectedToken {
                found: input.to_string(),
                expected: "minmax(min, max)".to_string(),
                line: 0,
                column: 0,
            });
        }
        let min = parse_single_track(parts[0].trim())?;
        let max = parse_single_track(parts[1].trim())?;
        return Ok(GridTrackSize::MinMax {
            min: Box::new(min),
            max: Box::new(max),
        });
    }

    // Fraction unit (e.g., 1fr, 2.5fr)
    if let Some(fr_str) = input.strip_suffix("fr") {
        let value = fr_str.trim().parse::<f64>().map_err(|_| ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "fraction value (e.g., 1fr)".to_string(),
            line: 0,
            column: 0,
        })?;
        return Ok(GridTrackSize::Fraction(value));
    }

    // Fixed size (e.g., 100px, 50%)
    if let Some(px) = parse_length_value(input) {
        return Ok(GridTrackSize::Fixed(px));
    }

    Err(ParseError::UnexpectedToken {
        found: input.to_string(),
        expected: "grid track size (auto, fr, px, minmax, etc.)".to_string(),
        line: 0,
        column: 0,
    })
}

/// Parse grid line value: "1 / 3", "1 / -1", "span 2", "1 / span 2"
fn parse_grid_line_value(input: &str) -> Result<GridLineValue, ParseError> {
    let input = input.trim();

    // Check for "/" separator
    if let Some(slash_pos) = input.find(" / ") {
        let start_str = input[..slash_pos].trim();
        let end_str = input[slash_pos + 3..].trim();

        let start = parse_single_grid_line(start_str)?;
        let end = parse_single_grid_line(end_str)?;

        return Ok(GridLineValue {
            start,
            end: Some(end),
        });
    }

    // Single value (e.g., "span 2" or "1")
    let start = parse_single_grid_line(input)?;
    Ok(GridLineValue { start, end: None })
}

/// Parse a single grid line reference: "1", "-1", "span 2", "header-start"
fn parse_single_grid_line(input: &str) -> Result<GridLine, ParseError> {
    let input = input.trim();

    // "auto"
    if input == "auto" {
        return Ok(GridLine::Auto);
    }

    // "span N"
    if let Some(span_str) = input.strip_prefix("span ") {
        let n = span_str.trim().parse::<u32>().map_err(|_| ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: "span count".to_string(),
            line: 0,
            column: 0,
        })?;
        return Ok(GridLine::Span(n));
    }

    // Number (can be negative)
    if let Ok(n) = input.parse::<i32>() {
        return Ok(GridLine::Number(n));
    }

    // Named line
    Ok(GridLine::Named(input.to_string()))
}

/// Split arguments for grid functions, respecting parentheses.
fn split_grid_args(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0;

    for (i, c) in input.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            ',' if paren_depth == 0 => {
                parts.push(input[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }

    if start < input.len() {
        parts.push(input[start..].trim());
    }

    parts
}

/// Parse rotate() transform: rotate(45deg)
fn parse_rotate_transform(input: &str) -> Result<Transform, ParseError> {
    let inner = extract_function_args(input, "rotate")?;
    let angle = parse_angle(inner.trim()).ok_or_else(|| ParseError::UnexpectedToken {
        found: inner.to_string(),
        expected: "angle (e.g., 45deg)".to_string(),
        line: 0,
        column: 0,
    })?;
    Ok(Transform::rotate(angle))
}

/// Parse scale() transform: scale(1.5) or scale(1.5, 2.0)
fn parse_scale_transform(input: &str) -> Result<Transform, ParseError> {
    let inner = extract_function_args(input, "scale")?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    let sx = parts[0].parse::<f64>().map_err(|_| ParseError::UnexpectedToken {
        found: parts[0].to_string(),
        expected: "scale factor".to_string(),
        line: 0,
        column: 0,
    })?;

    let sy = if parts.len() > 1 {
        parts[1].parse::<f64>().unwrap_or(sx)
    } else {
        sx
    };

    Ok(Transform::scale(sx, sy))
}

/// Parse translate() transform: translate(10px, 20px)
fn parse_translate_transform(input: &str) -> Result<Transform, ParseError> {
    let inner = extract_function_args(input, "translate")?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    let tx = parse_length_value(parts[0]).ok_or_else(|| ParseError::UnexpectedToken {
        found: parts[0].to_string(),
        expected: "translate x value".to_string(),
        line: 0,
        column: 0,
    })?;

    let ty = if parts.len() > 1 {
        parse_length_value(parts[1]).unwrap_or(0.0)
    } else {
        0.0
    };

    Ok(Transform::translate(tx, ty))
}

/// Parse skew() transform: skew(10deg, 20deg)
fn parse_skew_transform(input: &str) -> Result<Transform, ParseError> {
    let inner = extract_function_args(input, "skew")?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    let ax = parse_angle(parts[0]).ok_or_else(|| ParseError::UnexpectedToken {
        found: parts[0].to_string(),
        expected: "skew angle".to_string(),
        line: 0,
        column: 0,
    })?;

    let ay = if parts.len() > 1 {
        parse_angle(parts[1]).unwrap_or(0.0)
    } else {
        0.0
    };

    Ok(Transform { operations: vec![TransformOp::Skew(ax, ay)] })
}

/// Parse matrix() transform: matrix(a, b, c, d, e, f)
fn parse_matrix_transform(input: &str) -> Result<Transform, ParseError> {
    let inner = extract_function_args(input, "matrix")?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if parts.len() != 6 {
        return Err(ParseError::UnexpectedToken {
            found: inner.to_string(),
            expected: "6 matrix values".to_string(),
            line: 0,
            column: 0,
        });
    }

    let values: Result<Vec<f64>, _> = parts.iter().map(|s| s.parse::<f64>()).collect();
    let values = values.map_err(|_| ParseError::UnexpectedToken {
        found: inner.to_string(),
        expected: "numeric matrix values".to_string(),
        line: 0,
        column: 0,
    })?;

    Ok(Transform {
        operations: vec![TransformOp::Matrix([
            values[0], values[1], values[2], values[3], values[4], values[5],
        ])],
    })
}

/// Parse rgb() or rgba() color.
fn parse_rgb_color(input: &str) -> Option<Color> {
    let (name, inner) = if input.starts_with("rgba(") {
        ("rgba", input.strip_prefix("rgba(")?.strip_suffix(')')?)
    } else if input.starts_with("rgb(") {
        ("rgb", input.strip_prefix("rgb(")?.strip_suffix(')')?)
    } else {
        return None;
    };

    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if name == "rgba" && parts.len() == 4 {
        let r = parts[0].parse::<f32>().ok()? / 255.0;
        let g = parts[1].parse::<f32>().ok()? / 255.0;
        let b = parts[2].parse::<f32>().ok()? / 255.0;
        let a = parts[3].parse::<f32>().ok()?;
        Some(Color::rgba(r, g, b, a))
    } else if name == "rgb" && parts.len() == 3 {
        let r = parts[0].parse::<f32>().ok()? / 255.0;
        let g = parts[1].parse::<f32>().ok()? / 255.0;
        let b = parts[2].parse::<f32>().ok()? / 255.0;
        Some(Color::rgb(r, g, b))
    } else {
        None
    }
}

/// Extract the inner content of a function call.
fn extract_function_args<'a>(input: &'a str, name: &str) -> Result<&'a str, ParseError> {
    let prefix = format!("{}(", name);
    let inner = input
        .strip_prefix(&prefix)
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| ParseError::UnexpectedToken {
            found: input.to_string(),
            expected: format!("{}(...)", name),
            line: 0,
            column: 0,
        })?;
    Ok(inner)
}

/// Split gradient arguments by comma, respecting parentheses.
fn split_gradient_args(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0;

    for (i, c) in input.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            ',' if paren_depth == 0 => {
                parts.push(input[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }

    if start < input.len() {
        parts.push(input[start..].trim());
    }

    parts
}

/// Parse gradient direction (angle or keyword).
fn parse_gradient_direction(input: &str) -> Option<f64> {
    let input = input.trim();

    // Angle with unit
    if let Some(angle) = parse_angle(input) {
        return Some(angle);
    }

    // Direction keywords
    match input {
        "to right" => Some(0.0),
        "to top" => Some(90.0),
        "to left" => Some(180.0),
        "to bottom" => Some(270.0),
        "to top right" | "to right top" => Some(45.0),
        "to top left" | "to left top" => Some(135.0),
        "to bottom left" | "to left bottom" => Some(225.0),
        "to bottom right" | "to right bottom" => Some(315.0),
        _ => None,
    }
}

/// Parse an angle value (e.g., "90deg", "0.5turn", "1.57rad").
fn parse_angle(input: &str) -> Option<f64> {
    let input = input.trim();

    if let Some(deg) = input.strip_suffix("deg") {
        return deg.trim().parse::<f64>().ok();
    }
    if let Some(rad) = input.strip_suffix("rad") {
        return rad.trim().parse::<f64>().ok().map(|r| r * 180.0 / PI);
    }
    if let Some(turn) = input.strip_suffix("turn") {
        return turn.trim().parse::<f64>().ok().map(|t| t * 360.0);
    }
    if let Some(grad) = input.strip_suffix("grad") {
        return grad.trim().parse::<f64>().ok().map(|g| g * 0.9);
    }

    None
}

/// Parse color stops.
fn parse_color_stops(parts: &[&str]) -> Result<Vec<GradientStop>, ParseError> {
    let mut stops = Vec::new();
    let count = parts.len();

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Check if there's a position specified (e.g., "#ff0000 50%")
        let (color_str, position) = if let Some(space_pos) = part.rfind(' ') {
            let potential_pos = part[space_pos + 1..].trim();
            if let Some(pos) = parse_stop_position(potential_pos) {
                (&part[..space_pos], pos)
            } else {
                (part, default_stop_position(i, count))
            }
        } else {
            (part, default_stop_position(i, count))
        };

        let color = parse_color_value(color_str)?;
        stops.push(GradientStop::new(position, color));
    }

    Ok(stops)
}

/// Parse a stop position (percentage or length).
fn parse_stop_position(input: &str) -> Option<f64> {
    let input = input.trim();

    if let Some(pct) = input.strip_suffix('%') {
        return pct.trim().parse::<f64>().ok().map(|p| p / 100.0);
    }

    // For now, only support percentages
    None
}

/// Calculate default stop position based on index.
fn default_stop_position(index: usize, count: usize) -> f64 {
    if count <= 1 {
        0.5
    } else {
        index as f64 / (count - 1) as f64
    }
}

/// Parse a color value (hex or named).
fn parse_color_value(input: &str) -> Result<Color, ParseError> {
    let input = input.trim();

    // Hex color
    if let Some(hex) = input.strip_prefix('#') {
        if let Some(color) = Color::from_hex(hex) {
            return Ok(color);
        }
    }

    // Named colors
    match input.to_lowercase().as_str() {
        "white" => return Ok(Color::WHITE),
        "black" => return Ok(Color::BLACK),
        "red" => return Ok(Color::from_rgb8(255, 0, 0)),
        "green" => return Ok(Color::from_rgb8(0, 128, 0)),
        "blue" => return Ok(Color::from_rgb8(0, 0, 255)),
        "yellow" => return Ok(Color::from_rgb8(255, 255, 0)),
        "cyan" => return Ok(Color::from_rgb8(0, 255, 255)),
        "magenta" => return Ok(Color::from_rgb8(255, 0, 255)),
        "orange" => return Ok(Color::from_rgb8(255, 165, 0)),
        "purple" => return Ok(Color::from_rgb8(128, 0, 128)),
        "pink" => return Ok(Color::from_rgb8(255, 192, 203)),
        "transparent" => return Ok(Color::TRANSPARENT),
        _ => {}
    }

    Err(ParseError::UnexpectedToken {
        found: input.to_string(),
        expected: "color value".to_string(),
        line: 0,
        column: 0,
    })
}

/// Parse position from "circle at X% Y%" syntax.
fn parse_position(input: &str) -> Result<(f64, f64), ParseError> {
    if let Some(at_pos) = input.find(" at ") {
        let pos_str = &input[at_pos + 4..];
        parse_simple_position(pos_str)
    } else {
        Ok((0.5, 0.5))
    }
}

/// Parse a simple position like "50% 50%" or "center".
fn parse_simple_position(input: &str) -> Result<(f64, f64), ParseError> {
    let input = input.trim();

    // Keywords
    match input {
        "center" => return Ok((0.5, 0.5)),
        "top" => return Ok((0.5, 0.0)),
        "bottom" => return Ok((0.5, 1.0)),
        "left" => return Ok((0.0, 0.5)),
        "right" => return Ok((1.0, 0.5)),
        _ => {}
    }

    // Parse "X% Y%"
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.len() == 2 {
        let x = parse_stop_position(parts[0]).unwrap_or(0.5);
        let y = parse_stop_position(parts[1]).unwrap_or(0.5);
        return Ok((x, y));
    }

    Ok((0.5, 0.5))
}

/// Parse a constraint.
fn parse_constraint(input: &str) -> Result<Option<Constraint>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }

    // Try parsing different constraint types
    if let Some(constraint) = try_parse_equality_constraint(input)? {
        return Ok(Some(constraint));
    }

    if let Some(constraint) = try_parse_inequality_constraint(input)? {
        return Ok(Some(constraint));
    }

    if let Some(constraint) = try_parse_alignment_constraint(input)? {
        return Ok(Some(constraint));
    }

    if let Some(constraint) = try_parse_relative_constraint(input)? {
        return Ok(Some(constraint));
    }

    // Unknown constraint format
    Ok(None)
}

/// Try to parse an equality constraint: "width = 100px"
fn try_parse_equality_constraint(input: &str) -> Result<Option<Constraint>, ParseError> {
    // Find '=' that's not part of '>=' or '<='
    let eq_pos = input.find(|c| c == '=').and_then(|pos| {
        let before = input.chars().nth(pos.saturating_sub(1));
        if before == Some('<') || before == Some('>') {
            None
        } else {
            Some(pos)
        }
    });

    let Some(eq_pos) = eq_pos else {
        return Ok(None);
    };

    let property = input[..eq_pos].trim();
    let value_str = input[eq_pos + 1..].trim();

    let expression = parse_expression(value_str)?;

    Ok(Some(Constraint {
        kind: ConstraintKind::Equality {
            property: property.to_string(),
            value: expression,
        },
        priority: None,
        span: Span::default(),
    }))
}

/// Try to parse an inequality constraint: "width >= 100px" or "width <= 200px"
fn try_parse_inequality_constraint(input: &str) -> Result<Option<Constraint>, ParseError> {
    let (op, op_str) = if input.contains(">=") {
        (InequalityOp::GreaterThanOrEqual, ">=")
    } else if input.contains("<=") {
        (InequalityOp::LessThanOrEqual, "<=")
    } else if input.contains('>') && !input.contains(">=") {
        (InequalityOp::GreaterThan, ">")
    } else if input.contains('<') && !input.contains("<=") {
        (InequalityOp::LessThan, "<")
    } else {
        return Ok(None);
    };

    let parts: Vec<&str> = input.splitn(2, op_str).collect();
    if parts.len() != 2 {
        return Ok(None);
    }

    let property = parts[0].trim();
    let value_str = parts[1].trim();

    let expression = parse_expression(value_str)?;

    Ok(Some(Constraint {
        kind: ConstraintKind::Inequality {
            property: property.to_string(),
            op,
            value: expression,
        },
        priority: None,
        span: Span::default(),
    }))
}

/// Try to parse an alignment constraint: "left align Parent" or "center-x align Parent, gap: 16px"
fn try_parse_alignment_constraint(input: &str) -> Result<Option<Constraint>, ParseError> {
    if !input.contains(" align ") {
        return Ok(None);
    }

    let parts: Vec<&str> = input.splitn(2, " align ").collect();
    if parts.len() != 2 {
        return Ok(None);
    }

    let edge_str = parts[0].trim();
    let rest = parts[1].trim();

    let edge = parse_edge(edge_str).ok_or_else(|| ParseError::UnexpectedToken {
        found: edge_str.to_string(),
        expected: "edge name (left, right, top, bottom, center-x, center-y)".to_string(),
        line: 0,
        column: 0,
    })?;

    // Check for gap: "Parent, gap: 16px"
    let (target_str, _gap) = if let Some(comma_pos) = rest.find(',') {
        let target = rest[..comma_pos].trim();
        let gap_part = rest[comma_pos + 1..].trim();

        let gap = if let Some(gap_value) = gap_part.strip_prefix("gap:") {
            parse_gap_value(gap_value.trim())?
        } else {
            None
        };

        (target, gap)
    } else {
        (rest, None)
    };

    let target = parse_element_ref(target_str)?;

    // TODO: Extend Alignment AST to support gap

    Ok(Some(Constraint {
        kind: ConstraintKind::Alignment {
            edge,
            target,
            target_edge: Some(edge),
        },
        priority: None,
        span: Span::default(),
    }))
}

/// Try to parse a relative constraint: "below Header" or "below Header, gap: 16px"
fn try_parse_relative_constraint(input: &str) -> Result<Option<Constraint>, ParseError> {
    let relation = if input.starts_with("below ") {
        Some((Relation::Below, "below "))
    } else if input.starts_with("above ") {
        Some((Relation::Above, "above "))
    } else if input.starts_with("leftOf ") || input.starts_with("left-of ") {
        Some((Relation::LeftOf, if input.starts_with("leftOf ") { "leftOf " } else { "left-of " }))
    } else if input.starts_with("rightOf ") || input.starts_with("right-of ") {
        Some((Relation::RightOf, if input.starts_with("rightOf ") { "rightOf " } else { "right-of " }))
    } else {
        None
    };

    let Some((relation, prefix)) = relation else {
        return Ok(None);
    };

    let rest = &input[prefix.len()..];

    // Check for gap: "Header, gap: 16px"
    let (target_str, gap) = if let Some(comma_pos) = rest.find(',') {
        let target = rest[..comma_pos].trim();
        let gap_part = rest[comma_pos + 1..].trim();

        let gap = if let Some(gap_value) = gap_part.strip_prefix("gap:") {
            parse_gap_value(gap_value.trim())?
        } else {
            None
        };

        (target, gap)
    } else {
        (rest.trim(), None)
    };

    let target = parse_element_ref(target_str)?;

    Ok(Some(Constraint {
        kind: ConstraintKind::Relative {
            relation,
            target,
            gap,
        },
        priority: None,
        span: Span::default(),
    }))
}

/// Parse an edge name.
fn parse_edge(s: &str) -> Option<Edge> {
    match s {
        "left" => Some(Edge::Left),
        "right" => Some(Edge::Right),
        "top" => Some(Edge::Top),
        "bottom" => Some(Edge::Bottom),
        "center-x" | "centerX" => Some(Edge::CenterX),
        "center-y" | "centerY" => Some(Edge::CenterY),
        _ => None,
    }
}

/// Parse an element reference.
fn parse_element_ref(s: &str) -> Result<ElementRef, ParseError> {
    match s {
        "Parent" => Ok(ElementRef::Parent),
        "Previous" => Ok(ElementRef::Previous),
        "Next" => Ok(ElementRef::Next),
        _ => Ok(ElementRef::Named(Identifier(s.to_string()))),
    }
}

/// Parse a gap value.
fn parse_gap_value(s: &str) -> Result<Option<Length>, ParseError> {
    if let Ok((rest, (num, unit_str))) = parse_number_with_unit(s) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            return Ok(Some(make_length(num, unit_str)));
        }
    }
    Ok(None)
}

/// Parse an expression.
fn parse_expression(input: &str) -> Result<Expression, ParseError> {
    let input = input.trim();

    // Function call: min(...) or max(...)
    if input.starts_with("min(") || input.starts_with("max(") {
        return parse_function_expression(input);
    }

    // Binary expression with + or - (check BEFORE property ref to handle "Parent.width - 48px")
    // Find the last + or - that's not inside parentheses
    if let Some(expr) = try_parse_binary_expression(input)? {
        return Ok(expr);
    }

    // Property reference: Element.property (simple case without operators)
    if input.contains('.') && !input.starts_with('$') && !input.starts_with('#') {
        if let Some(expr) = try_parse_property_ref(input)? {
            return Ok(expr);
        }
    }

    // Token reference
    if let Some(token_path) = input.strip_prefix('$') {
        let path = TokenPath(token_path.split('.').map(String::from).collect());
        return Ok(Expression::TokenRef(path));
    }

    // Number with unit
    if let Ok((rest, (num, unit_str))) = parse_number_with_unit(input) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            return Ok(Expression::Length(make_length(num, unit_str)));
        }
    }

    // Plain number
    if let Ok((rest, num)) = number(input) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            return Ok(Expression::Literal(num));
        }
    }

    // Fallback
    Ok(Expression::Literal(0.0))
}

/// Try to parse a binary expression (handles + and -).
fn try_parse_binary_expression(input: &str) -> Result<Option<Expression>, ParseError> {
    let mut paren_depth = 0;
    let mut last_op_pos = None;
    let mut last_op = None;

    for (i, c) in input.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '+' | '-' if paren_depth == 0 && i > 0 => {
                // Check it's not a negative number at the start
                let before = input[..i].trim();
                if !before.is_empty() {
                    last_op_pos = Some(i);
                    last_op = Some(if c == '+' { BinaryOp::Add } else { BinaryOp::Sub });
                }
            }
            _ => {}
        }
    }

    if let (Some(pos), Some(op)) = (last_op_pos, last_op) {
        let left = input[..pos].trim();
        let right = input[pos + 1..].trim();

        let left_expr = parse_expression(left)?;
        let right_expr = parse_expression(right)?;

        return Ok(Some(Expression::BinaryOp {
            left: Box::new(left_expr),
            op,
            right: Box::new(right_expr),
        }));
    }

    Ok(None)
}

/// Parse a function expression like min(a, b) or max(a, b).
fn parse_function_expression(input: &str) -> Result<Expression, ParseError> {
    let paren_start = input.find('(').ok_or(ParseError::UnexpectedEof)?;
    let paren_end = input.rfind(')').ok_or(ParseError::UnexpectedEof)?;

    let name = input[..paren_start].trim();
    let args_str = &input[paren_start + 1..paren_end];

    // Split args by comma (respecting nested parens)
    let args = split_args(args_str);
    let mut parsed_args = Vec::new();

    for arg in args {
        parsed_args.push(parse_expression(arg.trim())?);
    }

    Ok(Expression::Function {
        name: name.to_string(),
        args: parsed_args,
    })
}

/// Try to parse a property reference like Parent.width.
fn try_parse_property_ref(input: &str) -> Result<Option<Expression>, ParseError> {
    let Some(dot_pos) = input.find('.') else {
        return Ok(None);
    };
    let element_str = &input[..dot_pos];
    let property = &input[dot_pos + 1..];

    // Must be a valid identifier for element
    if identifier(element_str).is_err() {
        return Ok(None);
    }

    let element = parse_element_ref(element_str)?;

    Ok(Some(Expression::PropertyRef {
        element,
        property: property.to_string(),
    }))
}

/// Split function arguments by comma, respecting parentheses.
fn split_args(input: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0;

    for (i, c) in input.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            ',' if paren_depth == 0 => {
                args.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }

    if start < input.len() {
        args.push(&input[start..]);
    }

    args
}

/// Parse number with unit.
fn parse_number_with_unit(input: &str) -> IResult<&str, (f64, &str)> {
    pair(number, unit)(input)
}

/// Create a Length from number and unit string.
fn make_length(value: f64, unit_str: &str) -> Length {
    let unit = match unit_str {
        "px" => LengthUnit::Px,
        "pt" => LengthUnit::Pt,
        "mm" => LengthUnit::Mm,
        "cm" => LengthUnit::Cm,
        "in" => LengthUnit::In,
        "%" => LengthUnit::Percent,
        "em" => LengthUnit::Em,
        "rem" => LengthUnit::Rem,
        _ => LengthUnit::Px,
    };
    Length { value, unit }
}

/// Parse SVG path data string into commands.
/// Supports M, L, H, V, C, S, Q, T, A, Z commands (absolute and relative).
fn parse_svg_path_data(input: &str) -> Result<Vec<SvgPathCommand>, ParseError> {
    let mut commands = Vec::new();
    let mut chars = input.chars().peekable();
    let mut current_command: Option<char> = None;

    while chars.peek().is_some() {
        // Skip whitespace and commas
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == ',' {
                chars.next();
            } else {
                break;
            }
        }

        if chars.peek().is_none() {
            break;
        }

        // Check if next char is a command letter
        if let Some(&c) = chars.peek() {
            if c.is_ascii_alphabetic() {
                current_command = Some(c);
                chars.next();
            }
        }

        let Some(cmd) = current_command else {
            break;
        };

        // Parse arguments based on command
        match cmd {
            'M' => {
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::MoveTo { x, y });
                // Subsequent coords are implicit LineTo
                current_command = Some('L');
            }
            'm' => {
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::MoveToRel { dx, dy });
                current_command = Some('l');
            }
            'L' => {
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::LineTo { x, y });
            }
            'l' => {
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::LineToRel { dx, dy });
            }
            'H' => {
                let x = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::HorizontalTo { x });
            }
            'h' => {
                let dx = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::HorizontalToRel { dx });
            }
            'V' => {
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::VerticalTo { y });
            }
            'v' => {
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::VerticalToRel { dy });
            }
            'C' => {
                let x1 = parse_svg_number(&mut chars)?;
                let y1 = parse_svg_number(&mut chars)?;
                let x2 = parse_svg_number(&mut chars)?;
                let y2 = parse_svg_number(&mut chars)?;
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::CubicTo { x1, y1, x2, y2, x, y });
            }
            'c' => {
                let dx1 = parse_svg_number(&mut chars)?;
                let dy1 = parse_svg_number(&mut chars)?;
                let dx2 = parse_svg_number(&mut chars)?;
                let dy2 = parse_svg_number(&mut chars)?;
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::CubicToRel { dx1, dy1, dx2, dy2, dx, dy });
            }
            'S' => {
                let x2 = parse_svg_number(&mut chars)?;
                let y2 = parse_svg_number(&mut chars)?;
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::SmoothCubicTo { x2, y2, x, y });
            }
            's' => {
                let dx2 = parse_svg_number(&mut chars)?;
                let dy2 = parse_svg_number(&mut chars)?;
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::SmoothCubicToRel { dx2, dy2, dx, dy });
            }
            'Q' => {
                let x1 = parse_svg_number(&mut chars)?;
                let y1 = parse_svg_number(&mut chars)?;
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::QuadTo { x1, y1, x, y });
            }
            'q' => {
                let dx1 = parse_svg_number(&mut chars)?;
                let dy1 = parse_svg_number(&mut chars)?;
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::QuadToRel { dx1, dy1, dx, dy });
            }
            'T' => {
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::SmoothQuadTo { x, y });
            }
            't' => {
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::SmoothQuadToRel { dx, dy });
            }
            'A' => {
                let rx = parse_svg_number(&mut chars)?;
                let ry = parse_svg_number(&mut chars)?;
                let x_rotation = parse_svg_number(&mut chars)?;
                let large_arc = parse_svg_flag(&mut chars)?;
                let sweep = parse_svg_flag(&mut chars)?;
                let x = parse_svg_number(&mut chars)?;
                let y = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::ArcTo { rx, ry, x_rotation, large_arc, sweep, x, y });
            }
            'a' => {
                let rx = parse_svg_number(&mut chars)?;
                let ry = parse_svg_number(&mut chars)?;
                let x_rotation = parse_svg_number(&mut chars)?;
                let large_arc = parse_svg_flag(&mut chars)?;
                let sweep = parse_svg_flag(&mut chars)?;
                let dx = parse_svg_number(&mut chars)?;
                let dy = parse_svg_number(&mut chars)?;
                commands.push(SvgPathCommand::ArcToRel { rx, ry, x_rotation, large_arc, sweep, dx, dy });
            }
            'Z' | 'z' => {
                commands.push(SvgPathCommand::ClosePath);
                current_command = None;
            }
            _ => {
                // Unknown command, skip
                current_command = None;
            }
        }
    }

    Ok(commands)
}

/// Parse a number from SVG path data.
fn parse_svg_number(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<f64, ParseError> {
    // Skip whitespace and commas
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ',' {
            chars.next();
        } else {
            break;
        }
    }

    let mut num_str = String::new();

    // Optional sign
    if let Some(&c) = chars.peek() {
        if c == '-' || c == '+' {
            num_str.push(c);
            chars.next();
        }
    }

    // Integer part
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            num_str.push(c);
            chars.next();
        } else {
            break;
        }
    }

    // Decimal part
    if let Some(&'.') = chars.peek() {
        num_str.push('.');
        chars.next();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                num_str.push(c);
                chars.next();
            } else {
                break;
            }
        }
    }

    // Exponent part
    if let Some(&c) = chars.peek() {
        if c == 'e' || c == 'E' {
            num_str.push(c);
            chars.next();
            if let Some(&c) = chars.peek() {
                if c == '-' || c == '+' {
                    num_str.push(c);
                    chars.next();
                }
            }
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    num_str.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
        }
    }

    if num_str.is_empty() || num_str == "-" || num_str == "+" {
        return Err(ParseError::UnexpectedToken {
            found: "no number".to_string(),
            expected: "number in SVG path".to_string(),
            line: 0,
            column: 0,
        });
    }

    num_str.parse::<f64>().map_err(|_| ParseError::UnexpectedToken {
        found: num_str.clone(),
        expected: "valid number".to_string(),
        line: 0,
        column: 0,
    })
}

/// Parse a flag (0 or 1) from SVG path data.
fn parse_svg_flag(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<bool, ParseError> {
    // Skip whitespace and commas
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() || c == ',' {
            chars.next();
        } else {
            break;
        }
    }

    match chars.next() {
        Some('0') => Ok(false),
        Some('1') => Ok(true),
        other => Err(ParseError::UnexpectedToken {
            found: other.map(|c| c.to_string()).unwrap_or_else(|| "EOF".to_string()),
            expected: "0 or 1 for arc flag".to_string(),
            line: 0,
            column: 0,
        }),
    }
}

/// Parse SVG viewBox attribute: "minX minY width height".
fn parse_viewbox(input: &str) -> Option<SvgViewBox> {
    let parts: Vec<f64> = input
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();

    if parts.len() == 4 {
        Some(SvgViewBox {
            min_x: parts[0],
            min_y: parts[1],
            width: parts[2],
            height: parts[3],
        })
    } else {
        None
    }
}

/// Parse image source string into ImageSource type.
fn parse_image_source(s: &str) -> ImageSource {
    let s = s.trim();

    // Check for data URL
    if s.starts_with("data:") {
        if let Some(comma_pos) = s.find(',') {
            let header = &s[5..comma_pos]; // after "data:"
            let data = &s[comma_pos + 1..];
            // Parse "image/png;base64" -> mime_type = "image/png"
            let mime_type = header.split(';').next().unwrap_or("application/octet-stream");
            return ImageSource::Data {
                mime_type: mime_type.to_string(),
                data: data.to_string(),
            };
        }
    }

    // Check for URL (http/https)
    if s.starts_with("http://") || s.starts_with("https://") {
        return ImageSource::Url(s.to_string());
    }

    // Otherwise treat as file path
    ImageSource::File(s.to_string())
}

/// Parse image fit mode string.
fn parse_image_fit(s: &str) -> ImageFit {
    match s.to_lowercase().as_str() {
        "cover" => ImageFit::Cover,
        "contain" => ImageFit::Contain,
        "fill" => ImageFit::Fill,
        "none" => ImageFit::None,
        "scale-down" | "scaledown" => ImageFit::ScaleDown,
        _ => ImageFit::Cover,
    }
}

/// Parse icon source string into IconSource type.
fn parse_icon_source(s: &str) -> IconSource {
    let s = s.trim();

    // Check for library:name format (e.g., "lucide:home", "material:settings")
    if let Some(colon_pos) = s.find(':') {
        let library = &s[..colon_pos];
        let name = &s[colon_pos + 1..];
        return IconSource::Named {
            library: Some(library.to_string()),
            name: name.to_string(),
        };
    }

    // Otherwise just the icon name
    IconSource::Named {
        library: None,
        name: s.to_string(),
    }
}

/// Parse a geometry value string into a Geometry type.
/// Supports: Box(w, h, d), Sphere(r), Cylinder(r, h), Import("path")
fn parse_geometry(s: &str) -> Result<Geometry, ParseError> {
    let s = s.trim();

    // Parse Box(width, height, depth)
    if let Some(args) = s.strip_prefix("Box(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = args.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let width = parse_geometry_length(parts[0])?;
            let height = parse_geometry_length(parts[1])?;
            let depth = parse_geometry_length(parts[2])?;
            return Ok(Geometry::Primitive(Primitive::Box { width, height, depth }));
        }
    }

    // Parse Sphere(radius)
    if let Some(args) = s.strip_prefix("Sphere(").and_then(|s| s.strip_suffix(')')) {
        let radius = parse_geometry_length(args.trim())?;
        return Ok(Geometry::Primitive(Primitive::Sphere { radius }));
    }

    // Parse Cylinder(radius, height)
    if let Some(args) = s.strip_prefix("Cylinder(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = args.split(',').map(|p| p.trim()).collect();
        if parts.len() == 2 {
            let radius = parse_geometry_length(parts[0])?;
            let height = parse_geometry_length(parts[1])?;
            return Ok(Geometry::Primitive(Primitive::Cylinder { radius, height }));
        }
    }

    // Parse Import("path") or Import("path", format: "step")
    if let Some(args) = s.strip_prefix("Import(").and_then(|s| s.strip_suffix(')')) {
        return parse_geometry_import(args);
    }

    // Default: couldn't parse
    Err(ParseError::UnexpectedToken {
        found: s.to_string(),
        expected: "geometry (Box, Sphere, Cylinder, or Import)".to_string(),
        line: 0,
        column: 0,
    })
}

/// Parse Import() arguments: "path" or "path", format: "step"
fn parse_geometry_import(args: &str) -> Result<Geometry, ParseError> {
    let args = args.trim();

    // Find the path string (first quoted string)
    let (path, rest) = parse_quoted_string_simple(args)?;

    // Check for optional format parameter
    let rest = rest.trim();
    let format = if rest.starts_with(',') {
        let rest = rest[1..].trim();
        if let Some(rest) = rest.strip_prefix("format:").or_else(|| rest.strip_prefix("format :")) {
            let rest = rest.trim();
            let (fmt, _) = parse_quoted_string_simple(rest)?;
            Some(fmt)
        } else {
            None
        }
    } else {
        None
    };

    Ok(Geometry::Import(GeometryImport {
        path,
        format,
        bounds: None, // Bounds are computed at load time, not in the source
    }))
}

/// Simple quoted string parser for Import paths.
fn parse_quoted_string_simple(s: &str) -> Result<(String, &str), ParseError> {
    let s = s.trim();

    // Find opening quote
    let quote_char = s.chars().next().ok_or_else(|| ParseError::UnexpectedToken {
        found: "end of input".to_string(),
        expected: "quoted string".to_string(),
        line: 0,
        column: 0,
    })?;

    if quote_char != '"' && quote_char != '\'' {
        return Err(ParseError::UnexpectedToken {
            found: quote_char.to_string(),
            expected: "quoted string".to_string(),
            line: 0,
            column: 0,
        });
    }

    // Find closing quote
    let rest = &s[1..];
    if let Some(end_pos) = rest.find(quote_char) {
        let content = &rest[..end_pos];
        let remaining = &rest[end_pos + 1..];
        Ok((content.to_string(), remaining))
    } else {
        Err(ParseError::UnexpectedToken {
            found: "unterminated string".to_string(),
            expected: "closing quote".to_string(),
            line: 0,
            column: 0,
        })
    }
}

/// Parse a length value for geometry (e.g., "100mm", "50px").
fn parse_geometry_length(s: &str) -> Result<Length, ParseError> {
    let s = s.trim();

    // Try to parse number with unit
    if let Ok((rest, (num, unit_str))) = parse_number_with_unit(s) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            return Ok(make_length(num, unit_str));
        }
    }

    // Try to parse just a number (assume mm for geometry)
    if let Ok((rest, num)) = number(s) {
        if rest.is_empty() || rest.chars().all(|c| c.is_whitespace()) {
            return Ok(Length::mm(num));
        }
    }

    Err(ParseError::UnexpectedToken {
        found: s.to_string(),
        expected: "length value".to_string(),
        line: 0,
        column: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_frame() {
        let input = r#"Frame Button:
  fill: #3B82F6
  cornerRadius: 8px
"#;
        let doc = parse(input).unwrap();
        assert_eq!(doc.elements.len(), 1);

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.name.as_ref().unwrap().0, "Button");
            assert_eq!(frame.properties.len(), 2);
            assert_eq!(frame.properties[0].name, "fill");
            assert_eq!(frame.properties[1].name, "cornerRadius");
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_frame_with_constraints() {
        let input = r#"Frame Box:
  fill: #FFFFFF
  constraints:
    - width = 100px
    - height = 50px
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.name.as_ref().unwrap().0, "Box");
            assert_eq!(frame.constraints.len(), 2);

            if let ConstraintKind::Equality { property, .. } = &frame.constraints[0].kind {
                assert_eq!(property, "width");
            } else {
                panic!("Expected equality constraint");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_alignment_constraint() {
        let input = r#"Frame Child:
  constraints:
    - left align Parent
    - center-x align Parent
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.constraints.len(), 2);

            if let ConstraintKind::Alignment { edge, target, .. } = &frame.constraints[0].kind {
                assert_eq!(*edge, Edge::Left);
                assert!(matches!(target, ElementRef::Parent));
            } else {
                panic!("Expected alignment constraint");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_relative_constraint() {
        let input = r#"Frame Content:
  constraints:
    - below Header
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.constraints.len(), 1);

            if let ConstraintKind::Relative { relation, target, gap } = &frame.constraints[0].kind {
                assert_eq!(*relation, Relation::Below);
                if let ElementRef::Named(name) = target {
                    assert_eq!(name.0, "Header");
                } else {
                    panic!("Expected named element ref");
                }
                assert!(gap.is_none());
            } else {
                panic!("Expected relative constraint");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_nested_frames() {
        let input = r#"Frame Parent:
  fill: #FFFFFF
  Frame Child:
    fill: #000000
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(parent) = &doc.elements[0] {
            assert_eq!(parent.name.as_ref().unwrap().0, "Parent");
            assert_eq!(parent.children.len(), 1);

            if let Element::Frame(child) = &parent.children[0] {
                assert_eq!(child.name.as_ref().unwrap().0, "Child");
            } else {
                panic!("Expected child Frame");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_text_element() {
        let input = r#"Text Label:
  content: "Hello World"
  fontSize: 14px
"#;
        let doc = parse(input).unwrap();

        if let Element::Text(text) = &doc.elements[0] {
            assert_eq!(text.name.as_ref().unwrap().0, "Label");
            if let TextContent::Literal(s) = &text.content {
                assert_eq!(s, "Hello World");
            } else {
                panic!("Expected literal text content");
            }
        } else {
            panic!("Expected Text element");
        }
    }

    #[test]
    fn test_parse_card_fixture() {
        let input = include_str!("../../../tests/fixtures/card.seed");
        let doc = parse(input).unwrap();

        assert_eq!(doc.elements.len(), 1);

        if let Element::Frame(card) = &doc.elements[0] {
            assert_eq!(card.name.as_ref().unwrap().0, "Card");
            assert_eq!(card.children.len(), 2); // Header and Content
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_expression_with_operator() {
        let expr = parse_expression("Parent.width - 48px").unwrap();
        if let Expression::BinaryOp { op, .. } = expr {
            assert_eq!(op, BinaryOp::Sub);
        } else {
            panic!("Expected binary op");
        }
    }

    #[test]
    fn test_parse_min_function() {
        let expr = parse_expression("min(320px, Parent.width)").unwrap();
        if let Expression::Function { name, args } = expr {
            assert_eq!(name, "min");
            assert_eq!(args.len(), 2);
        } else {
            panic!("Expected function expression");
        }
    }

    #[test]
    fn test_parse_relative_with_gap() {
        let input = r#"Frame Footer:
  constraints:
    - below Content, gap: 16px
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let ConstraintKind::Relative { relation, gap, .. } = &frame.constraints[0].kind {
                assert_eq!(*relation, Relation::Below);
                assert!(gap.is_some());
                assert_eq!(gap.unwrap().value, 16.0);
            } else {
                panic!("Expected relative constraint");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_inequality_constraint() {
        let input = r#"Frame Box:
  constraints:
    - width >= 100px
    - height <= 200px
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.constraints.len(), 2);

            if let ConstraintKind::Inequality { property, op, .. } = &frame.constraints[0].kind {
                assert_eq!(property, "width");
                assert_eq!(*op, InequalityOp::GreaterThanOrEqual);
            } else {
                panic!("Expected inequality constraint");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_linear_gradient() {
        let input = r#"Frame:
  fill: linear-gradient(90deg, #ff0000, #0000ff)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.properties.len(), 1);
            if let PropertyValue::Gradient(Gradient::Linear(lg)) = &frame.properties[0].value {
                assert_eq!(lg.angle, 90.0);
                assert_eq!(lg.stops.len(), 2);
                assert_eq!(lg.stops[0].position, 0.0);
                assert_eq!(lg.stops[1].position, 1.0);
            } else {
                panic!("Expected linear gradient");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_linear_gradient_direction() {
        let input = r#"Frame:
  fill: linear-gradient(to right, #ff0000, #00ff00, #0000ff)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Gradient(Gradient::Linear(lg)) = &frame.properties[0].value {
                assert_eq!(lg.angle, 0.0); // "to right" = 0 degrees
                assert_eq!(lg.stops.len(), 3);
                assert_eq!(lg.stops[0].position, 0.0);
                assert_eq!(lg.stops[1].position, 0.5);
                assert_eq!(lg.stops[2].position, 1.0);
            } else {
                panic!("Expected linear gradient");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_radial_gradient() {
        let input = r#"Frame:
  fill: radial-gradient(circle, #ffffff, #000000)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Gradient(Gradient::Radial(rg)) = &frame.properties[0].value {
                assert_eq!(rg.center_x, 0.5);
                assert_eq!(rg.center_y, 0.5);
                assert_eq!(rg.stops.len(), 2);
            } else {
                panic!("Expected radial gradient");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_gradient_with_positions() {
        let input = r#"Frame:
  fill: linear-gradient(180deg, #ff0000 0%, #00ff00 50%, #0000ff 100%)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Gradient(Gradient::Linear(lg)) = &frame.properties[0].value {
                assert_eq!(lg.stops.len(), 3);
                assert_eq!(lg.stops[0].position, 0.0);
                assert_eq!(lg.stops[1].position, 0.5);
                assert_eq!(lg.stops[2].position, 1.0);
            } else {
                panic!("Expected linear gradient");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_drop_shadow() {
        let input = r#"Frame:
  shadow: drop-shadow(4px 8px 10px #000000)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.properties.len(), 1);
            if let PropertyValue::Shadow(shadow) = &frame.properties[0].value {
                assert_eq!(shadow.offset_x, 4.0);
                assert_eq!(shadow.offset_y, 8.0);
                assert_eq!(shadow.blur, 10.0);
                assert!(!shadow.inset);
            } else {
                panic!("Expected shadow property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_box_shadow_with_spread() {
        let input = r#"Frame:
  shadow: box-shadow(2px 4px 6px 2px #ff0000)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Shadow(shadow) = &frame.properties[0].value {
                assert_eq!(shadow.offset_x, 2.0);
                assert_eq!(shadow.offset_y, 4.0);
                assert_eq!(shadow.blur, 6.0);
                assert_eq!(shadow.spread, 2.0);
                assert!(!shadow.inset);
            } else {
                panic!("Expected shadow property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_inset_shadow() {
        let input = r#"Frame:
  shadow: inset-shadow(2px 2px 5px black)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Shadow(shadow) = &frame.properties[0].value {
                assert_eq!(shadow.offset_x, 2.0);
                assert_eq!(shadow.offset_y, 2.0);
                assert_eq!(shadow.blur, 5.0);
                assert!(shadow.inset);
            } else {
                panic!("Expected shadow property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_rotate_transform() {
        let input = r#"Frame:
  transform: rotate(45deg)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Transform(transform) = &frame.properties[0].value {
                assert_eq!(transform.operations.len(), 1);
                if let TransformOp::Rotate(angle) = transform.operations[0] {
                    assert_eq!(angle, 45.0);
                } else {
                    panic!("Expected Rotate operation");
                }
            } else {
                panic!("Expected transform property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_scale_transform() {
        let input = r#"Frame:
  transform: scale(1.5, 2.0)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Transform(transform) = &frame.properties[0].value {
                if let TransformOp::Scale(sx, sy) = transform.operations[0] {
                    assert_eq!(sx, 1.5);
                    assert_eq!(sy, 2.0);
                } else {
                    panic!("Expected Scale operation");
                }
            } else {
                panic!("Expected transform property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_translate_transform() {
        let input = r#"Frame:
  transform: translate(10px, 20px)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            if let PropertyValue::Transform(transform) = &frame.properties[0].value {
                if let TransformOp::Translate(tx, ty) = transform.operations[0] {
                    assert_eq!(tx, 10.0);
                    assert_eq!(ty, 20.0);
                } else {
                    panic!("Expected Translate operation");
                }
            } else {
                panic!("Expected transform property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_svg_element() {
        let input = r#"Svg SearchIcon:
  viewBox: 0 0 24 24
  path: M10 2a8 8 0 105.3 14L21 22l1-1-6-5.7A8 8 0 0010 2z
    fill: #333333
"#;
        let doc = parse(input).unwrap();
        assert_eq!(doc.elements.len(), 1);

        if let Element::Svg(svg) = &doc.elements[0] {
            assert_eq!(svg.name.as_ref().unwrap().0, "SearchIcon");
            assert!(svg.view_box.is_some());
            let vb = svg.view_box.as_ref().unwrap();
            assert_eq!(vb.min_x, 0.0);
            assert_eq!(vb.min_y, 0.0);
            assert_eq!(vb.width, 24.0);
            assert_eq!(vb.height, 24.0);
            assert_eq!(svg.paths.len(), 1);
            assert!(!svg.paths[0].commands.is_empty());
            // First command should be MoveTo
            assert!(matches!(svg.paths[0].commands[0], SvgPathCommand::MoveTo { .. }));
        } else {
            panic!("Expected Svg element");
        }
    }

    #[test]
    fn test_parse_svg_with_stroke() {
        let input = r#"Svg Line:
  viewBox: 0 0 24 24
  path: M0 12 L24 12
    stroke: #FF0000
    stroke-width: 2
"#;
        let doc = parse(input).unwrap();

        if let Element::Svg(svg) = &doc.elements[0] {
            assert_eq!(svg.paths.len(), 1);
            assert!(svg.paths[0].stroke.is_some());
            assert_eq!(svg.paths[0].stroke_width, Some(2.0));
        } else {
            panic!("Expected Svg element");
        }
    }

    #[test]
    fn test_parse_svg_path_commands() {
        let input = r#"Svg:
  path: M0 0 L10 10 H20 V30 C40 40 50 50 60 60 Q70 70 80 80 A5 5 0 1 1 90 90 Z
"#;
        let doc = parse(input).unwrap();

        if let Element::Svg(svg) = &doc.elements[0] {
            let cmds = &svg.paths[0].commands;
            assert!(cmds.len() >= 7);
            assert!(matches!(cmds[0], SvgPathCommand::MoveTo { x: 0.0, y: 0.0 }));
            assert!(matches!(cmds[1], SvgPathCommand::LineTo { x: 10.0, y: 10.0 }));
            assert!(matches!(cmds[2], SvgPathCommand::HorizontalTo { x: 20.0 }));
            assert!(matches!(cmds[3], SvgPathCommand::VerticalTo { y: 30.0 }));
            assert!(matches!(cmds[4], SvgPathCommand::CubicTo { .. }));
            assert!(matches!(cmds[5], SvgPathCommand::QuadTo { .. }));
            assert!(matches!(cmds[6], SvgPathCommand::ArcTo { .. }));
            assert!(matches!(cmds[cmds.len() - 1], SvgPathCommand::ClosePath));
        } else {
            panic!("Expected Svg element");
        }
    }

    // Grid layout parsing tests

    #[test]
    fn test_parse_grid_tracks_simple() {
        let tracks = parse_grid_tracks("[1fr, 2fr, auto]").unwrap();
        assert_eq!(tracks.len(), 3);
        assert!(matches!(tracks[0], GridTrackSize::Fraction(f) if (f - 1.0).abs() < 0.001));
        assert!(matches!(tracks[1], GridTrackSize::Fraction(f) if (f - 2.0).abs() < 0.001));
        assert!(matches!(tracks[2], GridTrackSize::Auto));
    }

    #[test]
    fn test_parse_grid_tracks_fixed() {
        let tracks = parse_grid_tracks("[100px, 200px, 50px]").unwrap();
        assert_eq!(tracks.len(), 3);
        assert!(matches!(tracks[0], GridTrackSize::Fixed(f) if (f - 100.0).abs() < 0.001));
        assert!(matches!(tracks[1], GridTrackSize::Fixed(f) if (f - 200.0).abs() < 0.001));
        assert!(matches!(tracks[2], GridTrackSize::Fixed(f) if (f - 50.0).abs() < 0.001));
    }

    #[test]
    fn test_parse_grid_tracks_minmax() {
        let tracks = parse_grid_tracks("[minmax(100px, 1fr)]").unwrap();
        assert_eq!(tracks.len(), 1);
        if let GridTrackSize::MinMax { min, max } = &tracks[0] {
            assert!(matches!(**min, GridTrackSize::Fixed(f) if (f - 100.0).abs() < 0.001));
            assert!(matches!(**max, GridTrackSize::Fraction(f) if (f - 1.0).abs() < 0.001));
        } else {
            panic!("Expected MinMax track");
        }
    }

    #[test]
    fn test_parse_grid_repeat() {
        let tracks = parse_grid_repeat("repeat(3, 1fr)").unwrap();
        assert_eq!(tracks.len(), 1);
        if let GridTrackSize::Repeat { count, sizes } = &tracks[0] {
            assert!(matches!(count, RepeatCount::Count(3)));
            assert_eq!(sizes.len(), 1);
            assert!(matches!(sizes[0], GridTrackSize::Fraction(f) if (f - 1.0).abs() < 0.001));
        } else {
            panic!("Expected Repeat track");
        }
    }

    #[test]
    fn test_parse_grid_repeat_auto_fill() {
        let tracks = parse_grid_repeat("repeat(auto-fill, minmax(200px, 1fr))").unwrap();
        assert_eq!(tracks.len(), 1);
        if let GridTrackSize::Repeat { count, sizes } = &tracks[0] {
            assert!(matches!(count, RepeatCount::AutoFill));
            assert_eq!(sizes.len(), 1);
        } else {
            panic!("Expected Repeat track with auto-fill");
        }
    }

    #[test]
    fn test_parse_grid_line_single() {
        let line = parse_grid_line_value("1").unwrap();
        assert!(matches!(line.start, GridLine::Number(1)));
        assert!(line.end.is_none());
    }

    #[test]
    fn test_parse_grid_line_range() {
        let line = parse_grid_line_value("1 / 3").unwrap();
        assert!(matches!(line.start, GridLine::Number(1)));
        assert!(matches!(line.end, Some(GridLine::Number(3))));
    }

    #[test]
    fn test_parse_grid_line_negative() {
        let line = parse_grid_line_value("1 / -1").unwrap();
        assert!(matches!(line.start, GridLine::Number(1)));
        assert!(matches!(line.end, Some(GridLine::Number(-1))));
    }

    #[test]
    fn test_parse_grid_line_span() {
        let line = parse_grid_line_value("span 2").unwrap();
        assert!(matches!(line.start, GridLine::Span(2)));
        assert!(line.end.is_none());
    }

    #[test]
    fn test_parse_grid_frame() {
        let input = r#"Frame Grid:
  layout: grid
  grid-template-columns: [1fr, 2fr, 1fr]
  grid-template-rows: [auto, auto]
  gap: 16px
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.name.as_ref().unwrap().0, "Grid");

            // Find layout property
            let layout_prop = frame.properties.iter().find(|p| p.name == "layout").unwrap();
            assert!(matches!(layout_prop.value, PropertyValue::Enum(ref s) if s == "grid"));

            // Find columns property
            let cols_prop = frame.properties.iter().find(|p| p.name == "grid-template-columns").unwrap();
            if let PropertyValue::GridTracks(tracks) = &cols_prop.value {
                assert_eq!(tracks.len(), 3);
            } else {
                panic!("Expected GridTracks property");
            }

            // Find rows property
            let rows_prop = frame.properties.iter().find(|p| p.name == "grid-template-rows").unwrap();
            if let PropertyValue::GridTracks(tracks) = &rows_prop.value {
                assert_eq!(tracks.len(), 2);
            } else {
                panic!("Expected GridTracks property");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_grid_child_placement() {
        let input = r#"Frame Grid:
  layout: grid
  grid-template-columns: [1fr, 1fr]
  Frame Item:
    grid-column: 1 / 3
    grid-row: 1
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(grid) = &doc.elements[0] {
            assert_eq!(grid.children.len(), 1);
            if let Element::Frame(item) = &grid.children[0] {
                // Find grid-column property
                let col_prop = item.properties.iter().find(|p| p.name == "grid-column").unwrap();
                if let PropertyValue::GridLine(line) = &col_prop.value {
                    assert!(matches!(line.start, GridLine::Number(1)));
                    assert!(matches!(line.end, Some(GridLine::Number(3))));
                } else {
                    panic!("Expected GridLine property");
                }

                // Find grid-row property
                let row_prop = item.properties.iter().find(|p| p.name == "grid-row").unwrap();
                // Single number should be parsed as Number
                if let PropertyValue::Number(n) = row_prop.value {
                    assert!((n - 1.0).abs() < 0.001);
                } else if let PropertyValue::GridLine(line) = &row_prop.value {
                    assert!(matches!(line.start, GridLine::Number(1)));
                } else {
                    panic!("Expected Number or GridLine property, got {:?}", row_prop.value);
                }
            } else {
                panic!("Expected child Frame");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    // Part element tests

    #[test]
    fn test_parse_part_box() {
        let input = r#"Part MyBox:
  geometry: Box(100mm, 50mm, 25mm)
  color: #ff0000
"#;
        let doc = parse(input).unwrap();
        assert_eq!(doc.elements.len(), 1);

        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "MyBox");
            if let Geometry::Primitive(Primitive::Box { width, height, depth }) = &part.geometry {
                assert_eq!(width.value, 100.0);
                assert_eq!(width.unit, LengthUnit::Mm);
                assert_eq!(height.value, 50.0);
                assert_eq!(depth.value, 25.0);
            } else {
                panic!("Expected Box geometry");
            }
            // color property should be preserved
            assert!(part.properties.iter().any(|p| p.name == "color"));
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_part_sphere() {
        let input = r#"Part Ball:
  geometry: Sphere(50mm)
"#;
        let doc = parse(input).unwrap();

        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "Ball");
            if let Geometry::Primitive(Primitive::Sphere { radius }) = &part.geometry {
                assert_eq!(radius.value, 50.0);
                assert_eq!(radius.unit, LengthUnit::Mm);
            } else {
                panic!("Expected Sphere geometry");
            }
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_part_cylinder() {
        let input = r#"Part Tube:
  geometry: Cylinder(10mm, 100mm)
"#;
        let doc = parse(input).unwrap();

        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "Tube");
            if let Geometry::Primitive(Primitive::Cylinder { radius, height }) = &part.geometry {
                assert_eq!(radius.value, 10.0);
                assert_eq!(height.value, 100.0);
            } else {
                panic!("Expected Cylinder geometry");
            }
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_part_nested_in_frame() {
        let input = r#"Frame Assembly:
  Part Bolt:
    geometry: Cylinder(5mm, 20mm)
  Part Washer:
    geometry: Cylinder(10mm, 2mm)
"#;
        let doc = parse(input).unwrap();

        if let Element::Frame(frame) = &doc.elements[0] {
            assert_eq!(frame.name.as_ref().unwrap().0, "Assembly");
            assert_eq!(frame.children.len(), 2);

            if let Element::Part(bolt) = &frame.children[0] {
                assert_eq!(bolt.name.as_ref().unwrap().0, "Bolt");
            } else {
                panic!("Expected Part element for Bolt");
            }

            if let Element::Part(washer) = &frame.children[1] {
                assert_eq!(washer.name.as_ref().unwrap().0, "Washer");
            } else {
                panic!("Expected Part element for Washer");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_parse_part_without_name() {
        let input = r#"Part:
  geometry: Box(10mm, 10mm, 10mm)
"#;
        let doc = parse(input).unwrap();

        if let Element::Part(part) = &doc.elements[0] {
            assert!(part.name.is_none());
            assert!(matches!(part.geometry, Geometry::Primitive(Primitive::Box { .. })));
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_geometry_with_px_units() {
        let input = r#"Part:
  geometry: Box(100px, 50px, 25px)
"#;
        let doc = parse(input).unwrap();

        if let Element::Part(part) = &doc.elements[0] {
            if let Geometry::Primitive(Primitive::Box { width, .. }) = &part.geometry {
                assert_eq!(width.value, 100.0);
                assert_eq!(width.unit, LengthUnit::Px);
            } else {
                panic!("Expected Box geometry");
            }
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_part_import() {
        let input = r#"Part Gear:
  geometry: Import("./meshes/gear.step")
"#;
        let doc = parse(input).unwrap();
        assert_eq!(doc.elements.len(), 1);

        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "Gear");
            if let Geometry::Import(import) = &part.geometry {
                assert_eq!(import.path, "./meshes/gear.step");
                assert!(import.format.is_none());
                assert!(import.bounds.is_none());
            } else {
                panic!("Expected Import geometry");
            }
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_part_import_with_format() {
        let input = r#"Part Assembly:
  geometry: Import("model.glb", format: "gltf")
  color: #4080ff
"#;
        let doc = parse(input).unwrap();

        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "Assembly");
            if let Geometry::Import(import) = &part.geometry {
                assert_eq!(import.path, "model.glb");
                assert_eq!(import.format, Some("gltf".to_string()));
            } else {
                panic!("Expected Import geometry");
            }
            // Should also have color property
            assert!(!part.properties.is_empty());
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_just_meta() {
        let input = r#"@meta:
  profile: Seed/3D
  version: 1.0
"#;
        let doc = parse(input).unwrap();
        assert!(doc.meta.is_some());
        let meta = doc.meta.unwrap();
        assert_eq!(meta.profile, Profile::Seed3D);
        assert_eq!(meta.version.as_ref().unwrap(), "1.0");
    }

    #[test]
    fn test_parse_part_with_color_property() {
        // Test that Part can have a color property after geometry
        let input = r#"Part RedBox:
  geometry: Box(100mm, 50mm, 20mm)
  color: #ff0000
"#;
        let doc = parse(input).unwrap();
        assert_eq!(doc.elements.len(), 1);

        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "RedBox");
            // Check properties
            let color_prop = part.properties.iter().find(|p| p.name == "color");
            assert!(color_prop.is_some(), "Should have color property");
        } else {
            panic!("Expected Part element");
        }
    }

    #[test]
    fn test_parse_full_document_with_meta_and_part() {
        let input = r#"@meta:
  profile: Seed/3D
  version: 1.0

Part RedBox:
  geometry: Box(100mm, 50mm, 20mm)
  color: #ff0000
"#;
        let doc = parse(input).unwrap();

        // Verify meta
        assert!(doc.meta.is_some());
        let meta = doc.meta.unwrap();
        assert_eq!(meta.profile, Profile::Seed3D);
        assert_eq!(meta.version.as_ref().unwrap(), "1.0");

        // Verify element
        assert_eq!(doc.elements.len(), 1);
        if let Element::Part(part) = &doc.elements[0] {
            assert_eq!(part.name.as_ref().unwrap().0, "RedBox");
        } else {
            panic!("Expected Part element");
        }
    }
}
