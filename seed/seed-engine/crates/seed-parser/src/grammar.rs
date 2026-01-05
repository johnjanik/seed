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

        while self.current().is_some() {
            if let Some(elem) = self.parse_element(0)? {
                elements.push(elem);
            }
        }

        Ok(Document {
            meta: None,
            tokens: None,
            elements,
            span: Span::default(),
        })
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
            if content.starts_with("Frame ") || content.starts_with("Text ")
               || content == "Frame:" || content == "Text:" {
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
}
