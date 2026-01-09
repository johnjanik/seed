//! React code generator with hooks pattern.

use crate::error::Result;
use crate::model::{
    ComponentSpec, ComponentType, DesignSystem, InteractionGraph, NodeType,
    StateModel, StateType,
};
use super::{CodeGenerator, GeneratedFile, GeneratedProject, ProjectOptions};
use super::templates::TemplateEngine;
use convert_case::{Case, Casing};

/// React code generator.
pub struct ReactGenerator<'a> {
    engine: TemplateEngine<'a>,
    use_typescript: bool,
}

impl<'a> ReactGenerator<'a> {
    /// Create a new React generator (TypeScript by default).
    pub fn new() -> Self {
        Self {
            engine: TemplateEngine::new(),
            use_typescript: true,
        }
    }

    /// Create with JavaScript output.
    pub fn javascript() -> Self {
        Self {
            engine: TemplateEngine::new(),
            use_typescript: false,
        }
    }

    /// Get file extension.
    fn ext(&self) -> &'static str {
        if self.use_typescript {
            "tsx"
        } else {
            "jsx"
        }
    }

    /// Generate component.
    fn generate_component_code(&self, spec: &ComponentSpec) -> String {
        let name = spec.name.to_case(Case::Pascal);
        let mut lines = Vec::new();

        // Imports
        lines.push("import React from 'react';".to_string());

        if spec.state.is_some() || !spec.events.is_empty() {
            // React hooks are available via React namespace
        }

        lines.push(String::new());

        // Props interface (TypeScript)
        if self.use_typescript && !spec.props.is_empty() {
            lines.push(format!("interface {}Props {{", name));
            for prop in &spec.props {
                let ts_type = self.typescript_type(&prop.prop_type);
                let optional = if prop.required { "" } else { "?" };
                if let Some(ref doc) = prop.doc {
                    lines.push(format!("  /** {} */", doc));
                }
                lines.push(format!("  {}{}: {};", prop.name.to_case(Case::Camel), optional, ts_type));
            }
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // Component function
        let props_param = if spec.props.is_empty() {
            String::new()
        } else if self.use_typescript {
            format!(
                "{{ {} }}: {}Props",
                spec.props
                    .iter()
                    .map(|p| p.name.to_case(Case::Camel))
                    .collect::<Vec<_>>()
                    .join(", "),
                name
            )
        } else {
            format!(
                "{{ {} }}",
                spec.props
                    .iter()
                    .map(|p| p.name.to_case(Case::Camel))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        lines.push(format!("export function {}({}) {{", name, props_param));

        // Local state
        if spec.state.is_some() {
            lines.push("  // Local state".to_string());
            lines.push("  const [state, setState] = React.useState(null);".to_string());
            lines.push(String::new());
        }

        // Event handlers
        for event in &spec.events {
            let handler_name = format!("handle{}", event.name.to_case(Case::Pascal));
            lines.push(format!("  const {} = () => {{", handler_name));
            lines.push(format!("    // {}", event.action));
            lines.push("  };".to_string());
            lines.push(String::new());
        }

        // Return JSX
        lines.push("  return (".to_string());
        lines.push(self.generate_jsx(spec, 4));
        lines.push("  );".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate JSX for component.
    fn generate_jsx(&self, spec: &ComponentSpec, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let child_indent = indent + 2;

        match spec.component_type {
            ComponentType::View => {
                if spec.children.is_empty() {
                    format!("{}<div></div>", spaces)
                } else {
                    let mut lines = vec![format!("{}<div>", spaces)];
                    for child in &spec.children {
                        lines.push(self.generate_child_jsx(child, child_indent));
                    }
                    lines.push(format!("{}</div>", spaces));
                    lines.join("\n")
                }
            }
            ComponentType::Stack => {
                let mut lines = vec![format!("{}<div className=\"flex flex-col gap-2\">", spaces)];
                for child in &spec.children {
                    lines.push(self.generate_child_jsx(child, child_indent));
                }
                lines.push(format!("{}</div>", spaces));
                lines.join("\n")
            }
            ComponentType::Text => {
                format!("{}<span></span>", spaces)
            }
            ComponentType::Image => {
                format!("{}<img src=\"\" alt=\"\" />", spaces)
            }
            ComponentType::Button => {
                format!("{}<button type=\"button\"></button>", spaces)
            }
            ComponentType::Input => {
                format!(
                    "{}<input type=\"text\" className=\"border rounded px-2 py-1\" />",
                    spaces
                )
            }
            ComponentType::List => {
                format!("{}<ul className=\"list-disc\"></ul>", spaces)
            }
            ComponentType::Grid => {
                format!("{}<div className=\"grid grid-cols-2 gap-4\"></div>", spaces)
            }
            ComponentType::ScrollView => {
                let mut lines = vec![format!("{}<div className=\"overflow-auto\">", spaces)];
                for child in &spec.children {
                    lines.push(self.generate_child_jsx(child, child_indent));
                }
                lines.push(format!("{}</div>", spaces));
                lines.join("\n")
            }
            ComponentType::Navigation => {
                format!("{}<nav></nav>", spaces)
            }
            ComponentType::Modal => {
                format!("{}<dialog></dialog>", spaces)
            }
            ComponentType::Custom => {
                format!("{}<div></div>", spaces)
            }
        }
    }

    /// Generate child component JSX.
    fn generate_child_jsx(&self, spec: &ComponentSpec, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let name = spec.name.to_case(Case::Pascal);
        format!("{}<{} />", spaces, name)
    }

    /// Convert state type to TypeScript type.
    fn typescript_type(&self, state_type: &StateType) -> &'static str {
        state_type.to_typescript()
    }

    /// Generate custom hook for state.
    fn generate_state_hook(&self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push("import { useState, useCallback, useMemo } from 'react';".to_string());
        lines.push(String::new());

        // State interface
        if self.use_typescript {
            lines.push("interface AppState {".to_string());
            for (name, var) in &model.variables {
                let ts_type = self.typescript_type(&var.var_type);
                if let Some(ref doc) = var.doc {
                    lines.push(format!("  /** {} */", doc));
                }
                lines.push(format!("  {}: {};", name.to_case(Case::Camel), ts_type));
            }
            lines.push("}".to_string());
            lines.push(String::new());

            // Actions interface
            lines.push("interface AppActions {".to_string());
            for (name, action) in &model.actions {
                let params: Vec<String> = action
                    .parameters
                    .iter()
                    .map(|p| {
                        format!(
                            "{}: {}",
                            p.name.to_case(Case::Camel),
                            self.typescript_type(&p.param_type)
                        )
                    })
                    .collect();
                lines.push(format!(
                    "  {}: ({}) => void;",
                    name.to_case(Case::Camel),
                    params.join(", ")
                ));
            }
            lines.push("}".to_string());
            lines.push(String::new());
        }

        // Hook function
        let return_type = if self.use_typescript {
            ": { state: AppState; actions: AppActions }"
        } else {
            ""
        };

        lines.push(format!("export function useAppState(){} {{", return_type));

        // Initial state
        let state_type = if self.use_typescript {
            "<AppState>"
        } else {
            ""
        };

        lines.push(format!("  const [state, setState] = useState{}({{", state_type));
        for (name, var) in &model.variables {
            let default_val = var
                .default_value
                .as_ref()
                .map(|v| self.json_to_js(v))
                .unwrap_or_else(|| self.default_for_type(&var.var_type).to_string());
            lines.push(format!("    {}: {},", name.to_case(Case::Camel), default_val));
        }
        lines.push("  });".to_string());
        lines.push(String::new());

        // Computed properties
        for (name, computed) in &model.computed {
            lines.push(format!(
                "  const {} = useMemo(() => {}, [{}]);",
                name.to_case(Case::Camel),
                computed.expression,
                computed
                    .dependencies
                    .iter()
                    .map(|d| format!("state.{}", d.to_case(Case::Camel)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if !model.computed.is_empty() {
            lines.push(String::new());
        }

        // Actions
        lines.push("  const actions = {".to_string());
        for (name, action) in &model.actions {
            let params: Vec<String> = action
                .parameters
                .iter()
                .map(|p| {
                    if self.use_typescript {
                        format!(
                            "{}: {}",
                            p.name.to_case(Case::Camel),
                            self.typescript_type(&p.param_type)
                        )
                    } else {
                        p.name.to_case(Case::Camel)
                    }
                })
                .collect();

            lines.push(format!(
                "    {}: useCallback(({}) => {{",
                name.to_case(Case::Camel),
                params.join(", ")
            ));
            lines.push("      setState(prev => ({".to_string());
            lines.push("        ...prev,".to_string());
            for mutation in &action.mutations {
                lines.push(format!(
                    "        {}: {},",
                    mutation.variable.to_case(Case::Camel),
                    mutation.expression
                ));
            }
            lines.push("      }));".to_string());

            // Side effects
            for effect in &action.side_effects {
                lines.push(format!("      // Side effect: {:?}", effect.effect_type));
            }

            lines.push("    }, []),".to_string());
        }
        lines.push("  };".to_string());
        lines.push(String::new());

        // Return
        lines.push("  return { state, actions };".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Convert JSON to JavaScript.
    fn json_to_js(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("'{}'", s),
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| self.json_to_js(v)).collect();
                format!("[{}]", items.join(", "))
            }
            serde_json::Value::Object(obj) => {
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, self.json_to_js(v)))
                    .collect();
                format!("{{ {} }}", items.join(", "))
            }
        }
    }

    /// Get default value for type.
    fn default_for_type(&self, state_type: &StateType) -> &'static str {
        match state_type {
            StateType::String => "''",
            StateType::Int | StateType::Float => "0",
            StateType::Bool => "false",
            StateType::Array => "[]",
            StateType::Object => "{}",
            StateType::Optional | StateType::Custom => "null",
        }
    }

    /// Generate React Router navigation.
    fn generate_router(&self, graph: &InteractionGraph) -> String {
        let mut lines = Vec::new();

        lines.push("import React from 'react';".to_string());
        lines.push(
            "import { BrowserRouter, Routes, Route, useNavigate, Link } from 'react-router-dom';"
                .to_string(),
        );
        lines.push(String::new());

        // Import components
        for node_id in graph.nodes.keys() {
            let name = node_id.to_case(Case::Pascal);
            lines.push(format!("import {{ {} }} from './pages/{}';", name, name));
        }
        lines.push(String::new());

        // Route config
        if self.use_typescript {
            lines.push("interface RouteConfig {".to_string());
            lines.push("  path: string;".to_string());
            lines.push("  element: React.ReactNode;".to_string());
            lines.push("}".to_string());
            lines.push(String::new());
        }

        lines.push("const routes = [".to_string());
        for (node_id, node) in &graph.nodes {
            let name = node_id.to_case(Case::Pascal);
            let path = if Some(node_id.clone()) == graph.entry_point {
                "/".to_string()
            } else {
                format!("/{}", node_id.to_case(Case::Kebab))
            };
            lines.push(format!(
                "  {{ path: '{}', element: <{} /> }},",
                path, name
            ));
        }
        lines.push("];".to_string());
        lines.push(String::new());

        // Navigation hook
        lines.push("export function useAppNavigation() {".to_string());
        lines.push("  const navigate = useNavigate();".to_string());
        lines.push(String::new());
        lines.push("  return {".to_string());

        for (node_id, _) in &graph.nodes {
            let fn_name = format!("goTo{}", node_id.to_case(Case::Pascal));
            let path = if Some(node_id.clone()) == graph.entry_point {
                "/".to_string()
            } else {
                format!("/{}", node_id.to_case(Case::Kebab))
            };
            lines.push(format!("    {}: () => navigate('{}'),", fn_name, path));
        }

        lines.push("    goBack: () => navigate(-1),".to_string());
        lines.push("  };".to_string());
        lines.push("}".to_string());
        lines.push(String::new());

        // App Router component
        lines.push("export function AppRouter() {".to_string());
        lines.push("  return (".to_string());
        lines.push("    <BrowserRouter>".to_string());
        lines.push("      <Routes>".to_string());
        lines.push("        {routes.map((route, index) => (".to_string());
        lines.push("          <Route key={index} path={route.path} element={route.element} />"
            .to_string());
        lines.push("        ))}".to_string());
        lines.push("      </Routes>".to_string());
        lines.push("    </BrowserRouter>".to_string());
        lines.push("  );".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate CSS-in-JS theme.
    fn generate_theme(&self, ds: &DesignSystem) -> String {
        let mut lines = Vec::new();

        if self.use_typescript {
            lines.push("export interface Theme {".to_string());
            lines.push("  colors: Record<string, string>;".to_string());
            lines.push("  typography: Record<string, { fontSize: string; fontWeight: number; lineHeight?: string }>;".to_string());
            lines.push("  spacing: Record<string, string>;".to_string());
            lines.push("}".to_string());
            lines.push(String::new());
        }

        lines.push("export const theme = {".to_string());

        // Colors
        lines.push("  colors: {".to_string());
        for (name, color) in &ds.colors {
            let token_name = name.to_case(Case::Camel);
            lines.push(format!("    {}: '{}',", token_name, color.hex));
        }
        lines.push("  },".to_string());
        lines.push(String::new());

        // Typography
        lines.push("  typography: {".to_string());
        for (name, typo) in &ds.typography {
            let token_name = name.to_case(Case::Camel);
            lines.push(format!("    {}: {{", token_name));
            lines.push(format!("      fontSize: '{}px',", typo.size));
            lines.push(format!("      fontWeight: {},", typo.weight.to_numeric()));
            if let Some(lh) = typo.line_height {
                lines.push(format!("      lineHeight: '{}px',", lh));
            }
            lines.push("    },".to_string());
        }
        lines.push("  },".to_string());
        lines.push(String::new());

        // Spacing
        lines.push("  spacing: {".to_string());
        for (name, value) in &ds.spacing {
            let token_name = name.to_case(Case::Camel);
            lines.push(format!("    {}: '{}px',", token_name, value));
        }
        lines.push("  },".to_string());

        // Radii
        if !ds.radii.is_empty() {
            lines.push(String::new());
            lines.push("  radii: {".to_string());
            for (name, value) in &ds.radii {
                let token_name = name.to_case(Case::Camel);
                lines.push(format!("    {}: '{}px',", token_name, value));
            }
            lines.push("  },".to_string());
        }

        lines.push("};".to_string());
        lines.push(String::new());

        // CSS variables export
        lines.push("export function getCSSVariables() {".to_string());
        lines.push("  return {".to_string());
        for (name, color) in &ds.colors {
            let var_name = name.to_case(Case::Kebab);
            lines.push(format!("    '--color-{}': theme.colors.{},", var_name, name.to_case(Case::Camel)));
        }
        for (name, _) in &ds.spacing {
            let var_name = name.to_case(Case::Kebab);
            lines.push(format!(
                "    '--spacing-{}': theme.spacing.{},",
                var_name,
                name.to_case(Case::Camel)
            ));
        }
        lines.push("  };".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }
}

impl<'a> Default for ReactGenerator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> CodeGenerator for ReactGenerator<'a> {
    fn framework_name(&self) -> &'static str {
        "React"
    }

    fn generate_component(&self, spec: &ComponentSpec) -> Result<String> {
        Ok(self.generate_component_code(spec))
    }

    fn generate_state(&self, model: &StateModel) -> Result<String> {
        Ok(self.generate_state_hook(model))
    }

    fn generate_navigation(&self, graph: &InteractionGraph) -> Result<String> {
        Ok(self.generate_router(graph))
    }

    fn generate_design_system(&self, ds: &DesignSystem) -> Result<String> {
        Ok(self.generate_theme(ds))
    }

    fn generate_project(&self, options: &ProjectOptions) -> Result<GeneratedProject> {
        let mut files = Vec::new();
        let ext = self.ext();

        // Generate App component
        files.push(GeneratedFile {
            path: format!("src/App.{}", ext),
            content: self.generate_app_component(&options.name),
            is_scaffold: false,
        });

        // Generate components
        for spec in &options.components {
            let content = self.generate_component(spec)?;
            files.push(GeneratedFile {
                path: format!("src/components/{}.{}", spec.name.to_case(Case::Pascal), ext),
                content,
                is_scaffold: false,
            });
        }

        // Generate state hook
        if let Some(ref model) = options.state_model {
            let content = self.generate_state(model)?;
            let hook_ext = if self.use_typescript { "ts" } else { "js" };
            files.push(GeneratedFile {
                path: format!("src/hooks/useAppState.{}", hook_ext),
                content,
                is_scaffold: false,
            });
        }

        // Generate router
        if let Some(ref graph) = options.interaction_graph {
            let content = self.generate_navigation(graph)?;
            files.push(GeneratedFile {
                path: format!("src/router/AppRouter.{}", ext),
                content,
                is_scaffold: false,
            });
        }

        // Generate theme
        if let Some(ref ds) = options.design_system {
            let content = self.generate_design_system(ds)?;
            let theme_ext = if self.use_typescript { "ts" } else { "js" };
            files.push(GeneratedFile {
                path: format!("src/theme/theme.{}", theme_ext),
                content,
                is_scaffold: false,
            });
        }

        Ok(GeneratedProject {
            files,
            todos: vec![],
        })
    }
}

impl<'a> ReactGenerator<'a> {
    /// Generate main App component.
    fn generate_app_component(&self, name: &str) -> String {
        let app_name = name.to_case(Case::Pascal);
        format!(
            r#"import React from 'react';
import {{ AppRouter }} from './router/AppRouter';
import {{ useAppState }} from './hooks/useAppState';

export function App() {{
  const {{ state, actions }} = useAppState();

  return (
    <div className="app">
      <AppRouter />
    </div>
  );
}}

export default App;
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ComponentType;

    #[test]
    fn test_generate_component() {
        let gen = ReactGenerator::new();
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
        assert!(code.contains("export function Home()"));
        assert!(code.contains("return ("));
    }

    #[test]
    fn test_generate_state_hook() {
        let gen = ReactGenerator::new();
        let mut model = StateModel::new();
        model.add_variable(
            "count",
            crate::model::StateVariable {
                var_type: StateType::Int,
                default_value: Some(serde_json::json!(0)),
                observable: true,
                validation: None,
                doc: Some("Counter".to_string()),
            },
        );

        let code = gen.generate_state(&model).unwrap();
        assert!(code.contains("export function useAppState()"));
        assert!(code.contains("useState"));
    }

    #[test]
    fn test_generate_router() {
        let gen = ReactGenerator::new();
        let mut graph = InteractionGraph::new();
        graph.add_node(
            "home",
            crate::model::InteractionNode {
                name: "Home".to_string(),
                node_type: NodeType::Screen,
                component: None,
                local_state: vec![],
                on_enter: vec![],
                on_exit: vec![],
            },
        );

        let code = gen.generate_navigation(&graph).unwrap();
        assert!(code.contains("BrowserRouter"));
        assert!(code.contains("Routes"));
    }
}
