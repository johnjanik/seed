//! Kotlin Compose code generator.

use crate::error::Result;
use crate::model::{
    ComponentSpec, ComponentType, DesignSystem, FontWeight, InteractionGraph, NodeType,
    StateModel, StateType,
};
use super::{CodeGenerator, GeneratedFile, GeneratedProject, ProjectOptions};
use super::templates::TemplateEngine;
use convert_case::{Case, Casing};

/// Kotlin Compose code generator.
pub struct ComposeGenerator<'a> {
    engine: TemplateEngine<'a>,
    package_name: String,
}

impl<'a> ComposeGenerator<'a> {
    /// Create a new Compose generator.
    pub fn new() -> Self {
        Self::with_package("com.example.app")
    }

    /// Create with custom package name.
    pub fn with_package(package: &str) -> Self {
        Self {
            engine: TemplateEngine::new(),
            package_name: package.to_string(),
        }
    }

    /// Generate composable function.
    fn generate_composable(&self, spec: &ComponentSpec) -> String {
        let name = spec.name.to_case(Case::Pascal);
        let mut lines = Vec::new();

        lines.push(format!("package {}.ui.components", self.package_name));
        lines.push(String::new());

        // Imports
        lines.push("import androidx.compose.foundation.layout.*".to_string());
        lines.push("import androidx.compose.material3.*".to_string());
        lines.push("import androidx.compose.runtime.*".to_string());
        lines.push("import androidx.compose.ui.Alignment".to_string());
        lines.push("import androidx.compose.ui.Modifier".to_string());
        lines.push("import androidx.compose.ui.tooling.preview.Preview".to_string());
        lines.push("import androidx.compose.ui.unit.dp".to_string());
        lines.push(String::new());

        // Composable function
        lines.push("@Composable".to_string());

        // Function signature
        let mut params = Vec::new();
        for prop in &spec.props {
            let kotlin_type = self.kotlin_type(&prop.prop_type, !prop.required);
            if let Some(ref default) = prop.default {
                params.push(format!(
                    "{}: {} = {}",
                    prop.name.to_case(Case::Camel),
                    kotlin_type,
                    self.json_to_kotlin(default)
                ));
            } else if !prop.required {
                params.push(format!(
                    "{}: {} = null",
                    prop.name.to_case(Case::Camel),
                    kotlin_type
                ));
            } else {
                params.push(format!(
                    "{}: {}",
                    prop.name.to_case(Case::Camel),
                    kotlin_type
                ));
            }
        }
        params.push("modifier: Modifier = Modifier".to_string());

        if params.len() <= 2 {
            lines.push(format!("fun {}({}) {{", name, params.join(", ")));
        } else {
            lines.push(format!("fun {}(", name));
            for (i, param) in params.iter().enumerate() {
                let comma = if i < params.len() - 1 { "," } else { "" };
                lines.push(format!("    {}{}", param, comma));
            }
            lines.push(") {".to_string());
        }

        // Local state
        if spec.state.is_some() {
            lines.push("    // Local state".to_string());
            lines.push("    var localState by remember { mutableStateOf(\"\") }".to_string());
            lines.push(String::new());
        }

        // Body
        lines.push(self.generate_compose_body(spec, 4));

        lines.push("}".to_string());

        // Preview
        lines.push(String::new());
        lines.push("@Preview(showBackground = true)".to_string());
        lines.push("@Composable".to_string());
        lines.push(format!("fun {}Preview() {{", name));
        lines.push(format!("    {}()", name));
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate compose body.
    fn generate_compose_body(&self, spec: &ComponentSpec, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let child_indent = indent + 4;

        match spec.component_type {
            ComponentType::View => {
                if spec.children.is_empty() {
                    format!("{}Box(modifier = modifier) {{ }}", spaces)
                } else {
                    let mut lines = vec![format!("{}Box(modifier = modifier) {{", spaces)];
                    for child in &spec.children {
                        lines.push(self.generate_child_composable(child, child_indent));
                    }
                    lines.push(format!("{}}}", spaces));
                    lines.join("\n")
                }
            }
            ComponentType::Stack => {
                let mut lines = vec![format!(
                    "{}Column(\n{}    modifier = modifier,\n{}    verticalArrangement = Arrangement.spacedBy(8.dp)\n{}) {{",
                    spaces, spaces, spaces, spaces
                )];
                for child in &spec.children {
                    lines.push(self.generate_child_composable(child, child_indent));
                }
                lines.push(format!("{}}}", spaces));
                lines.join("\n")
            }
            ComponentType::Text => {
                format!("{}Text(text = \"\")", spaces)
            }
            ComponentType::Image => {
                format!(
                    "{}// Image composable\n{}// Image(painter = painterResource(id = R.drawable.placeholder), contentDescription = null)",
                    spaces, spaces
                )
            }
            ComponentType::Button => {
                format!(
                    "{}Button(\n{}    onClick = {{ }},\n{}    modifier = modifier\n{}) {{\n{}    Text(\"Button\")\n{}}}",
                    spaces, spaces, spaces, spaces, spaces, spaces
                )
            }
            ComponentType::Input => {
                format!(
                    "{}var text by remember {{ mutableStateOf(\"\") }}\n{}TextField(\n{}    value = text,\n{}    onValueChange = {{ text = it }},\n{}    modifier = modifier\n{})",
                    spaces, spaces, spaces, spaces, spaces, spaces
                )
            }
            ComponentType::List => {
                format!(
                    "{}LazyColumn(modifier = modifier) {{\n{}    // items {{ }}\n{}}}",
                    spaces, spaces, spaces
                )
            }
            ComponentType::Grid => {
                format!(
                    "{}LazyVerticalGrid(\n{}    columns = GridCells.Fixed(2),\n{}    modifier = modifier\n{}) {{\n{}    // items {{ }}\n{}}}",
                    spaces, spaces, spaces, spaces, spaces, spaces
                )
            }
            ComponentType::ScrollView => {
                let mut lines = vec![format!(
                    "{}Column(\n{}    modifier = modifier.verticalScroll(rememberScrollState())\n{}) {{",
                    spaces, spaces, spaces
                )];
                for child in &spec.children {
                    lines.push(self.generate_child_composable(child, child_indent));
                }
                lines.push(format!("{}}}", spaces));
                lines.join("\n")
            }
            ComponentType::Navigation => {
                format!("{}// Navigation host", spaces)
            }
            ComponentType::Modal => {
                format!(
                    "{}var showDialog by remember {{ mutableStateOf(false) }}\n{}if (showDialog) {{\n{}    AlertDialog(\n{}        onDismissRequest = {{ showDialog = false }},\n{}        confirmButton = {{ }},\n{}        text = {{ }}\n{}    )\n{}}}",
                    spaces, spaces, spaces, spaces, spaces, spaces, spaces, spaces
                )
            }
            ComponentType::Custom => {
                format!("{}Box(modifier = modifier) {{ }}", spaces)
            }
        }
    }

    /// Generate child composable call.
    fn generate_child_composable(&self, spec: &ComponentSpec, indent: usize) -> String {
        let spaces = " ".repeat(indent);
        let name = spec.name.to_case(Case::Pascal);
        format!("{}{}()", spaces, name)
    }

    /// Convert state type to Kotlin type.
    fn kotlin_type(&self, state_type: &StateType, nullable: bool) -> String {
        let base = match state_type {
            StateType::String => "String",
            StateType::Int => "Int",
            StateType::Float => "Double",
            StateType::Bool => "Boolean",
            StateType::Array => "List<Any>",
            StateType::Object => "Map<String, Any>",
            StateType::Optional => "Any?",
            StateType::Custom => "Any",
        };

        if nullable && !matches!(state_type, StateType::Optional) {
            format!("{}?", base)
        } else {
            base.to_string()
        }
    }

    /// Convert JSON to Kotlin literal.
    fn json_to_kotlin(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("\"{}\"", s),
            serde_json::Value::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| self.json_to_kotlin(v)).collect();
                format!("listOf({})", items.join(", "))
            }
            serde_json::Value::Object(obj) => {
                let items: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| format!("\"{}\" to {}", k, self.json_to_kotlin(v)))
                    .collect();
                format!("mapOf({})", items.join(", "))
            }
        }
    }

    /// Generate ViewModel.
    fn generate_viewmodel(&self, model: &StateModel) -> String {
        let mut lines = Vec::new();

        lines.push(format!("package {}.viewmodel", self.package_name));
        lines.push(String::new());

        lines.push("import androidx.compose.runtime.*".to_string());
        lines.push("import androidx.lifecycle.ViewModel".to_string());
        lines.push("import androidx.lifecycle.viewModelScope".to_string());
        lines.push("import kotlinx.coroutines.launch".to_string());
        lines.push(String::new());

        lines.push("class AppViewModel : ViewModel() {".to_string());

        // State variables
        for (name, var) in &model.variables {
            let kotlin_type = self.kotlin_type(&var.var_type, false);
            let default_val = var
                .default_value
                .as_ref()
                .map(|v| self.json_to_kotlin(v))
                .unwrap_or_else(|| self.default_for_type(&var.var_type));

            if let Some(ref doc) = var.doc {
                lines.push(format!("    /** {} */", doc));
            }
            lines.push(format!(
                "    var {} by mutableStateOf<{}>({})",
                name.to_case(Case::Camel),
                kotlin_type,
                default_val
            ));
            lines.push("        private set".to_string());
        }

        lines.push(String::new());

        // Computed properties
        for (name, computed) in &model.computed {
            let kotlin_type = self.kotlin_type(&computed.return_type, false);
            if let Some(ref doc) = computed.doc {
                lines.push(format!("    /** {} */", doc));
            }
            lines.push(format!(
                "    val {}: {}",
                name.to_case(Case::Camel),
                kotlin_type
            ));
            lines.push(format!("        get() = {}", computed.expression));
            lines.push(String::new());
        }

        // Actions
        for (name, action) in &model.actions {
            let params: Vec<String> = action
                .parameters
                .iter()
                .map(|p| {
                    let kotlin_type = self.kotlin_type(&p.param_type, p.optional);
                    format!("{}: {}", p.name.to_case(Case::Camel), kotlin_type)
                })
                .collect();

            if let Some(ref doc) = action.doc {
                lines.push(format!("    /** {} */", doc));
            }
            lines.push(format!(
                "    fun {}({}) {{",
                name.to_case(Case::Camel),
                params.join(", ")
            ));

            // Check for async side effects
            let has_async = action.side_effects.iter().any(|e| {
                matches!(
                    e.effect_type,
                    crate::model::SideEffectType::ApiCall | crate::model::SideEffectType::Navigate
                )
            });

            if has_async {
                lines.push("        viewModelScope.launch {".to_string());
                for mutation in &action.mutations {
                    lines.push(format!(
                        "            {} = {}",
                        mutation.variable.to_case(Case::Camel),
                        mutation.expression
                    ));
                }
                for effect in &action.side_effects {
                    lines.push(format!("            // {:?}", effect.effect_type));
                }
                lines.push("        }".to_string());
            } else {
                for mutation in &action.mutations {
                    lines.push(format!(
                        "        {} = {}",
                        mutation.variable.to_case(Case::Camel),
                        mutation.expression
                    ));
                }
            }

            lines.push("    }".to_string());
            lines.push(String::new());
        }

        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Get default value for type.
    fn default_for_type(&self, state_type: &StateType) -> String {
        match state_type {
            StateType::String => "\"\"".to_string(),
            StateType::Int => "0".to_string(),
            StateType::Float => "0.0".to_string(),
            StateType::Bool => "false".to_string(),
            StateType::Array => "emptyList()".to_string(),
            StateType::Object => "emptyMap()".to_string(),
            StateType::Optional | StateType::Custom => "null".to_string(),
        }
    }

    /// Generate navigation.
    fn generate_navigation_code(&self, graph: &InteractionGraph) -> String {
        let mut lines = Vec::new();

        lines.push(format!("package {}.navigation", self.package_name));
        lines.push(String::new());

        lines.push("import androidx.compose.runtime.Composable".to_string());
        lines.push("import androidx.navigation.NavHostController".to_string());
        lines.push("import androidx.navigation.compose.*".to_string());
        lines.push(String::new());

        // Sealed class for routes
        lines.push("sealed class Screen(val route: String) {".to_string());
        for node_id in graph.nodes.keys() {
            let class_name = node_id.to_case(Case::Pascal);
            let route = node_id.to_case(Case::Snake);
            lines.push(format!(
                "    object {} : Screen(\"{}\")",
                class_name, route
            ));
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // NavHost composable
        lines.push("@Composable".to_string());
        lines.push("fun AppNavHost(navController: NavHostController) {".to_string());

        let start_dest = graph
            .entry_point
            .as_ref()
            .map(|e| e.to_case(Case::Pascal))
            .unwrap_or_else(|| "Home".to_string());

        lines.push(format!(
            "    NavHost(navController = navController, startDestination = Screen.{}.route) {{",
            start_dest
        ));

        for (node_id, node) in &graph.nodes {
            let class_name = node_id.to_case(Case::Pascal);
            lines.push(format!("        composable(Screen.{}.route) {{", class_name));

            match node.node_type {
                NodeType::Screen | NodeType::Tab | NodeType::Step => {
                    lines.push(format!("            {}Screen(navController)", class_name));
                }
                NodeType::Modal | NodeType::Sheet | NodeType::Popover => {
                    lines.push(format!(
                        "            {}Dialog(onDismiss = {{ navController.popBackStack() }})",
                        class_name
                    ));
                }
            }

            lines.push("        }".to_string());
        }

        lines.push("    }".to_string());
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Generate theme/design system.
    fn generate_theme(&self, ds: &DesignSystem) -> String {
        let mut lines = Vec::new();

        lines.push(format!("package {}.ui.theme", self.package_name));
        lines.push(String::new());

        lines.push("import androidx.compose.ui.graphics.Color".to_string());
        lines.push("import androidx.compose.ui.text.TextStyle".to_string());
        lines.push("import androidx.compose.ui.text.font.FontWeight".to_string());
        lines.push("import androidx.compose.ui.unit.dp".to_string());
        lines.push("import androidx.compose.ui.unit.sp".to_string());
        lines.push(String::new());

        // Colors
        lines.push("object AppColors {".to_string());
        for (name, color) in &ds.colors {
            let token_name = name.to_case(Case::Pascal);
            if let Some([r, g, b]) = color.rgb {
                lines.push(format!(
                    "    val {} = Color(0xFF{:02X}{:02X}{:02X})",
                    token_name, r, g, b
                ));
            } else {
                let hex = color.hex.trim_start_matches('#');
                lines.push(format!("    val {} = Color(0xFF{})", token_name, hex));
            }
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Typography
        lines.push("object AppTypography {".to_string());
        for (name, typo) in &ds.typography {
            let token_name = name.to_case(Case::Camel);
            lines.push(format!("    val {} = TextStyle(", token_name));
            lines.push(format!("        fontSize = {}.sp,", typo.size));
            lines.push(format!(
                "        fontWeight = FontWeight.{}",
                self.kotlin_font_weight(&typo.weight)
            ));
            if let Some(lh) = typo.line_height {
                lines.push(format!("        lineHeight = {}.sp,", lh));
            }
            if let Some(ls) = typo.letter_spacing {
                lines.push(format!("        letterSpacing = {}.sp", ls));
            }
            lines.push("    )".to_string());
        }
        lines.push("}".to_string());
        lines.push(String::new());

        // Spacing
        lines.push("object AppSpacing {".to_string());
        for (name, value) in &ds.spacing {
            let token_name = name.to_case(Case::Pascal);
            lines.push(format!("    val {} = {}.dp", token_name, value));
        }
        lines.push("}".to_string());

        lines.join("\n")
    }

    /// Convert font weight to Kotlin.
    fn kotlin_font_weight(&self, weight: &FontWeight) -> &'static str {
        match weight {
            FontWeight::Thin => "Thin",
            FontWeight::ExtraLight => "ExtraLight",
            FontWeight::Light => "Light",
            FontWeight::Regular => "Normal",
            FontWeight::Medium => "Medium",
            FontWeight::SemiBold => "SemiBold",
            FontWeight::Bold => "Bold",
            FontWeight::ExtraBold => "ExtraBold",
            FontWeight::Black => "Black",
        }
    }
}

impl<'a> Default for ComposeGenerator<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> CodeGenerator for ComposeGenerator<'a> {
    fn framework_name(&self) -> &'static str {
        "Jetpack Compose"
    }

    fn generate_component(&self, spec: &ComponentSpec) -> Result<String> {
        Ok(self.generate_composable(spec))
    }

    fn generate_state(&self, model: &StateModel) -> Result<String> {
        Ok(self.generate_viewmodel(model))
    }

    fn generate_navigation(&self, graph: &InteractionGraph) -> Result<String> {
        Ok(self.generate_navigation_code(graph))
    }

    fn generate_design_system(&self, ds: &DesignSystem) -> Result<String> {
        Ok(self.generate_theme(ds))
    }

    fn generate_project(&self, options: &ProjectOptions) -> Result<GeneratedProject> {
        let mut files = Vec::new();
        let package_path = self.package_name.replace('.', "/");

        // Generate MainActivity
        files.push(GeneratedFile {
            path: format!("app/src/main/java/{}/MainActivity.kt", package_path),
            content: self.generate_main_activity(),
            is_scaffold: false,
        });

        // Generate components
        for spec in &options.components {
            let content = self.generate_component(spec)?;
            files.push(GeneratedFile {
                path: format!(
                    "app/src/main/java/{}/ui/components/{}Screen.kt",
                    package_path,
                    spec.name.to_case(Case::Pascal)
                ),
                content,
                is_scaffold: false,
            });
        }

        // Generate ViewModel
        if let Some(ref model) = options.state_model {
            let content = self.generate_state(model)?;
            files.push(GeneratedFile {
                path: format!(
                    "app/src/main/java/{}/viewmodel/AppViewModel.kt",
                    package_path
                ),
                content,
                is_scaffold: false,
            });
        }

        // Generate navigation
        if let Some(ref graph) = options.interaction_graph {
            let content = self.generate_navigation(graph)?;
            files.push(GeneratedFile {
                path: format!(
                    "app/src/main/java/{}/navigation/AppNavHost.kt",
                    package_path
                ),
                content,
                is_scaffold: false,
            });
        }

        // Generate theme
        if let Some(ref ds) = options.design_system {
            let content = self.generate_design_system(ds)?;
            files.push(GeneratedFile {
                path: format!("app/src/main/java/{}/ui/theme/Theme.kt", package_path),
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

impl<'a> ComposeGenerator<'a> {
    /// Generate MainActivity.
    fn generate_main_activity(&self) -> String {
        format!(
            r#"package {}

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.navigation.compose.rememberNavController
import {}.navigation.AppNavHost
import {}.ui.theme.AppTheme

class MainActivity : ComponentActivity() {{
    override fun onCreate(savedInstanceState: Bundle?) {{
        super.onCreate(savedInstanceState)
        setContent {{
            AppTheme {{
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {{
                    val navController = rememberNavController()
                    AppNavHost(navController = navController)
                }}
            }}
        }}
    }}
}}
"#,
            self.package_name, self.package_name, self.package_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ComponentType;

    #[test]
    fn test_generate_composable() {
        let gen = ComposeGenerator::new();
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
        assert!(code.contains("@Composable"));
        assert!(code.contains("fun Home("));
    }

    #[test]
    fn test_generate_viewmodel() {
        let gen = ComposeGenerator::new();
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
        assert!(code.contains("class AppViewModel : ViewModel()"));
        assert!(code.contains("var count by mutableStateOf"));
    }
}
