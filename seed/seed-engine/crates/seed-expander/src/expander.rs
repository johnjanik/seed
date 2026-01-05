//! Component expansion logic.
//!
//! Expands ComponentElement nodes by:
//! 1. Looking up the component definition
//! 2. Validating props
//! 3. Substituting prop values into the template
//! 4. Injecting children into slots

use std::collections::HashMap;
use seed_core::{
    Document, ExpandError,
    ast::{
        Element, FrameElement, TextElement, PartElement, ComponentElement,
        SlotElement, Property, PropertyValue, TextContent,
    },
};
use crate::ComponentRegistry;

/// Maximum nesting depth for component expansion (prevent infinite recursion).
const MAX_EXPANSION_DEPTH: u32 = 100;

/// Expand all component instances in a document.
pub fn expand_components(
    doc: &Document,
    registry: &ComponentRegistry,
) -> Result<Document, ExpandError> {
    let mut expander = ComponentExpander::new(registry);
    expander.expand_document(doc)
}

/// Context for prop substitution during expansion.
struct PropContext {
    /// Map of prop name to value.
    props: HashMap<String, PropertyValue>,
}

impl PropContext {
    fn new() -> Self {
        Self {
            props: HashMap::new(),
        }
    }

    fn get(&self, name: &str) -> Option<&PropertyValue> {
        self.props.get(name)
    }

    fn insert(&mut self, name: String, value: PropertyValue) {
        self.props.insert(name, value);
    }
}

/// Component expander state.
struct ComponentExpander<'a> {
    registry: &'a ComponentRegistry,
    /// Current expansion depth (for circular reference detection).
    depth: u32,
    /// Stack of component names being expanded (for error messages).
    expansion_stack: Vec<String>,
}

impl<'a> ComponentExpander<'a> {
    fn new(registry: &'a ComponentRegistry) -> Self {
        Self {
            registry,
            depth: 0,
            expansion_stack: Vec::new(),
        }
    }

    fn expand_document(&mut self, doc: &Document) -> Result<Document, ExpandError> {
        let mut expanded = doc.clone();
        expanded.elements = self.expand_elements(&doc.elements)?;
        Ok(expanded)
    }

    fn expand_elements(&mut self, elements: &[Element]) -> Result<Vec<Element>, ExpandError> {
        let mut result = Vec::with_capacity(elements.len());
        for element in elements {
            result.extend(self.expand_element(element)?);
        }
        Ok(result)
    }

    fn expand_element(&mut self, element: &Element) -> Result<Vec<Element>, ExpandError> {
        match element {
            Element::Frame(frame) => {
                Ok(vec![Element::Frame(self.expand_frame(frame)?)])
            }
            Element::Text(text) => {
                Ok(vec![Element::Text(text.clone())])
            }
            Element::Svg(svg) => {
                // SVG elements don't have children to expand
                Ok(vec![Element::Svg(svg.clone())])
            }
            Element::Image(image) => {
                // Image elements don't have children to expand
                Ok(vec![Element::Image(image.clone())])
            }
            Element::Icon(icon) => {
                // Icon elements don't have children to expand
                Ok(vec![Element::Icon(icon.clone())])
            }
            Element::Part(part) => {
                Ok(vec![Element::Part(self.expand_part(part)?)])
            }
            Element::Component(comp) => {
                self.expand_component(comp)
            }
            Element::Slot(slot) => {
                // Slots should only appear in component templates, not in final output
                // If we see one here, it means no children were injected - use fallback
                Ok(slot.fallback.clone())
            }
        }
    }

    fn expand_frame(&mut self, frame: &FrameElement) -> Result<FrameElement, ExpandError> {
        let mut expanded = frame.clone();
        expanded.children = self.expand_elements(&frame.children)?;
        Ok(expanded)
    }

    fn expand_part(&mut self, part: &PartElement) -> Result<PartElement, ExpandError> {
        // Parts don't have children, just return as-is
        Ok(part.clone())
    }

    fn expand_component(&mut self, comp: &ComponentElement) -> Result<Vec<Element>, ExpandError> {
        let component_name = &comp.component_name.0;

        // Check depth limit
        if self.depth >= MAX_EXPANSION_DEPTH {
            return Err(ExpandError::MaxDepthExceeded { depth: MAX_EXPANSION_DEPTH });
        }

        // Look up the component definition
        let definition = self.registry.get(component_name)
            .ok_or_else(|| ExpandError::UndefinedComponent {
                name: component_name.clone(),
                span: comp.span,
            })?;

        // Build prop context with provided props and defaults
        let prop_context = self.build_prop_context(comp, definition)?;

        // Push onto expansion stack
        self.expansion_stack.push(component_name.clone());
        self.depth += 1;

        // Expand the template with prop substitution
        let expanded_template = self.expand_template(
            &definition.template,
            &prop_context,
            &comp.children,
        )?;

        // Pop from expansion stack
        self.depth -= 1;
        self.expansion_stack.pop();

        // Recursively expand any nested components
        self.expand_elements(&expanded_template)
    }

    fn build_prop_context(
        &self,
        comp: &ComponentElement,
        definition: &seed_core::ast::ComponentDefinition,
    ) -> Result<PropContext, ExpandError> {
        let mut context = PropContext::new();

        // First, add all defaults
        for prop_def in &definition.props {
            if let Some(default) = &prop_def.default {
                context.insert(prop_def.name.clone(), default.clone());
            }
        }

        // Then, override with provided props
        for prop in &comp.props {
            context.insert(prop.name.clone(), prop.value.clone());
        }

        // Validate required props
        for prop_def in &definition.props {
            if prop_def.required && context.get(&prop_def.name).is_none() {
                return Err(ExpandError::MissingRequiredProp {
                    component: comp.component_name.0.clone(),
                    prop: prop_def.name.clone(),
                    span: comp.span,
                });
            }
        }

        Ok(context)
    }

    fn expand_template(
        &self,
        template: &[Element],
        prop_context: &PropContext,
        children: &[Element],
    ) -> Result<Vec<Element>, ExpandError> {
        let mut result = Vec::with_capacity(template.len());

        for element in template {
            match element {
                Element::Frame(frame) => {
                    result.push(Element::Frame(self.substitute_frame(frame, prop_context, children)?));
                }
                Element::Text(text) => {
                    result.push(Element::Text(self.substitute_text(text, prop_context)?));
                }
                Element::Svg(svg) => {
                    // SVG elements can have property substitution
                    result.push(Element::Svg(self.substitute_svg(svg, prop_context)?));
                }
                Element::Image(image) => {
                    // Image elements can have property substitution
                    result.push(Element::Image(self.substitute_image(image, prop_context)?));
                }
                Element::Icon(icon) => {
                    // Icon elements can have property substitution
                    result.push(Element::Icon(self.substitute_icon(icon, prop_context)?));
                }
                Element::Part(part) => {
                    result.push(Element::Part(self.substitute_part(part, prop_context)?));
                }
                Element::Component(comp) => {
                    // Keep component for later expansion, but substitute props
                    result.push(Element::Component(self.substitute_component(comp, prop_context)?));
                }
                Element::Slot(slot) => {
                    // Inject children or use fallback
                    let injected = self.inject_slot(slot, children)?;
                    result.extend(injected);
                }
            }
        }

        Ok(result)
    }

    fn substitute_frame(
        &self,
        frame: &FrameElement,
        prop_context: &PropContext,
        children: &[Element],
    ) -> Result<FrameElement, ExpandError> {
        let mut result = frame.clone();

        // Substitute props in properties
        result.properties = frame.properties
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        // Recursively process children
        result.children = self.expand_template(&frame.children, prop_context, children)?;

        Ok(result)
    }

    fn substitute_text(
        &self,
        text: &TextElement,
        prop_context: &PropContext,
    ) -> Result<TextElement, ExpandError> {
        let mut result = text.clone();

        // Substitute props in properties
        result.properties = text.properties
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        // Substitute prop refs in text content
        result.content = self.substitute_text_content(&text.content, prop_context)?;

        Ok(result)
    }

    fn substitute_svg(
        &self,
        svg: &seed_core::ast::SvgElement,
        prop_context: &PropContext,
    ) -> Result<seed_core::ast::SvgElement, ExpandError> {
        let mut result = svg.clone();

        // Substitute props in properties
        result.properties = svg.properties
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }

    fn substitute_image(
        &self,
        image: &seed_core::ast::ImageElement,
        prop_context: &PropContext,
    ) -> Result<seed_core::ast::ImageElement, ExpandError> {
        let mut result = image.clone();

        // Substitute props in properties
        result.properties = image.properties
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }

    fn substitute_icon(
        &self,
        icon: &seed_core::ast::IconElement,
        prop_context: &PropContext,
    ) -> Result<seed_core::ast::IconElement, ExpandError> {
        let mut result = icon.clone();

        // Substitute props in properties
        result.properties = icon.properties
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }

    fn substitute_part(
        &self,
        part: &PartElement,
        prop_context: &PropContext,
    ) -> Result<PartElement, ExpandError> {
        let mut result = part.clone();

        // Substitute props in properties
        result.properties = part.properties
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }

    fn substitute_component(
        &self,
        comp: &ComponentElement,
        prop_context: &PropContext,
    ) -> Result<ComponentElement, ExpandError> {
        let mut result = comp.clone();

        // Substitute props in component props
        result.props = comp.props
            .iter()
            .map(|p| self.substitute_property(p, prop_context))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(result)
    }

    fn substitute_property(
        &self,
        property: &Property,
        prop_context: &PropContext,
    ) -> Result<Property, ExpandError> {
        let mut result = property.clone();
        result.value = self.substitute_value(&property.value, prop_context)?;
        Ok(result)
    }

    fn substitute_value(
        &self,
        value: &PropertyValue,
        prop_context: &PropContext,
    ) -> Result<PropertyValue, ExpandError> {
        match value {
            PropertyValue::PropRef(prop_ref) => {
                // Look up the prop value
                if let Some(prop_value) = prop_context.get(&prop_ref.0) {
                    Ok(prop_value.clone())
                } else {
                    // Prop not found - keep the reference (might be an error)
                    Ok(value.clone())
                }
            }
            _ => Ok(value.clone()),
        }
    }

    fn substitute_text_content(
        &self,
        content: &TextContent,
        _prop_context: &PropContext,
    ) -> Result<TextContent, ExpandError> {
        // Text content doesn't have prop refs in current AST
        // But we can extend this in the future
        Ok(content.clone())
    }

    fn inject_slot(
        &self,
        slot: &SlotElement,
        children: &[Element],
    ) -> Result<Vec<Element>, ExpandError> {
        if slot.name.is_none() {
            // Default slot - inject all children
            if children.is_empty() {
                // Use fallback
                Ok(slot.fallback.clone())
            } else {
                Ok(children.to_vec())
            }
        } else {
            // Named slot - for now, just use fallback
            // Named slot injection would require children to be tagged
            Ok(slot.fallback.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ComponentBuilder;
    use seed_core::ast::{PropType, PropRef, Span};
    use seed_core::types::{Identifier, Length};

    fn make_frame(name: Option<&str>, props: Vec<Property>, children: Vec<Element>) -> FrameElement {
        FrameElement {
            name: name.map(|n| Identifier(n.to_string())),
            properties: props,
            constraints: vec![],
            children,
            span: Span::default(),
        }
    }

    fn make_prop(name: &str, value: PropertyValue) -> Property {
        Property {
            name: name.to_string(),
            value,
            span: Span::default(),
        }
    }

    #[test]
    fn test_expand_empty_document() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![],
            span: Span::default(),
        };
        let registry = ComponentRegistry::new();

        let result = expand_components(&doc, &registry);
        assert!(result.is_ok());
        assert!(result.unwrap().elements.is_empty());
    }

    #[test]
    fn test_expand_no_components() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Frame(make_frame(Some("Container"), vec![], vec![])),
            ],
            span: Span::default(),
        };
        let registry = ComponentRegistry::new();

        let result = expand_components(&doc, &registry).unwrap();
        assert_eq!(result.elements.len(), 1);
    }

    #[test]
    fn test_undefined_component() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Unknown".to_string()),
                    instance_name: None,
                    props: vec![],
                    children: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };
        let registry = ComponentRegistry::new();

        let result = expand_components(&doc, &registry);
        assert!(matches!(result, Err(ExpandError::UndefinedComponent { .. })));
    }

    #[test]
    fn test_expand_simple_component() {
        // Create a simple Button component
        let button = ComponentBuilder::new("Button")
            .prop("label", PropType::String)
            .template(vec![
                Element::Frame(make_frame(
                    None,
                    vec![make_prop("fill", PropertyValue::Color(seed_core::types::Color::rgb(0.0, 0.5, 1.0)))],
                    vec![],
                )),
            ])
            .build();

        let mut registry = ComponentRegistry::new();
        registry.register(button);

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Button".to_string()),
                    instance_name: None,
                    props: vec![make_prop("label", PropertyValue::String("Click me".to_string()))],
                    children: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = expand_components(&doc, &registry).unwrap();
        assert_eq!(result.elements.len(), 1);

        // Should be a Frame now (the Button's template)
        assert!(matches!(result.elements[0], Element::Frame(_)));
    }

    #[test]
    fn test_missing_required_prop() {
        let button = ComponentBuilder::new("Button")
            .prop("label", PropType::String)
            .build();

        let mut registry = ComponentRegistry::new();
        registry.register(button);

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Button".to_string()),
                    instance_name: None,
                    props: vec![], // Missing required label prop
                    children: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = expand_components(&doc, &registry);
        assert!(matches!(result, Err(ExpandError::MissingRequiredProp { .. })));
    }

    #[test]
    fn test_optional_prop_with_default() {
        let card = ComponentBuilder::new("Card")
            .optional_prop("padding", PropType::Length, PropertyValue::Length(Length::px(16.0)))
            .template(vec![
                Element::Frame(make_frame(
                    None,
                    vec![make_prop("padding", PropertyValue::PropRef(PropRef("padding".to_string())))],
                    vec![],
                )),
            ])
            .build();

        let mut registry = ComponentRegistry::new();
        registry.register(card);

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Card".to_string()),
                    instance_name: None,
                    props: vec![], // Not providing padding, should use default
                    children: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = expand_components(&doc, &registry).unwrap();
        assert_eq!(result.elements.len(), 1);

        if let Element::Frame(frame) = &result.elements[0] {
            // The PropRef should have been substituted with the default value
            assert_eq!(frame.properties.len(), 1);
            assert!(matches!(frame.properties[0].value, PropertyValue::Length(_)));
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_slot_injection() {
        let container = ComponentBuilder::new("Container")
            .default_slot()
            .template(vec![
                Element::Frame(make_frame(
                    Some("Wrapper"),
                    vec![],
                    vec![
                        Element::Slot(SlotElement {
                            name: None,
                            fallback: vec![],
                            span: Span::default(),
                        }),
                    ],
                )),
            ])
            .build();

        let mut registry = ComponentRegistry::new();
        registry.register(container);

        // Use container with children
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Container".to_string()),
                    instance_name: None,
                    props: vec![],
                    children: vec![
                        Element::Frame(make_frame(Some("Child1"), vec![], vec![])),
                        Element::Frame(make_frame(Some("Child2"), vec![], vec![])),
                    ],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = expand_components(&doc, &registry).unwrap();
        assert_eq!(result.elements.len(), 1);

        if let Element::Frame(frame) = &result.elements[0] {
            // Children should be injected into the slot
            assert_eq!(frame.children.len(), 2);
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_slot_fallback() {
        let container = ComponentBuilder::new("Container")
            .default_slot()
            .template(vec![
                Element::Frame(make_frame(
                    Some("Wrapper"),
                    vec![],
                    vec![
                        Element::Slot(SlotElement {
                            name: None,
                            fallback: vec![
                                Element::Frame(make_frame(Some("DefaultChild"), vec![], vec![])),
                            ],
                            span: Span::default(),
                        }),
                    ],
                )),
            ])
            .build();

        let mut registry = ComponentRegistry::new();
        registry.register(container);

        // Use container without children
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Container".to_string()),
                    instance_name: None,
                    props: vec![],
                    children: vec![], // No children - should use fallback
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = expand_components(&doc, &registry).unwrap();
        assert_eq!(result.elements.len(), 1);

        if let Element::Frame(frame) = &result.elements[0] {
            // Fallback should be used
            assert_eq!(frame.children.len(), 1);
            if let Element::Frame(child) = &frame.children[0] {
                assert_eq!(child.name.as_ref().unwrap().0, "DefaultChild");
            } else {
                panic!("Expected Frame child");
            }
        } else {
            panic!("Expected Frame element");
        }
    }

    #[test]
    fn test_nested_component_expansion() {
        // Inner component
        let inner = ComponentBuilder::new("Inner")
            .template(vec![
                Element::Frame(make_frame(Some("InnerFrame"), vec![], vec![])),
            ])
            .build();

        // Outer component that uses Inner
        let outer = ComponentBuilder::new("Outer")
            .template(vec![
                Element::Frame(make_frame(
                    Some("OuterFrame"),
                    vec![],
                    vec![
                        Element::Component(ComponentElement {
                            component_name: Identifier("Inner".to_string()),
                            instance_name: None,
                            props: vec![],
                            children: vec![],
                            span: Span::default(),
                        }),
                    ],
                )),
            ])
            .build();

        let mut registry = ComponentRegistry::new();
        registry.register(inner);
        registry.register(outer);

        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![
                Element::Component(ComponentElement {
                    component_name: Identifier("Outer".to_string()),
                    instance_name: None,
                    props: vec![],
                    children: vec![],
                    span: Span::default(),
                }),
            ],
            span: Span::default(),
        };

        let result = expand_components(&doc, &registry).unwrap();
        assert_eq!(result.elements.len(), 1);

        // Should be OuterFrame containing InnerFrame
        if let Element::Frame(outer_frame) = &result.elements[0] {
            assert_eq!(outer_frame.name.as_ref().unwrap().0, "OuterFrame");
            assert_eq!(outer_frame.children.len(), 1);

            if let Element::Frame(inner_frame) = &outer_frame.children[0] {
                assert_eq!(inner_frame.name.as_ref().unwrap().0, "InnerFrame");
            } else {
                panic!("Expected InnerFrame");
            }
        } else {
            panic!("Expected OuterFrame");
        }
    }
}
