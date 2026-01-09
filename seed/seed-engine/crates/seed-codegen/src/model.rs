//! Data models for code generation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use indexmap::IndexMap;

/// State model extracted from design constraints.
///
/// Represents the reactive state needed to implement the design.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateModel {
    /// State variables.
    pub variables: IndexMap<String, StateVariable>,
    /// Computed properties derived from state.
    pub computed: IndexMap<String, ComputedProperty>,
    /// Actions that modify state.
    pub actions: IndexMap<String, StateAction>,
    /// Bindings between state and UI elements.
    pub bindings: Vec<StateBinding>,
}

impl StateModel {
    /// Create a new empty state model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a state variable.
    pub fn add_variable(&mut self, name: impl Into<String>, var: StateVariable) {
        self.variables.insert(name.into(), var);
    }

    /// Add a computed property.
    pub fn add_computed(&mut self, name: impl Into<String>, prop: ComputedProperty) {
        self.computed.insert(name.into(), prop);
    }

    /// Add an action.
    pub fn add_action(&mut self, name: impl Into<String>, action: StateAction) {
        self.actions.insert(name.into(), action);
    }

    /// Add a binding.
    pub fn add_binding(&mut self, binding: StateBinding) {
        self.bindings.push(binding);
    }

    /// Get all variables that an element depends on.
    pub fn dependencies_for_element(&self, element_id: &str) -> Vec<&str> {
        self.bindings
            .iter()
            .filter(|b| b.element_id == element_id)
            .map(|b| b.variable.as_str())
            .collect()
    }
}

/// A state variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVariable {
    /// Variable type.
    pub var_type: StateType,
    /// Default value.
    pub default_value: Option<serde_json::Value>,
    /// Whether this is a published/observable property.
    pub observable: bool,
    /// Optional validation constraints.
    pub validation: Option<String>,
    /// Documentation comment.
    pub doc: Option<String>,
}

/// Type of state variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateType {
    String,
    Int,
    Float,
    Bool,
    Array,
    Object,
    Optional,
    Custom,
}

impl StateType {
    /// Convert to Swift type.
    pub fn to_swift(&self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Int => "Int",
            Self::Float => "Double",
            Self::Bool => "Bool",
            Self::Array => "[Any]",
            Self::Object => "[String: Any]",
            Self::Optional => "Any?",
            Self::Custom => "Any",
        }
    }

    /// Convert to Kotlin type.
    pub fn to_kotlin(&self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Int => "Int",
            Self::Float => "Double",
            Self::Bool => "Boolean",
            Self::Array => "List<Any>",
            Self::Object => "Map<String, Any>",
            Self::Optional => "Any?",
            Self::Custom => "Any",
        }
    }

    /// Convert to TypeScript type.
    pub fn to_typescript(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Int | Self::Float => "number",
            Self::Bool => "boolean",
            Self::Array => "any[]",
            Self::Object => "Record<string, any>",
            Self::Optional => "any | null",
            Self::Custom => "any",
        }
    }
}

/// A computed property derived from state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedProperty {
    /// Return type.
    pub return_type: StateType,
    /// Dependencies (state variable names).
    pub dependencies: Vec<String>,
    /// Computation expression.
    pub expression: String,
    /// Documentation comment.
    pub doc: Option<String>,
}

/// An action that modifies state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateAction {
    /// Parameters for the action.
    pub parameters: Vec<ActionParameter>,
    /// State modifications.
    pub mutations: Vec<StateMutation>,
    /// Side effects to perform.
    pub side_effects: Vec<SideEffect>,
    /// Documentation comment.
    pub doc: Option<String>,
}

/// Parameter for an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionParameter {
    /// Parameter name.
    pub name: String,
    /// Parameter type.
    pub param_type: StateType,
    /// Whether the parameter is optional.
    pub optional: bool,
}

/// A mutation to state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMutation {
    /// Variable to mutate.
    pub variable: String,
    /// Mutation expression.
    pub expression: String,
}

/// A side effect from an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffect {
    /// Type of side effect.
    pub effect_type: SideEffectType,
    /// Effect parameters.
    pub params: HashMap<String, serde_json::Value>,
}

/// Type of side effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectType {
    Navigate,
    ApiCall,
    LocalStorage,
    Analytics,
    Notification,
    Custom,
}

/// Binding between state and UI element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateBinding {
    /// Element ID.
    pub element_id: String,
    /// Property to bind.
    pub property: String,
    /// State variable name.
    pub variable: String,
    /// Transform expression (optional).
    pub transform: Option<String>,
    /// Whether this is a two-way binding.
    pub two_way: bool,
}

/// Graph of user interactions and flows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InteractionGraph {
    /// Nodes in the graph (screens/views).
    pub nodes: IndexMap<String, InteractionNode>,
    /// Edges (transitions between nodes).
    pub edges: Vec<InteractionEdge>,
    /// Entry point node ID.
    pub entry_point: Option<String>,
}

impl InteractionGraph {
    /// Create a new empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node.
    pub fn add_node(&mut self, id: impl Into<String>, node: InteractionNode) {
        let id = id.into();
        if self.entry_point.is_none() {
            self.entry_point = Some(id.clone());
        }
        self.nodes.insert(id, node);
    }

    /// Add an edge.
    pub fn add_edge(&mut self, edge: InteractionEdge) {
        self.edges.push(edge);
    }

    /// Get all outgoing edges from a node.
    pub fn outgoing_edges(&self, node_id: &str) -> Vec<&InteractionEdge> {
        self.edges.iter().filter(|e| e.from == node_id).collect()
    }

    /// Get all incoming edges to a node.
    pub fn incoming_edges(&self, node_id: &str) -> Vec<&InteractionEdge> {
        self.edges.iter().filter(|e| e.to == node_id).collect()
    }
}

/// A node in the interaction graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionNode {
    /// Human-readable name.
    pub name: String,
    /// Node type.
    pub node_type: NodeType,
    /// Associated component/view.
    pub component: Option<String>,
    /// Local state for this node.
    pub local_state: Vec<String>,
    /// Entry actions.
    pub on_enter: Vec<String>,
    /// Exit actions.
    pub on_exit: Vec<String>,
}

/// Type of interaction node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Screen,
    Modal,
    Sheet,
    Popover,
    Tab,
    Step,
}

/// An edge in the interaction graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionEdge {
    /// Source node ID.
    pub from: String,
    /// Target node ID.
    pub to: String,
    /// Trigger that causes the transition.
    pub trigger: Trigger,
    /// Transition animation.
    pub animation: Option<String>,
    /// Guard condition.
    pub guard: Option<String>,
    /// Actions to perform during transition.
    pub actions: Vec<String>,
}

/// Trigger for a transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    /// Trigger type.
    pub trigger_type: TriggerType,
    /// Element ID that triggers (if applicable).
    pub element_id: Option<String>,
    /// Event name.
    pub event: Option<String>,
}

/// Type of trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerType {
    Tap,
    LongPress,
    Swipe,
    Gesture,
    Timer,
    StateChange,
    Api,
    External,
}

/// Extracted design system.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesignSystem {
    /// Color tokens.
    pub colors: IndexMap<String, ColorToken>,
    /// Typography tokens.
    pub typography: IndexMap<String, TypographyToken>,
    /// Spacing tokens.
    pub spacing: IndexMap<String, f64>,
    /// Border radius tokens.
    pub radii: IndexMap<String, f64>,
    /// Shadow tokens.
    pub shadows: IndexMap<String, ShadowToken>,
    /// Component styles.
    pub components: IndexMap<String, ComponentStyle>,
}

impl DesignSystem {
    /// Create a new empty design system.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a color token.
    pub fn add_color(&mut self, name: impl Into<String>, color: ColorToken) {
        self.colors.insert(name.into(), color);
    }

    /// Add a typography token.
    pub fn add_typography(&mut self, name: impl Into<String>, typography: TypographyToken) {
        self.typography.insert(name.into(), typography);
    }

    /// Add a spacing token.
    pub fn add_spacing(&mut self, name: impl Into<String>, value: f64) {
        self.spacing.insert(name.into(), value);
    }
}

/// A color token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorToken {
    /// Hex color value.
    pub hex: String,
    /// RGB components.
    pub rgb: Option<[u8; 3]>,
    /// Alpha value.
    pub alpha: f64,
    /// Dark mode variant.
    pub dark_mode: Option<String>,
}

impl ColorToken {
    /// Create from hex string.
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let rgb = if hex.len() >= 6 {
            Some([
                u8::from_str_radix(&hex[0..2], 16).unwrap_or(0),
                u8::from_str_radix(&hex[2..4], 16).unwrap_or(0),
                u8::from_str_radix(&hex[4..6], 16).unwrap_or(0),
            ])
        } else {
            None
        };

        Self {
            hex: format!("#{}", hex),
            rgb,
            alpha: 1.0,
            dark_mode: None,
        }
    }
}

/// A typography token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyToken {
    /// Font family.
    pub family: String,
    /// Font size.
    pub size: f64,
    /// Font weight.
    pub weight: FontWeight,
    /// Line height.
    pub line_height: Option<f64>,
    /// Letter spacing.
    pub letter_spacing: Option<f64>,
}

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontWeight {
    Thin,
    ExtraLight,
    Light,
    Regular,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

impl FontWeight {
    /// Convert to numeric weight.
    pub fn to_numeric(&self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::ExtraLight => 200,
            Self::Light => 300,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
            Self::Black => 900,
        }
    }

    /// Convert to Swift font weight.
    pub fn to_swift(&self) -> &'static str {
        match self {
            Self::Thin => ".thin",
            Self::ExtraLight => ".ultraLight",
            Self::Light => ".light",
            Self::Regular => ".regular",
            Self::Medium => ".medium",
            Self::SemiBold => ".semibold",
            Self::Bold => ".bold",
            Self::ExtraBold => ".heavy",
            Self::Black => ".black",
        }
    }
}

/// A shadow token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowToken {
    /// X offset.
    pub x: f64,
    /// Y offset.
    pub y: f64,
    /// Blur radius.
    pub blur: f64,
    /// Spread radius.
    pub spread: f64,
    /// Shadow color.
    pub color: String,
}

/// Style for a component.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentStyle {
    /// Base styles.
    pub base: HashMap<String, serde_json::Value>,
    /// Variant styles.
    pub variants: HashMap<String, HashMap<String, serde_json::Value>>,
    /// State styles (hover, pressed, etc.).
    pub states: HashMap<String, HashMap<String, serde_json::Value>>,
}

/// Specification for a generated component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSpec {
    /// Component name.
    pub name: String,
    /// Component type.
    pub component_type: ComponentType,
    /// Properties/props.
    pub props: Vec<PropSpec>,
    /// Child components.
    pub children: Vec<ComponentSpec>,
    /// Associated state.
    pub state: Option<String>,
    /// Style references.
    pub styles: Vec<String>,
    /// Event handlers.
    pub events: Vec<EventSpec>,
    /// Accessibility attributes.
    pub accessibility: AccessibilitySpec,
}

/// Type of component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentType {
    View,
    Stack,
    Text,
    Image,
    Button,
    Input,
    List,
    Grid,
    ScrollView,
    Navigation,
    Modal,
    Custom,
}

/// Specification for a component prop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropSpec {
    /// Prop name.
    pub name: String,
    /// Prop type.
    pub prop_type: StateType,
    /// Whether required.
    pub required: bool,
    /// Default value.
    pub default: Option<serde_json::Value>,
    /// Documentation.
    pub doc: Option<String>,
}

/// Specification for an event handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSpec {
    /// Event name.
    pub name: String,
    /// Handler action.
    pub action: String,
    /// Parameters passed.
    pub params: Vec<String>,
}

/// Accessibility specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessibilitySpec {
    /// Accessibility label.
    pub label: Option<String>,
    /// Accessibility hint.
    pub hint: Option<String>,
    /// Role.
    pub role: Option<String>,
    /// Whether element is interactive.
    pub interactive: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_model() {
        let mut model = StateModel::new();
        model.add_variable("count", StateVariable {
            var_type: StateType::Int,
            default_value: Some(serde_json::json!(0)),
            observable: true,
            validation: None,
            doc: Some("Counter value".to_string()),
        });

        assert!(model.variables.contains_key("count"));
    }

    #[test]
    fn test_color_token() {
        let color = ColorToken::from_hex("#ff5500");
        assert_eq!(color.hex, "#ff5500");
        assert_eq!(color.rgb, Some([255, 85, 0]));
    }

    #[test]
    fn test_state_type_conversion() {
        assert_eq!(StateType::String.to_swift(), "String");
        assert_eq!(StateType::Bool.to_kotlin(), "Boolean");
        assert_eq!(StateType::Int.to_typescript(), "number");
    }
}
