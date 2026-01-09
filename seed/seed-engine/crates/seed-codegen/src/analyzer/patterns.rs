//! Interaction pattern recognition.

use crate::element::{CodegenElement, PropertyValue};
use std::collections::HashMap;

/// Recognized interaction pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionPattern {
    /// Form with submit action.
    Form {
        fields: Vec<String>,
        submit_action: Option<String>,
    },
    /// List with selectable items.
    SelectableList {
        list_id: String,
        selection_mode: SelectionMode,
    },
    /// Tabbed interface.
    TabbedInterface {
        tab_ids: Vec<String>,
        content_ids: Vec<String>,
    },
    /// Navigation drawer/sidebar.
    NavigationDrawer {
        menu_items: Vec<String>,
        toggle_id: Option<String>,
    },
    /// Modal/dialog trigger.
    ModalTrigger {
        trigger_id: String,
        modal_id: String,
    },
    /// Expandable/collapsible section.
    Accordion {
        section_ids: Vec<String>,
    },
    /// Carousel/slider.
    Carousel {
        container_id: String,
        slide_ids: Vec<String>,
    },
    /// Search with results.
    SearchInterface {
        input_id: String,
        results_id: String,
    },
    /// Filter controls.
    FilterControls {
        filter_ids: Vec<String>,
        target_id: String,
    },
    /// Pagination.
    Pagination {
        container_id: String,
        page_size: usize,
    },
    /// Infinite scroll.
    InfiniteScroll {
        list_id: String,
        load_more: Option<String>,
    },
    /// Drag and drop.
    DragAndDrop {
        draggable_ids: Vec<String>,
        drop_zone_ids: Vec<String>,
    },
    /// Wizard/stepper.
    Wizard {
        step_ids: Vec<String>,
    },
    /// Master-detail layout.
    MasterDetail {
        master_id: String,
        detail_id: String,
    },
    /// Counter pattern.
    Counter {
        value_id: String,
        increment_id: String,
        decrement_id: String,
    },
    /// Toggle pattern.
    Toggle {
        toggle_id: String,
        target_id: String,
    },
}

/// Selection mode for lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    Single,
    Multiple,
    None,
}

/// Recognizes interaction patterns in element trees.
pub struct PatternRecognizer {
    _private: (),
}

impl PatternRecognizer {
    /// Create a new pattern recognizer.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Recognize patterns in a list of elements.
    pub fn recognize(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        // Build element map for quick lookup
        let element_map = Self::build_element_map(elements);

        // Find form patterns
        patterns.extend(self.find_form_patterns(elements));

        // Find tab patterns
        patterns.extend(self.find_tab_patterns(elements));

        // Find list patterns
        patterns.extend(self.find_list_patterns(elements));

        // Find modal patterns
        patterns.extend(self.find_modal_patterns(elements, &element_map));

        // Find navigation patterns
        patterns.extend(self.find_navigation_patterns(elements));

        // Find counter patterns
        patterns.extend(self.find_counter_patterns(elements));

        // Find toggle patterns
        patterns.extend(self.find_toggle_patterns(elements));

        // Find search patterns
        patterns.extend(self.find_search_patterns(elements));

        // Find master-detail patterns
        patterns.extend(self.find_master_detail_patterns(elements));

        // Deduplicate
        patterns.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));
        patterns.dedup();

        patterns
    }

    /// Build element map for quick lookup.
    fn build_element_map(elements: &[CodegenElement]) -> HashMap<String, &CodegenElement> {
        let mut map = HashMap::new();
        Self::collect_elements(elements, &mut map);
        map
    }

    fn collect_elements<'a>(
        elements: &'a [CodegenElement],
        map: &mut HashMap<String, &'a CodegenElement>,
    ) {
        for (i, element) in elements.iter().enumerate() {
            if let Some(ref name) = element.name {
                map.insert(name.clone(), element);
            }
            let id = format!("{}_{}", element.element_type.to_lowercase(), i);
            map.insert(id, element);

            Self::collect_elements(&element.children, map);
        }
    }

    // Form patterns
    fn find_form_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if self.is_form_like(element) {
                let fields = self.find_form_fields(element);
                let submit_action = self.find_submit_action(element);

                if !fields.is_empty() {
                    patterns.push(InteractionPattern::Form {
                        fields,
                        submit_action,
                    });
                }
            }
        }

        patterns
    }

    fn is_form_like(&self, element: &CodegenElement) -> bool {
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        element.element_type == "Form"
            || name_lower.contains("form")
            || element.has_property("on_submit")
    }

    fn find_form_fields(&self, element: &CodegenElement) -> Vec<String> {
        let mut fields = Vec::new();
        self.collect_fields(&element.children, &mut fields);
        fields
    }

    fn collect_fields(&self, elements: &[CodegenElement], fields: &mut Vec<String>) {
        for element in elements {
            let is_field = matches!(
                element.element_type.as_str(),
                "Input" | "TextArea" | "Select" | "Checkbox" | "Radio" | "Switch"
            ) || element.has_property("placeholder")
                || element.has_property("value");

            if is_field {
                let id = element
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("field_{}", fields.len()));
                fields.push(id);
            }

            self.collect_fields(&element.children, fields);
        }
    }

    fn find_submit_action(&self, element: &CodegenElement) -> Option<String> {
        for child in &element.children {
            let name_lower = child
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if name_lower.contains("submit") || child.has_property("type") {
                if let Some(PropertyValue::String(s)) = child.get_property("action") {
                    return Some(s.clone());
                }
                if let Some(PropertyValue::String(s)) = child.get_property("on_click") {
                    return Some(s.clone());
                }
            }

            if let Some(action) = self.find_submit_action(child) {
                return Some(action);
            }
        }

        None
    }

    // Tab patterns
    fn find_tab_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if self.is_tab_container(element) {
                let (tab_ids, content_ids) = self.find_tabs_and_content(element);

                if !tab_ids.is_empty() {
                    patterns.push(InteractionPattern::TabbedInterface {
                        tab_ids,
                        content_ids,
                    });
                }
            }
        }

        patterns
    }

    fn is_tab_container(&self, element: &CodegenElement) -> bool {
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        name_lower.contains("tab") || element.element_type == "TabBar"
    }

    fn find_tabs_and_content(&self, element: &CodegenElement) -> (Vec<String>, Vec<String>) {
        let mut tab_ids = Vec::new();
        let mut content_ids = Vec::new();

        for (i, child) in element.children.iter().enumerate() {
            let name_lower = child
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if name_lower.contains("tab") && !name_lower.contains("content") {
                let id = child.name.clone().unwrap_or_else(|| format!("tab_{}", i));
                tab_ids.push(id);
            } else if name_lower.contains("content") || name_lower.contains("panel") {
                let id = child
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("content_{}", i));
                content_ids.push(id);
            }
        }

        (tab_ids, content_ids)
    }

    // List patterns
    fn find_list_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if self.is_selectable_list(element) {
                let list_id = element
                    .name
                    .clone()
                    .unwrap_or_else(|| "list".to_string());
                let selection_mode = self.determine_selection_mode(element);

                patterns.push(InteractionPattern::SelectableList {
                    list_id,
                    selection_mode,
                });
            }
        }

        patterns
    }

    fn is_selectable_list(&self, element: &CodegenElement) -> bool {
        (element.element_type == "List" || element.element_type == "Grid")
            && (element.has_property("on_select")
                || element.has_property("selectable")
                || element.has_property("selection"))
    }

    fn determine_selection_mode(&self, element: &CodegenElement) -> SelectionMode {
        if let Some(prop) = element
            .get_property("selection_mode")
            .or_else(|| element.get_property("selectable"))
        {
            if let PropertyValue::Keyword(k) = prop {
                match k.to_lowercase().as_str() {
                    "single" | "one" => return SelectionMode::Single,
                    "multiple" | "multi" => return SelectionMode::Multiple,
                    "none" | "false" => return SelectionMode::None,
                    _ => {}
                }
            }
        }

        SelectionMode::Single
    }

    // Modal patterns
    fn find_modal_patterns(
        &self,
        elements: &[CodegenElement],
        _element_map: &HashMap<String, &CodegenElement>,
    ) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if self.is_modal(element) {
                let modal_id = element
                    .name
                    .clone()
                    .unwrap_or_else(|| "modal".to_string());

                if let Some(trigger_id) = self.find_modal_trigger(elements, &modal_id) {
                    patterns.push(InteractionPattern::ModalTrigger {
                        trigger_id,
                        modal_id,
                    });
                }
            }
        }

        patterns
    }

    fn is_modal(&self, element: &CodegenElement) -> bool {
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        name_lower.contains("modal")
            || name_lower.contains("dialog")
            || name_lower.contains("popup")
            || element.element_type == "Modal"
            || element.element_type == "Dialog"
    }

    fn find_modal_trigger(&self, elements: &[CodegenElement], modal_id: &str) -> Option<String> {
        for element in elements {
            if let Some(PropertyValue::String(s)) = element.get_property("action") {
                if s.contains(modal_id) || s.contains("show") || s.contains("open") {
                    return element.name.clone();
                }
            }

            if let Some(PropertyValue::String(s)) = element.get_property("target") {
                if s == modal_id {
                    return element.name.clone();
                }
            }

            if let Some(id) = self.find_modal_trigger(&element.children, modal_id) {
                return Some(id);
            }
        }

        None
    }

    // Navigation patterns
    fn find_navigation_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if self.is_nav_drawer(element) {
                let menu_items = self.find_menu_items(element);
                let toggle_id = self.find_nav_toggle(elements, element);

                if !menu_items.is_empty() {
                    patterns.push(InteractionPattern::NavigationDrawer {
                        menu_items,
                        toggle_id,
                    });
                }
            }
        }

        patterns
    }

    fn is_nav_drawer(&self, element: &CodegenElement) -> bool {
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        name_lower.contains("drawer")
            || name_lower.contains("sidebar")
            || name_lower.contains("navigation")
            || element.element_type == "Drawer"
    }

    fn find_menu_items(&self, element: &CodegenElement) -> Vec<String> {
        let mut items = Vec::new();

        for (i, child) in element.children.iter().enumerate() {
            let name_lower = child
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if name_lower.contains("item")
                || name_lower.contains("link")
                || child.has_property("href")
            {
                let id = child
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("nav_item_{}", i));
                items.push(id);
            }
        }

        items
    }

    fn find_nav_toggle(
        &self,
        elements: &[CodegenElement],
        _drawer: &CodegenElement,
    ) -> Option<String> {
        for element in elements {
            let name_lower = element
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if name_lower.contains("toggle")
                || name_lower.contains("hamburger")
                || name_lower.contains("menu_button")
            {
                return element.name.clone();
            }

            if let Some(id) = self.find_nav_toggle(&element.children, _drawer) {
                return Some(id);
            }
        }

        None
    }

    // Counter patterns
    fn find_counter_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if let Some(pattern) = self.find_counter(element) {
                patterns.push(pattern);
            }
        }

        patterns
    }

    fn find_counter(&self, element: &CodegenElement) -> Option<InteractionPattern> {
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        if !name_lower.contains("counter") && !name_lower.contains("quantity") {
            for child in &element.children {
                if let Some(pattern) = self.find_counter(child) {
                    return Some(pattern);
                }
            }
            return None;
        }

        let mut value_id = None;
        let mut increment_id = None;
        let mut decrement_id = None;

        for child in &element.children {
            let child_name = child
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if child_name.contains("increment")
                || child_name.contains("plus")
                || child_name.contains("add")
            {
                increment_id = child.name.clone();
            } else if child_name.contains("decrement")
                || child_name.contains("minus")
                || child_name.contains("subtract")
            {
                decrement_id = child.name.clone();
            } else if child_name.contains("value") || child_name.contains("count") {
                value_id = child.name.clone();
            }
        }

        if let (Some(value), Some(inc), Some(dec)) = (value_id, increment_id, decrement_id) {
            Some(InteractionPattern::Counter {
                value_id: value,
                increment_id: inc,
                decrement_id: dec,
            })
        } else {
            None
        }
    }

    // Toggle patterns
    fn find_toggle_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            self.find_toggles(element, &mut patterns);
        }

        patterns
    }

    fn find_toggles(&self, element: &CodegenElement, patterns: &mut Vec<InteractionPattern>) {
        if let Some(PropertyValue::String(target_id)) = element
            .get_property("toggles")
            .or_else(|| element.get_property("target"))
        {
            if let Some(toggle_id) = &element.name {
                patterns.push(InteractionPattern::Toggle {
                    toggle_id: toggle_id.clone(),
                    target_id: target_id.clone(),
                });
            }
        }

        for child in &element.children {
            self.find_toggles(child, patterns);
        }
    }

    // Search patterns
    fn find_search_patterns(&self, elements: &[CodegenElement]) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        for element in elements {
            if let Some(pattern) = self.find_search(element) {
                patterns.push(pattern);
            }
        }

        patterns
    }

    fn find_search(&self, element: &CodegenElement) -> Option<InteractionPattern> {
        let name_lower = element
            .name
            .as_ref()
            .map(|n| n.to_lowercase())
            .unwrap_or_default();

        if !name_lower.contains("search") {
            for child in &element.children {
                if let Some(pattern) = self.find_search(child) {
                    return Some(pattern);
                }
            }
            return None;
        }

        let mut input_id = None;
        let mut results_id = None;

        for child in &element.children {
            let child_name = child
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if child_name.contains("input") || child.element_type == "Input" {
                input_id = child.name.clone();
            } else if child_name.contains("results") || child_name.contains("list") {
                results_id = child.name.clone();
            }
        }

        if let (Some(input), Some(results)) = (input_id, results_id) {
            Some(InteractionPattern::SearchInterface {
                input_id: input,
                results_id: results,
            })
        } else {
            None
        }
    }

    // Master-detail patterns
    fn find_master_detail_patterns(
        &self,
        elements: &[CodegenElement],
    ) -> Vec<InteractionPattern> {
        let mut patterns = Vec::new();

        let mut master_id = None;
        let mut detail_id = None;

        for element in elements {
            let name_lower = element
                .name
                .as_ref()
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            if name_lower.contains("master") || name_lower.contains("list") {
                master_id = element.name.clone();
            } else if name_lower.contains("detail") || name_lower.contains("preview") {
                detail_id = element.name.clone();
            }
        }

        if let (Some(master), Some(detail)) = (master_id, detail_id) {
            patterns.push(InteractionPattern::MasterDetail {
                master_id: master,
                detail_id: detail,
            });
        }

        patterns
    }
}

impl Default for PatternRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_recognizer_creation() {
        let recognizer = PatternRecognizer::new();
        // Just verify it creates
        let _ = recognizer;
    }

    #[test]
    fn test_selection_mode() {
        assert_eq!(SelectionMode::Single as u8, SelectionMode::Single as u8);
    }

    #[test]
    fn test_find_form_pattern() {
        let recognizer = PatternRecognizer::new();

        let form = CodegenElement::new("Form")
            .with_name("login_form")
            .with_child(
                CodegenElement::new("Input")
                    .with_name("username")
                    .with_property("placeholder", PropertyValue::String("Username".to_string())),
            )
            .with_child(
                CodegenElement::new("Input")
                    .with_name("password")
                    .with_property("placeholder", PropertyValue::String("Password".to_string())),
            );

        let patterns = recognizer.recognize(&[form]);

        assert!(patterns
            .iter()
            .any(|p| matches!(p, InteractionPattern::Form { .. })));
    }
}
