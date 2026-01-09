//! Intent analysis for code generation.

mod roles;
mod patterns;
mod state_builder;
mod graph_builder;

pub use roles::{SemanticRole, RoleDetector};
pub use patterns::{InteractionPattern, PatternRecognizer};
pub use state_builder::StateModelBuilder;
pub use graph_builder::InteractionGraphBuilder;

use crate::element::{CodegenElement, PropertyValue, from_ast};
use crate::error::Result;
use crate::model::{StateModel, InteractionGraph, DesignSystem};
use seed_core::ast::Document;

/// Analyzes Seed documents to extract intent and generate models.
pub struct IntentAnalyzer {
    role_detector: RoleDetector,
    pattern_recognizer: PatternRecognizer,
}

impl IntentAnalyzer {
    /// Create a new intent analyzer.
    pub fn new() -> Self {
        Self {
            role_detector: RoleDetector::new(),
            pattern_recognizer: PatternRecognizer::new(),
        }
    }

    /// Analyze a document and extract state model.
    pub fn extract_state_model(&self, doc: &Document) -> Result<StateModel> {
        let mut builder = StateModelBuilder::new();

        // Convert AST elements to CodegenElements
        let elements = from_ast::from_document(doc);

        // Detect roles for all elements
        for element in &elements {
            let roles = self.role_detector.detect_roles(element);
            builder.process_element(element, &roles)?;
        }

        // Recognize interaction patterns
        let patterns = self.pattern_recognizer.recognize(&elements);
        builder.process_patterns(&patterns)?;

        builder.build()
    }

    /// Analyze a document and extract interaction graph.
    pub fn extract_interaction_graph(&self, doc: &Document) -> Result<InteractionGraph> {
        let mut builder = InteractionGraphBuilder::new();

        // Convert AST elements to CodegenElements
        let elements = from_ast::from_document(doc);

        // Build nodes from top-level frames
        for element in &elements {
            builder.process_element(element)?;
        }

        // Recognize navigation patterns
        let patterns = self.pattern_recognizer.recognize(&elements);
        builder.process_patterns(&patterns)?;

        builder.build()
    }

    /// Extract design system from document.
    pub fn extract_design_system(&self, doc: &Document) -> Result<DesignSystem> {
        let mut design_system = DesignSystem::new();

        // Convert AST elements to CodegenElements
        let elements = from_ast::from_document(doc);

        // Extract colors from elements
        self.extract_colors(&elements, &mut design_system);

        // Extract typography
        self.extract_typography(&elements, &mut design_system);

        // Extract spacing patterns
        self.extract_spacing(&elements, &mut design_system);

        Ok(design_system)
    }

    /// Extract colors from elements.
    fn extract_colors(&self, elements: &[CodegenElement], ds: &mut DesignSystem) {
        use crate::model::ColorToken;

        // Common color property names
        let color_props = ["fill", "color", "background", "stroke", "border_color"];

        for element in elements {
            for prop_name in &color_props {
                if let Some(value) = element.properties.get(*prop_name) {
                    if let Some(hex) = Self::extract_hex_color(value) {
                        // Generate token name
                        let token_name = format!("color_{}", ds.colors.len());
                        ds.add_color(token_name, ColorToken::from_hex(&hex));
                    }
                }
            }
            // Recurse into children
            self.extract_colors(&element.children, ds);
        }
    }

    /// Extract typography from elements.
    fn extract_typography(&self, elements: &[CodegenElement], ds: &mut DesignSystem) {
        use crate::model::{TypographyToken, FontWeight};

        for element in elements {
            if element.element_type == "Text" {
                let font_size = element.properties.get("font_size")
                    .or(element.properties.get("font"))
                    .and_then(|v| Self::extract_number(v))
                    .unwrap_or(16.0);

                let weight = element.properties.get("font_weight")
                    .and_then(|v| Self::extract_font_weight(v))
                    .unwrap_or(FontWeight::Regular);

                let token_name = format!("text_{}", ds.typography.len());
                ds.add_typography(token_name, TypographyToken {
                    family: "System".to_string(),
                    size: font_size,
                    weight,
                    line_height: element.properties.get("line_height")
                        .and_then(|v| Self::extract_number(v)),
                    letter_spacing: element.properties.get("letter_spacing")
                        .and_then(|v| Self::extract_number(v)),
                });
            }

            // Recurse into children
            self.extract_typography(&element.children, ds);
        }
    }

    /// Extract spacing patterns from elements.
    fn extract_spacing(&self, elements: &[CodegenElement], ds: &mut DesignSystem) {
        let spacing_props = ["padding", "margin", "gap", "spacing"];

        for element in elements {
            for prop_name in &spacing_props {
                if let Some(value) = element.properties.get(*prop_name) {
                    if let Some(num) = Self::extract_number(value) {
                        // Check if this spacing value is already in the system
                        let exists = ds.spacing.values().any(|&v| (v - num).abs() < 0.01);
                        if !exists {
                            let token_name = format!("space_{}", (num as i32));
                            ds.add_spacing(token_name, num);
                        }
                    }
                }
            }
            // Recurse into children
            self.extract_spacing(&element.children, ds);
        }
    }

    /// Extract hex color from property value.
    fn extract_hex_color(value: &PropertyValue) -> Option<String> {
        match value {
            PropertyValue::Color(c) => Some(c.clone()),
            PropertyValue::String(s) if s.starts_with('#') => Some(s.clone()),
            _ => None,
        }
    }

    /// Extract number from property value.
    fn extract_number(value: &PropertyValue) -> Option<f64> {
        match value {
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Extract font weight from property value.
    fn extract_font_weight(value: &PropertyValue) -> Option<crate::model::FontWeight> {
        use crate::model::FontWeight;

        match value {
            PropertyValue::Keyword(k) => {
                match k.to_lowercase().as_str() {
                    "thin" => Some(FontWeight::Thin),
                    "extralight" | "extra-light" => Some(FontWeight::ExtraLight),
                    "light" => Some(FontWeight::Light),
                    "regular" | "normal" => Some(FontWeight::Regular),
                    "medium" => Some(FontWeight::Medium),
                    "semibold" | "semi-bold" => Some(FontWeight::SemiBold),
                    "bold" => Some(FontWeight::Bold),
                    "extrabold" | "extra-bold" => Some(FontWeight::ExtraBold),
                    "black" => Some(FontWeight::Black),
                    _ => None,
                }
            }
            PropertyValue::Number(n) => {
                match *n as u16 {
                    0..=149 => Some(FontWeight::Thin),
                    150..=249 => Some(FontWeight::ExtraLight),
                    250..=349 => Some(FontWeight::Light),
                    350..=449 => Some(FontWeight::Regular),
                    450..=549 => Some(FontWeight::Medium),
                    550..=649 => Some(FontWeight::SemiBold),
                    650..=749 => Some(FontWeight::Bold),
                    750..=849 => Some(FontWeight::ExtraBold),
                    _ => Some(FontWeight::Black),
                }
            }
            _ => None,
        }
    }
}

impl Default for IntentAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = IntentAnalyzer::new();
        // Just verify it creates without panic
        let _ = analyzer;
    }
}
