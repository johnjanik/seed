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
}
