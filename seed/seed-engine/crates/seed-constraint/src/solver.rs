//! High-level constraint solver interface.
//!
//! This module provides an interface between Seed's constraint AST and the
//! Cassowary simplex solver.

use std::collections::HashMap;

use seed_core::{
    ast::{self, ConstraintKind, Element, Expression as AstExpr, FrameElement, TextElement},
    types::{ElementId, Length},
    ConstraintError, ConstraintPriority, Document,
};
use indexmap::IndexMap;

use crate::cassowary::{self, Constraint, Expression, Relation, Solver, Strength, Variable};

/// Layout properties for an element.
#[derive(Debug, Clone, Copy)]
pub struct ElementVars {
    pub x: Variable,
    pub y: Variable,
    pub width: Variable,
    pub height: Variable,
}

impl ElementVars {
    /// Get center-x (x + width/2)
    pub fn center_x(&self, solver: &Solver) -> f64 {
        solver.get_value(self.x) + solver.get_value(self.width) / 2.0
    }

    /// Get center-y (y + height/2)
    pub fn center_y(&self, solver: &Solver) -> f64 {
        solver.get_value(self.y) + solver.get_value(self.height) / 2.0
    }

    /// Get right edge (x + width)
    pub fn right(&self, solver: &Solver) -> f64 {
        solver.get_value(self.x) + solver.get_value(self.width)
    }

    /// Get bottom edge (y + height)
    pub fn bottom(&self, solver: &Solver) -> f64 {
        solver.get_value(self.y) + solver.get_value(self.height)
    }
}

/// The solution to a constraint system.
#[derive(Debug, Clone, Default)]
pub struct Solution {
    pub variables: IndexMap<(ElementId, String), f64>,
}

impl Solution {
    /// Get a property value for an element.
    pub fn get(&self, element: ElementId, property: &str) -> Option<f64> {
        self.variables.get(&(element, property.to_string())).copied()
    }

    /// Get the bounding box for an element.
    pub fn get_bounds(&self, element: ElementId) -> Option<(f64, f64, f64, f64)> {
        let x = self.get(element, "x")?;
        let y = self.get(element, "y")?;
        let width = self.get(element, "width")?;
        let height = self.get(element, "height")?;
        Some((x, y, width, height))
    }
}

/// The constraint system - bridges Seed constraints to Cassowary.
#[derive(Debug)]
pub struct ConstraintSystem {
    solver: Solver,
    /// Map from element name to its variables
    element_vars: HashMap<String, ElementVars>,
    /// Map from element ID to its name
    element_names: HashMap<ElementId, String>,
    /// Counter for generating element IDs
    id_counter: u64,
    /// Parent stack for resolving Parent references
    parent_stack: Vec<String>,
}

impl Default for ConstraintSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstraintSystem {
    /// Create a new constraint system.
    pub fn new() -> Self {
        Self {
            solver: Solver::new(),
            element_vars: HashMap::new(),
            element_names: HashMap::new(),
            id_counter: 0,
            parent_stack: Vec::new(),
        }
    }

    /// Add constraints from a document.
    pub fn add_document(&mut self, doc: &Document) -> Result<(), ConstraintError> {
        for element in &doc.elements {
            self.add_element(element, None)?;
        }
        Ok(())
    }

    /// Add an element and its constraints.
    fn add_element(
        &mut self,
        element: &Element,
        parent: Option<&str>,
    ) -> Result<String, ConstraintError> {
        match element {
            Element::Frame(frame) => self.add_frame(frame, parent),
            Element::Text(text) => self.add_text(text, parent),
            _ => {
                // Skip unsupported element types for now
                Ok(String::new())
            }
        }
    }

    /// Add a Frame element.
    fn add_frame(
        &mut self,
        frame: &FrameElement,
        parent: Option<&str>,
    ) -> Result<String, ConstraintError> {
        // Generate name matching layout system's convention
        self.id_counter += 1;
        let name = frame
            .name
            .as_ref()
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("frame_{}", self.id_counter));

        // Create variables for layout properties
        let _vars = self.create_element_vars(&name);

        // Add parent-relative constraints if we have a parent
        if let Some(parent_name) = parent {
            self.add_parent_constraints(&name, parent_name)?;
        }

        // Process explicit constraints
        if let Some(parent_name) = parent {
            self.parent_stack.push(parent_name.to_string());
        }

        for constraint in &frame.constraints {
            self.add_constraint(&name, constraint)?;
        }

        // Also process layout-related properties as implicit constraints
        self.add_properties_as_constraints(&name, &frame.properties)?;

        // Process children
        for child in &frame.children {
            self.add_element(child, Some(&name))?;
        }

        if parent.is_some() {
            self.parent_stack.pop();
        }

        Ok(name)
    }

    /// Add a Text element.
    fn add_text(
        &mut self,
        text: &TextElement,
        parent: Option<&str>,
    ) -> Result<String, ConstraintError> {
        // Generate name matching layout system's convention
        self.id_counter += 1;
        let name = text
            .name
            .as_ref()
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("text_{}", self.id_counter));

        // Create variables for layout properties
        let _vars = self.create_element_vars(&name);

        if let Some(parent_name) = parent {
            self.add_parent_constraints(&name, parent_name)?;
        }

        // Process constraints
        if let Some(parent_name) = parent {
            self.parent_stack.push(parent_name.to_string());
        }

        for constraint in &text.constraints {
            self.add_constraint(&name, constraint)?;
        }

        if parent.is_some() {
            self.parent_stack.pop();
        }

        Ok(name)
    }

    /// Create variables for an element.
    fn create_element_vars(&mut self, name: &str) -> ElementVars {
        let vars = ElementVars {
            x: self.solver.new_variable(),
            y: self.solver.new_variable(),
            width: self.solver.new_variable(),
            height: self.solver.new_variable(),
        };

        // Use current counter value (already incremented by add_frame/add_text)
        let id = ElementId(self.id_counter);
        // Don't increment here - already done in add_frame/add_text

        self.element_vars.insert(name.to_string(), vars);
        self.element_names.insert(id, name.to_string());

        vars
    }

    /// Add constraints that position an element relative to its parent.
    fn add_parent_constraints(
        &mut self,
        _child: &str,
        _parent: &str,
    ) -> Result<(), ConstraintError> {
        // By default, children start at (0, 0) relative to parent
        // Explicit constraints will override this
        Ok(())
    }

    /// Convert layout-related properties to implicit constraints.
    fn add_properties_as_constraints(
        &mut self,
        element_name: &str,
        properties: &[ast::Property],
    ) -> Result<(), ConstraintError> {
        use seed_core::ast::{Expression as AstExpr, PropertyValue};

        for prop in properties {
            // Only process layout-related properties
            let layout_prop = match prop.name.as_str() {
                "width" | "height" | "x" | "y" | "left" | "top" => true,
                _ => false,
            };

            if !layout_prop {
                continue;
            }

            // Convert property value to expression
            let expression = match &prop.value {
                PropertyValue::Length(len) => AstExpr::Length(len.clone()),
                PropertyValue::Number(n) => AstExpr::Literal(*n),
                _ => continue, // Skip non-numeric values
            };

            // Map property name to constraint property
            let constraint_prop = match prop.name.as_str() {
                "left" => "x",
                "top" => "y",
                other => other,
            };

            // Add as equality constraint with required strength
            let vars = self.element_vars.get(element_name).ok_or_else(|| {
                ConstraintError::UnknownProperty {
                    property: element_name.to_string(),
                    span: Default::default(),
                }
            })?;

            let var = self.property_to_variable(vars, constraint_prop)?;
            let expr = self.build_expression(var, &expression)?;

            self.solver
                .add_constraint(Constraint::new(expr, Relation::Equal, Strength::REQUIRED))
                .map_err(|_| ConstraintError::Unsatisfiable {
                    constraint_desc: format!("{}.{} = {:?}", element_name, constraint_prop, prop.value),
                    span: Default::default(),
                })?;
        }

        Ok(())
    }

    /// Add a single constraint.
    fn add_constraint(
        &mut self,
        element_name: &str,
        constraint: &ast::Constraint,
    ) -> Result<(), ConstraintError> {
        let strength = self.convert_priority(constraint.priority);

        match &constraint.kind {
            ConstraintKind::Equality { property, value } => {
                self.add_equality_constraint(element_name, property, value, strength)?;
            }
            ConstraintKind::Inequality { property, op, value } => {
                self.add_inequality_constraint(element_name, property, *op, value, strength)?;
            }
            ConstraintKind::Alignment { edge, target, target_edge } => {
                self.add_alignment_constraint(
                    element_name,
                    *edge,
                    target,
                    *target_edge,
                    strength,
                )?;
            }
            ConstraintKind::Relative { relation, target, gap } => {
                self.add_relative_constraint(element_name, *relation, target, gap.as_ref(), strength)?;
            }
        }

        Ok(())
    }

    /// Convert Seed priority to Cassowary strength.
    fn convert_priority(&self, priority: Option<ConstraintPriority>) -> Strength {
        match priority.unwrap_or(ConstraintPriority::Required) {
            ConstraintPriority::Required => Strength::REQUIRED,
            ConstraintPriority::High => Strength::STRONG,
            ConstraintPriority::Medium => Strength::MEDIUM,
            ConstraintPriority::Low | ConstraintPriority::Weak => Strength::WEAK,
        }
    }

    /// Add an equality constraint (property = value).
    fn add_equality_constraint(
        &mut self,
        element_name: &str,
        property: &str,
        value: &AstExpr,
        strength: Strength,
    ) -> Result<(), ConstraintError> {
        let vars = self.element_vars.get(element_name).ok_or_else(|| {
            ConstraintError::UnknownProperty {
                property: element_name.to_string(),
                span: Default::default(),
            }
        })?;

        let var = self.property_to_variable(vars, property)?;
        let expr = self.build_expression(var, value)?;

        self.solver
            .add_constraint(Constraint::new(expr, Relation::Equal, strength))
            .map_err(|_| ConstraintError::Unsatisfiable {
                constraint_desc: format!("{}.{} = {:?}", element_name, property, value),
                span: Default::default(),
            })?;

        Ok(())
    }

    /// Add an inequality constraint.
    fn add_inequality_constraint(
        &mut self,
        element_name: &str,
        property: &str,
        op: ast::InequalityOp,
        value: &AstExpr,
        strength: Strength,
    ) -> Result<(), ConstraintError> {
        let vars = self.element_vars.get(element_name).ok_or_else(|| {
            ConstraintError::UnknownProperty {
                property: element_name.to_string(),
                span: Default::default(),
            }
        })?;

        let var = self.property_to_variable(vars, property)?;
        let expr = self.build_expression(var, value)?;

        let relation = match op {
            ast::InequalityOp::LessThan | ast::InequalityOp::LessThanOrEqual => Relation::LessOrEqual,
            ast::InequalityOp::GreaterThan | ast::InequalityOp::GreaterThanOrEqual => {
                Relation::GreaterOrEqual
            }
        };

        self.solver
            .add_constraint(Constraint::new(expr, relation, strength))
            .map_err(|_| ConstraintError::Unsatisfiable {
                constraint_desc: format!("{}.{} {:?} {:?}", element_name, property, op, value),
                span: Default::default(),
            })?;

        Ok(())
    }

    /// Add an alignment constraint (edge align target).
    fn add_alignment_constraint(
        &mut self,
        element_name: &str,
        edge: ast::Edge,
        target: &ast::ElementRef,
        target_edge: Option<ast::Edge>,
        strength: Strength,
    ) -> Result<(), ConstraintError> {
        let target_name = self.resolve_element_ref(target)?;
        let target_vars = self.element_vars.get(&target_name).cloned().ok_or_else(|| {
            ConstraintError::UnknownProperty {
                property: target_name.clone(),
                span: Default::default(),
            }
        })?;

        let element_vars = self.element_vars.get(element_name).cloned().ok_or_else(|| {
            ConstraintError::UnknownProperty {
                property: element_name.to_string(),
                span: Default::default(),
            }
        })?;

        let target_edge = target_edge.unwrap_or(edge);

        // Build expression: element.edge = target.target_edge
        let expr = self.build_alignment_expression(&element_vars, edge, &target_vars, target_edge)?;

        self.solver
            .add_constraint(Constraint::new(expr, Relation::Equal, strength))
            .map_err(|_| ConstraintError::Unsatisfiable {
                constraint_desc: format!("{:?} align {}", edge, target_name),
                span: Default::default(),
            })?;

        Ok(())
    }

    /// Add a relative constraint (below/above/leftOf/rightOf target).
    fn add_relative_constraint(
        &mut self,
        element_name: &str,
        relation: ast::Relation,
        target: &ast::ElementRef,
        gap: Option<&Length>,
        strength: Strength,
    ) -> Result<(), ConstraintError> {
        let target_name = self.resolve_element_ref(target)?;
        let target_vars = self.element_vars.get(&target_name).cloned().ok_or_else(|| {
            ConstraintError::UnknownProperty {
                property: target_name.clone(),
                span: Default::default(),
            }
        })?;

        let element_vars = self.element_vars.get(element_name).cloned().ok_or_else(|| {
            ConstraintError::UnknownProperty {
                property: element_name.to_string(),
                span: Default::default(),
            }
        })?;

        let gap_value = gap.and_then(|g| g.to_px(None)).unwrap_or(0.0);

        // Build the appropriate constraint based on relation
        let expr = match relation {
            ast::Relation::Below => {
                // element.y = target.y + target.height + gap
                let mut expr = Expression::from_variable(element_vars.y);
                expr.add_term(cassowary::Symbol::External(target_vars.y.0), -1.0);
                expr.add_term(cassowary::Symbol::External(target_vars.height.0), -1.0);
                expr.constant = -gap_value;
                expr
            }
            ast::Relation::Above => {
                // element.y + element.height + gap = target.y
                let mut expr = Expression::from_variable(element_vars.y);
                expr.add_term(cassowary::Symbol::External(element_vars.height.0), 1.0);
                expr.add_term(cassowary::Symbol::External(target_vars.y.0), -1.0);
                expr.constant = gap_value;
                expr
            }
            ast::Relation::RightOf => {
                // element.x = target.x + target.width + gap
                let mut expr = Expression::from_variable(element_vars.x);
                expr.add_term(cassowary::Symbol::External(target_vars.x.0), -1.0);
                expr.add_term(cassowary::Symbol::External(target_vars.width.0), -1.0);
                expr.constant = -gap_value;
                expr
            }
            ast::Relation::LeftOf => {
                // element.x + element.width + gap = target.x
                let mut expr = Expression::from_variable(element_vars.x);
                expr.add_term(cassowary::Symbol::External(element_vars.width.0), 1.0);
                expr.add_term(cassowary::Symbol::External(target_vars.x.0), -1.0);
                expr.constant = gap_value;
                expr
            }
        };

        self.solver
            .add_constraint(Constraint::new(expr, Relation::Equal, strength))
            .map_err(|_| ConstraintError::Unsatisfiable {
                constraint_desc: format!("{:?} {:?}", relation, target_name),
                span: Default::default(),
            })?;

        Ok(())
    }

    /// Resolve an element reference to a name.
    fn resolve_element_ref(&self, target: &ast::ElementRef) -> Result<String, ConstraintError> {
        match target {
            ast::ElementRef::Parent => self.parent_stack.last().cloned().ok_or_else(|| {
                ConstraintError::UnknownProperty {
                    property: "Parent".to_string(),
                    span: Default::default(),
                }
            }),
            ast::ElementRef::Named(name) => Ok(name.0.clone()),
            ast::ElementRef::Previous | ast::ElementRef::Next => {
                // TODO: Implement sibling references
                Err(ConstraintError::UnknownProperty {
                    property: "Previous/Next".to_string(),
                    span: Default::default(),
                })
            }
        }
    }

    /// Get the variable for a property.
    fn property_to_variable(
        &self,
        vars: &ElementVars,
        property: &str,
    ) -> Result<Variable, ConstraintError> {
        match property {
            "x" | "left" => Ok(vars.x),
            "y" | "top" => Ok(vars.y),
            "width" => Ok(vars.width),
            "height" => Ok(vars.height),
            _ => Err(ConstraintError::UnknownProperty {
                property: property.to_string(),
                span: Default::default(),
            }),
        }
    }

    /// Build an expression: var - value = 0 (i.e., var = value)
    fn build_expression(
        &self,
        var: Variable,
        value: &AstExpr,
    ) -> Result<Expression, ConstraintError> {
        let mut expr = Expression::from_variable(var);

        match value {
            AstExpr::Literal(n) => {
                expr.constant = -*n;
            }
            AstExpr::Length(len) => {
                let px = len.to_px(None).unwrap_or(0.0);
                expr.constant = -px;
            }
            AstExpr::PropertyRef { element, property } => {
                let target_name = self.resolve_element_ref(element)?;
                let target_vars = self.element_vars.get(&target_name).ok_or_else(|| {
                    ConstraintError::UnknownProperty {
                        property: target_name.clone(),
                        span: Default::default(),
                    }
                })?;
                let target_var = self.property_to_variable(target_vars, property)?;
                expr.add_term(cassowary::Symbol::External(target_var.0), -1.0);
            }
            AstExpr::BinaryOp { left, op, right } => {
                // For binary ops, we need to recursively evaluate
                // This is simplified - full implementation would handle complex expressions
                let left_val = self.evaluate_constant(left)?;
                let right_val = self.evaluate_constant(right)?;
                let result = match op {
                    ast::BinaryOp::Add => left_val + right_val,
                    ast::BinaryOp::Sub => left_val - right_val,
                    ast::BinaryOp::Mul => left_val * right_val,
                    ast::BinaryOp::Div => left_val / right_val,
                };
                expr.constant = -result;
            }
            AstExpr::Function { name, args } => {
                // Handle min/max functions
                let result = self.evaluate_function(name, args)?;
                expr.constant = -result;
            }
            AstExpr::TokenRef(_) => {
                // Token refs should be resolved before constraint solving
                expr.constant = 0.0;
            }
        }

        Ok(expr)
    }

    /// Evaluate a constant expression.
    fn evaluate_constant(&self, expr: &AstExpr) -> Result<f64, ConstraintError> {
        match expr {
            AstExpr::Literal(n) => Ok(*n),
            AstExpr::Length(len) => Ok(len.to_px(None).unwrap_or(0.0)),
            AstExpr::BinaryOp { left, op, right } => {
                let l = self.evaluate_constant(left)?;
                let r = self.evaluate_constant(right)?;
                Ok(match op {
                    ast::BinaryOp::Add => l + r,
                    ast::BinaryOp::Sub => l - r,
                    ast::BinaryOp::Mul => l * r,
                    ast::BinaryOp::Div => l / r,
                })
            }
            AstExpr::Function { name, args } => self.evaluate_function(name, args),
            _ => Ok(0.0), // Property refs need actual solving
        }
    }

    /// Evaluate a function (min, max).
    fn evaluate_function(&self, name: &str, args: &[AstExpr]) -> Result<f64, ConstraintError> {
        let values: Result<Vec<f64>, _> = args.iter().map(|a| self.evaluate_constant(a)).collect();
        let values = values?;

        match name {
            "min" => Ok(values.iter().copied().fold(f64::INFINITY, f64::min)),
            "max" => Ok(values.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
            _ => Ok(0.0),
        }
    }

    /// Build an alignment expression.
    fn build_alignment_expression(
        &self,
        element: &ElementVars,
        edge: ast::Edge,
        target: &ElementVars,
        target_edge: ast::Edge,
    ) -> Result<Expression, ConstraintError> {
        // element.edge = target.target_edge
        // => element.edge - target.target_edge = 0

        let mut expr = Expression::default();

        // Add element side
        match edge {
            ast::Edge::Left => {
                expr.add_term(cassowary::Symbol::External(element.x.0), 1.0);
            }
            ast::Edge::Right => {
                expr.add_term(cassowary::Symbol::External(element.x.0), 1.0);
                expr.add_term(cassowary::Symbol::External(element.width.0), 1.0);
            }
            ast::Edge::Top => {
                expr.add_term(cassowary::Symbol::External(element.y.0), 1.0);
            }
            ast::Edge::Bottom => {
                expr.add_term(cassowary::Symbol::External(element.y.0), 1.0);
                expr.add_term(cassowary::Symbol::External(element.height.0), 1.0);
            }
            ast::Edge::CenterX => {
                expr.add_term(cassowary::Symbol::External(element.x.0), 1.0);
                expr.add_term(cassowary::Symbol::External(element.width.0), 0.5);
            }
            ast::Edge::CenterY => {
                expr.add_term(cassowary::Symbol::External(element.y.0), 1.0);
                expr.add_term(cassowary::Symbol::External(element.height.0), 0.5);
            }
        }

        // Subtract target side
        match target_edge {
            ast::Edge::Left => {
                expr.add_term(cassowary::Symbol::External(target.x.0), -1.0);
            }
            ast::Edge::Right => {
                expr.add_term(cassowary::Symbol::External(target.x.0), -1.0);
                expr.add_term(cassowary::Symbol::External(target.width.0), -1.0);
            }
            ast::Edge::Top => {
                expr.add_term(cassowary::Symbol::External(target.y.0), -1.0);
            }
            ast::Edge::Bottom => {
                expr.add_term(cassowary::Symbol::External(target.y.0), -1.0);
                expr.add_term(cassowary::Symbol::External(target.height.0), -1.0);
            }
            ast::Edge::CenterX => {
                expr.add_term(cassowary::Symbol::External(target.x.0), -1.0);
                expr.add_term(cassowary::Symbol::External(target.width.0), -0.5);
            }
            ast::Edge::CenterY => {
                expr.add_term(cassowary::Symbol::External(target.y.0), -1.0);
                expr.add_term(cassowary::Symbol::External(target.height.0), -0.5);
            }
        }

        Ok(expr)
    }

    /// Solve the constraint system.
    pub fn solve(&mut self) -> Result<Solution, ConstraintError> {
        self.solver.update_variables();

        let mut solution = Solution::default();

        for (name, vars) in &self.element_vars {
            // Find the element ID for this name
            let id = self
                .element_names
                .iter()
                .find(|(_, n)| *n == name)
                .map(|(id, _)| *id)
                .unwrap_or(ElementId(0));

            solution
                .variables
                .insert((id, "x".to_string()), self.solver.get_value(vars.x));
            solution
                .variables
                .insert((id, "y".to_string()), self.solver.get_value(vars.y));
            solution
                .variables
                .insert((id, "width".to_string()), self.solver.get_value(vars.width));
            solution
                .variables
                .insert((id, "height".to_string()), self.solver.get_value(vars.height));
        }

        Ok(solution)
    }

    /// Get the element ID for a given name.
    pub fn get_element_id(&self, name: &str) -> Option<ElementId> {
        self.element_names
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(id, _)| *id)
    }

    /// Get the element name for a given ID.
    pub fn get_element_name(&self, id: ElementId) -> Option<&str> {
        self.element_names.get(&id).map(|s| s.as_str())
    }

    /// Iterate over all element IDs and names.
    pub fn elements(&self) -> impl Iterator<Item = (ElementId, &str)> {
        self.element_names.iter().map(|(id, name)| (*id, name.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seed_core::ast::*;
    use seed_core::types::Identifier;

    fn make_frame(name: &str, constraints: Vec<Constraint>) -> Element {
        Element::Frame(FrameElement {
            name: Some(Identifier(name.to_string())),
            properties: vec![],
            constraints: constraints
                .into_iter()
                .map(|c| seed_core::ast::Constraint {
                    kind: c.kind,
                    priority: None,
                    span: Span::default(),
                })
                .collect(),
            children: vec![],
            span: Span::default(),
        })
    }

    struct Constraint {
        kind: ConstraintKind,
    }

    #[test]
    fn test_simple_size_constraints() {
        let doc = Document {
            meta: None,
            tokens: None,
            elements: vec![make_frame(
                "Box",
                vec![
                    Constraint {
                        kind: ConstraintKind::Equality {
                            property: "width".to_string(),
                            value: AstExpr::Length(Length::px(100.0)),
                        },
                    },
                    Constraint {
                        kind: ConstraintKind::Equality {
                            property: "height".to_string(),
                            value: AstExpr::Length(Length::px(50.0)),
                        },
                    },
                ],
            )],
            span: Span::default(),
        };

        let mut system = ConstraintSystem::new();
        system.add_document(&doc).unwrap();
        let solution = system.solve().unwrap();

        // Find the Box element
        let box_id = *system.element_names.iter().find(|(_, n)| *n == "Box").unwrap().0;

        assert!((solution.get(box_id, "width").unwrap() - 100.0).abs() < 0.001);
        assert!((solution.get(box_id, "height").unwrap() - 50.0).abs() < 0.001);
    }
}
