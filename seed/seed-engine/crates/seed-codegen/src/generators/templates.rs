//! Template engine for code generation.

use crate::error::{CodegenError, Result};
use handlebars::Handlebars;
use serde::Serialize;

/// Template engine using Handlebars.
pub struct TemplateEngine<'a> {
    handlebars: Handlebars<'a>,
}

impl<'a> TemplateEngine<'a> {
    /// Create a new template engine.
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register custom helpers
        Self::register_helpers(&mut handlebars);

        Self { handlebars }
    }

    /// Register a template.
    pub fn register_template(&mut self, name: &str, template: &str) -> Result<()> {
        self.handlebars
            .register_template_string(name, template)
            .map_err(CodegenError::InvalidTemplate)?;
        Ok(())
    }

    /// Render a template.
    pub fn render<T: Serialize>(&self, name: &str, data: &T) -> Result<String> {
        self.handlebars
            .render(name, data)
            .map_err(CodegenError::TemplateError)
    }

    /// Render a template string directly.
    pub fn render_string<T: Serialize>(&self, template: &str, data: &T) -> Result<String> {
        self.handlebars
            .render_template(template, data)
            .map_err(CodegenError::TemplateError)
    }

    /// Register custom helpers.
    fn register_helpers(handlebars: &mut Handlebars) {
        // Pascal case helper
        handlebars.register_helper(
            "pascal_case",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let param = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    out.write(&to_pascal_case(param))?;
                    Ok(())
                },
            ),
        );

        // Camel case helper
        handlebars.register_helper(
            "camel_case",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let param = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    out.write(&to_camel_case(param))?;
                    Ok(())
                },
            ),
        );

        // Snake case helper
        handlebars.register_helper(
            "snake_case",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let param = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    out.write(&to_snake_case(param))?;
                    Ok(())
                },
            ),
        );

        // Kebab case helper
        handlebars.register_helper(
            "kebab_case",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let param = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    out.write(&to_kebab_case(param))?;
                    Ok(())
                },
            ),
        );

        // Indent helper
        handlebars.register_helper(
            "indent",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let content = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    let spaces = h
                        .param(1)
                        .and_then(|v| v.value().as_u64())
                        .unwrap_or(4) as usize;

                    let indent = " ".repeat(spaces);
                    let indented = content
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                line.to_string()
                            } else {
                                format!("{}{}", indent, line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    out.write(&indented)?;
                    Ok(())
                },
            ),
        );

        // Join helper
        handlebars.register_helper(
            "join",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let arr = h.param(0).and_then(|v| v.value().as_array());
                    let sep = h
                        .param(1)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or(", ");

                    if let Some(items) = arr {
                        let joined = items
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(sep);
                        out.write(&joined)?;
                    }
                    Ok(())
                },
            ),
        );

        // Upper case helper
        handlebars.register_helper(
            "upper",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let param = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    out.write(&param.to_uppercase())?;
                    Ok(())
                },
            ),
        );

        // Lower case helper
        handlebars.register_helper(
            "lower",
            Box::new(
                |h: &handlebars::Helper,
                 _r: &Handlebars,
                 _ctx: &handlebars::Context,
                 _rc: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output| {
                    let param = h
                        .param(0)
                        .and_then(|v| v.value().as_str())
                        .unwrap_or("");
                    out.write(&param.to_lowercase())?;
                    Ok(())
                },
            ),
        );
    }
}

impl<'a> Default for TemplateEngine<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert to PascalCase.
fn to_pascal_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Pascal)
}

/// Convert to camelCase.
fn to_camel_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Camel)
}

/// Convert to snake_case.
fn to_snake_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Snake)
}

/// Convert to kebab-case.
fn to_kebab_case(s: &str) -> String {
    use convert_case::{Case, Casing};
    s.to_case(Case::Kebab)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_render_simple() {
        let mut engine = TemplateEngine::new();
        engine
            .register_template("hello", "Hello, {{name}}!")
            .unwrap();

        let result = engine.render("hello", &json!({"name": "World"})).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_pascal_case_helper() {
        let engine = TemplateEngine::new();
        let result = engine
            .render_string("{{pascal_case name}}", &json!({"name": "my_component"}))
            .unwrap();
        assert_eq!(result, "MyComponent");
    }

    #[test]
    fn test_camel_case_helper() {
        let engine = TemplateEngine::new();
        let result = engine
            .render_string("{{camel_case name}}", &json!({"name": "MyComponent"}))
            .unwrap();
        assert_eq!(result, "myComponent");
    }
}
