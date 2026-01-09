//! SwiftUI code generator.

use crate::error::Result;
use crate::model::{
    ComponentSpec, ComponentType, DesignSystem, FontWeight, InteractionGraph,
    NodeType, PropSpec, StateModel, StateType,
};
use super::{CodeGenerator, GeneratedFile, GeneratedProject, ProjectOptions};
use super::templates::TemplateEngine;
use convert_case::{Case, Casing};

/// SwiftUI code generator.
pub struct SwiftUIGenerator<'a> {
    engine: TemplateEngine<'a>,
}

impl<'a> SwiftUIGenerator<'a> {
    /// Create a new SwiftUI generator.
    pub fn new() -> Self {
        let mut engine = TemplateEngine::new();
        Self::register_templates(&mut engine);
        Self { engine }
    }

    /// Register SwiftUI templates.
    fn register_templates(engine: &mut TemplateEngine) {
        // Component template
        let _ = engine.register_template(
            "swiftui_component",
            r#"import SwiftUI

{{#if props}}
struct {{pascal_case name}}Props {
{{#each props}}
    var {{camel_case name}}: {{swift_type type}}{{#unless required}}?{{/unless}}
{{/each}}
}
{{/if}}

struct {{pascal_case name}}View: View {
{{#if has_state}}
    @StateObject private var viewModel = {{pascal_case name}}ViewModel()
{{/if}}
{{#if props}}
    let props: {{pascal_case name}}Props
{{/if}}

    var body: some View {
        {{body}}
    }
}

#if DEBUG
struct {{pascal_case name}}View_Previews: PreviewProvider {
    static var previews: some View {
        {{pascal_case name}}View()
    }
}
#endif
"#,
        );

        // State template
        let _ = engine.register_template(
            "swiftui_state",
            r#"import SwiftUI
import Combine

class {{name}}State: ObservableObject {
{{#each variables}}
    @Published var {{camel_case name}}: {{swift_type type}}{{#if default}} = {{default}}{{/if}}
{{/each}}

{{#each actions}}
    func {{camel_case name}}({{params}}) {
{{#each mutations}}
        {{variable}} = {{expression}}
{{/each}}
    }

{{/each}}
}
"#,
        );
    }

    /// Generate component code.
    fn generate_view(&self, spec: &ComponentSpec) -> String {
        let name = spec.name.to_case(Case::Pascal);
        let mut lines = Vec::new();

        lines.push("import SwiftUI".to_string());
        lines.push(String::new());

        // View struct
        lines.push(format!("struct {}View: View {{", name));

        // Environment objects
        if spec.state.is_some() {
            lines.push("    @EnvironmentObject var appState: AppState".to_string());
        }

        // Props
        for prop in &spec.props {
            let swift_type = self.swift_type(&prop.prop_type, !prop.required);
            if let Some(ref default) = prop.default {
                lines.push(format!(
                    "    var {}: {} = {}",
                    prop.name.to_case(Case::Camel),
                    swift_type,
                    self.json_to_swift(default)
                ));
            } else {
                lines.push(format!(
                    "    var {}: {}",
                    prop.name.to_case(Case::Camel),
                    swift_type
                ));
            }
        }

        lines.push(String::new());

        // Body
        lines.push("    var body: some View {".to_string());
        lines.push(self.generate_view_body(spec, 8));
        lines.push("    }".to_string());

        // Event handlers
        for event in &spec.events {
            lines.push(String::new());
            lines.push(format!(
                "    private func on{}() {{",
                event.name.to_case(Case::Pascal)
            ));
            lines.push(format!("        // {}", event.action));
            lines.push("    }".to_string());
        }

        lines.push("}".to_string());

        // Preview
        lines.push(String::new());
        lines.push("#if DEBUG".to_string());
        lines.push(format!("struct {}View_Previews: PreviewProvider {{", name));
        lines.push("    static var previews: some View {".to_string());
        lines.push(format!("        {}View()", name));
        lines.push("    }".to_string());
        lines.push("}".to_string());
        lines.push("#endif".to_string());

        lines.join("\n")
    }

    /// Generate view body based on component type.
    fn generate_view_body(&self, spec: &ComponentSpec, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let child_indent = indent + 4;

        match spec.component_type {
            ComponentType::View => {
                if spec.children.is_empty() {
                    format!("{}EmptyView()", spaces)
                } else {
                    let mut lines = vec![format!("{}Group {{", spaces)];
                    for child in &spec.children {
                        lines.push(self.generate_child_view(child, child_indent));
                    }
                    lines.push(format!("{}}}", spaces));
                    lines.join("\n")
                }
            }
            ComponentType::Stack => {
                let mut lines = vec![format!("{}VStack {{", spaces)];
                for child in &spec.children {
                    lines.push(self.generate_child_view(child, child_indent));
                }
                lines.push(format!("{}}}", spaces));
                lines.join("\n")
            }
            ComponentType::Text => {
                format!("{}Text(\"\")", spaces)
            }
            ComponentType::Image => {
                format!("{}Image(systemName: \"photo\")", spaces)
            }
            ComponentType::Button => {
                format!(
                    "{}Button(action: {{}}) {{\n{}    Text(\"Button\")\n{}}}",
                    spaces, spaces, spaces
                )
            }
            ComponentType::Input => {
                format!("{}TextField(\"\", text: .constant(\"\"))", spaces)
            }
            ComponentType::List => {
                format!(
                    "{}List {{\n{}    // List content\n{}}}",
                    spaces, spaces, spaces
                )
            }
            ComponentType::Grid => {
                format!(
                    "{}LazyVGrid(columns: [GridItem(.flexible())]) {{\n{}    // Grid content\n{}}}",
                    spaces, spaces, spaces
                )
            }
            ComponentType::ScrollView => {
                let mut lines = vec![format!("{}ScrollView {{", spaces)];
                for child in &spec.children {
                    lines.push(self.generate_child_view(child, child_indent));
                }
                lines.push(format!("{}}}", spaces));
                lines.join("\n")
            }
            ComponentType::Navigation => {
                format!(
                    "{}NavigationStack {{\n{}    // Navigation content\n{}}}",
                    spaces, spaces, spaces
                )
            }
            ComponentType::Modal => {
                format!(
                    "{}EmptyView()\n{}    .sheet(isPresented: .constant(false)) {{\n{}        // Modal content\n{}    }}",
                    spaces, spaces, spaces, spaces
                )
            }
            ComponentType::Custom => {
                format!("{}EmptyView()", spaces)
            }
        }
    }

    /// Generate child view.
    fn generate_child_view(&self, spec: &ComponentSpec, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let name = spec.name.to_case(Case::Pascal);
        format!("{}{}View()", spaces, name)
    }

    /// Convert state type to Swift type.
    fn swift_type(&self, state_type: &StateType, optional: bool) -> String {
        let base = match state_type {
            StateType::String => "String",
            StateType::Int => "Int",
            StateType::Float => "Double",
            StateType::Bool => "Bool",
            StateType::Array => "[Any]",
            StateType::Object => "[String: Any]",
            StateType::Optional => "Any?",
            StateType::Custom => "Any",
        };

        if optional && !matches!(state_type, StateType::Optional) {
            format!("{}?", base)
        } else {
            base.to_string()
        }
    }

    /// Convert JSON value to Swift literal.
    fn json_to_swift(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "nil".to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("\"{}\"", s),
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| self.json_to_swift(v)).collect();
                format!("[{}]", items.join(", "))
            }
            serde_json::Value::Object(obj) => {
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, self.json_to_swift(v)))
                    .collect();
                format!("[{}]", items.join(", "))
            }
        }
    }

    /// Generate state class.
    fn generate_state_class(&self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push("import SwiftUI".to_string());
        lines.push("import Combine".to_string());
        lines.push(String::new());

        lines.push("class AppState: ObservableObject {".to_string());

        // Variables
        for (name, var) in &model.variables {
            let swift_type = self.swift_type(&var.var_type, false);
            let default_val = var
                .default_value
                .as_ref()
                .map(|v| format!(" = {}", self.json_to_swift(v)))
                .unwrap_or_default();

            if let Some(ref doc) = var.doc {
                lines.push(format!("    /// {}", doc));
            }
            lines.push(format!(
                "    @Published var {}: {}{}",
                name.to_case(Case::Camel),
                swift_type,
                default_val
            ));
        }

        lines.push(String::new());

        // Computed properties
        for (name, computed) in &model.computed {
            let swift_type = self.swift_type(&computed.return_type, false);
            if let Some(ref doc) = computed.doc {
                lines.push(format!("    /// {}", doc));
            }
            lines.push(format!("    var {}: {} {{", name.to_case(Case::Camel), swift_type));
            lines.push(format!("        {}", computed.expression));
            lines.push("    }".to_string());
            lines.push(String::new());
        }

        // Actions
        for (name, action) in &model.actions {
            let params: Vec<String> = action
                .parameters
                .iter()
                .map(|p| {
                    let swift_type = self.swift_type(&p.param_type, p.optional);
                    format!("{}: {}", p.name.to_case(Case::Camel), swift_type)
                })
                .collect();

            if let Some(ref doc) = action.doc {
                lines.push(format!("    /// {}", doc));
            }
            lines.push(format!("    func {}({}) {{", name.to_case(Case::Camel), params.join(", ")));

            for mutation in &action.mutations {
                lines.push(format!(
                    "        {} = {}",
                    mutation.variable.to_case(Case::Camel),
                    mutation.expression
                ));
            }

            // Side effects
            for effect in &action.side_effects {
                lines.push(format!("        // Side effect: {:?}", effect.effect_type));
            }

            lines.push("    }".to_string());
            lines.push(String::new());
        }

        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate navigation coordinator.
    fn generate_navigation_code(&self, graph: &InteractionGraph) -> String {
        let mut lines = Vec::new();

        lines.push("import SwiftUI".to_string());
        lines.push(String::new());

        // Route enum
        lines.push("enum Route: Hashable {".to_string());
        for node_id in graph.nodes.keys() {
            lines.push(format!("    case {}", node_id.to_case(Case::Camel)));
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Router class
        lines.push("class Router: ObservableObject {".to_string());
        lines.push("    @Published var path = NavigationPath()".to_string());
        lines.push(String::new());

        // Entry point
        if let Some(ref entry) = graph.entry_point {
            lines.push(format!(
                "    var initialRoute: Route {{ .{} }}",
                entry.to_case(Case::Camel)
            ));
        }
        lines.push(String::new());

        // Navigation methods
        lines.push("    func navigate(to route: Route) {".to_string());
        lines.push("        path.append(route)".to_string());
        lines.push("    }".to_string());
        lines.push(String::new());

        lines.push("    func goBack() {".to_string());
        lines.push("        if !path.isEmpty {".to_string());
        lines.push("            path.removeLast()".to_string());
        lines.push("        }".to_string());
        lines.push("    }".to_string());
        lines.push(String::new());

        lines.push("    func popToRoot() {".to_string());
        lines.push("        path = NavigationPath()".to_string());
        lines.push("    }".to_string());

        lines.push("}".to_string());
        lines.push(String::new());

        // ContentView with NavigationStack
        lines.push("struct ContentView: View {".to_string());
        lines.push("    @StateObject private var router = Router()".to_string());
        lines.push(String::new());
        lines.push("    var body: some View {".to_string());
        lines.push("        NavigationStack(path: $router.path) {".to_string());
        lines.push("            routeView(for: router.initialRoute)".to_string());
        lines.push("                .navigationDestination(for: Route.self) { route in".to_string());
        lines.push("                    routeView(for: route)".to_string());
        lines.push("                }".to_string());
        lines.push("        }".to_string());
        lines.push("        .environmentObject(router)".to_string());
        lines.push("    }".to_string());
        lines.push(String::new());

        // Route view builder
        lines.push("    @ViewBuilder".to_string());
        lines.push("    func routeView(for route: Route) -> some View {".to_string());
        lines.push("        switch route {".to_string());
        for (node_id, node) in &graph.nodes {
            let view_name = node_id.to_case(Case::Pascal);
            lines.push(format!("        case .{}:", node_id.to_case(Case::Camel)));
            match node.node_type {
                NodeType::Screen => {
                    lines.push(format!("            {}View()", view_name));
                }
                NodeType::Modal | NodeType::Sheet => {
                    lines.push(format!("            {}View() // Present as sheet", view_name));
                }
                _ => {
                    lines.push(format!("            {}View()", view_name));
                }
            }
        }
        lines.push("        }".to_string());
        lines.push("    }".to_string());

        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate design system.
    fn generate_design_tokens(&self, ds: &DesignSystem) -> String {
        let mut lines = Vec::new();

        lines.push("import SwiftUI".to_string());
        lines.push(String::new());

        // Colors
        lines.push("extension Color {".to_string());
        for (name, color) in &ds.colors {
            let token_name = name.to_case(Case::Camel);
            if let Some([r, g, b]) = color.rgb {
                lines.push(format!(
                    "    static let {} = Color(red: {:.3}, green: {:.3}, blue: {:.3})",
                    token_name,
                    r as f64 / 255.0,
                    g as f64 / 255.0,
                    b as f64 / 255.0
                ));
            } else {
                lines.push(format!(
                    "    static let {} = Color(hex: \"{}\")",
                    token_name, color.hex
                ));
            }
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Typography
        lines.push("enum Typography {".to_string());
        for (name, typo) in &ds.typography {
            let token_name = name.to_case(Case::Camel);
            lines.push(format!(
                "    static let {} = Font.system(size: {}, weight: {})",
                token_name, typo.size, self.swift_font_weight(&typo.weight)
            ));
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Spacing
        lines.push("enum Spacing {".to_string());
        for (name, value) in &ds.spacing {
            let token_name = name.to_case(Case::Camel);
            lines.push(format!("    static let {}: CGFloat = {}", token_name, value));
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Radii
        if !ds.radii.is_empty() {
            lines.push("enum CornerRadius {".to_string());
            for (name, value) in &ds.radii {
                let token_name = name.to_case(Case::Camel);
                lines.push(format!("    static let {}: CGFloat = {}", token_name, value));
            }
            lines.push("}".to_string());
        }

        lines.join("\n")
    }

    /// Convert font weight to Swift.
    fn swift_font_weight(&self, weight: &FontWeight) -> &'static str {
        weight.to_swift()
    }
}

impl<'a> Default for SwiftUIGenerator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> CodeGenerator for SwiftUIGenerator<'a> {
    fn framework_name(&self) -> &'static str {
        "SwiftUI"
    }

    fn generate_component(&self, spec: &ComponentSpec) -> Result<String> {
        Ok(self.generate_view(spec))
    }

    fn generate_state(&self, model: &StateModel) -> Result<String> {
        Ok(self.generate_state_class(model))
    }

    fn generate_navigation(&self, graph: &InteractionGraph) -> Result<String> {
        Ok(self.generate_navigation_code(graph))
    }

    fn generate_design_system(&self, ds: &DesignSystem) -> Result<String> {
        Ok(self.generate_design_tokens(ds))
    }

    fn generate_project(&self, options: &ProjectOptions) -> Result<GeneratedProject> {
        let mut files = Vec::new();
        let mut todos = Vec::new();

        // Generate App.swift
        files.push(GeneratedFile {
            path: format!("{}/{}App.swift", options.name, options.name),
            content: self.generate_app_file(&options.name),
            is_scaffold: false,
        });

        // Generate components
        for spec in &options.components {
            let content = self.generate_component(spec)?;
            files.push(GeneratedFile {
                path: format!(
                    "{}/Views/{}View.swift",
                    options.name,
                    spec.name.to_case(Case::Pascal)
                ),
                content,
                is_scaffold: false,
            });
        }

        // Generate state if provided
        if let Some(ref model) = options.state_model {
            let content = self.generate_state(model)?;
            files.push(GeneratedFile {
                path: format!("{}/State/AppState.swift", options.name),
                content,
                is_scaffold: false,
            });
        }

        // Generate navigation if provided
        if let Some(ref graph) = options.interaction_graph {
            let content = self.generate_navigation(graph)?;
            files.push(GeneratedFile {
                path: format!("{}/Navigation/Router.swift", options.name),
                content,
                is_scaffold: false,
            });
        }

        // Generate design system if provided
        if let Some(ref ds) = options.design_system {
            let content = self.generate_design_system(ds)?;
            files.push(GeneratedFile {
                path: format!("{}/Theme/DesignTokens.swift", options.name),
                content,
                is_scaffold: false,
            });
        }

        Ok(GeneratedProject { files, todos })
    }
}

impl<'a> SwiftUIGenerator<'a> {
    /// Generate main App file.
    fn generate_app_file(&self, name: &str) -> String {
        format!(
            r#"import SwiftUI

@main
struct {}App: App {{
    @StateObject private var appState = AppState()
    @StateObject private var router = Router()

    var body: some Scene {{
        WindowGroup {{
            ContentView()
                .environmentObject(appState)
                .environmentObject(router)
        }}
    }}
}}
"#,
            name.to_case(Case::Pascal)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ComponentType;

    #[test]
    fn test_generate_simple_view() {
        let gen = SwiftUIGenerator::new();
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

        let code = gen.generate_component(&spec).unwrap();
        assert!(code.contains("struct HomeView: View"));
        assert!(code.contains("var body: some View"));
    }

    #[test]
    fn test_generate_with_props() {
        let gen = SwiftUIGenerator::new();
        let spec = ComponentSpec {
            name: "user_card".to_string(),
            component_type: ComponentType::View,
            props: vec![PropSpec {
                name: "user_name".to_string(),
                prop_type: StateType::String,
                required: true,
                default: None,
                doc: None,
            }],
            children: vec![],
            state: None,
            styles: vec![],
            events: vec![],
            accessibility: Default::default(),
        };

        let code = gen.generate_component(&spec).unwrap();
        assert!(code.contains("var userName: String"));
    }
}
