//! DSL for declarative invariants
//!
//! This module provides a domain-specific language for writing invariants in a
//! declarative style. Instead of implementing the `Invariant` trait manually,
//! you can write expressions like:
//!
//! ```text
//! INVARIANT task.artifacts.length <= 100
//! INVARIANT task.status.state == "completed" OR task.status.state == "failed"
//! INVARIANT task.history.length > 0 AND task.history.length <= 1000
//! ```
//!
//! # Grammar
//!
//! ```text
//! invariant    ::= "INVARIANT" expression
//! expression   ::= logical_or
//! logical_or   ::= logical_and ("OR" logical_and)*
//! logical_and  ::= comparison ("AND" comparison)*
//! comparison   ::= term (comp_op term)?
//! comp_op      ::= "==" | "!=" | "<=" | ">=" | "<" | ">"
//! term         ::= "NOT" term | "(" expression ")" | field_access | literal
//! field_access ::= identifier ("." identifier)*
//! literal      ::= number | string | boolean
//! identifier   ::= [a-zA-Z_][a-zA-Z0-9_]*
//! number       ::= [0-9]+
//! string       ::= "\"" [^"]* "\""
//! boolean      ::= "true" | "false"
//! ```
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::invariants::dsl::{parse_invariant, InvariantExpr};
//! use a2a_rs::domain::Task;
//!
//! // Parse an invariant expression
//! let expr = parse_invariant("INVARIANT task.artifacts.length <= 100").unwrap();
//!
//! // Evaluate against a task
//! let task = Task::new("task-1".to_string(), "ctx-1".to_string());
//! let result = expr.evaluate(&task);
//! assert!(result.is_ok());
//! ```

use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::{map, opt, recognize},
    multi::many0,
    sequence::{delimited, pair, preceded, tuple},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use super::{Invariant, InvariantResult, InvariantViolation};

/// AST node for invariant expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Literal value (number, string, boolean)
    Literal(LiteralValue),
    /// Field access (e.g., task.artifacts.length)
    FieldAccess(Vec<String>),
    /// Binary comparison
    Comparison {
        left: Box<Expr>,
        op: ComparisonOp,
        right: Box<Expr>,
    },
    /// Logical AND
    And(Box<Expr>, Box<Expr>),
    /// Logical OR
    Or(Box<Expr>, Box<Expr>),
    /// Logical NOT
    Not(Box<Expr>),
}

/// Literal values in expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LiteralValue {
    Number(i64),
    String(String),
    Boolean(bool),
}

/// Comparison operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

impl fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComparisonOp::Equal => write!(f, "=="),
            ComparisonOp::NotEqual => write!(f, "!="),
            ComparisonOp::LessThan => write!(f, "<"),
            ComparisonOp::LessThanOrEqual => write!(f, "<="),
            ComparisonOp::GreaterThan => write!(f, ">"),
            ComparisonOp::GreaterThanOrEqual => write!(f, ">="),
        }
    }
}

/// Parsed invariant expression ready for evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvariantExpr {
    /// The parsed expression AST
    pub expr: Expr,
    /// The original source text
    pub source: String,
}

impl InvariantExpr {
    /// Evaluate the invariant against a value
    ///
    /// The value is serialized to JSON and then the expression is evaluated
    /// against the resulting JSON structure.
    pub fn evaluate<T: Serialize>(&self, value: &T) -> InvariantResult {
        // Serialize to JSON for uniform access
        let json_value = serde_json::to_value(value).map_err(|e| InvariantViolation::Custom {
            name: "dsl_invariant".to_string(),
            reason: format!("Failed to serialize value: {}", e),
        })?;

        self.evaluate_json(&json_value)
    }

    /// Evaluate against a JSON value directly
    pub fn evaluate_json(&self, value: &Value) -> InvariantResult {
        match eval_expr(&self.expr, value) {
            Ok(EvalResult::Boolean(true)) => Ok(()),
            Ok(EvalResult::Boolean(false)) => Err(InvariantViolation::Custom {
                name: "dsl_invariant".to_string(),
                reason: format!("Invariant violated: {}", self.source),
            }),
            Ok(result) => Err(InvariantViolation::Custom {
                name: "dsl_invariant".to_string(),
                reason: format!(
                    "Invariant expression must evaluate to boolean, got: {:?}",
                    result
                ),
            }),
            Err(e) => Err(InvariantViolation::Custom {
                name: "dsl_invariant".to_string(),
                reason: format!("Evaluation error: {}", e),
            }),
        }
    }
}

impl<T: Serialize> Invariant<T> for InvariantExpr {
    fn check(&self, value: &T) -> InvariantResult {
        self.evaluate(value)
    }

    fn name(&self) -> &str {
        "dsl_invariant"
    }

    fn description(&self) -> &str {
        &self.source
    }
}

/// Result of evaluating an expression
#[derive(Debug, Clone, PartialEq)]
enum EvalResult {
    Number(i64),
    String(String),
    Boolean(bool),
    Null,
}

impl fmt::Display for EvalResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalResult::Number(n) => write!(f, "{}", n),
            EvalResult::String(s) => write!(f, "\"{}\"", s),
            EvalResult::Boolean(b) => write!(f, "{}", b),
            EvalResult::Null => write!(f, "null"),
        }
    }
}

/// Evaluate an expression against a JSON value
fn eval_expr(expr: &Expr, context: &Value) -> Result<EvalResult, String> {
    match expr {
        Expr::Literal(lit) => Ok(match lit {
            LiteralValue::Number(n) => EvalResult::Number(*n),
            LiteralValue::String(s) => EvalResult::String(s.clone()),
            LiteralValue::Boolean(b) => EvalResult::Boolean(*b),
        }),

        Expr::FieldAccess(path) => {
            let mut current = context;
            for field in path {
                // Special handling for .length on arrays
                if field == "length" {
                    if let Value::Array(arr) = current {
                        return Ok(EvalResult::Number(arr.len() as i64));
                    } else {
                        return Err(format!("Cannot get length of non-array: {:?}", current));
                    }
                }

                // Special handling for .exists on any value
                if field == "exists" {
                    return Ok(EvalResult::Boolean(!current.is_null()));
                }

                // Regular field access
                match current {
                    Value::Object(obj) => {
                        current = obj.get(field).unwrap_or(&Value::Null);
                    }
                    Value::Null => return Ok(EvalResult::Null),
                    _ => {
                        return Err(format!(
                            "Cannot access field '{}' on non-object: {:?}",
                            field, current
                        ));
                    }
                }
            }

            // Convert final value to EvalResult
            value_to_result(current)
        }

        Expr::Comparison { left, op, right } => {
            let left_val = eval_expr(left, context)?;
            let right_val = eval_expr(right, context)?;
            Ok(EvalResult::Boolean(compare(&left_val, *op, &right_val)?))
        }

        Expr::And(left, right) => {
            let left_val = eval_expr(left, context)?;
            let right_val = eval_expr(right, context)?;

            match (left_val, right_val) {
                (EvalResult::Boolean(l), EvalResult::Boolean(r)) => Ok(EvalResult::Boolean(l && r)),
                _ => Err("AND requires boolean operands".to_string()),
            }
        }

        Expr::Or(left, right) => {
            let left_val = eval_expr(left, context)?;
            let right_val = eval_expr(right, context)?;

            match (left_val, right_val) {
                (EvalResult::Boolean(l), EvalResult::Boolean(r)) => Ok(EvalResult::Boolean(l || r)),
                _ => Err("OR requires boolean operands".to_string()),
            }
        }

        Expr::Not(inner) => {
            let val = eval_expr(inner, context)?;
            match val {
                EvalResult::Boolean(b) => Ok(EvalResult::Boolean(!b)),
                _ => Err("NOT requires boolean operand".to_string()),
            }
        }
    }
}

/// Convert a JSON value to an EvalResult
fn value_to_result(value: &Value) -> Result<EvalResult, String> {
    match value {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(EvalResult::Number(i))
            } else {
                Err(format!("Unsupported number type: {}", n))
            }
        }
        Value::String(s) => Ok(EvalResult::String(s.clone())),
        Value::Bool(b) => Ok(EvalResult::Boolean(*b)),
        Value::Null => Ok(EvalResult::Null),
        _ => Err(format!("Cannot convert value to result: {:?}", value)),
    }
}

/// Compare two values using the given operator
fn compare(left: &EvalResult, op: ComparisonOp, right: &EvalResult) -> Result<bool, String> {
    match (left, right) {
        (EvalResult::Number(l), EvalResult::Number(r)) => Ok(match op {
            ComparisonOp::Equal => l == r,
            ComparisonOp::NotEqual => l != r,
            ComparisonOp::LessThan => l < r,
            ComparisonOp::LessThanOrEqual => l <= r,
            ComparisonOp::GreaterThan => l > r,
            ComparisonOp::GreaterThanOrEqual => l >= r,
        }),
        (EvalResult::String(l), EvalResult::String(r)) => Ok(match op {
            ComparisonOp::Equal => l == r,
            ComparisonOp::NotEqual => l != r,
            ComparisonOp::LessThan => l < r,
            ComparisonOp::LessThanOrEqual => l <= r,
            ComparisonOp::GreaterThan => l > r,
            ComparisonOp::GreaterThanOrEqual => l >= r,
        }),
        (EvalResult::Boolean(l), EvalResult::Boolean(r)) => Ok(match op {
            ComparisonOp::Equal => l == r,
            ComparisonOp::NotEqual => l != r,
            _ => return Err(format!("Operator {} not supported for booleans", op)),
        }),
        _ => Err(format!(
            "Cannot compare {:?} and {:?} with {}",
            left, right, op
        )),
    }
}

// ============================================================================
// PARSER IMPLEMENTATION
// ============================================================================

/// Parse an invariant expression from a string
///
/// # Example
///
/// ```rust
/// use a2a_rs::construct::invariants::dsl::parse_invariant;
///
/// let expr = parse_invariant("INVARIANT task.artifacts.length <= 100").unwrap();
/// ```
pub fn parse_invariant(input: &str) -> Result<InvariantExpr, String> {
    match invariant(input) {
        Ok((remaining, expr)) => {
            let remaining = remaining.trim();
            if !remaining.is_empty() {
                Err(format!("Unexpected input after invariant: {}", remaining))
            } else {
                Ok(InvariantExpr {
                    expr,
                    source: input.to_string(),
                })
            }
        }
        Err(e) => Err(format!("Parse error: {}", e)),
    }
}

/// Parse the full invariant (INVARIANT <expression>)
fn invariant(input: &str) -> IResult<&str, Expr> {
    preceded(tuple((tag_no_case("INVARIANT"), multispace1)), expression)(input)
}

/// Parse an expression (logical OR level)
fn expression(input: &str) -> IResult<&str, Expr> {
    logical_or(input)
}

/// Parse logical OR expression
fn logical_or(input: &str) -> IResult<&str, Expr> {
    let (input, first) = logical_and(input)?;
    let (input, rest) = many0(preceded(
        delimited(multispace0, tag_no_case("OR"), multispace0),
        logical_and,
    ))(input)?;

    Ok((
        input,
        rest.into_iter()
            .fold(first, |acc, expr| Expr::Or(Box::new(acc), Box::new(expr))),
    ))
}

/// Parse logical AND expression
fn logical_and(input: &str) -> IResult<&str, Expr> {
    let (input, first) = comparison(input)?;
    let (input, rest) = many0(preceded(
        delimited(multispace0, tag_no_case("AND"), multispace0),
        comparison,
    ))(input)?;

    Ok((
        input,
        rest.into_iter()
            .fold(first, |acc, expr| Expr::And(Box::new(acc), Box::new(expr))),
    ))
}

/// Parse comparison expression
fn comparison(input: &str) -> IResult<&str, Expr> {
    let (input, left) = term(input)?;
    let (input, op_and_right) = opt(pair(
        delimited(multispace0, comparison_op, multispace0),
        term,
    ))(input)?;

    Ok((
        input,
        if let Some((op, right)) = op_and_right {
            Expr::Comparison {
                left: Box::new(left),
                op,
                right: Box::new(right),
            }
        } else {
            left
        },
    ))
}

/// Parse comparison operator
fn comparison_op(input: &str) -> IResult<&str, ComparisonOp> {
    alt((
        map(tag("=="), |_| ComparisonOp::Equal),
        map(tag("!="), |_| ComparisonOp::NotEqual),
        map(tag("<="), |_| ComparisonOp::LessThanOrEqual),
        map(tag(">="), |_| ComparisonOp::GreaterThanOrEqual),
        map(tag("<"), |_| ComparisonOp::LessThan),
        map(tag(">"), |_| ComparisonOp::GreaterThan),
    ))(input)
}

/// Parse a term (primary expression)
fn term(input: &str) -> IResult<&str, Expr> {
    alt((
        // NOT expression
        map(
            preceded(tuple((tag_no_case("NOT"), multispace1)), term),
            |expr| Expr::Not(Box::new(expr)),
        ),
        // Parenthesized expression
        delimited(
            char('('),
            delimited(multispace0, expression, multispace0),
            char(')'),
        ),
        // Field access
        map(field_access, Expr::FieldAccess),
        // Literals
        map(literal, Expr::Literal),
    ))(input)
}

/// Parse field access (e.g., task.artifacts.length)
fn field_access(input: &str) -> IResult<&str, Vec<String>> {
    let (input, first) = identifier(input)?;
    let (input, rest) = many0(preceded(char('.'), identifier))(input)?;

    let mut path = vec![first];
    path.extend(rest);

    Ok((input, path))
}

/// Parse an identifier
fn identifier(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            take_while1(|c: char| c.is_ascii_alphabetic() || c == '_'),
            take_while1(|c: char| c.is_ascii_alphanumeric() || c == '_'),
        )),
        |s: &str| s.to_string(),
    )(input)
    .or_else(|_| {
        // Also accept single-character identifiers
        map(
            take_while1(|c: char| c.is_ascii_alphabetic() || c == '_'),
            |s: &str| s.to_string(),
        )(input)
    })
}

/// Parse a literal value
fn literal(input: &str) -> IResult<&str, LiteralValue> {
    alt((
        map(boolean, LiteralValue::Boolean),
        map(number, LiteralValue::Number),
        map(string, LiteralValue::String),
    ))(input)
}

/// Parse a number
fn number(input: &str) -> IResult<&str, i64> {
    map(
        recognize(pair(
            opt(char('-')),
            take_while1(|c: char| c.is_ascii_digit()),
        )),
        |s: &str| s.parse::<i64>().unwrap(),
    )(input)
}

/// Parse a string literal
fn string(input: &str) -> IResult<&str, String> {
    delimited(
        char('"'),
        map(take_while1(|c| c != '"'), |s: &str| s.to_string()),
        char('"'),
    )(input)
    .or_else(|_| {
        // Also handle empty strings
        delimited(
            char('"'),
            map(nom::combinator::success(""), |s: &str| s.to_string()),
            char('"'),
        )(input)
    })
}

/// Parse a boolean
fn boolean(input: &str) -> IResult<&str, bool> {
    alt((
        map(tag_no_case("true"), |_| true),
        map(tag_no_case("false"), |_| false),
    ))(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Task, TaskState};

    #[test]
    fn test_parse_simple_comparison() {
        let expr = parse_invariant("INVARIANT task.artifacts.length <= 100").unwrap();
        assert!(matches!(expr.expr, Expr::Comparison { .. }));
    }

    #[test]
    fn test_parse_logical_and() {
        let expr = parse_invariant("INVARIANT x > 0 AND x < 100").unwrap();
        assert!(matches!(expr.expr, Expr::And(_, _)));
    }

    #[test]
    fn test_parse_logical_or() {
        let expr = parse_invariant("INVARIANT x == 1 OR x == 2").unwrap();
        assert!(matches!(expr.expr, Expr::Or(_, _)));
    }

    #[test]
    fn test_parse_not() {
        let expr = parse_invariant("INVARIANT NOT x == 0").unwrap();
        assert!(matches!(expr.expr, Expr::Not(_)));
    }

    #[test]
    fn test_parse_parentheses() {
        let expr = parse_invariant("INVARIANT (x > 0 AND x < 10) OR x == 100").unwrap();
        assert!(matches!(expr.expr, Expr::Or(_, _)));
    }

    #[test]
    fn test_eval_field_access() {
        let task = Task::new("task-1".to_string(), "ctx-1".to_string());
        let expr = parse_invariant("INVARIANT id == \"task-1\"").unwrap();
        assert!(expr.evaluate(&task).is_ok());
    }

    #[test]
    fn test_eval_array_length() {
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
        task.artifacts = Some(vec![]);

        let expr = parse_invariant("INVARIANT artifacts.length <= 100").unwrap();
        assert!(expr.evaluate(&task).is_ok());

        let expr = parse_invariant("INVARIANT artifacts.length > 100").unwrap();
        assert!(expr.evaluate(&task).is_err());
    }

    #[test]
    fn test_eval_comparison_operators() {
        let json = serde_json::json!({ "x": 42 });

        let test_cases = vec![
            ("INVARIANT x == 42", true),
            ("INVARIANT x != 42", false),
            ("INVARIANT x < 50", true),
            ("INVARIANT x <= 42", true),
            ("INVARIANT x > 40", true),
            ("INVARIANT x >= 42", true),
        ];

        for (invariant_str, expected) in test_cases {
            let expr = parse_invariant(invariant_str).unwrap();
            let result = expr.evaluate_json(&json);
            assert_eq!(result.is_ok(), expected, "Failed for: {}", invariant_str);
        }
    }

    #[test]
    fn test_eval_logical_operators() {
        let json = serde_json::json!({ "x": 42, "y": 10 });

        let test_cases = vec![
            ("INVARIANT x > 0 AND y > 0", true),
            ("INVARIANT x > 0 AND y > 20", false),
            ("INVARIANT x > 0 OR y > 20", true),
            ("INVARIANT x < 0 OR y < 0", false),
            ("INVARIANT NOT x < 0", true),
            ("INVARIANT NOT x > 0", false),
        ];

        for (invariant_str, expected) in test_cases {
            let expr = parse_invariant(invariant_str).unwrap();
            let result = expr.evaluate_json(&json);
            assert_eq!(result.is_ok(), expected, "Failed for: {}", invariant_str);
        }
    }

    #[test]
    fn test_eval_string_comparison() {
        let json = serde_json::json!({ "state": "completed" });

        let expr = parse_invariant("INVARIANT state == \"completed\"").unwrap();
        assert!(expr.evaluate_json(&json).is_ok());

        let expr = parse_invariant("INVARIANT state == \"failed\"").unwrap();
        assert!(expr.evaluate_json(&json).is_err());
    }

    #[test]
    fn test_complex_invariant() {
        let mut task = Task::new("task-1".to_string(), "ctx-1".to_string());
        task.update_status(TaskState::Completed, None);

        let expr = parse_invariant(
            "INVARIANT (status.state == \"completed\" OR status.state == \"failed\") AND id == \"task-1\""
        ).unwrap();

        // Note: TaskState serializes as kebab-case
        let json = serde_json::to_value(&task).unwrap();
        assert!(expr.evaluate_json(&json).is_ok());
    }

    #[test]
    fn test_invariant_trait_impl() {
        let task = Task::new("task-1".to_string(), "ctx-1".to_string());
        let expr = parse_invariant("INVARIANT id == \"task-1\"").unwrap();

        // Test as an Invariant trait object
        let invariant: &dyn Invariant<Task> = &expr;
        assert!(invariant.check(&task).is_ok());
        assert_eq!(invariant.name(), "dsl_invariant");
    }
}
