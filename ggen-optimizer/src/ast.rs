//! Abstract Syntax Tree for SPARQL CONSTRUCT queries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// A complete SPARQL CONSTRUCT query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Query {
    /// PREFIX declarations mapping prefix names to IRIs.
    pub prefixes: HashMap<String, String>,
    /// The CONSTRUCT template.
    pub construct: ConstructTemplate,
    /// The WHERE clause pattern.
    pub where_clause: GraphPattern,
}

/// A CONSTRUCT query with both template and pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructQuery {
    /// Prefixes used in the query.
    pub prefixes: HashMap<String, String>,
    /// Template patterns to construct in the result graph.
    pub template: Vec<TriplePattern>,
    /// WHERE clause patterns to match.
    pub pattern: GraphPattern,
}

/// The CONSTRUCT template - a list of triple patterns to generate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstructTemplate {
    /// Triple patterns in the template.
    pub patterns: Vec<TriplePattern>,
}

/// A graph pattern in a WHERE clause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphPattern {
    /// Basic graph pattern - a set of triple patterns.
    Basic(Vec<TriplePattern>),

    /// OPTIONAL { pattern }
    Optional(Box<GraphPattern>),

    /// UNION { pattern1 } { pattern2 }
    Union(Box<GraphPattern>, Box<GraphPattern>),

    /// FILTER constraint
    Filter {
        /// The filter expression.
        expr: FilterExpr,
        /// The pattern to filter.
        pattern: Box<GraphPattern>,
    },

    /// Sequence of patterns (implicit conjunction).
    Group(Vec<GraphPattern>),

    /// BIND expression
    Bind {
        /// The expression to bind.
        expr: BindExpr,
        /// The variable to bind to.
        var: Var,
    },
}

/// A triple pattern: subject predicate object.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TriplePattern {
    /// Subject term.
    pub subject: Term,
    /// Predicate term.
    pub predicate: Term,
    /// Object term.
    pub object: Term,
}

/// A term in a triple pattern (subject, predicate, or object).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Term {
    /// Variable (e.g., ?x, ?name)
    Var(Var),
    /// IRI (e.g., <http://example.org/foo>)
    Iri(Iri),
    /// Prefixed name (e.g., a2a:Entity)
    PrefixedName { prefix: String, local: String },
    /// Literal value (e.g., "hello", 42, true)
    Literal(Literal),
    /// Blank node
    BlankNode(String),
}

/// A variable reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Var {
    /// Variable name (without the ? or $ prefix).
    pub name: String,
}

/// An IRI reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Iri {
    /// The full IRI string.
    pub iri: String,
}

/// A literal value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Literal {
    /// The lexical form of the literal.
    pub value: String,
    /// Optional datatype IRI.
    pub datatype: Option<Iri>,
    /// Optional language tag.
    pub language: Option<String>,
}

/// A FILTER expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterExpr {
    /// Relational operator (=, !=, <, >, <=, >=)
    Relational {
        op: RelationalOp,
        left: Box<FilterExpr>,
        right: Box<FilterExpr>,
    },

    /// Logical operator (&&, ||)
    Logical {
        op: LogicalOp,
        left: Box<FilterExpr>,
        right: Box<FilterExpr>,
    },

    /// Unary negation (!)
    Not(Box<FilterExpr>),

    /// Built-in function call
    Function { name: String, args: Vec<FilterExpr> },

    /// Variable reference
    Var(Var),

    /// Literal value
    Literal(Literal),

    /// BOUND(?var)
    Bound(Var),
}

/// A BIND expression (typically an IRI construction or function call).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindExpr {
    /// IRI function: IRI(expr)
    Iri(Box<BindExpr>),

    /// CONCAT function
    Concat(Vec<BindExpr>),

    /// STR function
    Str(Box<BindExpr>),

    /// IF(condition, then, else)
    If {
        condition: FilterExpr,
        then_expr: Box<BindExpr>,
        else_expr: Box<BindExpr>,
    },

    /// Variable reference
    Var(Var),

    /// Literal
    Literal(Literal),
}

/// Relational operators for FILTER expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalOp {
    /// Equal (=)
    Eq,
    /// Not equal (!=)
    Ne,
    /// Less than (<)
    Lt,
    /// Less than or equal (<=)
    Le,
    /// Greater than (>)
    Gt,
    /// Greater than or equal (>=)
    Ge,
}

/// Logical operators for FILTER expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalOp {
    /// Logical AND (&&)
    And,
    /// Logical OR (||)
    Or,
}

impl Query {
    /// Create a new query.
    pub fn new(
        prefixes: HashMap<String, String>,
        construct: ConstructTemplate,
        where_clause: GraphPattern,
    ) -> Self {
        Self {
            prefixes,
            construct,
            where_clause,
        }
    }

    /// Get all variables used in the query.
    pub fn variables(&self) -> Vec<&Var> {
        let mut vars = Vec::new();
        self.construct.collect_vars(&mut vars);
        self.where_clause.collect_vars(&mut vars);
        vars
    }

    /// Expand prefixed names to full IRIs.
    pub fn expand_prefixed_name(&self, prefix: &str, local: &str) -> Option<String> {
        self.prefixes
            .get(prefix)
            .map(|iri| format!("{}{}", iri, local))
    }
}

impl ConstructTemplate {
    /// Create a new construct template.
    pub fn new(patterns: Vec<TriplePattern>) -> Self {
        Self { patterns }
    }

    /// Collect all variables used in the template.
    pub fn collect_vars<'a>(&'a self, vars: &mut Vec<&'a Var>) {
        for pattern in &self.patterns {
            pattern.collect_vars(vars);
        }
    }
}

impl GraphPattern {
    /// Collect all variables used in this pattern.
    pub fn collect_vars<'a>(&'a self, vars: &mut Vec<&'a Var>) {
        match self {
            GraphPattern::Basic(patterns) => {
                for pattern in patterns {
                    pattern.collect_vars(vars);
                }
            }
            GraphPattern::Optional(pattern) => pattern.collect_vars(vars),
            GraphPattern::Union(left, right) => {
                left.collect_vars(vars);
                right.collect_vars(vars);
            }
            GraphPattern::Filter { pattern, expr } => {
                pattern.collect_vars(vars);
                expr.collect_vars(vars);
            }
            GraphPattern::Group(patterns) => {
                for pattern in patterns {
                    pattern.collect_vars(vars);
                }
            }
            GraphPattern::Bind { var, expr } => {
                vars.push(var);
                expr.collect_vars(vars);
            }
        }
    }

    /// Check if this pattern is a basic graph pattern.
    pub fn is_basic(&self) -> bool {
        matches!(self, GraphPattern::Basic(_))
    }

    /// Get the triple patterns if this is a basic graph pattern.
    pub fn as_basic(&self) -> Option<&[TriplePattern]> {
        match self {
            GraphPattern::Basic(patterns) => Some(patterns),
            _ => None,
        }
    }
}

impl TriplePattern {
    /// Create a new triple pattern.
    pub fn new(subject: Term, predicate: Term, object: Term) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }

    /// Collect all variables in this triple pattern.
    pub fn collect_vars<'a>(&'a self, vars: &mut Vec<&'a Var>) {
        self.subject.collect_vars(vars);
        self.predicate.collect_vars(vars);
        self.object.collect_vars(vars);
    }

    /// Get all variables in this triple pattern.
    pub fn variables(&self) -> Vec<&Var> {
        let mut vars = Vec::new();
        self.collect_vars(&mut vars);
        vars
    }

    /// Check if this pattern contains a specific variable.
    pub fn contains_var(&self, target: &Var) -> bool {
        self.subject.is_var(target) || self.predicate.is_var(target) || self.object.is_var(target)
    }
}

impl Term {
    /// Create a variable term.
    pub fn var(name: impl Into<String>) -> Self {
        Self::Var(Var { name: name.into() })
    }

    /// Create an IRI term.
    pub fn iri(iri: impl Into<String>) -> Self {
        Self::Iri(Iri { iri: iri.into() })
    }

    /// Create a prefixed name term.
    pub fn prefixed(prefix: impl Into<String>, local: impl Into<String>) -> Self {
        Self::PrefixedName {
            prefix: prefix.into(),
            local: local.into(),
        }
    }

    /// Create a string literal term.
    pub fn literal(value: impl Into<String>) -> Self {
        Self::Literal(Literal {
            value: value.into(),
            datatype: None,
            language: None,
        })
    }

    /// Check if this term is a variable.
    pub fn is_var(&self, target: &Var) -> bool {
        match self {
            Term::Var(v) => v == target,
            _ => false,
        }
    }

    /// Get the variable if this is a variable term.
    pub fn as_var(&self) -> Option<&Var> {
        match self {
            Term::Var(v) => Some(v),
            _ => None,
        }
    }

    /// Collect all variables in this term.
    pub fn collect_vars<'a>(&'a self, vars: &mut Vec<&'a Var>) {
        if let Term::Var(v) = self {
            vars.push(v);
        }
    }
}

impl FilterExpr {
    /// Collect all variables in this expression.
    pub fn collect_vars<'a>(&'a self, vars: &mut Vec<&'a Var>) {
        match self {
            FilterExpr::Relational { left, right, .. } => {
                left.collect_vars(vars);
                right.collect_vars(vars);
            }
            FilterExpr::Logical { left, right, .. } => {
                left.collect_vars(vars);
                right.collect_vars(vars);
            }
            FilterExpr::Not(expr) => expr.collect_vars(vars),
            FilterExpr::Function { args, .. } => {
                for arg in args {
                    arg.collect_vars(vars);
                }
            }
            FilterExpr::Var(v) => vars.push(v),
            FilterExpr::Literal(_) => {}
            FilterExpr::Bound(v) => vars.push(v),
        }
    }
}

impl BindExpr {
    /// Collect all variables in this expression.
    pub fn collect_vars<'a>(&'a self, vars: &mut Vec<&'a Var>) {
        match self {
            BindExpr::Iri(expr) => expr.collect_vars(vars),
            BindExpr::Concat(exprs) => {
                for expr in exprs {
                    expr.collect_vars(vars);
                }
            }
            BindExpr::Str(expr) => expr.collect_vars(vars),
            BindExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                condition.collect_vars(vars);
                then_expr.collect_vars(vars);
                else_expr.collect_vars(vars);
            }
            BindExpr::Var(v) => vars.push(v),
            BindExpr::Literal(_) => {}
        }
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}

impl fmt::Display for RelationalOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelationalOp::Eq => write!(f, "="),
            RelationalOp::Ne => write!(f, "!="),
            RelationalOp::Lt => write!(f, "<"),
            RelationalOp::Le => write!(f, "<="),
            RelationalOp::Gt => write!(f, ">"),
            RelationalOp::Ge => write!(f, ">="),
        }
    }
}

impl fmt::Display for LogicalOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicalOp::And => write!(f, "&&"),
            LogicalOp::Or => write!(f, "||"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triple_pattern_variables() {
        let pattern = TriplePattern::new(
            Term::var("s"),
            Term::iri("http://example.org/pred"),
            Term::var("o"),
        );

        let vars = pattern.variables();
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "s");
        assert_eq!(vars[1].name, "o");
    }

    #[test]
    fn test_term_creation() {
        let var = Term::var("x");
        assert!(matches!(var, Term::Var(_)));

        let iri = Term::iri("http://example.org/");
        assert!(matches!(iri, Term::Iri(_)));

        let prefixed = Term::prefixed("a2a", "Entity");
        assert!(matches!(prefixed, Term::PrefixedName { .. }));
    }

    #[test]
    fn test_graph_pattern_basic() {
        let pattern = GraphPattern::Basic(vec![]);
        assert!(pattern.is_basic());
        assert_eq!(pattern.as_basic(), Some(&[][..]));
    }
}
