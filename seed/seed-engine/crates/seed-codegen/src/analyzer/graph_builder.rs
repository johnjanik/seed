//! Interaction graph builder from user flows.

use crate::error::Result;
use crate::model::{
    InteractionEdge, InteractionGraph, InteractionNode, NodeType, Trigger, TriggerType,
};
use crate::element::{CodegenElement, PropertyValue};
use super::patterns::InteractionPattern;
use std::collections::HashSet;

/// Builds an InteractionGraph from analyzed design elements.
pub struct InteractionGraphBuilder {
    graph: InteractionGraph,
    processed_elements: HashSet<String>,
}

impl InteractionGraphBuilder {
    /// Create a new graph builder.
    pub fn new() -> Self {
        Self {
            graph: InteractionGraph::new(),
            processed_elements: HashSet::new(),
        }
    }

    /// Process an element to extract navigation nodes.
    pub fn process_element(&mut self, element: &CodegenElement) -> Result<()> {
        let element_id = element
            .name
            .clone()
            .unwrap_or_else(|| format!("node_{}", self.processed_elements.len()));

        if self.processed_elements.contains(&element_id) {
            return Ok(());
        }
        self.processed_elements.insert(element_id.clone());

        // Check if element represents a screen/view
        let node_type = self.detect_node_type(element);
        if let Some(nt) = node_type {
            let node = InteractionNode {
                name: element_id.clone(),
                node_type: nt,
                component: Some(element.element_type.clone()),
                local_state: self.extract_local_state(element),
                on_enter: self.extract_lifecycle_actions(element, "on_enter"),
                on_exit: self.extract_lifecycle_actions(element, "on_exit"),
            };
            self.graph.add_node(&element_id, node);

            // Look for navigation triggers in children
            self.find_navigation_triggers(&element_id, element)?;
        }

        // Process children that might be separate screens
        for child in &element.children {
            self.process_element(child)?;
        }

        Ok(())
    }

    /// Process recognized patterns for navigation.
    pub fn process_patterns(&mut self, patterns: &[InteractionPattern]) -> Result<()> {
        for pattern in patterns {
            self.extract_edges_from_pattern(pattern)?;
        }
        Ok(())
    }

    /// Build the final interaction graph.
    pub fn build(self) -> Result<InteractionGraph> {
        Ok(self.graph)
    }

    /// Detect node type from element.
    fn detect_node_type(&self, element: &CodegenElement) -> Option<NodeType> {
        let type_lower = element.element_type.to_lowercase();
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        // Check for explicit role property
        if let Some(PropertyValue::Keyword(role)) = element.properties.get("role") {
            match role.to_lowercase().as_str() {
                "screen" | "page" | "view" => return Some(NodeType::Screen),
                "modal" => return Some(NodeType::Modal),
                "sheet" => return Some(NodeType::Sheet),
                "popover" => return Some(NodeType::Popover),
                "tab" => return Some(NodeType::Tab),
                "step" => return Some(NodeType::Step),
                _ => {}
            }
        }

        // Detect from type/name
        if type_lower.contains("screen")
            || type_lower.contains("page")
            || name_lower.contains("screen")
            || name_lower.contains("page")
        {
            return Some(NodeType::Screen);
        }

        if type_lower.contains("modal") || name_lower.contains("modal") {
            return Some(NodeType::Modal);
        }

        if type_lower.contains("sheet") || name_lower.contains("sheet") {
            return Some(NodeType::Sheet);
        }

        if type_lower.contains("popover") || name_lower.contains("popover") {
            return Some(NodeType::Popover);
        }

        if type_lower.contains("tab") {
            return Some(NodeType::Tab);
        }

        // Top-level frames are usually screens
        if type_lower == "frame" && element.properties.contains_key("title") {
            return Some(NodeType::Screen);
        }

        None
    }

    /// Extract local state variable names.
    fn extract_local_state(&self, element: &CodegenElement) -> Vec<String> {
        let mut state = Vec::new();

        // Look for state-related properties
        for key in element.properties.keys() {
            if key.starts_with("state:") || key.starts_with("local:") {
                let var_name = key
                    .trim_start_matches("state:")
                    .trim_start_matches("local:");
                state.push(var_name.to_string());
            }
        }

        state
    }

    /// Extract lifecycle actions.
    fn extract_lifecycle_actions(&self, element: &CodegenElement, lifecycle: &str) -> Vec<String> {
        let mut actions = Vec::new();

        if let Some(value) = element.properties.get(lifecycle) {
            match value {
                PropertyValue::String(s) => actions.push(s.clone()),
                PropertyValue::Keyword(k) => actions.push(k.clone()),
                PropertyValue::Array(arr) => {
                    for item in arr {
                        if let PropertyValue::String(s) = item {
                            actions.push(s.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        actions
    }

    /// Find navigation triggers in element children.
    fn find_navigation_triggers(&mut self, from_node: &str, element: &CodegenElement) -> Result<()> {
        for child in &element.children {
            // Check for navigation properties
            if let Some(target) = self.extract_navigation_target(child) {
                let trigger = self.extract_trigger(child);
                let animation = child
                    .properties
                    .get("transition")
                    .or_else(|| child.properties.get("animation"))
                    .and_then(|v| match v {
                        PropertyValue::String(s) => Some(s.clone()),
                        PropertyValue::Keyword(k) => Some(k.clone()),
                        _ => None,
                    });

                let guard = child.properties.get("guard").and_then(|v| match v {
                    PropertyValue::String(s) => Some(s.clone()),
                    _ => None,
                });

                self.graph.add_edge(InteractionEdge {
                    from: from_node.to_string(),
                    to: target,
                    trigger,
                    animation,
                    guard,
                    actions: vec![],
                });
            }

            // Recurse into children
            self.find_navigation_triggers(from_node, child)?;
        }

        Ok(())
    }

    /// Extract navigation target from element.
    fn extract_navigation_target(&self, element: &CodegenElement) -> Option<String> {
        // Check for navigate/href/to properties
        let nav_props = ["navigate", "href", "to", "target", "goto"];

        for prop in &nav_props {
            if let Some(value) = element.properties.get(*prop) {
                match value {
                    PropertyValue::String(s) => return Some(s.clone()),
                    PropertyValue::Keyword(k) => return Some(k.clone()),
                    _ => {}
                }
            }
        }

        None
    }

    /// Extract trigger from element.
    fn extract_trigger(&self, element: &CodegenElement) -> Trigger {
        let element_id = element.name.clone();

        // Check for explicit trigger type
        if let Some(PropertyValue::Keyword(trigger)) = element.properties.get("trigger") {
            let trigger_type = match trigger.to_lowercase().as_str() {
                "tap" | "click" => TriggerType::Tap,
                "long_press" | "longpress" => TriggerType::LongPress,
                "swipe" => TriggerType::Swipe,
                "gesture" => TriggerType::Gesture,
                "timer" | "timeout" => TriggerType::Timer,
                "state" | "state_change" => TriggerType::StateChange,
                "api" | "response" => TriggerType::Api,
                _ => TriggerType::External,
            };

            return Trigger {
                trigger_type,
                element_id,
                event: Some(trigger.clone()),
            };
        }

        // Default to tap for interactive elements
        let type_lower = element.element_type.to_lowercase();
        if type_lower == "button"
            || element.properties.contains_key("on_click")
            || element.properties.contains_key("action")
        {
            return Trigger {
                trigger_type: TriggerType::Tap,
                element_id,
                event: Some("tap".to_string()),
            };
        }

        // Check for gesture-based triggers
        if element.properties.contains_key("on_swipe") {
            return Trigger {
                trigger_type: TriggerType::Swipe,
                element_id,
                event: Some("swipe".to_string()),
            };
        }

        // Default
        Trigger {
            trigger_type: TriggerType::External,
            element_id,
            event: None,
        }
    }

    /// Extract edges from interaction patterns.
    fn extract_edges_from_pattern(&mut self, pattern: &InteractionPattern) -> Result<()> {
        match pattern {
            InteractionPattern::TabbedInterface {
                tab_ids,
                content_ids,
            } => {
                // Create tab nodes if they don't exist
                for (i, (tab_id, content_id)) in
                    tab_ids.iter().zip(content_ids.iter()).enumerate()
                {
                    if !self.graph.nodes.contains_key(content_id) {
                        self.graph.add_node(
                            content_id,
                            InteractionNode {
                                name: content_id.clone(),
                                node_type: NodeType::Tab,
                                component: None,
                                local_state: vec![],
                                on_enter: vec![],
                                on_exit: vec![],
                            },
                        );
                    }

                    // Add edges between tabs
                    for (j, other_content_id) in content_ids.iter().enumerate() {
                        if i != j {
                            self.graph.add_edge(InteractionEdge {
                                from: content_id.clone(),
                                to: other_content_id.clone(),
                                trigger: Trigger {
                                    trigger_type: TriggerType::Tap,
                                    element_id: Some(tab_ids[j].clone()),
                                    event: Some("tab_switch".to_string()),
                                },
                                animation: Some("crossfade".to_string()),
                                guard: None,
                                actions: vec![format!("set_active_tab({})", j)],
                            });
                        }
                    }
                }
            }

            InteractionPattern::ModalTrigger {
                trigger_id,
                modal_id,
            } => {
                // Add modal node if not exists
                if !self.graph.nodes.contains_key(modal_id) {
                    self.graph.add_node(
                        modal_id,
                        InteractionNode {
                            name: modal_id.clone(),
                            node_type: NodeType::Modal,
                            component: None,
                            local_state: vec![format!("{}_visible", modal_id)],
                            on_enter: vec![],
                            on_exit: vec![],
                        },
                    );
                }

                // Edge to open modal (from any screen that contains the trigger)
                // This is a simplified version - in practice we'd need to know the source screen
                self.graph.add_edge(InteractionEdge {
                    from: "main".to_string(), // Placeholder
                    to: modal_id.clone(),
                    trigger: Trigger {
                        trigger_type: TriggerType::Tap,
                        element_id: Some(trigger_id.clone()),
                        event: Some("open_modal".to_string()),
                    },
                    animation: Some("fade_in".to_string()),
                    guard: None,
                    actions: vec![format!("show_{}", modal_id)],
                });

                // Edge to close modal
                self.graph.add_edge(InteractionEdge {
                    from: modal_id.clone(),
                    to: "main".to_string(), // Placeholder
                    trigger: Trigger {
                        trigger_type: TriggerType::Tap,
                        element_id: None,
                        event: Some("close_modal".to_string()),
                    },
                    animation: Some("fade_out".to_string()),
                    guard: None,
                    actions: vec![format!("hide_{}", modal_id)],
                });
            }

            InteractionPattern::NavigationDrawer {
                menu_items,
                toggle_id,
            } => {
                // Each menu item could be a navigation destination
                for (i, item_id) in menu_items.iter().enumerate() {
                    let screen_id = format!("screen_{}", item_id);

                    if !self.graph.nodes.contains_key(&screen_id) {
                        self.graph.add_node(
                            &screen_id,
                            InteractionNode {
                                name: screen_id.clone(),
                                node_type: NodeType::Screen,
                                component: None,
                                local_state: vec![],
                                on_enter: vec![],
                                on_exit: vec![],
                            },
                        );
                    }

                    // Navigation edges from drawer to each screen
                    self.graph.add_edge(InteractionEdge {
                        from: "drawer".to_string(),
                        to: screen_id,
                        trigger: Trigger {
                            trigger_type: TriggerType::Tap,
                            element_id: Some(item_id.clone()),
                            event: Some("navigate".to_string()),
                        },
                        animation: Some("slide".to_string()),
                        guard: None,
                        actions: vec![format!("navigate_to_item({})", i)],
                    });
                }

                // Toggle drawer edge
                if let Some(toggle) = toggle_id {
                    self.graph.add_edge(InteractionEdge {
                        from: "main".to_string(),
                        to: "drawer".to_string(),
                        trigger: Trigger {
                            trigger_type: TriggerType::Tap,
                            element_id: Some(toggle.clone()),
                            event: Some("toggle_drawer".to_string()),
                        },
                        animation: Some("slide_in".to_string()),
                        guard: None,
                        actions: vec!["toggle_drawer".to_string()],
                    });
                }
            }

            InteractionPattern::Wizard { step_ids } => {
                // Create edges between steps
                for (i, step_id) in step_ids.iter().enumerate() {
                    if !self.graph.nodes.contains_key(step_id) {
                        self.graph.add_node(
                            step_id,
                            InteractionNode {
                                name: step_id.clone(),
                                node_type: NodeType::Step,
                                component: None,
                                local_state: vec![],
                                on_enter: vec![],
                                on_exit: vec![],
                            },
                        );
                    }

                    // Next step edge
                    if i + 1 < step_ids.len() {
                        self.graph.add_edge(InteractionEdge {
                            from: step_id.clone(),
                            to: step_ids[i + 1].clone(),
                            trigger: Trigger {
                                trigger_type: TriggerType::Tap,
                                element_id: Some(format!("{}_next", step_id)),
                                event: Some("next_step".to_string()),
                            },
                            animation: Some("slide_left".to_string()),
                            guard: Some(format!("step_{}_valid", i)),
                            actions: vec![format!("set_step({})", i + 1)],
                        });
                    }

                    // Previous step edge
                    if i > 0 {
                        self.graph.add_edge(InteractionEdge {
                            from: step_id.clone(),
                            to: step_ids[i - 1].clone(),
                            trigger: Trigger {
                                trigger_type: TriggerType::Tap,
                                element_id: Some(format!("{}_back", step_id)),
                                event: Some("prev_step".to_string()),
                            },
                            animation: Some("slide_right".to_string()),
                            guard: None,
                            actions: vec![format!("set_step({})", i - 1)],
                        });
                    }
                }
            }

            InteractionPattern::InfiniteScroll { list_id, load_more } => {
                // Add edge for load more action
                if let Some(load_action) = load_more {
                    let list_node = format!("{}_screen", list_id);

                    if !self.graph.nodes.contains_key(&list_node) {
                        self.graph.add_node(
                            &list_node,
                            InteractionNode {
                                name: list_node.clone(),
                                node_type: NodeType::Screen,
                                component: Some(list_id.clone()),
                                local_state: vec![
                                    format!("{}_items", list_id),
                                    format!("{}_loading", list_id),
                                ],
                                on_enter: vec![load_action.clone()],
                                on_exit: vec![],
                            },
                        );
                    }

                    // Self-loop for loading more
                    self.graph.add_edge(InteractionEdge {
                        from: list_node.clone(),
                        to: list_node,
                        trigger: Trigger {
                            trigger_type: TriggerType::Gesture,
                            element_id: Some(list_id.clone()),
                            event: Some("scroll_end".to_string()),
                        },
                        animation: None,
                        guard: Some(format!("!{}_loading", list_id)),
                        actions: vec![load_action.clone()],
                    });
                }
            }

            _ => {}
        }

        Ok(())
    }
}

impl Default for InteractionGraphBuilder {
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

    fn make_screen(name: &str) -> CodegenElement {
        CodegenElement::new("Frame")
            .with_name(name)
            .with_property("role", PropertyValue::Keyword("screen".to_string()))
    }

    #[test]
    fn test_screen_detection() {
        let mut builder = InteractionGraphBuilder::new();
        let screen = make_screen("home_screen");

        builder.process_element(&screen).unwrap();
        let graph = builder.build().unwrap();

        assert!(graph.nodes.contains_key("home_screen"));
        assert_eq!(graph.nodes["home_screen"].node_type, NodeType::Screen);
    }

    #[test]
    fn test_tabbed_interface_pattern() {
        let mut builder = InteractionGraphBuilder::new();
        let patterns = vec![InteractionPattern::TabbedInterface {
            tab_ids: vec!["tab1".to_string(), "tab2".to_string()],
            content_ids: vec!["content1".to_string(), "content2".to_string()],
        }];

        builder.process_patterns(&patterns).unwrap();
        let graph = builder.build().unwrap();

        assert!(graph.nodes.contains_key("content1"));
        assert!(graph.nodes.contains_key("content2"));
        assert!(!graph.edges.is_empty());
    }

    #[test]
    fn test_wizard_pattern() {
        let mut builder = InteractionGraphBuilder::new();
        let patterns = vec![InteractionPattern::Wizard {
            step_ids: vec!["step1".to_string(), "step2".to_string(), "step3".to_string()],
        }];

        builder.process_patterns(&patterns).unwrap();
        let graph = builder.build().unwrap();

        assert_eq!(graph.nodes.len(), 3);
        // Should have forward and back edges between steps
        // step1 -> step2, step2 -> step3, step2 -> step1, step3 -> step2
        assert_eq!(graph.edges.len(), 4);
    }
}
