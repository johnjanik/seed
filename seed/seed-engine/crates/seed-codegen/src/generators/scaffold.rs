//! Scaffold generator with TODO markers.

use crate::error::Result;
use crate::model::{ComponentSpec, ComponentType, StateModel};
use super::{GeneratedFile, ScaffoldTodo, TodoPriority};

/// Options for scaffold generation.
#[derive(Debug, Clone, Default)]
pub struct ScaffoldOptions {
    /// Framework to generate for.
    pub framework: ScaffoldFramework,
    /// Include detailed comments.
    pub verbose_comments: bool,
    /// Include type stubs.
    pub include_types: bool,
    /// Include test stubs.
    pub include_tests: bool,
}

/// Target framework for scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScaffoldFramework {
    #[default]
    Generic,
    SwiftUI,
    Compose,
    React,
    Flutter,
}

/// Generates scaffold code with TODO markers.
pub struct ScaffoldGenerator {
    options: ScaffoldOptions,
    todos: Vec<ScaffoldTodo>,
    current_file: String,
    current_line: usize,
}

impl ScaffoldGenerator {
    /// Create a new scaffold generator.
    pub fn new(options: ScaffoldOptions) -> Self {
        Self {
            options,
            todos: Vec::new(),
            current_file: String::new(),
            current_line: 0,
        }
    }

    /// Generate scaffold for a component.
    pub fn generate_component(&mut self, spec: &ComponentSpec) -> Result<GeneratedFile> {
        let file_name = self.component_filename(&spec.name);
        self.current_file = file_name.clone();
        self.current_line = 1;

        let content = match self.options.framework {
            ScaffoldFramework::SwiftUI => self.swift_component_scaffold(spec),
            ScaffoldFramework::Compose => self.compose_component_scaffold(spec),
            ScaffoldFramework::React => self.react_component_scaffold(spec),
            _ => self.generic_component_scaffold(spec),
        };

        Ok(GeneratedFile {
            path: file_name,
            content,
            is_scaffold: true,
        })
    }

    /// Generate scaffold for state.
    pub fn generate_state(&mut self, model: &StateModel) -> Result<GeneratedFile> {
        let file_name = self.state_filename();
        self.current_file = file_name.clone();
        self.current_line = 1;

        let content = match self.options.framework {
            ScaffoldFramework::SwiftUI => self.swift_state_scaffold(model),
            ScaffoldFramework::Compose => self.compose_state_scaffold(model),
            ScaffoldFramework::React => self.react_state_scaffold(model),
            _ => self.generic_state_scaffold(model),
        };

        Ok(GeneratedFile {
            path: file_name,
            content,
            is_scaffold: true,
        })
    }

    /// Get all collected TODOs.
    pub fn todos(&self) -> &[ScaffoldTodo] {
        &self.todos
    }

    /// Take all collected TODOs.
    pub fn take_todos(self) -> Vec<ScaffoldTodo> {
        self.todos
    }

    /// Add a TODO marker.
    fn add_todo(&mut self, description: &str, priority: TodoPriority) {
        self.todos.push(ScaffoldTodo {
            file: self.current_file.clone(),
            line: self.current_line,
            description: description.to_string(),
            priority,
        });
    }

    /// Get component filename.
    fn component_filename(&self, name: &str) -> String {
        match self.options.framework {
            ScaffoldFramework::SwiftUI => format!("{}View.swift", to_pascal_case(name)),
            ScaffoldFramework::Compose => format!("{}Screen.kt", to_pascal_case(name)),
            ScaffoldFramework::React => format!("{}.tsx", to_pascal_case(name)),
            _ => format!("{}.txt", to_snake_case(name)),
        }
    }

    /// Get state filename.
    fn state_filename(&self) -> String {
        match self.options.framework {
            ScaffoldFramework::SwiftUI => "AppState.swift".to_string(),
            ScaffoldFramework::Compose => "AppState.kt".to_string(),
            ScaffoldFramework::React => "useAppState.ts".to_string(),
            _ => "state.txt".to_string(),
        }
    }

    /// Generate SwiftUI component scaffold.
    fn swift_component_scaffold(&mut self, spec: &ComponentSpec) -> String {
        let name = to_pascal_case(&spec.name);
        let mut lines = Vec::new();

        lines.push("import SwiftUI".to_string());
        lines.push(String::new());

        // Props struct
        if !spec.props.is_empty() {
            self.current_line = lines.len() + 1;
            self.add_todo(
                &format!("Define props for {} component", name),
                TodoPriority::High,
            );
            lines.push(format!("// TODO: Define props for {} component", name));
            lines.push(format!("struct {}Props {{", name));
            for prop in &spec.props {
                let swift_type = prop.prop_type.to_swift();
                let optional = if prop.required { "" } else { "?" };
                lines.push(format!(
                    "    var {}: {}{}",
                    to_camel_case(&prop.name),
                    swift_type,
                    optional
                ));
            }
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // View struct
        self.current_line = lines.len() + 1;
        lines.push(format!("struct {}View: View {{", name));

        // State
        if spec.state.is_some() {
            self.current_line = lines.len() + 1;
            self.add_todo("Implement state management", TodoPriority::High);
            lines.push("    // TODO: Implement state management".to_string());
            lines.push("    @StateObject private var viewModel = ViewModel()".to_string());
        }

        // Props
        if !spec.props.is_empty() {
            lines.push(format!("    let props: {}Props", name));
        }

        lines.push(String::new());

        // Body
        lines.push("    var body: some View {".to_string());
        self.current_line = lines.len() + 1;
        self.add_todo("Implement view body", TodoPriority::High);

        let view_code = self.swift_view_for_type(spec.component_type);
        lines.push(format!("        // TODO: Implement view body"));
        lines.push(format!("        {}", view_code));
        lines.push("    }".to_string());

        // Event handlers
        for event in &spec.events {
            lines.push(String::new());
            self.current_line = lines.len() + 1;
            self.add_todo(
                &format!("Implement {} handler", event.name),
                TodoPriority::Medium,
            );
            lines.push(format!(
                "    // TODO: Implement {} handler",
                event.name
            ));
            lines.push(format!(
                "    private func handle{}() {{",
                to_pascal_case(&event.name)
            ));
            lines.push(format!("        // Action: {}", event.action));
            lines.push("    }".to_string());
        }

        lines.push("}".to_string());

        // Preview
        lines.push(String::new());
        lines.push("#if DEBUG".to_string());
        lines.push(format!("struct {}View_Previews: PreviewProvider {{", name));
        lines.push("    static var previews: some View {".to_string());
        if spec.props.is_empty() {
            lines.push(format!("        {}View()", name));
        } else {
            self.current_line = lines.len() + 1;
            self.add_todo("Provide preview props", TodoPriority::Low);
            lines.push("        // TODO: Provide preview props".to_string());
            lines.push(format!(
                "        {}View(props: {}Props())",
                name, name
            ));
        }
        lines.push("    }".to_string());
        lines.push("}".to_string());
        lines.push("#endif".to_string());

        lines.join("\n")
    }

    /// Get SwiftUI view code for component type.
    fn swift_view_for_type(&self, comp_type: ComponentType) -> &'static str {
        match comp_type {
            ComponentType::View => "EmptyView()",
            ComponentType::Stack => "VStack { }",
            ComponentType::Text => "Text(\"\")",
            ComponentType::Image => "Image(systemName: \"photo\")",
            ComponentType::Button => "Button(\"Action\") { }",
            ComponentType::Input => "TextField(\"\", text: .constant(\"\"))",
            ComponentType::List => "List { }",
            ComponentType::Grid => "LazyVGrid(columns: []) { }",
            ComponentType::ScrollView => "ScrollView { }",
            ComponentType::Navigation => "NavigationStack { }",
            ComponentType::Modal => "EmptyView().sheet(isPresented: .constant(false)) { }",
            ComponentType::Custom => "EmptyView()",
        }
    }

    /// Generate Compose component scaffold.
    fn compose_component_scaffold(&mut self, spec: &ComponentSpec) -> String {
        let name = to_pascal_case(&spec.name);
        let mut lines = Vec::new();

        lines.push("package com.example.app.ui".to_string());
        lines.push(String::new());
        lines.push("import androidx.compose.foundation.layout.*".to_string());
        lines.push("import androidx.compose.material3.*".to_string());
        lines.push("import androidx.compose.runtime.*".to_string());
        lines.push("import androidx.compose.ui.Modifier".to_string());
        lines.push("import androidx.compose.ui.tooling.preview.Preview".to_string());
        lines.push(String::new());

        // Composable function
        self.current_line = lines.len() + 1;
        self.add_todo(
            &format!("Implement {} composable", name),
            TodoPriority::High,
        );
        lines.push(format!("// TODO: Implement {} composable", name));
        lines.push("@Composable".to_string());

        // Function signature with props
        if spec.props.is_empty() {
            lines.push(format!("fun {}Screen(", name));
        } else {
            lines.push(format!("fun {}Screen(", name));
            for (i, prop) in spec.props.iter().enumerate() {
                let kotlin_type = prop.prop_type.to_kotlin();
                let nullable = if prop.required { "" } else { "?" };
                let comma = if i < spec.props.len() - 1 { "," } else { "" };
                lines.push(format!(
                    "    {}: {}{}{}",
                    to_camel_case(&prop.name),
                    kotlin_type,
                    nullable,
                    comma
                ));
            }
        }
        lines.push("    modifier: Modifier = Modifier".to_string());
        lines.push(") {".to_string());

        // State
        if spec.state.is_some() {
            self.current_line = lines.len() + 1;
            self.add_todo("Implement state management", TodoPriority::High);
            lines.push("    // TODO: Implement state management".to_string());
            lines.push("    var state by remember { mutableStateOf(\"\") }".to_string());
        }

        // Body
        lines.push(String::new());
        let compose_code = self.compose_view_for_type(spec.component_type);
        lines.push(format!("    {}", compose_code));

        lines.push("}".to_string());

        // Preview
        lines.push(String::new());
        lines.push("@Preview".to_string());
        lines.push("@Composable".to_string());
        lines.push(format!("fun {}ScreenPreview() {{", name));
        self.current_line = lines.len() + 1;
        self.add_todo("Add preview with sample data", TodoPriority::Low);
        lines.push("    // TODO: Add preview with sample data".to_string());
        lines.push(format!("    {}Screen()", name));
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Get Compose view code for component type.
    fn compose_view_for_type(&self, comp_type: ComponentType) -> &'static str {
        match comp_type {
            ComponentType::View => "Box(modifier = modifier) { }",
            ComponentType::Stack => "Column(modifier = modifier) { }",
            ComponentType::Text => "Text(text = \"\")",
            ComponentType::Image => "Image(painter = painterResource(id = 0), contentDescription = null)",
            ComponentType::Button => "Button(onClick = { }) { Text(\"\") }",
            ComponentType::Input => "TextField(value = \"\", onValueChange = { })",
            ComponentType::List => "LazyColumn(modifier = modifier) { }",
            ComponentType::Grid => "LazyVerticalGrid(columns = GridCells.Fixed(2)) { }",
            ComponentType::ScrollView => "Column(modifier = modifier.verticalScroll(rememberScrollState())) { }",
            ComponentType::Navigation => "// Navigation composable",
            ComponentType::Modal => "// Dialog composable",
            ComponentType::Custom => "Box(modifier = modifier) { }",
        }
    }

    /// Generate React component scaffold.
    fn react_component_scaffold(&mut self, spec: &ComponentSpec) -> String {
        let name = to_pascal_case(&spec.name);
        let mut lines = Vec::new();

        lines.push("import React from 'react';".to_string());

        if spec.state.is_some() {
            // useState already imported via React
        }

        lines.push(String::new());

        // Props interface
        if !spec.props.is_empty() {
            self.current_line = lines.len() + 1;
            self.add_todo(
                &format!("Define props interface for {}", name),
                TodoPriority::High,
            );
            lines.push(format!("// TODO: Define props interface for {}", name));
            lines.push(format!("interface {}Props {{", name));
            for prop in &spec.props {
                let ts_type = prop.prop_type.to_typescript();
                let optional = if prop.required { "" } else { "?" };
                lines.push(format!(
                    "  {}{}: {};",
                    to_camel_case(&prop.name),
                    optional,
                    ts_type
                ));
            }
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // Component
        self.current_line = lines.len() + 1;
        self.add_todo(&format!("Implement {} component", name), TodoPriority::High);
        lines.push(format!("// TODO: Implement {} component", name));

        if spec.props.is_empty() {
            lines.push(format!("export function {}() {{", name));
        } else {
            lines.push(format!(
                "export function {}({{ {} }}: {}Props) {{",
                name,
                spec.props
                    .iter()
                    .map(|p| to_camel_case(&p.name))
                    .collect::<Vec<_>>()
                    .join(", "),
                name
            ));
        }

        // State hooks
        if spec.state.is_some() {
            self.current_line = lines.len() + 1;
            self.add_todo("Implement state hooks", TodoPriority::High);
            lines.push("  // TODO: Implement state hooks".to_string());
            lines.push("  const [state, setState] = React.useState(null);".to_string());
            lines.push(String::new());
        }

        // Event handlers
        for event in &spec.events {
            self.current_line = lines.len() + 1;
            self.add_todo(
                &format!("Implement {} handler", event.name),
                TodoPriority::Medium,
            );
            lines.push(format!(
                "  // TODO: Implement {} handler",
                event.name
            ));
            lines.push(format!(
                "  const handle{} = () => {{",
                to_pascal_case(&event.name)
            ));
            lines.push(format!("    // Action: {}", event.action));
            lines.push("  };".to_string());
            lines.push(String::new());
        }

        // Return JSX
        lines.push("  return (".to_string());
        let jsx = self.react_jsx_for_type(spec.component_type);
        lines.push(format!("    {}", jsx));
        lines.push("  );".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Get React JSX for component type.
    fn react_jsx_for_type(&self, comp_type: ComponentType) -> &'static str {
        match comp_type {
            ComponentType::View => "<div></div>",
            ComponentType::Stack => "<div className=\"flex flex-col\"></div>",
            ComponentType::Text => "<span></span>",
            ComponentType::Image => "<img src=\"\" alt=\"\" />",
            ComponentType::Button => "<button></button>",
            ComponentType::Input => "<input type=\"text\" />",
            ComponentType::List => "<ul></ul>",
            ComponentType::Grid => "<div className=\"grid\"></div>",
            ComponentType::ScrollView => "<div className=\"overflow-auto\"></div>",
            ComponentType::Navigation => "<nav></nav>",
            ComponentType::Modal => "<dialog></dialog>",
            ComponentType::Custom => "<div></div>",
        }
    }

    /// Generate generic component scaffold.
    fn generic_component_scaffold(&mut self, spec: &ComponentSpec) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Component: {}", spec.name));
        lines.push(format!("Type: {:?}", spec.component_type));
        lines.push(String::new());

        if !spec.props.is_empty() {
            lines.push("Props:".to_string());
            for prop in &spec.props {
                lines.push(format!(
                    "  - {}: {:?} (required: {})",
                    prop.name, prop.prop_type, prop.required
                ));
            }
            lines.push(String::new());
        }

        if !spec.events.is_empty() {
            lines.push("Events:".to_string());
            for event in &spec.events {
                lines.push(format!("  - {}: {}", event.name, event.action));
            }
            lines.push(String::new());
        }

        self.add_todo("Implement component", TodoPriority::High);
        lines.push("TODO: Implement component".to_string());

        lines.join("\n")
    }

    /// Generate SwiftUI state scaffold.
    fn swift_state_scaffold(&mut self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push("import SwiftUI".to_string());
        lines.push("import Combine".to_string());
        lines.push(String::new());

        self.current_line = lines.len() + 1;
        self.add_todo("Implement AppState", TodoPriority::High);
        lines.push("// TODO: Implement AppState".to_string());
        lines.push("class AppState: ObservableObject {".to_string());

        // Variables
        for (name, var) in &model.variables {
            let swift_type = var.var_type.to_swift();
            let default_val = var
                .default_value
                .as_ref()
                .map(|v| format!(" = {}", json_to_swift(v)))
                .unwrap_or_default();
            lines.push(format!(
                "    @Published var {}: {}{}",
                to_camel_case(name),
                swift_type,
                default_val
            ));
        }

        lines.push(String::new());

        // Actions
        for (name, action) in &model.actions {
            self.current_line = lines.len() + 1;
            self.add_todo(&format!("Implement {} action", name), TodoPriority::Medium);
            lines.push(format!("    // TODO: Implement {} action", name));

            let params = action
                .parameters
                .iter()
                .map(|p| format!("{}: {}", to_camel_case(&p.name), p.param_type.to_swift()))
                .collect::<Vec<_>>()
                .join(", ");

            lines.push(format!("    func {}({}) {{", to_camel_case(name), params));
            for mutation in &action.mutations {
                lines.push(format!(
                    "        {} = {}",
                    to_camel_case(&mutation.variable),
                    mutation.expression
                ));
            }
            lines.push("    }".to_string());
            lines.push(String::new());
        }

        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate Compose state scaffold.
    fn compose_state_scaffold(&mut self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push("package com.example.app.state".to_string());
        lines.push(String::new());
        lines.push("import androidx.compose.runtime.*".to_string());
        lines.push("import androidx.lifecycle.ViewModel".to_string());
        lines.push(String::new());

        self.current_line = lines.len() + 1;
        self.add_todo("Implement AppViewModel", TodoPriority::High);
        lines.push("// TODO: Implement AppViewModel".to_string());
        lines.push("class AppViewModel : ViewModel() {".to_string());

        // Variables
        for (name, var) in &model.variables {
            let kotlin_type = var.var_type.to_kotlin();
            let default_val = var
                .default_value
                .as_ref()
                .map(|v| format!("{}", json_to_kotlin(v)))
                .unwrap_or_else(|| "null".to_string());
            lines.push(format!(
                "    var {} by mutableStateOf<{}>({})",
                to_camel_case(name),
                kotlin_type,
                default_val
            ));
        }

        lines.push(String::new());

        // Actions
        for (name, action) in &model.actions {
            self.current_line = lines.len() + 1;
            self.add_todo(&format!("Implement {} action", name), TodoPriority::Medium);
            lines.push(format!("    // TODO: Implement {} action", name));

            let params = action
                .parameters
                .iter()
                .map(|p| format!("{}: {}", to_camel_case(&p.name), p.param_type.to_kotlin()))
                .collect::<Vec<_>>()
                .join(", ");

            lines.push(format!("    fun {}({}) {{", to_camel_case(name), params));
            for mutation in &action.mutations {
                lines.push(format!(
                    "        {} = {}",
                    to_camel_case(&mutation.variable),
                    mutation.expression
                ));
            }
            lines.push("    }".to_string());
            lines.push(String::new());
        }

        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate React state scaffold.
    fn react_state_scaffold(&mut self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push("import { useState, useCallback } from 'react';".to_string());
        lines.push(String::new());

        // State interface
        self.current_line = lines.len() + 1;
        self.add_todo("Define AppState interface", TodoPriority::High);
        lines.push("// TODO: Define AppState interface".to_string());
        lines.push("interface AppState {".to_string());
        for (name, var) in &model.variables {
            let ts_type = var.var_type.to_typescript();
            lines.push(format!("  {}: {};", to_camel_case(name), ts_type));
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Custom hook
        self.current_line = lines.len() + 1;
        self.add_todo("Implement useAppState hook", TodoPriority::High);
        lines.push("// TODO: Implement useAppState hook".to_string());
        lines.push("export function useAppState() {".to_string());

        // State initialization
        lines.push("  const [state, setState] = useState<AppState>({".to_string());
        for (name, var) in &model.variables {
            let default_val = var
                .default_value
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "null".to_string());
            lines.push(format!("    {}: {},", to_camel_case(name), default_val));
        }
        lines.push("  });".to_string());
        lines.push(String::new());

        // Actions
        for (name, action) in &model.actions {
            self.current_line = lines.len() + 1;
            self.add_todo(&format!("Implement {} action", name), TodoPriority::Medium);
            lines.push(format!("  // TODO: Implement {} action", name));

            let params = action
                .parameters
                .iter()
                .map(|p| format!("{}: {}", to_camel_case(&p.name), p.param_type.to_typescript()))
                .collect::<Vec<_>>()
                .join(", ");

            lines.push(format!(
                "  const {} = useCallback(({}) => {{",
                to_camel_case(name),
                params
            ));
            lines.push("    setState(prev => ({".to_string());
            lines.push("      ...prev,".to_string());
            for mutation in &action.mutations {
                lines.push(format!(
                    "      {}: {},",
                    to_camel_case(&mutation.variable),
                    mutation.expression
                ));
            }
            lines.push("    }));".to_string());
            lines.push("  }, []);".to_string());
            lines.push(String::new());
        }

        // Return
        lines.push("  return {".to_string());
        lines.push("    state,".to_string());
        for name in model.actions.keys() {
            lines.push(format!("    {},", to_camel_case(name)));
        }
        lines.push("  };".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate generic state scaffold.
    fn generic_state_scaffold(&mut self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push("State Model".to_string());
        lines.push("===========".to_string());
        lines.push(String::new());

        lines.push("Variables:".to_string());
        for (name, var) in &model.variables {
            lines.push(format!("  - {}: {:?}", name, var.var_type));
        }
        lines.push(String::new());

        lines.push("Actions:".to_string());
        for (name, action) in &model.actions {
            lines.push(format!("  - {}", name));
            for mutation in &action.mutations {
                lines.push(format!("    -> {} = {}", mutation.variable, mutation.expression));
            }
        }

        self.add_todo("Implement state management", TodoPriority::High);
        lines.push(String::new());
        lines.push("TODO: Implement state management".to_string());

        lines.join("\n")
    }
}

// Helper functions

fn to_pascal_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Pascal)
}

fn to_camel_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Camel)
}

fn to_snake_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Snake)
}

fn json_to_swift(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "nil".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Array(_) => "[]".to_string(),
        serde_json::Value::Object(_) => "[:]".to_string(),
    }
}

fn json_to_kotlin(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Array(_) => "emptyList()".to_string(),
        serde_json::Value::Object(_) => "emptyMap()".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ComponentType;

    #[test]
    fn test_swift_scaffold() {
        let mut gen = ScaffoldGenerator::new(ScaffoldOptions {
            framework: ScaffoldFramework::SwiftUI,
            ..Default::default()
        });

        let spec = ComponentSpec {
            name: "home".to_string(),
            component_type: ComponentType::View,
            props: vec![],
            children: vec![],
            state: None,
            styles: vec![],
            events: vec![],
            accessibility: Default::default(),
        };

        let file = gen.generate_component(&spec).unwrap();
        assert!(file.path.ends_with(".swift"));
        assert!(file.content.contains("struct HomeView: View"));
    }

    #[test]
    fn test_react_scaffold() {
        let mut gen = ScaffoldGenerator::new(ScaffoldOptions {
            framework: ScaffoldFramework::React,
            ..Default::default()
        });

        let spec = ComponentSpec {
            name: "dashboard".to_string(),
            component_type: ComponentType::View,
            props: vec![],
            children: vec![],
            state: None,
            styles: vec![],
            events: vec![],
            accessibility: Default::default(),
        };

        let file = gen.generate_component(&spec).unwrap();
        assert!(file.path.ends_with(".tsx"));
        assert!(file.content.contains("export function Dashboard()"));
    }
}
