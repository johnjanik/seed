//! Element reference resolution.
//!
//! Resolves element references (Parent, Named, Previous, Next) to ensure
//! they are valid and can be looked up during constraint solving.

use std::collections::HashMap;
use seed_core::{
    Document, ResolveError,
    ast::{
        Element, FrameElement, TextElement, PartElement, ComponentElement,
        ImageElement, IconElement,
        Constraint, ConstraintKind, ElementRef,
    },
};

/// Resolve all element references in a document.
pub fn resolve_references(doc: &Document) -> Result<Document, ResolveError> {
    let mut resolver = ReferenceResolver::new();
    resolver.resolve_document(doc)
}

/// Element resolution context.
struct ElementContext {
    /// Named elements at this level.
    named_elements: HashMap<String, usize>,
    /// Whether we have a parent.
    has_parent: bool,
    /// Index of current element (for sibling references).
    current_index: usize,
    /// Total number of siblings.
    sibling_count: usize,
}

impl ElementContext {
    fn new() -> Self {
        Self {
            named_elements: HashMap::new(),
            has_parent: false,
            current_index: 0,
            sibling_count: 0,
        }
    }

    fn with_parent() -> Self {
        Self {
            named_elements: HashMap::new(),
            has_parent: true,
            current_index: 0,
            sibling_count: 0,
        }
    }
}

struct ReferenceResolver {
    /// Stack of contexts as we descend into nested elements.
    context_stack: Vec<ElementContext>,
}

impl ReferenceResolver {
    fn new() -> Self {
        Self {
            context_stack: Vec::new(),
        }
    }

    fn resolve_document(&mut self, doc: &Document) -> Result<Document, ResolveError> {
        let mut resolved = doc.clone();

        // Build element name index at document level
        let mut ctx = ElementContext::new();
        ctx.sibling_count = doc.elements.len();
        for (i, element) in doc.elements.iter().enumerate() {
            if let Some(name) = get_element_name(element) {
                ctx.named_elements.insert(name.clone(), i);
            }
        }
        self.context_stack.push(ctx);

        // Resolve elements
        resolved.elements = doc.elements
            .iter()
            .enumerate()
            .map(|(i, e)| {
                if let Some(ctx) = self.context_stack.last_mut() {
                    ctx.current_index = i;
                }
                self.resolve_element(e)
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.context_stack.pop();
        Ok(resolved)
    }

    fn resolve_element(&mut self, element: &Element) -> Result<Element, ResolveError> {
        match element {
            Element::Frame(frame) => Ok(Element::Frame(self.resolve_frame(frame)?)),
            Element::Text(text) => Ok(Element::Text(self.resolve_text(text)?)),
            Element::Svg(svg) => Ok(Element::Svg(self.resolve_svg(svg)?)),
            Element::Image(image) => Ok(Element::Image(self.resolve_image(image)?)),
            Element::Icon(icon) => Ok(Element::Icon(self.resolve_icon(icon)?)),
            Element::Part(part) => Ok(Element::Part(self.resolve_part(part)?)),
            Element::Component(comp) => Ok(Element::Component(self.resolve_component(comp)?)),
            Element::Slot(slot) => Ok(Element::Slot(slot.clone())),
        }
    }

    fn resolve_image(&mut self, image: &ImageElement) -> Result<ImageElement, ResolveError> {
        let mut resolved = image.clone();

        // Validate constraints
        resolved.constraints = image.constraints
            .iter()
            .map(|c| self.validate_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_icon(&mut self, icon: &IconElement) -> Result<IconElement, ResolveError> {
        let mut resolved = icon.clone();

        // Validate constraints
        resolved.constraints = icon.constraints
            .iter()
            .map(|c| self.validate_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_frame(&mut self, frame: &FrameElement) -> Result<FrameElement, ResolveError> {
        let mut resolved = frame.clone();

        // Validate constraints
        resolved.constraints = frame.constraints
            .iter()
            .map(|c| self.validate_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        // Resolve children
        if !frame.children.is_empty() {
            // Push new context for children
            let mut child_ctx = ElementContext::with_parent();
            child_ctx.sibling_count = frame.children.len();
            for (i, child) in frame.children.iter().enumerate() {
                if let Some(name) = get_element_name(child) {
                    child_ctx.named_elements.insert(name.clone(), i);
                }
            }
            self.context_stack.push(child_ctx);

            resolved.children = frame.children
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    if let Some(ctx) = self.context_stack.last_mut() {
                        ctx.current_index = i;
                    }
                    self.resolve_element(e)
                })
                .collect::<Result<Vec<_>, _>>()?;

            self.context_stack.pop();
        }

        Ok(resolved)
    }

    fn resolve_text(&mut self, text: &TextElement) -> Result<TextElement, ResolveError> {
        // Text elements don't have constraints or children
        Ok(text.clone())
    }

    fn resolve_svg(&mut self, svg: &seed_core::ast::SvgElement) -> Result<seed_core::ast::SvgElement, ResolveError> {
        let mut resolved = svg.clone();

        // Validate constraints
        resolved.constraints = svg.constraints
            .iter()
            .map(|c| self.validate_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_part(&mut self, part: &PartElement) -> Result<PartElement, ResolveError> {
        let mut resolved = part.clone();

        // Validate constraints
        resolved.constraints = part.constraints
            .iter()
            .map(|c| self.validate_constraint(c))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(resolved)
    }

    fn resolve_component(&mut self, comp: &ComponentElement) -> Result<ComponentElement, ResolveError> {
        let mut resolved = comp.clone();

        // Resolve children
        if !comp.children.is_empty() {
            // Push new context for children
            let mut child_ctx = ElementContext::with_parent();
            child_ctx.sibling_count = comp.children.len();
            for (i, child) in comp.children.iter().enumerate() {
                if let Some(name) = get_element_name(child) {
                    child_ctx.named_elements.insert(name.clone(), i);
                }
            }
            self.context_stack.push(child_ctx);

            resolved.children = comp.children
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    if let Some(ctx) = self.context_stack.last_mut() {
                        ctx.current_index = i;
                    }
                    self.resolve_element(e)
                })
                .collect::<Result<Vec<_>, _>>()?;

            self.context_stack.pop();
        }

        Ok(resolved)
    }

    fn validate_constraint(&self, constraint: &Constraint) -> Result<Constraint, ResolveError> {
        // Validate element references in the constraint
        match &constraint.kind {
            ConstraintKind::Alignment { target, .. } => {
                self.validate_element_ref(target, &constraint.span)?;
            }
            ConstraintKind::Relative { target, .. } => {
                self.validate_element_ref(target, &constraint.span)?;
            }
            ConstraintKind::Equality { .. } | ConstraintKind::Inequality { .. } => {
                // Equality/Inequality constraints use expressions, not direct element refs
            }
        }
        Ok(constraint.clone())
    }

    fn validate_element_ref(
        &self,
        element_ref: &ElementRef,
        span: &seed_core::ast::Span,
    ) -> Result<(), ResolveError> {
        let ctx = self.context_stack.last();

        match element_ref {
            ElementRef::Parent => {
                if let Some(ctx) = ctx {
                    if !ctx.has_parent {
                        return Err(ResolveError::InvalidReference {
                            reference: "Parent".to_string(),
                            reason: "no parent element exists at document level".to_string(),
                            span: *span,
                        });
                    }
                }
            }
            ElementRef::Named(name) => {
                let name_str = &name.0;
                let found = ctx.map(|c| c.named_elements.contains_key(name_str)).unwrap_or(false);
                if !found {
                    return Err(ResolveError::InvalidReference {
                        reference: name_str.clone(),
                        reason: format!("no element named '{}' found in scope", name_str),
                        span: *span,
                    });
                }
            }
            ElementRef::Previous => {
                if let Some(ctx) = ctx {
                    if ctx.current_index == 0 {
                        return Err(ResolveError::InvalidReference {
                            reference: "Previous".to_string(),
                            reason: "no previous sibling exists (this is the first element)".to_string(),
                            span: *span,
                        });
                    }
                }
            }
            ElementRef::Next => {
                if let Some(ctx) = ctx {
                    if ctx.current_index + 1 >= ctx.sibling_count {
                        return Err(ResolveError::InvalidReference {
                            reference: "Next".to_string(),
                            reason: "no next sibling exists (this is the last element)".to_string(),
                            span: *span,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Get the name of an element if it has one.
fn get_element_name(element: &Element) -> Option<String> {
    match element {
        Element::Frame(f) => f.name.as_ref().map(|id| id.0.clone()),
        Element::Text(t) => t.name.as_ref().map(|id| id.0.clone()),
        Element::Svg(s) => s.name.as_ref().map(|id| id.0.clone()),
        Element::Image(i) => i.name.as_ref().map(|id| id.0.clone()),
        Element::Icon(i) => i.name.as_ref().map(|id| id.0.clone()),
        Element::Part(p) => p.name.as_ref().map(|id| id.0.clone()),
        Element::Component(c) => c.instance_name.as_ref().map(|id| id.0.clone()),
        Element::Slot(_) => None, // Slots don't have names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::Span;
    use seed_core::types::Identifier;

    #[test]
    fn test_resolve_empty_document() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parent_reference_at_document_level() {
        // Create a frame with a Parent reference at the document level
        let frame = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![Constraint {
                kind: ConstraintKind::Alignment {
                    edge: seed_core::ast::Edge::Left,
                    target: ElementRef::Parent,
                    target_edge: None,
                },
                priority: None,
                span: Span::default(),
            }],
            children: vec![],
            span: Span::default(),
        };

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![Element::Frame(frame)],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(matches!(result, Err(ResolveError::InvalidReference { .. })));
    }

    #[test]
    fn test_parent_reference_in_nested_element() {
        // Create a nested frame with a Parent reference
        let child = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![Constraint {
                kind: ConstraintKind::Alignment {
                    edge: seed_core::ast::Edge::Left,
                    target: ElementRef::Parent,
                    target_edge: None,
                },
                priority: None,
                span: Span::default(),
            }],
            children: vec![],
            span: Span::default(),
        };

        let parent = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![],
            children: vec![Element::Frame(child)],
            span: Span::default(),
        };

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![Element::Frame(parent)],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_named_reference_not_found() {
        let frame = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![Constraint {
                kind: ConstraintKind::Alignment {
                    edge: seed_core::ast::Edge::Left,
                    target: ElementRef::Named(Identifier("Header".to_string())),
                    target_edge: None,
                },
                priority: None,
                span: Span::default(),
            }],
            children: vec![],
            span: Span::default(),
        };

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![Element::Frame(frame)],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(matches!(result, Err(ResolveError::InvalidReference { .. })));
    }

    #[test]
    fn test_named_reference_found() {
        let header = FrameElement {
            name: Some(Identifier("Header".to_string())),
            properties: vec![],
            constraints: vec![],
            children: vec![],
            span: Span::default(),
        };

        let content = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![Constraint {
                kind: ConstraintKind::Relative {
                    relation: seed_core::ast::Relation::Below,
                    target: ElementRef::Named(Identifier("Header".to_string())),
                    gap: None,
                },
                priority: None,
                span: Span::default(),
            }],
            children: vec![],
            span: Span::default(),
        };

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![Element::Frame(header), Element::Frame(content)],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(result.is_ok());
    }

    #[test]
    fn test_previous_sibling_first_element() {
        let frame = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![Constraint {
                kind: ConstraintKind::Relative {
                    relation: seed_core::ast::Relation::Below,
                    target: ElementRef::Previous,
                    gap: None,
                },
                priority: None,
                span: Span::default(),
            }],
            children: vec![],
            span: Span::default(),
        };

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![Element::Frame(frame)],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(matches!(result, Err(ResolveError::InvalidReference { .. })));
    }

    #[test]
    fn test_previous_sibling_valid() {
        let first = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![],
            children: vec![],
            span: Span::default(),
        };

        let second = FrameElement {
            name: None,
            properties: vec![],
            constraints: vec![Constraint {
                kind: ConstraintKind::Relative {
                    relation: seed_core::ast::Relation::Below,
                    target: ElementRef::Previous,
                    gap: None,
                },
                priority: None,
                span: Span::default(),
            }],
            children: vec![],
            span: Span::default(),
        };

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![Element::Frame(first), Element::Frame(second)],
            span: Span::default(),
        };

        let result = resolve_references(&doc);
        assert!(result.is_ok());
    }
}
