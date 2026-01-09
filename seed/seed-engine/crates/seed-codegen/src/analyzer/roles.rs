//! Semantic role detection for UI elements.

use crate::element::{CodegenElement, PropertyValue};
use std::collections::HashSet;

/// Semantic role of a UI element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticRole {
    // Layout roles
    Container,
    Header,
    Footer,
    Sidebar,
    Main,
    Section,
    Card,
    List,
    ListItem,
    Grid,
    GridCell,

    // Navigation roles
    Navigation,
    NavItem,
    TabBar,
    Tab,
    Breadcrumb,
    Link,

    // Interactive roles
    Button,
    IconButton,
    ToggleButton,
    Input,
    TextArea,
    Select,
    Checkbox,
    Radio,
    Slider,
    Switch,
    DatePicker,

    // Content roles
    Heading,
    Paragraph,
    Label,
    Caption,
    Badge,
    Tag,
    Avatar,
    Icon,
    Image,
    Video,

    // Feedback roles
    Alert,
    Toast,
    Tooltip,
    Progress,
    Spinner,
    Skeleton,

    // Overlay roles
    Modal,
    Dialog,
    Popover,
    Dropdown,
    Menu,
    MenuItem,
    Sheet,

    // Form roles
    Form,
    FormField,
    FormGroup,

    // Misc
    Divider,
    Spacer,
    Unknown,
}

impl SemanticRole {
    /// Check if this role represents interactive content.
    pub fn is_interactive(&self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::IconButton
                | Self::ToggleButton
                | Self::Input
                | Self::TextArea
                | Self::Select
                | Self::Checkbox
                | Self::Radio
                | Self::Slider
                | Self::Switch
                | Self::DatePicker
                | Self::Link
                | Self::Tab
                | Self::NavItem
                | Self::MenuItem
        )
    }

    /// Check if this role represents a container.
    pub fn is_container(&self) -> bool {
        matches!(
            self,
            Self::Container
                | Self::Header
                | Self::Footer
                | Self::Sidebar
                | Self::Main
                | Self::Section
                | Self::Card
                | Self::List
                | Self::Grid
                | Self::Form
                | Self::FormGroup
                | Self::Navigation
                | Self::Modal
                | Self::Dialog
        )
    }

    /// Get the default accessibility role.
    pub fn accessibility_role(&self) -> &'static str {
        match self {
            Self::Button | Self::IconButton | Self::ToggleButton => "button",
            Self::Link => "link",
            Self::Input | Self::TextArea => "textbox",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Slider => "slider",
            Self::Switch => "switch",
            Self::Tab => "tab",
            Self::TabBar => "tablist",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Heading => "heading",
            Self::Image => "img",
            Self::Navigation => "navigation",
            Self::Main => "main",
            Self::Header => "banner",
            Self::Footer => "contentinfo",
            Self::Alert => "alert",
            Self::Dialog | Self::Modal => "dialog",
            Self::Progress => "progressbar",
            Self::Form => "form",
            _ => "generic",
        }
    }
}

/// Detects semantic roles for UI elements.
pub struct RoleDetector {
    /// Keywords that suggest specific roles.
    role_keywords: Vec<(SemanticRole, Vec<&'static str>)>,
}

impl RoleDetector {
    /// Create a new role detector.
    pub fn new() -> Self {
        Self {
            role_keywords: vec![
                // Layout
                (SemanticRole::Header, vec!["header", "top_bar", "app_bar", "toolbar"]),
                (SemanticRole::Footer, vec!["footer", "bottom_bar"]),
                (SemanticRole::Sidebar, vec!["sidebar", "side_bar", "drawer", "side_menu"]),
                (SemanticRole::Main, vec!["main", "content", "body"]),
                (SemanticRole::Card, vec!["card", "tile", "panel"]),
                (SemanticRole::List, vec!["list", "feed", "timeline"]),
                (SemanticRole::Grid, vec!["grid", "gallery", "mosaic"]),

                // Navigation
                (SemanticRole::Navigation, vec!["nav", "navigation", "menu"]),
                (SemanticRole::TabBar, vec!["tab_bar", "tabs", "tab_strip"]),
                (SemanticRole::Tab, vec!["tab"]),
                (SemanticRole::Breadcrumb, vec!["breadcrumb", "crumb"]),

                // Interactive
                (SemanticRole::Button, vec!["button", "btn", "cta", "action"]),
                (SemanticRole::IconButton, vec!["icon_button", "icon_btn"]),
                (SemanticRole::Input, vec!["input", "text_field", "text_input", "field"]),
                (SemanticRole::TextArea, vec!["textarea", "text_area", "multiline"]),
                (SemanticRole::Select, vec!["select", "dropdown", "picker", "chooser"]),
                (SemanticRole::Checkbox, vec!["checkbox", "check_box", "check"]),
                (SemanticRole::Radio, vec!["radio", "radio_button"]),
                (SemanticRole::Switch, vec!["switch", "toggle"]),
                (SemanticRole::Slider, vec!["slider", "range"]),

                // Content
                (SemanticRole::Heading, vec!["heading", "title", "h1", "h2", "h3"]),
                (SemanticRole::Paragraph, vec!["paragraph", "text", "body_text"]),
                (SemanticRole::Label, vec!["label"]),
                (SemanticRole::Caption, vec!["caption", "subtitle", "description"]),
                (SemanticRole::Badge, vec!["badge", "chip", "pill"]),
                (SemanticRole::Avatar, vec!["avatar", "profile_image", "user_image"]),
                (SemanticRole::Icon, vec!["icon", "symbol"]),
                (SemanticRole::Image, vec!["image", "img", "photo", "picture"]),

                // Feedback
                (SemanticRole::Alert, vec!["alert", "warning", "error", "info", "success"]),
                (SemanticRole::Toast, vec!["toast", "snackbar", "notification"]),
                (SemanticRole::Progress, vec!["progress", "loading", "loader"]),
                (SemanticRole::Spinner, vec!["spinner", "activity_indicator"]),
                (SemanticRole::Skeleton, vec!["skeleton", "placeholder"]),

                // Overlay
                (SemanticRole::Modal, vec!["modal", "overlay"]),
                (SemanticRole::Dialog, vec!["dialog", "popup", "alert_dialog"]),
                (SemanticRole::Popover, vec!["popover"]),
                (SemanticRole::Menu, vec!["menu", "context_menu"]),
                (SemanticRole::Sheet, vec!["sheet", "bottom_sheet", "action_sheet"]),

                // Form
                (SemanticRole::Form, vec!["form"]),
                (SemanticRole::FormField, vec!["field", "form_field"]),
                (SemanticRole::FormGroup, vec!["form_group", "field_group"]),

                // Misc
                (SemanticRole::Divider, vec!["divider", "separator", "hr"]),
                (SemanticRole::Spacer, vec!["spacer", "gap"]),
            ],
        }
    }

    /// Detect semantic roles for an element.
    pub fn detect_roles(&self, element: &CodegenElement) -> HashSet<SemanticRole> {
        let mut roles = HashSet::new();

        // Check element type first
        if let Some(role) = self.role_from_type(&element.element_type) {
            roles.insert(role);
        }

        // Check element name/ID
        if let Some(ref name) = element.name {
            self.detect_from_name(name, &mut roles);
        }

        // Check for role property
        if let Some(PropertyValue::Keyword(k)) = element.get_property("role") {
            if let Some(role) = self.role_from_keyword(k) {
                roles.insert(role);
            }
        }
        if let Some(PropertyValue::String(k)) = element.get_property("role") {
            if let Some(role) = self.role_from_keyword(k) {
                roles.insert(role);
            }
        }

        // Check for semantic hints from properties
        self.detect_from_properties(element, &mut roles);

        // If no roles found, mark as unknown
        if roles.is_empty() {
            roles.insert(SemanticRole::Unknown);
        }

        roles
    }

    /// Detect role from element type.
    fn role_from_type(&self, element_type: &str) -> Option<SemanticRole> {
        match element_type.to_lowercase().as_str() {
            "frame" => Some(SemanticRole::Container),
            "text" => Some(SemanticRole::Paragraph),
            "image" => Some(SemanticRole::Image),
            "icon" => Some(SemanticRole::Icon),
            "button" => Some(SemanticRole::Button),
            "input" => Some(SemanticRole::Input),
            "select" => Some(SemanticRole::Select),
            "checkbox" => Some(SemanticRole::Checkbox),
            "switch" => Some(SemanticRole::Switch),
            "slider" => Some(SemanticRole::Slider),
            "list" => Some(SemanticRole::List),
            "grid" => Some(SemanticRole::Grid),
            _ => None,
        }
    }

    /// Detect roles from element name.
    fn detect_from_name(&self, name: &str, roles: &mut HashSet<SemanticRole>) {
        let name_lower = name.to_lowercase();

        for (role, keywords) in &self.role_keywords {
            for keyword in keywords {
                if name_lower.contains(keyword) {
                    roles.insert(*role);
                    break;
                }
            }
        }
    }

    /// Detect role from keyword.
    fn role_from_keyword(&self, keyword: &str) -> Option<SemanticRole> {
        let keyword_lower = keyword.to_lowercase();

        for (role, keywords) in &self.role_keywords {
            if keywords.contains(&keyword_lower.as_str()) {
                return Some(*role);
            }
        }

        None
    }

    /// Detect roles from element properties.
    fn detect_from_properties(&self, element: &CodegenElement, roles: &mut HashSet<SemanticRole>) {
        // Check for onClick -> likely interactive
        if element.has_property("on_click")
            || element.has_property("onClick")
            || element.has_property("action")
        {
            if !roles.iter().any(|r| r.is_interactive()) {
                roles.insert(SemanticRole::Button);
            }
        }

        // Check for href/link -> Link role
        if element.has_property("href") || element.has_property("link") {
            roles.insert(SemanticRole::Link);
        }

        // Check for placeholder -> likely Input
        if element.has_property("placeholder") {
            if !roles.contains(&SemanticRole::Input)
                && !roles.contains(&SemanticRole::TextArea)
            {
                roles.insert(SemanticRole::Input);
            }
        }

        // Check for options -> likely Select
        if element.has_property("options") {
            roles.insert(SemanticRole::Select);
        }
    }
}

impl Default for RoleDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_is_interactive() {
        assert!(SemanticRole::Button.is_interactive());
        assert!(SemanticRole::Input.is_interactive());
        assert!(!SemanticRole::Container.is_interactive());
        assert!(!SemanticRole::Heading.is_interactive());
    }

    #[test]
    fn test_role_is_container() {
        assert!(SemanticRole::Container.is_container());
        assert!(SemanticRole::Card.is_container());
        assert!(!SemanticRole::Button.is_container());
    }

    #[test]
    fn test_role_from_keyword() {
        let detector = RoleDetector::new();
        assert_eq!(detector.role_from_keyword("button"), Some(SemanticRole::Button));
        assert_eq!(detector.role_from_keyword("card"), Some(SemanticRole::Card));
        assert_eq!(detector.role_from_keyword("unknown_role"), None);
    }

    #[test]
    fn test_detect_roles_from_element() {
        let detector = RoleDetector::new();
        let element = CodegenElement::new("Button")
            .with_name("submit_button");

        let roles = detector.detect_roles(&element);
        assert!(roles.contains(&SemanticRole::Button));
    }
}
