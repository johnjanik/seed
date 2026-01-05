//! Token resolution.
//!
//! Resolves token references ($token.path) to their actual values.

use std::collections::HashSet;
use seed_core::{
    Document, TokenMap, ResolveError, ResolvedToken,
    ast::{
        Element, FrameElement, TextElement, PartElement, ComponentElement,
        Property, PropertyValue, TextContent, TokenPath, Constraint,
        Expression, ConstraintKind,
    },
};

/// Resolve all token references in a document.
pub fn resolve_tokens(doc: &Document, tokens: &TokenMap) -> Result<Document, ResolveError> {
    let mut resolver = TokenResolver::new(tokens);
    resolver.resolve_document(doc)
}

struct TokenResolver<'a> {
    tokens: &'a TokenMap,
    /// Track the current resolution path for circular reference detection.
    resolution_stack: Vec<String>,
}

impl<'a> TokenResolver<'a> {
    fn new(tokens: &'a TokenMap) -> Self {
        Self {
            tokens,
            resolution_stack: Vec::new(),
        }
    }

    fn resolve_document(&mut self, doc: &Document) -> Result<Document, ResolveError> {
        let mut resolved = doc.clone();

        // Resolve elements
        resolved.elements = doc.elements
            .iter()
            .map(|e| self.resolve_element(e))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_element(&mut self, element: &Element) -> Result<Element, ResolveError> {
        match element {
            Element::Frame(frame) => {
                Ok(Element::Frame(self.resolve_frame(frame)?))
            }
            Element::Text(text) => {
                Ok(Element::Text(self.resolve_text(text)?))
            }
            Element::Part(part) => {
                Ok(Element::Part(self.resolve_part(part)?))
            }
            Element::Component(comp) => {
                Ok(Element::Component(self.resolve_component(comp)?))
            }
            Element::Slot(slot) => {
                // Slots are handled during component expansion, pass through
                Ok(Element::Slot(slot.clone()))
            }
        }
    }

    fn resolve_frame(&mut self, frame: &FrameElement) -> Result<FrameElement, ResolveError> {
        let mut resolved = frame.clone();

        // Resolve properties
        resolved.properties = frame.properties
            .iter()
            .map(|p| self.resolve_property(p))
            .collect::<Result<Vec<_>, _>>()?;

        // Resolve constraints
        resolved.constraints = frame.constraints
            .iter()
            .map(|c| self.resolve_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        // Resolve children
        resolved.children = frame.children
            .iter()
            .map(|e| self.resolve_element(e))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_text(&mut self, text: &TextElement) -> Result<TextElement, ResolveError> {
        let mut resolved = text.clone();

        // Resolve text content if it's a token reference
        resolved.content = self.resolve_text_content(&text.content)?;

        // Resolve properties
        resolved.properties = text.properties
            .iter()
            .map(|p| self.resolve_property(p))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_part(&mut self, part: &PartElement) -> Result<PartElement, ResolveError> {
        let mut resolved = part.clone();

        // Resolve properties
        resolved.properties = part.properties
            .iter()
            .map(|p| self.resolve_property(p))
            .collect::<Result<Vec<_>, _>>()?;

        // Resolve constraints
        resolved.constraints = part.constraints
            .iter()
            .map(|c| self.resolve_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_component(&mut self, comp: &ComponentElement) -> Result<ComponentElement, ResolveError> {
        let mut resolved = comp.clone();

        // Resolve props
        resolved.props = comp.props
            .iter()
            .map(|p| self.resolve_property(p))
            .collect::<Result<Vec<_>, _>>()?;

        // Resolve children
        resolved.children = comp.children
            .iter()
            .map(|e| self.resolve_element(e))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_property(&mut self, prop: &Property) -> Result<Property, ResolveError> {
        let mut resolved = prop.clone();
        resolved.value = self.resolve_property_value(&prop.value, &prop.span)?;
        Ok(resolved)
    }

    fn resolve_property_value(
        &mut self,
        value: &PropertyValue,
        span: &seed_core::ast::Span,
    ) -> Result<PropertyValue, ResolveError> {
        match value {
            PropertyValue::TokenRef(path) => {
                self.resolve_token_to_property_value(path, span)
            }
            _ => Ok(value.clone()),
        }
    }

    fn resolve_token_to_property_value(
        &mut self,
        path: &TokenPath,
        span: &seed_core::ast::Span,
    ) -> Result<PropertyValue, ResolveError> {
        let path_str = path.0.join(".");

        // Check for circular reference
        if self.resolution_stack.contains(&path_str) {
            let mut cycle = self.resolution_stack.clone();
            cycle.push(path_str);
            return Err(ResolveError::CircularTokenReference { cycle });
        }

        // Get the token
        let token = self.tokens.get(&path_str)
            .ok_or_else(|| ResolveError::UndefinedToken {
                path: path_str.clone(),
                span: span.clone(),
            })?;

        // Convert ResolvedToken to PropertyValue
        match token {
            ResolvedToken::Color(c) => Ok(PropertyValue::Color(*c)),
            ResolvedToken::Length(l) => Ok(PropertyValue::Length(*l)),
            ResolvedToken::Number(n) => Ok(PropertyValue::Number(*n)),
            ResolvedToken::String(s) => Ok(PropertyValue::String(s.clone())),
        }
    }

    fn resolve_text_content(&mut self, content: &TextContent) -> Result<TextContent, ResolveError> {
        match content {
            TextContent::TokenRef(path) => {
                let path_str = path.0.join(".");

                // Check for circular reference
                if self.resolution_stack.contains(&path_str) {
                    let mut cycle = self.resolution_stack.clone();
                    cycle.push(path_str);
                    return Err(ResolveError::CircularTokenReference { cycle });
                }

                // Get the token
                if let Some(token) = self.tokens.get(&path_str) {
                    // Convert to literal string
                    let text = match token {
                        ResolvedToken::String(s) => s.clone(),
                        ResolvedToken::Number(n) => n.to_string(),
                        ResolvedToken::Color(c) => {
                            let (r, g, b, _) = c.to_rgba8();
                            format!("#{:02x}{:02x}{:02x}", r, g, b)
                        }
                        ResolvedToken::Length(l) => format!("{:?}", l),
                    };
                    Ok(TextContent::Literal(text))
                } else {
                    // Keep as token ref if not found (might be resolved later)
                    Ok(content.clone())
                }
            }
            TextContent::Literal(_) => Ok(content.clone()),
        }
    }

    fn resolve_constraint(&mut self, constraint: &Constraint) -> Result<Constraint, ResolveError> {
        let mut resolved = constraint.clone();
        resolved.kind = self.resolve_constraint_kind(&constraint.kind, &constraint.span)?;
        Ok(resolved)
    }

    fn resolve_constraint_kind(
        &mut self,
        kind: &ConstraintKind,
        span: &seed_core::ast::Span,
    ) -> Result<ConstraintKind, ResolveError> {
        match kind {
            ConstraintKind::Equality { property, value } => {
                Ok(ConstraintKind::Equality {
                    property: property.clone(),
                    value: self.resolve_expression(value, span)?,
                })
            }
            // Other constraint kinds don't contain token references
            _ => Ok(kind.clone()),
        }
    }

    fn resolve_expression(
        &mut self,
        expr: &Expression,
        span: &seed_core::ast::Span,
    ) -> Result<Expression, ResolveError> {
        match expr {
            Expression::TokenRef(path) => {
                let path_str = path.0.join(".");

                // Check for circular reference
                if self.resolution_stack.contains(&path_str) {
                    let mut cycle = self.resolution_stack.clone();
                    cycle.push(path_str);
                    return Err(ResolveError::CircularTokenReference { cycle });
                }

                // Get the token
                if let Some(token) = self.tokens.get(&path_str) {
                    match token {
                        ResolvedToken::Number(n) => Ok(Expression::Literal(*n)),
                        ResolvedToken::Length(l) => {
                            if let Some(px) = l.to_px(None) {
                                Ok(Expression::Literal(px))
                            } else {
                                Ok(expr.clone())
                            }
                        }
                        _ => Ok(expr.clone()),
                    }
                } else {
                    Err(ResolveError::UndefinedToken {
                        path: path_str,
                        span: span.clone(),
                    })
                }
            }
            Expression::BinaryOp { left, op, right } => {
                Ok(Expression::BinaryOp {
                    left: Box::new(self.resolve_expression(left, span)?),
                    op: *op,
                    right: Box::new(self.resolve_expression(right, span)?),
                })
            }
            Expression::Function { name, args } => {
                let resolved_args = args
                    .iter()
                    .map(|a| self.resolve_expression(a, span))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Expression::Function {
                    name: name.clone(),
                    args: resolved_args,
                })
            }
            _ => Ok(expr.clone()),
        }
    }
}

/// Resolve token definitions within a token block (handles token-to-token references).
#[allow(dead_code)]
pub fn resolve_token_definitions(
    tokens: &mut TokenMap,
    definitions: &[(String, seed_core::ast::TokenValue)],
) -> Result<(), ResolveError> {
    let mut resolved_set: HashSet<String> = HashSet::new();

    for (path, value) in definitions {
        resolve_token_value(tokens, path, value, &mut resolved_set, &mut Vec::new())?;
    }

    Ok(())
}

fn resolve_token_value(
    tokens: &mut TokenMap,
    path: &str,
    value: &seed_core::ast::TokenValue,
    resolved_set: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Result<ResolvedToken, ResolveError> {
    // Already resolved?
    if let Some(resolved) = tokens.get(path) {
        return Ok(resolved.clone());
    }

    // Check for circular reference
    if stack.contains(&path.to_string()) {
        let mut cycle = stack.clone();
        cycle.push(path.to_string());
        return Err(ResolveError::CircularTokenReference { cycle });
    }

    stack.push(path.to_string());

    let resolved = match value {
        seed_core::ast::TokenValue::Color(c) => ResolvedToken::Color(*c),
        seed_core::ast::TokenValue::Length(l) => ResolvedToken::Length(*l),
        seed_core::ast::TokenValue::Number(n) => ResolvedToken::Number(*n),
        seed_core::ast::TokenValue::String(s) => ResolvedToken::String(s.clone()),
        seed_core::ast::TokenValue::Reference(ref_path) => {
            let ref_path_str = ref_path.0.join(".");

            // The referenced token should already be defined
            if let Some(ref_value) = tokens.get(&ref_path_str) {
                ref_value.clone()
            } else {
                return Err(ResolveError::UndefinedToken {
                    path: ref_path_str,
                    span: seed_core::ast::Span::default(),
                });
            }
        }
    };

    stack.pop();
    tokens.insert(path, resolved.clone());
    resolved_set.insert(path.to_string());

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::Span;
    use seed_core::types::{Color, Length};

    fn make_token_path(parts: &[&str]) -> TokenPath {
        TokenPath(parts.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn test_resolve_color_token() {
        let mut tokens = TokenMap::new();
        tokens.insert("colors.primary", ResolvedToken::Color(Color::rgb(1.0, 0.0, 0.0)));

        let prop = Property {
            name: "fill".to_string(),
            value: PropertyValue::TokenRef(make_token_path(&["colors", "primary"])),
            span: Span::default(),
        };

        let mut resolver = TokenResolver::new(&tokens);
        let resolved = resolver.resolve_property(&prop).unwrap();

        match resolved.value {
            PropertyValue::Color(c) => {
                assert!((c.r - 1.0).abs() < 0.001);
                assert!(c.g.abs() < 0.001);
            }
            _ => panic!("Expected color"),
        }
    }

    #[test]
    fn test_resolve_length_token() {
        let mut tokens = TokenMap::new();
        tokens.insert("spacing.medium", ResolvedToken::Length(Length::px(16.0)));

        let prop = Property {
            name: "padding".to_string(),
            value: PropertyValue::TokenRef(make_token_path(&["spacing", "medium"])),
            span: Span::default(),
        };

        let mut resolver = TokenResolver::new(&tokens);
        let resolved = resolver.resolve_property(&prop).unwrap();

        match resolved.value {
            PropertyValue::Length(l) => {
                assert!((l.to_px(None).unwrap() - 16.0).abs() < 0.001);
            }
            _ => panic!("Expected length"),
        }
    }

    #[test]
    fn test_undefined_token_error() {
        let tokens = TokenMap::new();

        let prop = Property {
            name: "fill".to_string(),
            value: PropertyValue::TokenRef(make_token_path(&["nonexistent", "token"])),
            span: Span::default(),
        };

        let mut resolver = TokenResolver::new(&tokens);
        let result = resolver.resolve_property(&prop);

        assert!(matches!(result, Err(ResolveError::UndefinedToken { .. })));
    }

    #[test]
    fn test_resolve_text_content_token() {
        let mut tokens = TokenMap::new();
        tokens.insert("labels.submit", ResolvedToken::String("Submit".to_string()));

        let content = TextContent::TokenRef(make_token_path(&["labels", "submit"]));

        let mut resolver = TokenResolver::new(&tokens);
        let resolved = resolver.resolve_text_content(&content).unwrap();

        match resolved {
            TextContent::Literal(s) => assert_eq!(s, "Submit"),
            _ => panic!("Expected literal"),
        }
    }

    #[test]
    fn test_circular_reference_detection() {
        let tokens = TokenMap::new();
        // Manually set up a situation where we check for circular refs
        // (In reality, this would be caught during token definition resolution)

        let mut resolver = TokenResolver::new(&tokens);
        resolver.resolution_stack.push("a".to_string());
        resolver.resolution_stack.push("b".to_string());

        let path = make_token_path(&["a"]);
        let result = resolver.resolve_token_to_property_value(&path, &Span::default());

        assert!(matches!(result, Err(ResolveError::CircularTokenReference { .. })));
    }

    #[test]
    fn test_resolve_empty_document() {
        let tokens = TokenMap::new();
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };

        let result = resolve_tokens(&doc, &tokens);
        assert!(result.is_ok());
    }
}
