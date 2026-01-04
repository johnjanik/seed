//! Component registry for storing and looking up component definitions.

use std::collections::HashMap;
use seed_core::ast::{ComponentDefinition, PropDefinition, SlotDefinition, PropType, Span};
use seed_core::types::Identifier;

/// A registry of component definitions.
#[derive(Debug, Clone, Default)]
pub struct ComponentRegistry {
    components: HashMap<String, ComponentDefinition>,
}

impl ComponentRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Register a component definition.
    pub fn register(&mut self, component: ComponentDefinition) {
        self.components.insert(component.name.0.clone(), component);
    }

    /// Get a component by name.
    pub fn get(&self, name: &str) -> Option<&ComponentDefinition> {
        self.components.get(name)
    }

    /// Check if a component exists.
    pub fn contains(&self, name: &str) -> bool {
        self.components.contains_key(name)
    }

    /// Get all component names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.components.keys().map(|s| s.as_str())
    }

    /// Number of registered components.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

/// Builder for creating component definitions.
pub struct ComponentBuilder {
    name: String,
    props: Vec<PropDefinition>,
    slots: Vec<SlotDefinition>,
    template: Vec<seed_core::ast::Element>,
}

impl ComponentBuilder {
    /// Create a new component builder.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            props: Vec::new(),
            slots: Vec::new(),
            template: Vec::new(),
        }
    }

    /// Add a required prop.
    pub fn prop(mut self, name: &str, prop_type: PropType) -> Self {
        self.props.push(PropDefinition {
            name: name.to_string(),
            prop_type,
            default: None,
            required: true,
            span: Span::default(),
        });
        self
    }

    /// Add an optional prop with a default value.
    pub fn optional_prop(
        mut self,
        name: &str,
        prop_type: PropType,
        default: seed_core::ast::PropertyValue,
    ) -> Self {
        self.props.push(PropDefinition {
            name: name.to_string(),
            prop_type,
            default: Some(default),
            required: false,
            span: Span::default(),
        });
        self
    }

    /// Add a default slot.
    pub fn default_slot(mut self) -> Self {
        self.slots.push(SlotDefinition {
            name: None,
            span: Span::default(),
        });
        self
    }

    /// Add a named slot.
    pub fn named_slot(mut self, name: &str) -> Self {
        self.slots.push(SlotDefinition {
            name: Some(name.to_string()),
            span: Span::default(),
        });
        self
    }

    /// Set the template elements.
    pub fn template(mut self, elements: Vec<seed_core::ast::Element>) -> Self {
        self.template = elements;
        self
    }

    /// Build the component definition.
    pub fn build(self) -> ComponentDefinition {
        ComponentDefinition {
            name: Identifier(self.name),
            props: self.props,
            slots: self.slots,
            template: self.template,
            span: Span::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = ComponentRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ComponentRegistry::new();
        let component = ComponentBuilder::new("Button")
            .prop("label", PropType::String)
            .build();

        registry.register(component);
        assert!(registry.contains("Button"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_get() {
        let mut registry = ComponentRegistry::new();
        let component = ComponentBuilder::new("Card")
            .prop("title", PropType::String)
            .optional_prop("padding", PropType::Length, seed_core::ast::PropertyValue::Length(seed_core::types::Length::px(16.0)))
            .default_slot()
            .build();

        registry.register(component);

        let card = registry.get("Card").unwrap();
        assert_eq!(card.name.0, "Card");
        assert_eq!(card.props.len(), 2);
        assert_eq!(card.slots.len(), 1);
    }

    #[test]
    fn test_component_builder() {
        let component = ComponentBuilder::new("Header")
            .prop("text", PropType::String)
            .prop("level", PropType::Number)
            .optional_prop("color", PropType::Color, seed_core::ast::PropertyValue::Color(seed_core::types::Color::rgb(0.0, 0.0, 0.0)))
            .named_slot("icon")
            .default_slot()
            .build();

        assert_eq!(component.name.0, "Header");
        assert_eq!(component.props.len(), 3);
        assert!(component.props[0].required);
        assert!(component.props[1].required);
        assert!(!component.props[2].required);
        assert_eq!(component.slots.len(), 2);
    }
}
