//! Cassowary constraint solver implementation.
//!
//! This is an implementation of the Cassowary linear constraint solving algorithm,
//! as described in "The Cassowary Linear Arithmetic Constraint Solving Algorithm"
//! by Greg J. Badros and Alan Borning.
//!
//! The algorithm uses a variation of the simplex method optimized for incremental
//! constraint solving with priorities (strengths).

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Variable(pub(crate) usize);

impl Variable {
    /// Create a new variable with the given ID.
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

/// Symbol types used internally in the solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Symbol {
    /// An external variable (the actual unknowns we're solving for)
    External(usize),
    /// A slack variable (for inequality constraints)
    Slack(usize),
    /// An error variable (for non-required constraints)
    Error(usize),
    /// A dummy variable (for required equality constraints)
    Dummy(usize),
}

impl Symbol {
    fn is_external(&self) -> bool {
        matches!(self, Symbol::External(_))
    }

    fn is_slack(&self) -> bool {
        matches!(self, Symbol::Slack(_))
    }

    fn is_error(&self) -> bool {
        matches!(self, Symbol::Error(_))
    }

    fn is_dummy(&self) -> bool {
        matches!(self, Symbol::Dummy(_))
    }

    fn is_pivotable(&self) -> bool {
        self.is_slack() || self.is_error()
    }
}

/// A linear expression in the form: constant + Σ(coefficient * symbol)
#[derive(Debug, Clone, Default)]
pub struct Expression {
    pub constant: f64,
    terms: HashMap<Symbol, f64>,
}

impl Expression {
    /// Create a constant expression.
    pub fn from_constant(value: f64) -> Self {
        Self {
            constant: value,
            terms: HashMap::new(),
        }
    }

    /// Create an expression from a single variable.
    pub fn from_variable(var: Variable) -> Self {
        let mut terms = HashMap::new();
        terms.insert(Symbol::External(var.0), 1.0);
        Self { constant: 0.0, terms }
    }

    /// Add a term to the expression.
    pub fn add_term(&mut self, symbol: Symbol, coefficient: f64) {
        if coefficient.abs() < EPSILON {
            self.terms.remove(&symbol);
        } else {
            let entry = self.terms.entry(symbol).or_insert(0.0);
            *entry += coefficient;
            if entry.abs() < EPSILON {
                self.terms.remove(&symbol);
            }
        }
    }

    /// Multiply the expression by a scalar.
    pub fn multiply(&mut self, scalar: f64) {
        self.constant *= scalar;
        for coeff in self.terms.values_mut() {
            *coeff *= scalar;
        }
    }

    /// Add another expression to this one.
    pub fn add_expression(&mut self, other: &Expression, multiplier: f64) {
        self.constant += other.constant * multiplier;
        for (&symbol, &coeff) in &other.terms {
            self.add_term(symbol, coeff * multiplier);
        }
    }

    /// Get the coefficient for a symbol.
    pub fn coefficient(&self, symbol: Symbol) -> f64 {
        self.terms.get(&symbol).copied().unwrap_or(0.0)
    }

    /// Check if this expression contains the given symbol.
    pub fn contains(&self, symbol: Symbol) -> bool {
        self.terms.contains_key(&symbol)
    }

    /// Substitute a symbol with an expression.
    pub fn substitute(&mut self, symbol: Symbol, expr: &Expression) {
        if let Some(coeff) = self.terms.remove(&symbol) {
            self.add_expression(expr, coeff);
        }
    }

    /// Get an iterator over the terms.
    pub fn terms(&self) -> impl Iterator<Item = (&Symbol, &f64)> {
        self.terms.iter()
    }
}

/// Tolerance for floating-point comparisons.
const EPSILON: f64 = 1e-8;

/// Near-zero check for floating point values.
fn near_zero(value: f64) -> bool {
    value.abs() < EPSILON
}

/// Constraint strength levels.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Strength(pub f64);

impl Strength {
    pub const REQUIRED: Strength = Strength(1_001_001_000.0);
    pub const STRONG: Strength = Strength(1_000_000.0);
    pub const MEDIUM: Strength = Strength(1_000.0);
    pub const WEAK: Strength = Strength(1.0);

    /// Create a custom strength.
    pub fn new(value: f64) -> Self {
        Self(value.min(Self::REQUIRED.0))
    }

    /// Check if this is a required constraint.
    pub fn is_required(&self) -> bool {
        self.0 >= Self::REQUIRED.0
    }
}

/// The relation of a constraint (equality or inequality).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    LessOrEqual,
    Equal,
    GreaterOrEqual,
}

/// A constraint to be added to the solver.
#[derive(Debug, Clone)]
pub struct Constraint {
    pub expression: Expression,
    pub relation: Relation,
    pub strength: Strength,
}

impl Constraint {
    /// Create a new constraint.
    pub fn new(expression: Expression, relation: Relation, strength: Strength) -> Self {
        Self {
            expression,
            relation,
            strength,
        }
    }
}

/// A row in the simplex tableau.
#[derive(Debug, Clone)]
struct Row {
    constant: f64,
    cells: HashMap<Symbol, f64>,
}

impl Row {
    fn new(constant: f64) -> Self {
        Self {
            constant,
            cells: HashMap::new(),
        }
    }

    fn add(&mut self, symbol: Symbol, coefficient: f64) {
        let entry = self.cells.entry(symbol).or_insert(0.0);
        *entry += coefficient;
        if near_zero(*entry) {
            self.cells.remove(&symbol);
        }
    }

    fn insert_symbol(&mut self, symbol: Symbol, coefficient: f64) {
        if near_zero(coefficient) {
            self.cells.remove(&symbol);
        } else {
            self.cells.insert(symbol, coefficient);
        }
    }

    fn coefficient(&self, symbol: Symbol) -> f64 {
        self.cells.get(&symbol).copied().unwrap_or(0.0)
    }

    /// Substitute a symbol in this row with another row.
    fn substitute(&mut self, symbol: Symbol, row: &Row) {
        if let Some(coeff) = self.cells.remove(&symbol) {
            self.constant += coeff * row.constant;
            for (&s, &c) in &row.cells {
                self.add(s, c * coeff);
            }
        }
    }

    /// Solve for a symbol in this row.
    fn solve_for(&mut self, symbol: Symbol) {
        let coeff = self.cells.remove(&symbol).unwrap_or(1.0);
        let multiplier = -1.0 / coeff;
        self.constant *= multiplier;
        for c in self.cells.values_mut() {
            *c *= multiplier;
        }
    }

    /// Solve for two symbols in this row.
    fn solve_for_symbols(&mut self, lhs: Symbol, rhs: Symbol) {
        self.insert_symbol(lhs, -1.0);
        self.solve_for(rhs);
    }
}

/// The Cassowary constraint solver.
#[derive(Debug)]
pub struct Solver {
    /// Counter for generating unique variable IDs
    var_counter: usize,
    /// Counter for generating unique symbol IDs
    symbol_counter: usize,
    /// The objective function row
    objective: Row,
    /// Artificial objective for phase 1
    artificial: Option<Row>,
    /// The tableau rows, keyed by their basic symbol
    rows: HashMap<Symbol, Row>,
    /// Mapping from variables to their marker symbols
    var_symbols: HashMap<Variable, Symbol>,
    /// Mapping from constraints to their marker and other symbols
    constraints: HashMap<usize, (Symbol, Symbol)>,
    /// Counter for constraint IDs
    constraint_counter: usize,
    /// Infeasible rows that need optimization
    infeasible_rows: Vec<Symbol>,
    /// External variables and their values
    var_data: HashMap<Variable, f64>,
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}

impl Solver {
    /// Create a new solver.
    pub fn new() -> Self {
        Self {
            var_counter: 0,
            symbol_counter: 0,
            objective: Row::new(0.0),
            artificial: None,
            rows: HashMap::new(),
            var_symbols: HashMap::new(),
            constraints: HashMap::new(),
            constraint_counter: 0,
            infeasible_rows: Vec::new(),
            var_data: HashMap::new(),
        }
    }

    /// Create a new variable.
    pub fn new_variable(&mut self) -> Variable {
        let var = Variable(self.var_counter);
        self.var_counter += 1;
        self.var_data.insert(var, 0.0);
        var
    }

    /// Get the current value of a variable.
    pub fn get_value(&self, var: Variable) -> f64 {
        // Check if the variable is basic (has a row)
        if let Some(&symbol) = self.var_symbols.get(&var) {
            if let Some(row) = self.rows.get(&symbol) {
                return row.constant;
            }
        }

        // Check the external symbol directly
        let symbol = Symbol::External(var.0);
        if let Some(row) = self.rows.get(&symbol) {
            return row.constant;
        }

        // Return stored value or 0
        self.var_data.get(&var).copied().unwrap_or(0.0)
    }

    /// Add a constraint to the solver.
    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<usize, SolverError> {
        let id = self.constraint_counter;
        self.constraint_counter += 1;

        let (row, tag) = self.create_row(&constraint)?;

        // Choose the subject (basic variable) for this row
        let subject = self.choose_subject(&row, &tag);

        // Only return unsatisfiable if the row contains no pivotable or external symbols
        // (i.e., only dummy variables). If there are slack/error variables from
        // substituted constraints, we can still use an artificial variable to make progress.
        if subject.is_none() && row.cells.keys().all(|s| !s.is_pivotable() && !s.is_external()) {
            // The row contains only dummy symbols - check for conflicts
            if !near_zero(row.constant) {
                return Err(SolverError::UnsatisfiableConstraint);
            }
            // The constraint is redundant
            return Ok(id);
        }

        let subject = subject.or_else(|| {
            // Add an artificial variable
            self.add_artificial_variable(&row)
        });

        if let Some(subject) = subject {
            let mut row = row;
            row.solve_for(subject);
            self.substitute(subject, &row);
            self.rows.insert(subject, row);
        }

        self.constraints.insert(id, tag);
        self.optimize(&self.objective.clone())?;

        Ok(id)
    }

    /// Remove a constraint from the solver.
    pub fn remove_constraint(&mut self, id: usize) -> Result<(), SolverError> {
        let tag = self.constraints.remove(&id)
            .ok_or(SolverError::UnknownConstraint)?;

        self.remove_constraint_effects(&tag);

        // Try to remove the marker from the tableau
        if self.rows.remove(&tag.0).is_none() {
            // The marker is not basic - need to pivot it out
            let (row_symbol, _) = self.get_leaving_row(tag.0)
                .ok_or(SolverError::InternalError("No leaving row found"))?;

            if let Some(mut row) = self.rows.remove(&row_symbol) {
                row.solve_for_symbols(row_symbol, tag.0);
                self.substitute(tag.0, &row);
            }
        }

        self.optimize(&self.objective.clone())?;
        Ok(())
    }

    /// Suggest a value for a variable (creates an edit constraint).
    pub fn suggest_value(&mut self, var: Variable, value: f64) -> Result<(), SolverError> {
        // Create an edit constraint: var == value with STRONG strength
        let mut expr = Expression::from_variable(var);
        expr.constant = -value;

        let constraint = Constraint::new(expr, Relation::Equal, Strength::STRONG);
        self.add_constraint(constraint)?;
        Ok(())
    }

    /// Update all variable values after solving.
    pub fn update_variables(&mut self) {
        for (&var, value) in &mut self.var_data {
            let symbol = Symbol::External(var.0);
            if let Some(row) = self.rows.get(&symbol) {
                *value = row.constant;
            }
        }
    }

    /// Create a row for a constraint.
    fn create_row(&mut self, constraint: &Constraint) -> Result<(Row, (Symbol, Symbol)), SolverError> {
        let mut row = Row::new(constraint.expression.constant);

        // Add the terms to the row, substituting basic variables
        for (&symbol, &coeff) in constraint.expression.terms.iter() {
            if near_zero(coeff) {
                continue;
            }

            if let Some(basic_row) = self.rows.get(&symbol) {
                row.constant += coeff * basic_row.constant;
                for (&s, &c) in &basic_row.cells {
                    row.add(s, c * coeff);
                }
            } else {
                row.add(symbol, coeff);
            }
        }

        let mut tag = (self.new_symbol(Symbol::Dummy(0)), self.new_symbol(Symbol::Dummy(0)));

        match constraint.relation {
            Relation::LessOrEqual | Relation::GreaterOrEqual => {
                let coeff = if constraint.relation == Relation::LessOrEqual {
                    1.0
                } else {
                    -1.0
                };

                let slack = self.new_symbol(Symbol::Slack(0));
                tag.0 = slack;
                row.insert_symbol(slack, coeff);

                if !constraint.strength.is_required() {
                    let error = self.new_symbol(Symbol::Error(0));
                    tag.1 = error;
                    row.insert_symbol(error, -coeff);
                    self.objective.insert_symbol(error, constraint.strength.0);
                }
            }
            Relation::Equal => {
                if constraint.strength.is_required() {
                    let dummy = self.new_symbol(Symbol::Dummy(0));
                    tag.0 = dummy;
                    row.insert_symbol(dummy, 1.0);
                } else {
                    let errplus = self.new_symbol(Symbol::Error(0));
                    let errminus = self.new_symbol(Symbol::Error(0));
                    tag.0 = errplus;
                    tag.1 = errminus;
                    row.insert_symbol(errplus, -1.0);
                    row.insert_symbol(errminus, 1.0);
                    self.objective.insert_symbol(errplus, constraint.strength.0);
                    self.objective.insert_symbol(errminus, constraint.strength.0);
                }
            }
        }

        // Ensure the constant is non-negative
        if row.constant < 0.0 {
            row.constant = -row.constant;
            for coeff in row.cells.values_mut() {
                *coeff = -*coeff;
            }
        }

        Ok((row, tag))
    }

    /// Generate a new symbol.
    fn new_symbol(&mut self, kind: Symbol) -> Symbol {
        let id = self.symbol_counter;
        self.symbol_counter += 1;
        match kind {
            Symbol::External(_) => Symbol::External(id),
            Symbol::Slack(_) => Symbol::Slack(id),
            Symbol::Error(_) => Symbol::Error(id),
            Symbol::Dummy(_) => Symbol::Dummy(id),
        }
    }

    /// Choose a subject for the row.
    fn choose_subject(&self, row: &Row, tag: &(Symbol, Symbol)) -> Option<Symbol> {
        // First, check for external symbols
        for &symbol in row.cells.keys() {
            if symbol.is_external() {
                return Some(symbol);
            }
        }

        // Check for slack or error symbols from the tag
        if tag.0.is_pivotable() && row.coefficient(tag.0).abs() > EPSILON {
            return Some(tag.0);
        }
        if tag.1.is_pivotable() && row.coefficient(tag.1).abs() > EPSILON {
            return Some(tag.1);
        }

        None
    }

    /// Add an artificial variable for the row.
    fn add_artificial_variable(&mut self, row: &Row) -> Option<Symbol> {
        let art = self.new_symbol(Symbol::Slack(0));

        // Add the artificial row
        let mut art_row = Row::new(row.constant);
        for (&symbol, &coeff) in &row.cells {
            if !symbol.is_dummy() {
                art_row.insert_symbol(symbol, coeff);
            }
        }

        // Create the artificial objective
        let mut artificial = Row::new(0.0);
        artificial.constant = -row.constant;
        for (&symbol, &coeff) in &row.cells {
            artificial.insert_symbol(symbol, -coeff);
        }

        self.artificial = Some(artificial);
        self.rows.insert(art, art_row);

        // Optimize the artificial objective
        if let Some(ref objective) = self.artificial.clone() {
            let _ = self.optimize(objective);
        }

        // Check if the artificial is satisfied
        if let Some(ref art_obj) = self.artificial {
            if !near_zero(art_obj.constant) {
                self.artificial = None;
                return None;
            }
        }

        // Remove the artificial variable from the tableau
        if let Some(art_row) = self.rows.remove(&art) {
            // If the artificial row is empty, we're done
            if art_row.cells.is_empty() {
                self.artificial = None;
                return Some(art);
            }

            // Pivot out the artificial variable
            let entering = art_row.cells.keys().next().copied();
            if let Some(entering) = entering {
                let mut row = art_row;
                row.solve_for_symbols(art, entering);
                self.substitute(entering, &row);
                self.rows.insert(entering, row);
            }
        }

        self.artificial = None;
        Some(art)
    }

    /// Substitute a symbol throughout the tableau.
    fn substitute(&mut self, symbol: Symbol, row: &Row) {
        for r in self.rows.values_mut() {
            r.substitute(symbol, row);
        }
        self.objective.substitute(symbol, row);
        if let Some(ref mut art) = self.artificial {
            art.substitute(symbol, row);
        }
    }

    /// Optimize the objective function using the simplex algorithm.
    fn optimize(&mut self, objective: &Row) -> Result<(), SolverError> {
        let mut objective = objective.clone();

        loop {
            // Find the entering variable (most negative coefficient in objective)
            let entering = objective
                .cells
                .iter()
                .filter(|(s, c)| !s.is_dummy() && **c < -EPSILON)
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(&s, _)| s);

            let Some(entering) = entering else {
                break; // Optimal
            };

            // Find the leaving variable (minimum ratio test)
            let leaving = self.find_leaving_row(entering);

            let Some((leaving, mut row)) = leaving else {
                // Unbounded - shouldn't happen with valid constraints
                return Err(SolverError::InternalError("Unbounded objective"));
            };

            // Pivot
            row.solve_for_symbols(leaving, entering);
            self.substitute(entering, &row);
            objective.substitute(entering, &row);
            self.rows.insert(entering, row);
        }

        Ok(())
    }

    /// Find the row to leave the basis.
    fn find_leaving_row(&mut self, entering: Symbol) -> Option<(Symbol, Row)> {
        let mut min_ratio = f64::MAX;
        let mut leaving = None;

        for (&symbol, row) in &self.rows {
            if symbol.is_external() {
                continue;
            }

            let coeff = row.coefficient(entering);
            if coeff < -EPSILON {
                let ratio = -row.constant / coeff;
                if ratio < min_ratio {
                    min_ratio = ratio;
                    leaving = Some(symbol);
                }
            }
        }

        leaving.map(|s| (s, self.rows.remove(&s).unwrap()))
    }

    /// Get a leaving row for a specific symbol.
    fn get_leaving_row(&self, symbol: Symbol) -> Option<(Symbol, &Row)> {
        let mut min_ratio = f64::MAX;
        let mut result = None;

        for (&row_symbol, row) in &self.rows {
            if row_symbol.is_external() {
                continue;
            }

            let coeff = row.coefficient(symbol);
            if coeff.abs() > EPSILON {
                let ratio = row.constant / coeff;
                if ratio < min_ratio {
                    min_ratio = ratio;
                    result = Some((row_symbol, row));
                }
            }
        }

        result
    }

    /// Remove constraint effects from the objective.
    fn remove_constraint_effects(&mut self, tag: &(Symbol, Symbol)) {
        if tag.1.is_error() {
            if let Some(row) = self.rows.get(&tag.1) {
                for (&symbol, &coeff) in &row.cells {
                    self.objective.add(symbol, -coeff);
                }
            } else {
                self.objective.add(tag.1, -1.0);
            }
        }
    }
}

/// Errors that can occur during constraint solving.
#[derive(Debug, Clone)]
pub enum SolverError {
    /// A required constraint could not be satisfied.
    UnsatisfiableConstraint,
    /// The specified constraint was not found.
    UnknownConstraint,
    /// An internal error occurred.
    InternalError(&'static str),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverError::UnsatisfiableConstraint => {
                write!(f, "The constraint cannot be satisfied")
            }
            SolverError::UnknownConstraint => {
                write!(f, "The constraint is not in the solver")
            }
            SolverError::InternalError(msg) => {
                write!(f, "Internal solver error: {}", msg)
            }
        }
    }
}

impl std::error::Error for SolverError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_variable() {
        let mut solver = Solver::new();
        let v1 = solver.new_variable();
        let v2 = solver.new_variable();
        assert_ne!(v1.0, v2.0);
    }

    #[test]
    fn test_simple_equality() {
        let mut solver = Solver::new();
        let x = solver.new_variable();

        // x == 100
        let mut expr = Expression::from_variable(x);
        expr.constant = -100.0;
        let constraint = Constraint::new(expr, Relation::Equal, Strength::REQUIRED);
        solver.add_constraint(constraint).unwrap();

        solver.update_variables();
        assert!((solver.get_value(x) - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_two_variables() {
        let mut solver = Solver::new();
        let x = solver.new_variable();
        let y = solver.new_variable();

        // x == 100
        let mut expr1 = Expression::from_variable(x);
        expr1.constant = -100.0;
        solver.add_constraint(Constraint::new(expr1, Relation::Equal, Strength::REQUIRED)).unwrap();

        // y == x + 50
        let mut expr2 = Expression::from_variable(y);
        expr2.add_term(Symbol::External(x.0), -1.0);
        expr2.constant = -50.0;
        solver.add_constraint(Constraint::new(expr2, Relation::Equal, Strength::REQUIRED)).unwrap();

        solver.update_variables();
        assert!((solver.get_value(x) - 100.0).abs() < 0.001);
        assert!((solver.get_value(y) - 150.0).abs() < 0.001);
    }

    #[test]
    fn test_inequality() {
        let mut solver = Solver::new();
        let x = solver.new_variable();

        // x >= 50
        let mut expr1 = Expression::from_variable(x);
        expr1.constant = -50.0;
        solver.add_constraint(Constraint::new(expr1, Relation::GreaterOrEqual, Strength::REQUIRED)).unwrap();

        // x <= 100 (weak - prefer x == 100)
        let mut expr2 = Expression::from_variable(x);
        expr2.constant = -100.0;
        solver.add_constraint(Constraint::new(expr2, Relation::Equal, Strength::WEAK)).unwrap();

        solver.update_variables();
        let value = solver.get_value(x);
        assert!(value >= 49.999, "x should be >= 50, got {}", value);
    }

    #[test]
    fn test_strength_ordering() {
        let mut solver = Solver::new();
        let x = solver.new_variable();

        // x == 100 (weak)
        let mut expr1 = Expression::from_variable(x);
        expr1.constant = -100.0;
        solver.add_constraint(Constraint::new(expr1, Relation::Equal, Strength::WEAK)).unwrap();

        // x == 50 (strong)
        let mut expr2 = Expression::from_variable(x);
        expr2.constant = -50.0;
        solver.add_constraint(Constraint::new(expr2, Relation::Equal, Strength::STRONG)).unwrap();

        solver.update_variables();
        // Strong constraint should win
        assert!((solver.get_value(x) - 50.0).abs() < 0.001);
    }
}
