//! State model builder from design constraints.

use crate::error::Result;
use crate::model::{
    ActionParameter, ComputedProperty, SideEffect, SideEffectType, StateAction, StateBinding,
    StateModel, StateMutation, StateType, StateVariable,
};
use crate::element::{CodegenElement, PropertyValue};
use super::patterns::InteractionPattern;
use super::roles::SemanticRole;
use std::collections::HashSet;

/// Builds a StateModel from analyzed design elements.
pub struct StateModelBuilder {
    model: StateModel,
    processed_elements: HashSet<String>,
}

impl StateModelBuilder {
    /// Create a new state model builder.
    pub fn new() -> Self {
        Self {
            model: StateModel::new(),
            processed_elements: HashSet::new(),
        }
    }

    /// Process an element and its roles to extract state.
    pub fn process_element(
        &mut self,
        element: &CodegenElement,
        roles: &HashSet<SemanticRole>,
    ) -> Result<()> {
        let element_id = element
            .name
            .clone()
            .unwrap_or_else(|| format!("element_{}", self.processed_elements.len()));

        if self.processed_elements.contains(&element_id) {
            return Ok(());
        }
        self.processed_elements.insert(element_id.clone());

        // Extract state variables based on roles
        for role in roles {
            self.extract_state_for_role(&element_id, element, *role)?;
        }

        // Check for explicit state bindings in properties
        self.extract_explicit_bindings(&element_id, element)?;

        // Process children recursively
        for child in &element.children {
            let child_roles = self.detect_child_roles(child);
            self.process_element(child, &child_roles)?;
        }

        Ok(())
    }

    /// Process recognized interaction patterns.
    pub fn process_patterns(&mut self, patterns: &[InteractionPattern]) -> Result<()> {
        for pattern in patterns {
            self.extract_state_for_pattern(pattern)?;
        }
        Ok(())
    }

    /// Build the final state model.
    pub fn build(self) -> Result<StateModel> {
        Ok(self.model)
    }

    /// Extract state based on semantic role.
    fn extract_state_for_role(
        &mut self,
        element_id: &str,
        element: &CodegenElement,
        role: SemanticRole,
    ) -> Result<()> {
        match role {
            SemanticRole::Input | SemanticRole::TextArea => {
                self.add_input_state(element_id, element)?;
            }
            SemanticRole::Checkbox | SemanticRole::Switch => {
                self.add_toggle_state(element_id, element)?;
            }
            SemanticRole::Select => {
                self.add_select_state(element_id, element)?;
            }
            SemanticRole::Slider => {
                self.add_slider_state(element_id, element)?;
            }
            SemanticRole::List => {
                self.add_list_state(element_id, element)?;
            }
            SemanticRole::Tab | SemanticRole::TabBar => {
                self.add_tab_state(element_id)?;
            }
            SemanticRole::Modal | SemanticRole::Dialog => {
                self.add_modal_state(element_id)?;
            }
            SemanticRole::Button => {
                self.add_button_actions(element_id, element)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Add state for text input.
    fn add_input_state(&mut self, element_id: &str, element: &CodegenElement) -> Result<()> {
        let var_name = format!("{}_value", element_id);

        let default = element
            .properties
            .get("value")
            .or_else(|| element.properties.get("default_value"))
            .and_then(|v| match v {
                PropertyValue::String(s) => Some(serde_json::json!(s)),
                _ => None,
            });

        self.model.add_variable(
            var_name.clone(),
            StateVariable {
                var_type: StateType::String,
                default_value: default,
                observable: true,
                validation: element
                    .properties
                    .get("validation")
                    .and_then(|v| match v {
                        PropertyValue::String(s) => Some(s.clone()),
                        _ => None,
                    }),
                doc: Some(format!("Value for {}", element_id)),
            },
        );

        self.model.add_binding(StateBinding {
            element_id: element_id.to_string(),
            property: "value".to_string(),
            variable: var_name.clone(),
            transform: None,
            two_way: true,
        });

        // Add onChange action
        self.model.add_action(
            format!("set_{}", var_name),
            StateAction {
                parameters: vec![ActionParameter {
                    name: "newValue".to_string(),
                    param_type: StateType::String,
                    optional: false,
                }],
                mutations: vec![StateMutation {
                    variable: var_name,
                    expression: "newValue".to_string(),
                }],
                side_effects: vec![],
                doc: Some(format!("Update {} value", element_id)),
            },
        );

        Ok(())
    }

    /// Add state for toggle controls.
    fn add_toggle_state(&mut self, element_id: &str, element: &CodegenElement) -> Result<()> {
        let var_name = format!("{}_checked", element_id);

        let default = element
            .properties
            .get("checked")
            .or_else(|| element.properties.get("value"))
            .and_then(|v| match v {
                PropertyValue::Boolean(b) => Some(serde_json::json!(b)),
                _ => None,
            })
            .unwrap_or(serde_json::json!(false));

        self.model.add_variable(
            var_name.clone(),
            StateVariable {
                var_type: StateType::Bool,
                default_value: Some(default),
                observable: true,
                validation: None,
                doc: Some(format!("Checked state for {}", element_id)),
            },
        );

        self.model.add_binding(StateBinding {
            element_id: element_id.to_string(),
            property: "checked".to_string(),
            variable: var_name.clone(),
            transform: None,
            two_way: true,
        });

        self.model.add_action(
            format!("toggle_{}", element_id),
            StateAction {
                parameters: vec![],
                mutations: vec![StateMutation {
                    variable: var_name.clone(),
                    expression: format!("!{}", var_name),
                }],
                side_effects: vec![],
                doc: Some(format!("Toggle {}", element_id)),
            },
        );

        Ok(())
    }

    /// Add state for select/dropdown.
    fn add_select_state(&mut self, element_id: &str, element: &CodegenElement) -> Result<()> {
        let var_name = format!("{}_selected", element_id);

        self.model.add_variable(
            var_name.clone(),
            StateVariable {
                var_type: StateType::Optional,
                default_value: element.properties.get("value").and_then(|v| match v {
                    PropertyValue::String(s) => Some(serde_json::json!(s)),
                    PropertyValue::Number(n) => Some(serde_json::json!(n)),
                    _ => None,
                }),
                observable: true,
                validation: None,
                doc: Some(format!("Selected value for {}", element_id)),
            },
        );

        self.model.add_binding(StateBinding {
            element_id: element_id.to_string(),
            property: "value".to_string(),
            variable: var_name.clone(),
            transform: None,
            two_way: true,
        });

        self.model.add_action(
            format!("select_{}", element_id),
            StateAction {
                parameters: vec![ActionParameter {
                    name: "option".to_string(),
                    param_type: StateType::String,
                    optional: false,
                }],
                mutations: vec![StateMutation {
                    variable: var_name,
                    expression: "option".to_string(),
                }],
                side_effects: vec![],
                doc: Some(format!("Select option for {}", element_id)),
            },
        );

        Ok(())
    }

    /// Add state for slider.
    fn add_slider_state(&mut self, element_id: &str, element: &CodegenElement) -> Result<()> {
        let var_name = format!("{}_value", element_id);

        let default = element
            .properties
            .get("value")
            .and_then(|v| match v {
                PropertyValue::Number(n) => Some(serde_json::json!(n)),
                _ => None,
            })
            .unwrap_or(serde_json::json!(0));

        self.model.add_variable(
            var_name.clone(),
            StateVariable {
                var_type: StateType::Float,
                default_value: Some(default),
                observable: true,
                validation: None,
                doc: Some(format!("Slider value for {}", element_id)),
            },
        );

        self.model.add_binding(StateBinding {
            element_id: element_id.to_string(),
            property: "value".to_string(),
            variable: var_name.clone(),
            transform: None,
            two_way: true,
        });

        Ok(())
    }

    /// Add state for lists.
    fn add_list_state(&mut self, element_id: &str, _element: &CodegenElement) -> Result<()> {
        let items_var = format!("{}_items", element_id);
        let selected_var = format!("{}_selected", element_id);

        self.model.add_variable(
            items_var.clone(),
            StateVariable {
                var_type: StateType::Array,
                default_value: Some(serde_json::json!([])),
                observable: true,
                validation: None,
                doc: Some(format!("Items for {}", element_id)),
            },
        );

        self.model.add_variable(
            selected_var.clone(),
            StateVariable {
                var_type: StateType::Optional,
                default_value: None,
                observable: true,
                validation: None,
                doc: Some(format!("Selected item index for {}", element_id)),
            },
        );

        self.model.add_action(
            format!("select_{}_item", element_id),
            StateAction {
                parameters: vec![ActionParameter {
                    name: "index".to_string(),
                    param_type: StateType::Int,
                    optional: false,
                }],
                mutations: vec![StateMutation {
                    variable: selected_var,
                    expression: "index".to_string(),
                }],
                side_effects: vec![],
                doc: Some(format!("Select item in {}", element_id)),
            },
        );

        Ok(())
    }

    /// Add state for tabs.
    fn add_tab_state(&mut self, element_id: &str) -> Result<()> {
        let var_name = format!("{}_active_tab", element_id);

        self.model.add_variable(
            var_name.clone(),
            StateVariable {
                var_type: StateType::Int,
                default_value: Some(serde_json::json!(0)),
                observable: true,
                validation: None,
                doc: Some(format!("Active tab index for {}", element_id)),
            },
        );

        self.model.add_action(
            format!("switch_tab_{}", element_id),
            StateAction {
                parameters: vec![ActionParameter {
                    name: "tabIndex".to_string(),
                    param_type: StateType::Int,
                    optional: false,
                }],
                mutations: vec![StateMutation {
                    variable: var_name,
                    expression: "tabIndex".to_string(),
                }],
                side_effects: vec![],
                doc: Some(format!("Switch tab for {}", element_id)),
            },
        );

        Ok(())
    }

    /// Add state for modals.
    fn add_modal_state(&mut self, element_id: &str) -> Result<()> {
        let var_name = format!("{}_visible", element_id);

        self.model.add_variable(
            var_name.clone(),
            StateVariable {
                var_type: StateType::Bool,
                default_value: Some(serde_json::json!(false)),
                observable: true,
                validation: None,
                doc: Some(format!("Visibility state for {}", element_id)),
            },
        );

        self.model.add_action(
            format!("show_{}", element_id),
            StateAction {
                parameters: vec![],
                mutations: vec![StateMutation {
                    variable: var_name.clone(),
                    expression: "true".to_string(),
                }],
                side_effects: vec![],
                doc: Some(format!("Show {}", element_id)),
            },
        );

        self.model.add_action(
            format!("hide_{}", element_id),
            StateAction {
                parameters: vec![],
                mutations: vec![StateMutation {
                    variable: var_name.clone(),
                    expression: "false".to_string(),
                }],
                side_effects: vec![],
                doc: Some(format!("Hide {}", element_id)),
            },
        );

        self.model.add_action(
            format!("toggle_{}", element_id),
            StateAction {
                parameters: vec![],
                mutations: vec![StateMutation {
                    variable: var_name.clone(),
                    expression: format!("!{}", var_name),
                }],
                side_effects: vec![],
                doc: Some(format!("Toggle {}", element_id)),
            },
        );

        Ok(())
    }

    /// Add actions for buttons.
    fn add_button_actions(&mut self, element_id: &str, element: &CodegenElement) -> Result<()> {
        // Check for action property
        if let Some(action) = element.properties.get("action").or_else(|| element.properties.get("on_click")) {
            let action_name = match action {
                PropertyValue::String(s) => s.clone(),
                PropertyValue::Keyword(k) => k.clone(),
                _ => format!("{}_click", element_id),
            };

            // Check if action already exists
            if !self.model.actions.contains_key(&action_name) {
                // Check for navigation
                let side_effects = if let Some(href) = element.properties.get("href") {
                    match href {
                        PropertyValue::String(url) => vec![SideEffect {
                            effect_type: SideEffectType::Navigate,
                            params: [("url".to_string(), serde_json::json!(url))]
                                .into_iter()
                                .collect(),
                        }],
                        _ => vec![],
                    }
                } else {
                    vec![]
                };

                self.model.add_action(
                    action_name,
                    StateAction {
                        parameters: vec![],
                        mutations: vec![],
                        side_effects,
                        doc: Some(format!("Click handler for {}", element_id)),
                    },
                );
            }
        }

        Ok(())
    }

    /// Extract explicit state bindings from properties.
    fn extract_explicit_bindings(&mut self, element_id: &str, element: &CodegenElement) -> Result<()> {
        // Look for bind:* properties
        for (key, value) in &element.properties {
            if key.starts_with("bind:") {
                let property = key.trim_start_matches("bind:");
                if let PropertyValue::String(var_name) = value {
                    self.model.add_binding(StateBinding {
                        element_id: element_id.to_string(),
                        property: property.to_string(),
                        variable: var_name.clone(),
                        transform: None,
                        two_way: true,
                    });
                }
            }
        }

        Ok(())
    }

    /// Extract state from interaction patterns.
    fn extract_state_for_pattern(&mut self, pattern: &InteractionPattern) -> Result<()> {
        match pattern {
            InteractionPattern::Form {
                fields,
                submit_action,
            } => {
                // Add form validation state
                let is_valid_var = "form_is_valid".to_string();

                self.model.add_variable(
                    is_valid_var.clone(),
                    StateVariable {
                        var_type: StateType::Bool,
                        default_value: Some(serde_json::json!(false)),
                        observable: true,
                        validation: None,
                        doc: Some("Form validation state".to_string()),
                    },
                );

                // Add computed validation
                if !fields.is_empty() {
                    self.model.add_computed(
                        is_valid_var,
                        ComputedProperty {
                            return_type: StateType::Bool,
                            dependencies: fields.iter().map(|f| format!("{}_value", f)).collect(),
                            expression: fields
                                .iter()
                                .map(|f| format!("{}_value.length > 0", f))
                                .collect::<Vec<_>>()
                                .join(" && "),
                            doc: Some("Check if all form fields are valid".to_string()),
                        },
                    );
                }

                // Add submit action
                if let Some(action_name) = submit_action {
                    if !self.model.actions.contains_key(action_name) {
                        self.model.add_action(
                            action_name.clone(),
                            StateAction {
                                parameters: vec![],
                                mutations: vec![],
                                side_effects: vec![SideEffect {
                                    effect_type: SideEffectType::ApiCall,
                                    params: [("action".to_string(), serde_json::json!("submit"))]
                                        .into_iter()
                                        .collect(),
                                }],
                                doc: Some("Submit form".to_string()),
                            },
                        );
                    }
                }
            }

            InteractionPattern::Counter {
                value_id,
                increment_id,
                decrement_id,
            } => {
                let var_name = format!("{}_count", value_id);

                self.model.add_variable(
                    var_name.clone(),
                    StateVariable {
                        var_type: StateType::Int,
                        default_value: Some(serde_json::json!(0)),
                        observable: true,
                        validation: None,
                        doc: Some("Counter value".to_string()),
                    },
                );

                self.model.add_action(
                    format!("{}_increment", increment_id),
                    StateAction {
                        parameters: vec![],
                        mutations: vec![StateMutation {
                            variable: var_name.clone(),
                            expression: format!("{} + 1", var_name),
                        }],
                        side_effects: vec![],
                        doc: Some("Increment counter".to_string()),
                    },
                );

                self.model.add_action(
                    format!("{}_decrement", decrement_id),
                    StateAction {
                        parameters: vec![],
                        mutations: vec![StateMutation {
                            variable: var_name.clone(),
                            expression: format!("{} - 1", var_name),
                        }],
                        side_effects: vec![],
                        doc: Some("Decrement counter".to_string()),
                    },
                );
            }

            InteractionPattern::Accordion { section_ids } => {
                let var_name = "expanded_sections".to_string();

                self.model.add_variable(
                    var_name.clone(),
                    StateVariable {
                        var_type: StateType::Array,
                        default_value: Some(serde_json::json!([])),
                        observable: true,
                        validation: None,
                        doc: Some("Expanded accordion sections".to_string()),
                    },
                );

                for section_id in section_ids {
                    self.model.add_action(
                        format!("toggle_section_{}", section_id),
                        StateAction {
                            parameters: vec![],
                            mutations: vec![StateMutation {
                                variable: var_name.clone(),
                                expression: format!(
                                    "expanded_sections.includes('{}') ? expanded_sections.filter(s => s !== '{}') : [...expanded_sections, '{}']",
                                    section_id, section_id, section_id
                                ),
                            }],
                            side_effects: vec![],
                            doc: Some(format!("Toggle section {}", section_id)),
                        },
                    );
                }
            }

            InteractionPattern::ModalTrigger {
                trigger_id,
                modal_id,
            } => {
                let var_name = format!("{}_visible", modal_id);

                if !self.model.variables.contains_key(&var_name) {
                    self.model.add_variable(
                        var_name.clone(),
                        StateVariable {
                            var_type: StateType::Bool,
                            default_value: Some(serde_json::json!(false)),
                            observable: true,
                            validation: None,
                            doc: Some(format!("Visibility for {}", modal_id)),
                        },
                    );
                }

                self.model.add_action(
                    format!("{}_open_modal", trigger_id),
                    StateAction {
                        parameters: vec![],
                        mutations: vec![StateMutation {
                            variable: var_name,
                            expression: "true".to_string(),
                        }],
                        side_effects: vec![],
                        doc: Some(format!("Open modal from {}", trigger_id)),
                    },
                );
            }

            InteractionPattern::TabbedInterface {
                tab_ids,
                content_ids: _,
            } => {
                let var_name = "active_tab".to_string();

                self.model.add_variable(
                    var_name.clone(),
                    StateVariable {
                        var_type: StateType::Int,
                        default_value: Some(serde_json::json!(0)),
                        observable: true,
                        validation: None,
                        doc: Some("Active tab index".to_string()),
                    },
                );

                for (i, tab_id) in tab_ids.iter().enumerate() {
                    self.model.add_action(
                        format!("activate_{}", tab_id),
                        StateAction {
                            parameters: vec![],
                            mutations: vec![StateMutation {
                                variable: var_name.clone(),
                                expression: i.to_string(),
                            }],
                            side_effects: vec![],
                            doc: Some(format!("Activate tab {}", tab_id)),
                        },
                    );
                }
            }

            _ => {}
        }

        Ok(())
    }

    /// Simple role detection for child elements.
    fn detect_child_roles(&self, element: &CodegenElement) -> HashSet<SemanticRole> {
        let mut roles = HashSet::new();

        // Detect from element type
        match element.element_type.to_lowercase().as_str() {
            "input" => {
                roles.insert(SemanticRole::Input);
            }
            "button" => {
                roles.insert(SemanticRole::Button);
            }
            "checkbox" => {
                roles.insert(SemanticRole::Checkbox);
            }
            "switch" => {
                roles.insert(SemanticRole::Switch);
            }
            "select" => {
                roles.insert(SemanticRole::Select);
            }
            "slider" => {
                roles.insert(SemanticRole::Slider);
            }
            "list" => {
                roles.insert(SemanticRole::List);
            }
            _ => {}
        }

        if roles.is_empty() {
            roles.insert(SemanticRole::Unknown);
        }

        roles
    }
}

impl Default for StateModelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_element(element_type: &str, name: Option<&str>) -> CodegenElement {
        let mut elem = CodegenElement::new(element_type);
        elem.name = name.map(|s| s.to_string());
        elem
    }

    #[test]
    fn test_input_state() {
        let mut builder = StateModelBuilder::new();
        let element = make_element("Input", Some("username"));
        let mut roles = HashSet::new();
        roles.insert(SemanticRole::Input);

        builder.process_element(&element, &roles).unwrap();
        let model = builder.build().unwrap();

        assert!(model.variables.contains_key("username_value"));
        assert!(model.actions.contains_key("set_username_value"));
    }

    #[test]
    fn test_toggle_state() {
        let mut builder = StateModelBuilder::new();
        let element = make_element("Checkbox", Some("agree"));
        let mut roles = HashSet::new();
        roles.insert(SemanticRole::Checkbox);

        builder.process_element(&element, &roles).unwrap();
        let model = builder.build().unwrap();

        assert!(model.variables.contains_key("agree_checked"));
        assert!(model.actions.contains_key("toggle_agree"));
    }

    #[test]
    fn test_counter_pattern() {
        let mut builder = StateModelBuilder::new();
        let patterns = vec![InteractionPattern::Counter {
            value_id: "count".to_string(),
            increment_id: "plus".to_string(),
            decrement_id: "minus".to_string(),
        }];

        builder.process_patterns(&patterns).unwrap();
        let model = builder.build().unwrap();

        assert!(model.variables.contains_key("count_count"));
        assert!(model.actions.contains_key("plus_increment"));
        assert!(model.actions.contains_key("minus_decrement"));
    }
}
