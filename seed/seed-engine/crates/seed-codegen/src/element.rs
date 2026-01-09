//! Simplified element model for code generation.
//!
//! This provides a flattened view of design elements suitable for
//! code generation analysis, separate from the seed-core AST.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// A simplified element for code generation analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodegenElement {
    /// Element type (Frame, Text, Button, etc.)
    pub element_type: String,
    /// Element name/ID
    pub name: Option<String>,
    /// Properties as key-value pairs
    pub properties: HashMap<String, PropertyValue>,
    /// Child elements
    pub children: Vec<CodegenElement>,
}

impl CodegenElement {
    /// Create a new element.
    pub fn new(element_type: impl Into<String>) -> Self {
        Self {
            element_type: element_type.into(),
            name: None,
            properties: HashMap::new(),
            children: Vec::new(),
        }
    }

    /// Set the element name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a property.
    pub fn with_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Add a child element.
    pub fn with_child(mut self, child: CodegenElement) -> Self {
        self.children.push(child);
        self
    }

    /// Get a property value.
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Check if element has a property.
    pub fn has_property(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }
}

/// A property value for code generation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropertyValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Color(String),
    Keyword(String),
    Array(Vec<PropertyValue>),
    Object(HashMap<String, PropertyValue>),
}

impl PropertyValue {
    /// Get as string if it's a string value.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            PropertyValue::String(s) => Some(s),
            PropertyValue::Keyword(k) => Some(k),
            _ => None,
        }
    }

    /// Get as number if it's a number value.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as boolean if it's a boolean value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PropertyValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

/// Convert from seed_core AST to codegen element.
pub mod from_ast {
    use super::*;
    use seed_core::ast;

    /// Convert a seed_core Element to a CodegenElement.
    pub fn from_element(element: &ast::Element) -> CodegenElement {
        match element {
            ast::Element::Frame(frame) => from_frame(frame),
            ast::Element::Text(text) => from_text(text),
            ast::Element::Svg(svg) => from_svg(svg),
            ast::Element::Image(img) => from_image(img),
            ast::Element::Icon(icon) => from_icon(icon),
            ast::Element::Part(part) => from_part(part),
            ast::Element::Component(comp) => from_component(comp),
            ast::Element::Slot(slot) => from_slot(slot),
        }
    }

    fn from_frame(frame: &ast::FrameElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Frame");
        elem.name = frame.name.as_ref().map(|id| id.0.clone());

        for prop in &frame.properties {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        for child in &frame.children {
            elem.children.push(from_element(child));
        }

        elem
    }

    fn from_text(text: &ast::TextElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Text");
        elem.name = text.name.as_ref().map(|id| id.0.clone());

        // Add content property
        match &text.content {
            ast::TextContent::Literal(s) => {
                elem.properties.insert("content".to_string(), PropertyValue::String(s.clone()));
            }
            ast::TextContent::TokenRef(path) => {
                let path_str = path.0.join(".");
                elem.properties.insert("content".to_string(), PropertyValue::String(format!("${{{}}}", path_str)));
            }
        }

        for prop in &text.properties {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        elem
    }

    fn from_svg(svg: &ast::SvgElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Svg");
        elem.name = svg.name.as_ref().map(|id| id.0.clone());

        for prop in &svg.properties {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        elem
    }

    fn from_image(img: &ast::ImageElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Image");
        elem.name = img.name.as_ref().map(|id| id.0.clone());

        // Add source property
        match &img.source {
            ast::ImageSource::Url(url) => {
                elem.properties.insert("src".to_string(), PropertyValue::String(url.clone()));
            }
            ast::ImageSource::File(path) => {
                elem.properties.insert("src".to_string(), PropertyValue::String(path.clone()));
            }
            ast::ImageSource::Data { mime_type, data } => {
                elem.properties.insert("src".to_string(),
                    PropertyValue::String(format!("data:{};base64,{}", mime_type, data)));
            }
            ast::ImageSource::TokenRef(path) => {
                let path_str = path.0.join(".");
                elem.properties.insert("src".to_string(), PropertyValue::String(format!("${{{}}}", path_str)));
            }
        }

        if let Some(ref alt) = img.alt {
            elem.properties.insert("alt".to_string(), PropertyValue::String(alt.clone()));
        }

        for prop in &img.properties {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        elem
    }

    fn from_icon(icon: &ast::IconElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Icon");
        elem.name = icon.name.as_ref().map(|id| id.0.clone());

        // Add icon source
        match &icon.icon {
            ast::IconSource::Named { library, name } => {
                if let Some(lib) = library {
                    elem.properties.insert("icon".to_string(),
                        PropertyValue::String(format!("{}:{}", lib, name)));
                } else {
                    elem.properties.insert("icon".to_string(), PropertyValue::String(name.clone()));
                }
            }
            ast::IconSource::TokenRef(path) => {
                let path_str = path.0.join(".");
                elem.properties.insert("icon".to_string(), PropertyValue::String(format!("${{{}}}", path_str)));
            }
            _ => {}
        }

        for prop in &icon.properties {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        elem
    }

    fn from_part(part: &ast::PartElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Part");
        elem.name = part.name.as_ref().map(|id| id.0.clone());

        for prop in &part.properties {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        elem
    }

    fn from_component(comp: &ast::ComponentElement) -> CodegenElement {
        let mut elem = CodegenElement::new(comp.component_name.0.clone());
        elem.name = comp.instance_name.as_ref().map(|id| id.0.clone());

        for prop in &comp.props {
            if let Some(value) = convert_property_value(&prop.value) {
                elem.properties.insert(prop.name.clone(), value);
            }
        }

        for child in &comp.children {
            elem.children.push(from_element(child));
        }

        elem
    }

    fn from_slot(slot: &ast::SlotElement) -> CodegenElement {
        let mut elem = CodegenElement::new("Slot");
        elem.name = slot.name.clone();

        for child in &slot.fallback {
            elem.children.push(from_element(child));
        }

        elem
    }

    fn convert_property_value(value: &ast::PropertyValue) -> Option<PropertyValue> {
        Some(match value {
            ast::PropertyValue::String(s) => PropertyValue::String(s.clone()),
            ast::PropertyValue::Number(n) => PropertyValue::Number(*n),
            ast::PropertyValue::Boolean(b) => PropertyValue::Boolean(*b),
            ast::PropertyValue::Color(c) => PropertyValue::Color(c.to_hex()),
            ast::PropertyValue::Length(l) => PropertyValue::Number(l.value),
            ast::PropertyValue::Enum(s) => PropertyValue::Keyword(s.clone()),
            ast::PropertyValue::TokenRef(path) => {
                PropertyValue::String(format!("${{{}}}", path.0.join(".")))
            }
            _ => return None,
        })
    }

    /// Convert a seed_core Document to a list of CodegenElements.
    pub fn from_document(doc: &ast::Document) -> Vec<CodegenElement> {
        doc.elements.iter().map(from_element).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegen_element_creation() {
        let elem = CodegenElement::new("Button")
            .with_name("submit_btn")
            .with_property("label", PropertyValue::String("Submit".to_string()))
            .with_property("disabled", PropertyValue::Boolean(false));

        assert_eq!(elem.element_type, "Button");
        assert_eq!(elem.name, Some("submit_btn".to_string()));
        assert!(elem.has_property("label"));
    }
}
