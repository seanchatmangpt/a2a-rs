//! DSL parser for declarative guard definitions.
//!
//! This module provides a text-based DSL for defining guards without writing Rust code.
//! Guards can be specified as simple text rules that compile to Guard trait objects.
//!
//! # Syntax
//!
//! The DSL supports the following rule types:
//!
//! - Type checks: `REQUIRE <path> TYPE <type>`
//! - Enum checks: `REQUIRE <path> IN [<value>, ...]`
//! - Range checks: `REQUIRE <path> RANGE <min>..<max>`
//! - String length: `REQUIRE <path> LENGTH <min>..<max>`
//! - Required fields: `REQUIRE FIELD <name>`
//! - State transitions: `ALLOW TRANSITION <from> -> <to>`
//!
//! # Examples
//!
//! ```
//! use a2a_rs::construct::guards::dsl::parse_guard;
//!
//! // Type check
//! let guard = parse_guard("REQUIRE task.name TYPE string").unwrap();
//!
//! // Enum check
//! let guard = parse_guard("REQUIRE task.status IN [submitted, working, completed]").unwrap();
//!
//! // Range check
//! let guard = parse_guard("REQUIRE amount RANGE 0..100").unwrap();
//!
//! // String length
//! let guard = parse_guard("REQUIRE name LENGTH 3..50").unwrap();
//!
//! // Required field
//! let guard = parse_guard("REQUIRE FIELD status").unwrap();
//! ```

use super::{
    EnumGuard, Guard, RangeGuard, RequiredFieldGuard, StateTransitionGuard, StringLengthGuard,
    TypeGuard,
};
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::{map, opt},
    multi::separated_list1,
    number::complete::double,
    sequence::{delimited, tuple},
};
use std::collections::HashMap;

/// Error type for DSL parsing failures.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DslError {
    /// Parsing failed with nom error details
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Unsupported guard type
    #[error("Unsupported guard type: {0}")]
    UnsupportedGuard(String),

    /// Invalid syntax
    #[error("Invalid syntax: {0}")]
    InvalidSyntax(String),
}

/// Represents a parsed guard rule ready to be compiled.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardRule {
    /// Type check: field must be of specified JSON type
    TypeCheck { path: String, expected_type: String },

    /// Enum check: field must be one of the allowed values
    EnumCheck {
        path: String,
        allowed_values: Vec<String>,
    },

    /// Range check: numeric field must be within bounds
    RangeCheck {
        path: String,
        min: Option<f64>,
        max: Option<f64>,
    },

    /// String length check: string field length must be within bounds
    StringLengthCheck {
        path: String,
        min_length: Option<usize>,
        max_length: Option<usize>,
    },

    /// Required field: object must contain this field
    RequiredField { field_name: String },

    /// State transition: transition from one state to another is allowed
    StateTransition { from: String, to: String },
}

/// Parse whitespace (required).
fn ws1(input: &str) -> IResult<&str, &str> {
    multispace1(input)
}

/// Parse optional whitespace.
fn ws0(input: &str) -> IResult<&str, &str> {
    multispace0(input)
}

/// Parse an identifier (alphanumeric + underscore + hyphen).
fn identifier(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        |s: &str| s.to_string(),
    )(input)
}

/// Parse a field path (e.g., "task.status" or "name").
fn field_path(input: &str) -> IResult<&str, String> {
    map(
        take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '.' || c == '-'),
        |s: &str| s.to_string(),
    )(input)
}

/// Parse a JSON type name.
fn json_type(input: &str) -> IResult<&str, String> {
    map(
        alt((
            tag_no_case("string"),
            tag_no_case("number"),
            tag_no_case("boolean"),
            tag_no_case("object"),
            tag_no_case("array"),
            tag_no_case("null"),
        )),
        |s: &str| s.to_lowercase(),
    )(input)
}

/// Parse a range (e.g., "0..100" or "..100" or "0..").
fn range(input: &str) -> IResult<&str, (Option<f64>, Option<f64>)> {
    let (input, min) = opt(double)(input)?;
    let (input, _) = tag("..")(input)?;
    let (input, max) = opt(double)(input)?;
    Ok((input, (min, max)))
}

/// Parse a list of enum values (e.g., "[submitted, working, completed]").
fn enum_values(input: &str) -> IResult<&str, Vec<String>> {
    delimited(
        tuple((char('['), ws0)),
        separated_list1(tuple((ws0, char(','), ws0)), identifier),
        tuple((ws0, char(']'))),
    )(input)
}

/// Parse a TYPE check rule.
/// Syntax: REQUIRE <path> TYPE <type>
fn parse_type_check(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = tag_no_case("REQUIRE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, path) = field_path(input)?;
    let (input, _) = ws1(input)?;
    let (input, _) = tag_no_case("TYPE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, expected_type) = json_type(input)?;

    Ok((
        input,
        GuardRule::TypeCheck {
            path,
            expected_type,
        },
    ))
}

/// Parse an IN check rule.
/// Syntax: REQUIRE <path> IN [value1, value2, ...]
fn parse_enum_check(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = tag_no_case("REQUIRE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, path) = field_path(input)?;
    let (input, _) = ws1(input)?;
    let (input, _) = tag_no_case("IN")(input)?;
    let (input, _) = ws0(input)?;
    let (input, allowed_values) = enum_values(input)?;

    Ok((
        input,
        GuardRule::EnumCheck {
            path,
            allowed_values,
        },
    ))
}

/// Parse a RANGE check rule.
/// Syntax: REQUIRE <path> RANGE <min>..<max>
fn parse_range_check(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = tag_no_case("REQUIRE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, path) = field_path(input)?;
    let (input, _) = ws1(input)?;
    let (input, _) = tag_no_case("RANGE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, (min, max)) = range(input)?;

    Ok((input, GuardRule::RangeCheck { path, min, max }))
}

/// Parse a LENGTH check rule.
/// Syntax: REQUIRE <path> LENGTH <min>..<max>
fn parse_length_check(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = tag_no_case("REQUIRE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, path) = field_path(input)?;
    let (input, _) = ws1(input)?;
    let (input, _) = tag_no_case("LENGTH")(input)?;
    let (input, _) = ws1(input)?;
    let (input, (min, max)) = range(input)?;

    // Convert f64 to usize for length checks
    let min_length = min.map(|v| v as usize);
    let max_length = max.map(|v| v as usize);

    Ok((
        input,
        GuardRule::StringLengthCheck {
            path,
            min_length,
            max_length,
        },
    ))
}

/// Parse a FIELD check rule.
/// Syntax: REQUIRE FIELD <name>
fn parse_required_field(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = tag_no_case("REQUIRE")(input)?;
    let (input, _) = ws1(input)?;
    let (input, _) = tag_no_case("FIELD")(input)?;
    let (input, _) = ws1(input)?;
    let (input, field_name) = identifier(input)?;

    Ok((input, GuardRule::RequiredField { field_name }))
}

/// Parse a state TRANSITION rule.
/// Syntax: ALLOW TRANSITION <from> -> <to>
fn parse_state_transition(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = tag_no_case("ALLOW")(input)?;
    let (input, _) = ws1(input)?;
    let (input, _) = tag_no_case("TRANSITION")(input)?;
    let (input, _) = ws1(input)?;
    let (input, from) = identifier(input)?;
    let (input, _) = ws0(input)?;
    let (input, _) = tag("->")(input)?;
    let (input, _) = ws0(input)?;
    let (input, to) = identifier(input)?;

    Ok((input, GuardRule::StateTransition { from, to }))
}

/// Parse a single guard rule from text.
fn parse_guard_rule(input: &str) -> IResult<&str, GuardRule> {
    let (input, _) = ws0(input)?;
    let (input, rule) = alt((
        parse_type_check,
        parse_enum_check,
        parse_range_check,
        parse_length_check,
        parse_required_field,
        parse_state_transition,
    ))(input)?;
    let (input, _) = ws0(input)?;
    Ok((input, rule))
}

/// Parse a guard definition from text and return a compiled Guard trait object.
///
/// # Arguments
///
/// * `input` - The DSL text to parse
///
/// # Returns
///
/// * `Ok(Box<dyn Guard>)` - A compiled guard ready to use
/// * `Err(DslError)` - Parsing or compilation error
///
/// # Examples
///
/// ```
/// use a2a_rs::construct::guards::dsl::parse_guard;
///
/// let guard = parse_guard("REQUIRE task.status IN [submitted, working]").unwrap();
/// ```
pub fn parse_guard(input: &str) -> Result<Box<dyn Guard>, DslError> {
    let (remaining, rule) = parse_guard_rule(input)
        .map_err(|e| DslError::ParseError(format!("Failed to parse guard rule: {:?}", e)))?;

    // Ensure we consumed all input
    if !remaining.trim().is_empty() {
        return Err(DslError::InvalidSyntax(format!(
            "Unexpected trailing input: '{}'",
            remaining
        )));
    }

    compile_guard(rule)
}

/// Helper trait for extracting values from JSON using field paths.
trait FieldExtractor {
    fn extract_field<'a>(&self, value: &'a serde_json::Value) -> Option<&'a serde_json::Value>;
}

impl FieldExtractor for String {
    fn extract_field<'a>(&self, value: &'a serde_json::Value) -> Option<&'a serde_json::Value> {
        let parts: Vec<&str> = self.split('.').collect();
        let mut current = value;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }
}

/// A wrapper guard that extracts a field value before checking.
#[derive(Debug)]
struct FieldPathGuard {
    path: String,
    inner_guard: Box<dyn Guard>,
}

impl FieldPathGuard {
    fn new(path: String, inner_guard: Box<dyn Guard>) -> Self {
        Self { path, inner_guard }
    }
}

impl Guard for FieldPathGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), super::RefusalReceipt> {
        // Extract the field value using the path
        let field_value = self.path.extract_field(input).ok_or_else(|| {
            super::RefusalReceipt::new(
                super::RefusalCode::MissingRequiredField,
                format!("FieldPathGuard({})", self.path),
                super::compute_input_hash(input),
                policy_epoch,
                format!("Field path '{}' not found", self.path),
            )
        })?;

        // Check the extracted field value
        self.inner_guard.check(
            field_value,
            &format!("{}.{}", context, self.path),
            policy_epoch,
        )
    }

    fn name(&self) -> &str {
        "FieldPathGuard"
    }

    fn description(&self) -> String {
        format!(
            "Extract {} and {}",
            self.path,
            self.inner_guard.description()
        )
    }
}

/// Compile a parsed guard rule into a Guard trait object.
fn compile_guard(rule: GuardRule) -> Result<Box<dyn Guard>, DslError> {
    match rule {
        GuardRule::TypeCheck {
            path,
            expected_type,
        } => {
            let type_guard = TypeGuard::new(expected_type.clone());
            if path.contains('.') {
                Ok(Box::new(FieldPathGuard::new(path, Box::new(type_guard))))
            } else {
                // For simple field names, we need to wrap in a field extractor too
                Ok(Box::new(FieldPathGuard::new(path, Box::new(type_guard))))
            }
        }

        GuardRule::EnumCheck {
            path,
            allowed_values,
        } => {
            let values: Vec<serde_json::Value> = allowed_values
                .into_iter()
                .map(serde_json::Value::String)
                .collect();
            let enum_guard = EnumGuard::new(values);
            Ok(Box::new(FieldPathGuard::new(path, Box::new(enum_guard))))
        }

        GuardRule::RangeCheck { path, min, max } => {
            let range_guard = RangeGuard::new(min, max);
            Ok(Box::new(FieldPathGuard::new(path, Box::new(range_guard))))
        }

        GuardRule::StringLengthCheck {
            path,
            min_length,
            max_length,
        } => {
            let length_guard = StringLengthGuard::new(min_length, max_length);
            Ok(Box::new(FieldPathGuard::new(path, Box::new(length_guard))))
        }

        GuardRule::RequiredField { field_name } => {
            Ok(Box::new(RequiredFieldGuard::new(field_name)))
        }

        GuardRule::StateTransition { from, to } => {
            // For state transitions, we create a single-transition guard
            let mut transitions = HashMap::new();
            transitions.insert(from.clone(), vec![to.clone()]);
            Ok(Box::new(StateTransitionGuard::new(transitions)))
        }
    }
}

/// Parse multiple guard rules from text (one per line) and return a vector of compiled guards.
///
/// Lines starting with '#' are treated as comments and ignored.
/// Empty lines are ignored.
///
/// # Examples
///
/// ```
/// use a2a_rs::construct::guards::dsl::parse_guards;
///
/// let rules = r#"
/// # Task status validation
/// REQUIRE task.status IN [submitted, working, completed]
/// REQUIRE task.name TYPE string
/// REQUIRE task.priority RANGE 1..10
/// "#;
///
/// let guards = parse_guards(rules).unwrap();
/// assert_eq!(guards.len(), 3);
/// ```
pub fn parse_guards(input: &str) -> Result<Vec<Box<dyn Guard>>, DslError> {
    let mut guards = Vec::new();

    for (line_num, line) in input.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let guard = parse_guard(trimmed)
            .map_err(|e| DslError::ParseError(format!("Line {}: {}", line_num + 1, e)))?;

        guards.push(guard);
    }

    Ok(guards)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type_check() {
        let guard = parse_guard("REQUIRE task.name TYPE string").unwrap();
        let valid = serde_json::json!({"task": {"name": "Test Task"}});
        let invalid = serde_json::json!({"task": {"name": 123}});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_parse_enum_check() {
        let guard = parse_guard("REQUIRE task.status IN [submitted, working, completed]").unwrap();
        let valid = serde_json::json!({"task": {"status": "working"}});
        let invalid = serde_json::json!({"task": {"status": "unknown"}});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_parse_range_check() {
        let guard = parse_guard("REQUIRE amount RANGE 0..100").unwrap();
        let valid = serde_json::json!({"amount": 50});
        let invalid = serde_json::json!({"amount": 150});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_parse_length_check() {
        let guard = parse_guard("REQUIRE name LENGTH 3..50").unwrap();
        let valid = serde_json::json!({"name": "Alice"});
        let invalid = serde_json::json!({"name": "AB"});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_parse_required_field() {
        let guard = parse_guard("REQUIRE FIELD status").unwrap();
        let valid = serde_json::json!({"status": "active"});
        let invalid = serde_json::json!({"name": "test"});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_parse_state_transition() {
        let guard = parse_guard("ALLOW TRANSITION draft -> published").unwrap();
        let valid = serde_json::json!({"from": "draft", "to": "published"});
        let invalid = serde_json::json!({"from": "draft", "to": "archived"});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_parse_multiple_guards() {
        let rules = r#"
        # Task validation rules
        REQUIRE task.status IN [submitted, working, completed]
        REQUIRE task.name TYPE string
        REQUIRE task.priority RANGE 1..10
        "#;

        let guards = parse_guards(rules).unwrap();
        assert_eq!(guards.len(), 3);
    }

    #[test]
    fn test_case_insensitive() {
        let guard1 = parse_guard("require task.name type string").unwrap();
        let guard2 = parse_guard("REQUIRE TASK.NAME TYPE STRING").unwrap();

        let input = serde_json::json!({"task": {"name": "Test"}});
        assert!(guard1.check(&input, "test", 1).is_ok());
        assert!(guard2.check(&input, "test", 1).is_ok());
    }

    #[test]
    fn test_nested_field_path() {
        let guard = parse_guard("REQUIRE task.metadata.priority TYPE number").unwrap();
        let valid = serde_json::json!({"task": {"metadata": {"priority": 5}}});
        let invalid = serde_json::json!({"task": {"metadata": {"priority": "high"}}});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_range_min_only() {
        let guard = parse_guard("REQUIRE age RANGE 18..").unwrap();
        let valid = serde_json::json!({"age": 25});
        let invalid = serde_json::json!({"age": 15});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_range_max_only() {
        let guard = parse_guard("REQUIRE age RANGE ..65").unwrap();
        let valid = serde_json::json!({"age": 30});
        let invalid = serde_json::json!({"age": 70});

        assert!(guard.check(&valid, "test", 1).is_ok());
        assert!(guard.check(&invalid, "test", 1).is_err());
    }

    #[test]
    fn test_invalid_syntax() {
        let result = parse_guard("INVALID SYNTAX HERE");
        assert!(result.is_err());
    }

    #[test]
    fn test_trailing_input() {
        let result = parse_guard("REQUIRE name TYPE string extra stuff");
        assert!(result.is_err());
        if let Err(DslError::InvalidSyntax(msg)) = result {
            assert!(msg.contains("trailing"));
        } else {
            panic!("Expected InvalidSyntax error");
        }
    }
}
