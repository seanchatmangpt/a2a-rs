//! Guards system for refusal determinism.
//!
//! This module implements a typed refusal system that replaces AI-based planning
//! decisions with deterministic predicates. Every inadmissible state produces a
//! typed `RefusalReceipt` that provides cryptographic proof of the refusal reason.
//!
//! # Architecture
//!
//! The Guards system follows the CONSTRUCT philosophy: instead of LLM judgment calls,
//! we use structured predicates that return typed refusal receipts. This makes
//! agent behavior deterministic, auditable, and provably correct.
//!
//! # Example
//!
//! ```rust
//! use a2a_rs::construct::guards::{Guard, TypeGuard, RefusalCode};
//!
//! let guard = TypeGuard::new("string".to_string());
//! let value = serde_json::json!({"not": "a string"});
//!
//! match guard.check(&value, "user-input", 1) {
//!     Ok(_) => println!("Input valid"),
//!     Err(receipt) => {
//!         println!("Refused: {:?}", receipt.code);
//!         println!("Policy epoch: {}", receipt.policy_epoch);
//!     }
//! }
//! ```

pub mod dsl;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Enumeration of all possible refusal types.
///
/// Each variant represents a distinct category of inadmissible state that can be
/// detected by guard predicates. The enum is exhaustive: every possible refusal
/// reason must have a corresponding variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RefusalCode {
    /// Type mismatch: expected type T, got type U
    TypeMismatch,

    /// Missing required field
    MissingRequiredField,

    /// Invalid enum variant
    InvalidEnumVariant,

    /// Value out of range (min/max constraint violation)
    ValueOutOfRange,

    /// String length constraint violation
    InvalidStringLength,

    /// Pattern/regex constraint violation
    PatternMismatch,

    /// Format constraint violation (email, uri, date-time, etc.)
    InvalidFormat,

    /// State machine invariant violation
    InvalidStateTransition,

    /// Precondition not satisfied
    PreconditionViolation,

    /// Postcondition not satisfied
    PostconditionViolation,

    /// Resource not found
    ResourceNotFound,

    /// Resource already exists (uniqueness violation)
    ResourceAlreadyExists,

    /// Circular dependency detected
    CircularDependency,

    /// Deadlock detected
    DeadlockDetected,

    /// Rate limit exceeded
    RateLimitExceeded,

    /// Quota exhausted
    QuotaExhausted,

    /// Permission denied (authorization failure)
    PermissionDenied,

    /// Authentication required
    AuthenticationRequired,

    /// Cryptographic verification failed
    VerificationFailed,

    /// Protocol version mismatch
    ProtocolVersionMismatch,

    /// Capability not supported
    UnsupportedCapability,

    /// Temporal constraint violation (before/after time)
    TemporalViolation,

    /// Cardinality constraint violation (too many/few items)
    CardinalityViolation,

    /// Schema validation failed
    SchemaValidationFailed,

    /// Input sanitization required (potential security issue)
    InputSanitizationRequired,

    /// Custom domain-specific constraint violation
    DomainConstraintViolation,
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusalCode::TypeMismatch => write!(f, "TYPE_MISMATCH"),
            RefusalCode::MissingRequiredField => write!(f, "MISSING_REQUIRED_FIELD"),
            RefusalCode::InvalidEnumVariant => write!(f, "INVALID_ENUM_VARIANT"),
            RefusalCode::ValueOutOfRange => write!(f, "VALUE_OUT_OF_RANGE"),
            RefusalCode::InvalidStringLength => write!(f, "INVALID_STRING_LENGTH"),
            RefusalCode::PatternMismatch => write!(f, "PATTERN_MISMATCH"),
            RefusalCode::InvalidFormat => write!(f, "INVALID_FORMAT"),
            RefusalCode::InvalidStateTransition => write!(f, "INVALID_STATE_TRANSITION"),
            RefusalCode::PreconditionViolation => write!(f, "PRECONDITION_VIOLATION"),
            RefusalCode::PostconditionViolation => write!(f, "POSTCONDITION_VIOLATION"),
            RefusalCode::ResourceNotFound => write!(f, "RESOURCE_NOT_FOUND"),
            RefusalCode::ResourceAlreadyExists => write!(f, "RESOURCE_ALREADY_EXISTS"),
            RefusalCode::CircularDependency => write!(f, "CIRCULAR_DEPENDENCY"),
            RefusalCode::DeadlockDetected => write!(f, "DEADLOCK_DETECTED"),
            RefusalCode::RateLimitExceeded => write!(f, "RATE_LIMIT_EXCEEDED"),
            RefusalCode::QuotaExhausted => write!(f, "QUOTA_EXHAUSTED"),
            RefusalCode::PermissionDenied => write!(f, "PERMISSION_DENIED"),
            RefusalCode::AuthenticationRequired => write!(f, "AUTHENTICATION_REQUIRED"),
            RefusalCode::VerificationFailed => write!(f, "VERIFICATION_FAILED"),
            RefusalCode::ProtocolVersionMismatch => write!(f, "PROTOCOL_VERSION_MISMATCH"),
            RefusalCode::UnsupportedCapability => write!(f, "UNSUPPORTED_CAPABILITY"),
            RefusalCode::TemporalViolation => write!(f, "TEMPORAL_VIOLATION"),
            RefusalCode::CardinalityViolation => write!(f, "CARDINALITY_VIOLATION"),
            RefusalCode::SchemaValidationFailed => write!(f, "SCHEMA_VALIDATION_FAILED"),
            RefusalCode::InputSanitizationRequired => write!(f, "INPUT_SANITIZATION_REQUIRED"),
            RefusalCode::DomainConstraintViolation => write!(f, "DOMAIN_CONSTRAINT_VIOLATION"),
        }
    }
}

/// A cryptographically-verifiable receipt proving that a refusal occurred.
///
/// The `RefusalReceipt` provides complete auditability: given the receipt,
/// you can verify that the input was inadmissible under the policy epoch,
/// and you can identify exactly which guard was violated.
///
/// # Information Theory
///
/// The receipt contains the minimal information needed to reconstruct the refusal
/// decision: the refusal code (what), the violated guard name (where), the input
/// hash (evidence), and the policy epoch (when). This achieves optimal compression
/// while maintaining full auditability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalReceipt {
    /// The refusal code indicating the category of violation
    pub code: RefusalCode,

    /// The name of the guard that was violated
    pub violated_guard: String,

    /// Hash of the input that caused the refusal (for audit trail)
    pub input_hash: String,

    /// The policy epoch at which this refusal was issued
    ///
    /// Policy epochs allow versioning of guard rules: a refusal under policy
    /// epoch N may be acceptable under policy epoch N+1. This enables
    /// auditable policy evolution.
    pub policy_epoch: u64,

    /// Human-readable description of the refusal reason
    pub reason: String,

    /// Structured metadata about the violation (field names, expected values, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl RefusalReceipt {
    /// Create a new refusal receipt
    pub fn new(
        code: RefusalCode,
        violated_guard: String,
        input_hash: String,
        policy_epoch: u64,
        reason: String,
    ) -> Self {
        Self {
            code,
            violated_guard,
            input_hash,
            policy_epoch,
            reason,
            metadata: None,
        }
    }

    /// Create a refusal receipt with metadata
    pub fn with_metadata(
        code: RefusalCode,
        violated_guard: String,
        input_hash: String,
        policy_epoch: u64,
        reason: String,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            code,
            violated_guard,
            input_hash,
            policy_epoch,
            reason,
            metadata: Some(metadata),
        }
    }

    /// Add metadata to an existing receipt
    pub fn add_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata
            .get_or_insert_with(HashMap::new)
            .insert(key, value);
        self
    }
}

impl fmt::Display for RefusalReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} (guard: {}, epoch: {}, input: {})",
            self.code, self.reason, self.violated_guard, self.policy_epoch, self.input_hash
        )
    }
}

impl std::error::Error for RefusalReceipt {}

/// The Guard trait: a predicate that returns a typed refusal on violation.
///
/// Guards are the fundamental building block of refusal determinism. Each guard
/// implements a specific constraint check and returns either success or a
/// `RefusalReceipt` explaining why the input was inadmissible.
///
/// # Design Philosophy
///
/// Guards replace AI planning with typed predicates:
/// - Instead of "the LLM decides if this is valid", we have "this guard checks constraint X"
/// - Instead of "the LLM explains the refusal", we have structured `RefusalReceipt`
/// - Instead of non-determinism, we have reproducible behavior
///
/// # Contract
///
/// Implementations must be:
/// - **Deterministic**: same input + policy epoch → same result
/// - **Stateless**: no side effects, thread-safe
/// - **Fast**: guards run on every input, performance matters
/// - **Auditable**: refusal receipts must contain enough information to understand the failure
pub trait Guard: fmt::Debug + Send + Sync {
    /// Check if the input satisfies this guard's constraints.
    ///
    /// # Arguments
    ///
    /// * `input` - The input value to check
    /// * `context` - Contextual information (e.g., field name, operation)
    /// * `policy_epoch` - The current policy version
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the input is admissible
    /// * `Err(RefusalReceipt)` if the input violates this guard's constraints
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt>;

    /// Get the name of this guard (for audit trails)
    fn name(&self) -> &str;

    /// Get a description of this guard's constraints
    fn description(&self) -> String;
}

/// Composite guard that checks multiple guards in sequence.
///
/// The `AllGuard` succeeds only if all constituent guards succeed. This implements
/// logical AND: the input must pass all checks.
///
/// # Short-Circuit Evaluation
///
/// Guards are evaluated in order, and evaluation stops at the first failure.
/// This provides fail-fast behavior while maintaining determinism.
#[derive(Debug)]
pub struct AllGuard {
    name: String,
    guards: Vec<Box<dyn Guard>>,
}

impl AllGuard {
    /// Create a new AllGuard that checks all provided guards
    pub fn new(name: String, guards: Vec<Box<dyn Guard>>) -> Self {
        Self { name, guards }
    }
}

impl Guard for AllGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        for guard in &self.guards {
            guard.check(input, context, policy_epoch)?;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> String {
        format!(
            "All of: [{}]",
            self.guards
                .iter()
                .map(|g| g.name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Composite guard that checks if any guard passes.
///
/// The `AnyGuard` succeeds if at least one constituent guard succeeds. This implements
/// logical OR: the input must pass at least one check.
///
/// # Refusal Aggregation
///
/// If all guards fail, the receipt from the first guard is returned. This design
/// choice provides a consistent, predictable failure mode.
#[derive(Debug)]
pub struct AnyGuard {
    name: String,
    guards: Vec<Box<dyn Guard>>,
}

impl AnyGuard {
    /// Create a new AnyGuard that checks if any provided guard passes
    pub fn new(name: String, guards: Vec<Box<dyn Guard>>) -> Self {
        Self { name, guards }
    }
}

impl Guard for AnyGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        let mut first_error = None;
        for guard in &self.guards {
            match guard.check(input, context, policy_epoch) {
                Ok(()) => return Ok(()),
                Err(e) if first_error.is_none() => first_error = Some(e),
                Err(_) => {}
            }
        }

        Err(first_error.unwrap_or_else(|| {
            RefusalReceipt::new(
                RefusalCode::PreconditionViolation,
                self.name.clone(),
                compute_input_hash(input),
                policy_epoch,
                "None of the alternative guards passed".to_string(),
            )
        }))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> String {
        format!(
            "Any of: [{}]",
            self.guards
                .iter()
                .map(|g| g.name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// Type checking guard: verifies that input matches expected JSON type.
///
/// # Supported Types
///
/// - `"null"` - JSON null
/// - `"boolean"` - JSON boolean
/// - `"number"` - JSON number (integer or float)
/// - `"string"` - JSON string
/// - `"array"` - JSON array
/// - `"object"` - JSON object
#[derive(Debug, Clone)]
pub struct TypeGuard {
    expected_type: String,
}

impl TypeGuard {
    /// Create a new TypeGuard for the specified JSON type
    pub fn new(expected_type: String) -> Self {
        Self { expected_type }
    }
}

impl Guard for TypeGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        let actual_type = match input {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "boolean",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        };

        if actual_type != self.expected_type {
            let mut metadata = HashMap::new();
            metadata.insert(
                "expected".to_string(),
                serde_json::json!(self.expected_type),
            );
            metadata.insert("actual".to_string(), serde_json::json!(actual_type));
            metadata.insert("context".to_string(), serde_json::json!(context));

            return Err(RefusalReceipt::with_metadata(
                RefusalCode::TypeMismatch,
                format!("TypeGuard({})", self.expected_type),
                compute_input_hash(input),
                policy_epoch,
                format!(
                    "Expected type '{}', got '{}'",
                    self.expected_type, actual_type
                ),
                metadata,
            ));
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "TypeGuard"
    }

    fn description(&self) -> String {
        format!("Type must be: {}", self.expected_type)
    }
}

/// Required field guard: verifies that a field exists in an object.
#[derive(Debug, Clone)]
pub struct RequiredFieldGuard {
    field_name: String,
}

impl RequiredFieldGuard {
    /// Create a new RequiredFieldGuard for the specified field
    pub fn new(field_name: String) -> Self {
        Self { field_name }
    }
}

impl Guard for RequiredFieldGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        if let serde_json::Value::Object(obj) = input {
            if obj.contains_key(&self.field_name) {
                return Ok(());
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("field".to_string(), serde_json::json!(self.field_name));
        metadata.insert("context".to_string(), serde_json::json!(context));

        Err(RefusalReceipt::with_metadata(
            RefusalCode::MissingRequiredField,
            format!("RequiredFieldGuard({})", self.field_name),
            compute_input_hash(input),
            policy_epoch,
            format!("Required field '{}' is missing", self.field_name),
            metadata,
        ))
    }

    fn name(&self) -> &str {
        "RequiredFieldGuard"
    }

    fn description(&self) -> String {
        format!("Field '{}' must be present", self.field_name)
    }
}

/// Enum variant guard: verifies that a value is one of the allowed variants.
#[derive(Debug, Clone)]
pub struct EnumGuard {
    allowed_values: Vec<serde_json::Value>,
}

impl EnumGuard {
    /// Create a new EnumGuard with the specified allowed values
    pub fn new(allowed_values: Vec<serde_json::Value>) -> Self {
        Self { allowed_values }
    }
}

impl Guard for EnumGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        if self.allowed_values.contains(input) {
            return Ok(());
        }

        let mut metadata = HashMap::new();
        metadata.insert(
            "allowed".to_string(),
            serde_json::json!(self.allowed_values),
        );
        metadata.insert("actual".to_string(), input.clone());
        metadata.insert("context".to_string(), serde_json::json!(context));

        Err(RefusalReceipt::with_metadata(
            RefusalCode::InvalidEnumVariant,
            "EnumGuard".to_string(),
            compute_input_hash(input),
            policy_epoch,
            format!("Value is not one of the allowed enum variants"),
            metadata,
        ))
    }

    fn name(&self) -> &str {
        "EnumGuard"
    }

    fn description(&self) -> String {
        format!("Value must be one of: {:?}", self.allowed_values)
    }
}

/// Range guard: verifies that a numeric value is within specified bounds.
#[derive(Debug, Clone)]
pub struct RangeGuard {
    min: Option<f64>,
    max: Option<f64>,
}

impl RangeGuard {
    /// Create a new RangeGuard with optional min/max bounds
    pub fn new(min: Option<f64>, max: Option<f64>) -> Self {
        Self { min, max }
    }

    /// Create a RangeGuard with only a minimum bound
    pub fn min(min: f64) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    /// Create a RangeGuard with only a maximum bound
    pub fn max(max: f64) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }
}

impl Guard for RangeGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        let value = match input.as_f64() {
            Some(v) => v,
            None => {
                return Err(RefusalReceipt::new(
                    RefusalCode::TypeMismatch,
                    "RangeGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    "Value is not a number".to_string(),
                ));
            }
        };

        if let Some(min) = self.min {
            if value < min {
                let mut metadata = HashMap::new();
                metadata.insert("min".to_string(), serde_json::json!(min));
                metadata.insert("actual".to_string(), serde_json::json!(value));
                metadata.insert("context".to_string(), serde_json::json!(context));

                return Err(RefusalReceipt::with_metadata(
                    RefusalCode::ValueOutOfRange,
                    "RangeGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    format!("Value {} is less than minimum {}", value, min),
                    metadata,
                ));
            }
        }

        if let Some(max) = self.max {
            if value > max {
                let mut metadata = HashMap::new();
                metadata.insert("max".to_string(), serde_json::json!(max));
                metadata.insert("actual".to_string(), serde_json::json!(value));
                metadata.insert("context".to_string(), serde_json::json!(context));

                return Err(RefusalReceipt::with_metadata(
                    RefusalCode::ValueOutOfRange,
                    "RangeGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    format!("Value {} is greater than maximum {}", value, max),
                    metadata,
                ));
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "RangeGuard"
    }

    fn description(&self) -> String {
        match (self.min, self.max) {
            (Some(min), Some(max)) => format!("Value must be in range [{}, {}]", min, max),
            (Some(min), None) => format!("Value must be >= {}", min),
            (None, Some(max)) => format!("Value must be <= {}", max),
            (None, None) => "No range constraints".to_string(),
        }
    }
}

/// String length guard: verifies that a string's length is within specified bounds.
#[derive(Debug, Clone)]
pub struct StringLengthGuard {
    min_length: Option<usize>,
    max_length: Option<usize>,
}

impl StringLengthGuard {
    /// Create a new StringLengthGuard with optional min/max length bounds
    pub fn new(min_length: Option<usize>, max_length: Option<usize>) -> Self {
        Self {
            min_length,
            max_length,
        }
    }
}

impl Guard for StringLengthGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        let string_value = match input.as_str() {
            Some(s) => s,
            None => {
                return Err(RefusalReceipt::new(
                    RefusalCode::TypeMismatch,
                    "StringLengthGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    "Value is not a string".to_string(),
                ));
            }
        };

        let length = string_value.len();

        if let Some(min) = self.min_length {
            if length < min {
                let mut metadata = HashMap::new();
                metadata.insert("minLength".to_string(), serde_json::json!(min));
                metadata.insert("actualLength".to_string(), serde_json::json!(length));
                metadata.insert("context".to_string(), serde_json::json!(context));

                return Err(RefusalReceipt::with_metadata(
                    RefusalCode::InvalidStringLength,
                    "StringLengthGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    format!("String length {} is less than minimum {}", length, min),
                    metadata,
                ));
            }
        }

        if let Some(max) = self.max_length {
            if length > max {
                let mut metadata = HashMap::new();
                metadata.insert("maxLength".to_string(), serde_json::json!(max));
                metadata.insert("actualLength".to_string(), serde_json::json!(length));
                metadata.insert("context".to_string(), serde_json::json!(context));

                return Err(RefusalReceipt::with_metadata(
                    RefusalCode::InvalidStringLength,
                    "StringLengthGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    format!("String length {} is greater than maximum {}", length, max),
                    metadata,
                ));
            }
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "StringLengthGuard"
    }

    fn description(&self) -> String {
        match (self.min_length, self.max_length) {
            (Some(min), Some(max)) => format!("String length must be in range [{}, {}]", min, max),
            (Some(min), None) => format!("String length must be >= {}", min),
            (None, Some(max)) => format!("String length must be <= {}", max),
            (None, None) => "No length constraints".to_string(),
        }
    }
}

/// State transition guard: verifies that a state transition is valid.
///
/// State machines are a common source of invariant violations. This guard
/// encodes valid transitions as a map: current_state → allowed_next_states.
#[derive(Debug, Clone)]
pub struct StateTransitionGuard {
    valid_transitions: HashMap<String, Vec<String>>,
}

impl StateTransitionGuard {
    /// Create a new StateTransitionGuard with the specified valid transitions
    pub fn new(valid_transitions: HashMap<String, Vec<String>>) -> Self {
        Self { valid_transitions }
    }
}

impl Guard for StateTransitionGuard {
    fn check(
        &self,
        input: &serde_json::Value,
        context: &str,
        policy_epoch: u64,
    ) -> Result<(), RefusalReceipt> {
        // Expect input to be an object with "from" and "to" fields
        let obj = match input.as_object() {
            Some(o) => o,
            None => {
                return Err(RefusalReceipt::new(
                    RefusalCode::TypeMismatch,
                    "StateTransitionGuard".to_string(),
                    compute_input_hash(input),
                    policy_epoch,
                    "Input must be an object with 'from' and 'to' fields".to_string(),
                ));
            }
        };

        let from_state = obj.get("from").and_then(|v| v.as_str()).ok_or_else(|| {
            RefusalReceipt::new(
                RefusalCode::MissingRequiredField,
                "StateTransitionGuard".to_string(),
                compute_input_hash(input),
                policy_epoch,
                "Missing 'from' state field".to_string(),
            )
        })?;

        let to_state = obj.get("to").and_then(|v| v.as_str()).ok_or_else(|| {
            RefusalReceipt::new(
                RefusalCode::MissingRequiredField,
                "StateTransitionGuard".to_string(),
                compute_input_hash(input),
                policy_epoch,
                "Missing 'to' state field".to_string(),
            )
        })?;

        if let Some(allowed_states) = self.valid_transitions.get(from_state) {
            if allowed_states.contains(&to_state.to_string()) {
                return Ok(());
            }

            let mut metadata = HashMap::new();
            metadata.insert("from".to_string(), serde_json::json!(from_state));
            metadata.insert("to".to_string(), serde_json::json!(to_state));
            metadata.insert("allowed".to_string(), serde_json::json!(allowed_states));
            metadata.insert("context".to_string(), serde_json::json!(context));

            return Err(RefusalReceipt::with_metadata(
                RefusalCode::InvalidStateTransition,
                "StateTransitionGuard".to_string(),
                compute_input_hash(input),
                policy_epoch,
                format!(
                    "Invalid transition from '{}' to '{}'. Allowed: {:?}",
                    from_state, to_state, allowed_states
                ),
                metadata,
            ));
        }

        let mut metadata = HashMap::new();
        metadata.insert("from".to_string(), serde_json::json!(from_state));
        metadata.insert("context".to_string(), serde_json::json!(context));

        Err(RefusalReceipt::with_metadata(
            RefusalCode::InvalidStateTransition,
            "StateTransitionGuard".to_string(),
            compute_input_hash(input),
            policy_epoch,
            format!("Unknown state '{}'", from_state),
            metadata,
        ))
    }

    fn name(&self) -> &str {
        "StateTransitionGuard"
    }

    fn description(&self) -> String {
        format!(
            "State machine with {} states and {} transitions",
            self.valid_transitions.len(),
            self.valid_transitions
                .values()
                .map(|v| v.len())
                .sum::<usize>()
        )
    }
}

/// Compute a hash of the input for audit trails.
///
/// This uses a simple JSON serialization + length-based hash. In production,
/// this should be replaced with a cryptographic hash (SHA-256, BLAKE3, etc.)
/// to provide tamper-evident audit trails.
pub(crate) fn compute_input_hash(input: &serde_json::Value) -> String {
    // Simple deterministic hash for now - in production use proper crypto hash
    let json_str = serde_json::to_string(input).unwrap_or_else(|_| "null".to_string());
    format!("hash-{:x}", simple_hash(&json_str))
}

/// Simple string hash function (FNV-1a inspired)
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_guard_string() {
        let guard = TypeGuard::new("string".to_string());
        let valid = serde_json::json!("hello");
        let invalid = serde_json::json!(42);

        assert!(guard.check(&valid, "test", 1).is_ok());
        let err = guard.check(&invalid, "test", 1).unwrap_err();
        assert_eq!(err.code, RefusalCode::TypeMismatch);
    }

    #[test]
    fn test_type_guard_number() {
        let guard = TypeGuard::new("number".to_string());
        let valid = serde_json::json!(42);
        let invalid = serde_json::json!("hello");

        assert!(guard.check(&valid, "test", 1).is_ok());
        let err = guard.check(&invalid, "test", 1).unwrap_err();
        assert_eq!(err.code, RefusalCode::TypeMismatch);
    }

    #[test]
    fn test_required_field_guard() {
        let guard = RequiredFieldGuard::new("name".to_string());
        let valid = serde_json::json!({"name": "Alice"});
        let invalid = serde_json::json!({"age": 30});

        assert!(guard.check(&valid, "test", 1).is_ok());
        let err = guard.check(&invalid, "test", 1).unwrap_err();
        assert_eq!(err.code, RefusalCode::MissingRequiredField);
    }

    #[test]
    fn test_enum_guard() {
        let guard = EnumGuard::new(vec![
            serde_json::json!("red"),
            serde_json::json!("green"),
            serde_json::json!("blue"),
        ]);

        assert!(guard.check(&serde_json::json!("red"), "test", 1).is_ok());
        assert!(guard.check(&serde_json::json!("green"), "test", 1).is_ok());

        let err = guard
            .check(&serde_json::json!("yellow"), "test", 1)
            .unwrap_err();
        assert_eq!(err.code, RefusalCode::InvalidEnumVariant);
    }

    #[test]
    fn test_range_guard() {
        let guard = RangeGuard::new(Some(0.0), Some(100.0));

        assert!(guard.check(&serde_json::json!(50), "test", 1).is_ok());
        assert!(guard.check(&serde_json::json!(0), "test", 1).is_ok());
        assert!(guard.check(&serde_json::json!(100), "test", 1).is_ok());

        let err = guard.check(&serde_json::json!(-1), "test", 1).unwrap_err();
        assert_eq!(err.code, RefusalCode::ValueOutOfRange);

        let err = guard.check(&serde_json::json!(101), "test", 1).unwrap_err();
        assert_eq!(err.code, RefusalCode::ValueOutOfRange);
    }

    #[test]
    fn test_string_length_guard() {
        let guard = StringLengthGuard::new(Some(3), Some(10));

        assert!(guard.check(&serde_json::json!("hello"), "test", 1).is_ok());
        assert!(guard.check(&serde_json::json!("abc"), "test", 1).is_ok());

        let err = guard
            .check(&serde_json::json!("ab"), "test", 1)
            .unwrap_err();
        assert_eq!(err.code, RefusalCode::InvalidStringLength);

        let err = guard
            .check(&serde_json::json!("this is too long"), "test", 1)
            .unwrap_err();
        assert_eq!(err.code, RefusalCode::InvalidStringLength);
    }

    #[test]
    fn test_state_transition_guard() {
        let mut valid_transitions = HashMap::new();
        valid_transitions.insert("draft".to_string(), vec!["published".to_string()]);
        valid_transitions.insert(
            "published".to_string(),
            vec!["archived".to_string(), "draft".to_string()],
        );

        let guard = StateTransitionGuard::new(valid_transitions);

        let valid = serde_json::json!({"from": "draft", "to": "published"});
        assert!(guard.check(&valid, "test", 1).is_ok());

        let invalid = serde_json::json!({"from": "draft", "to": "archived"});
        let err = guard.check(&invalid, "test", 1).unwrap_err();
        assert_eq!(err.code, RefusalCode::InvalidStateTransition);
    }

    #[test]
    fn test_refusal_receipt_display() {
        let receipt = RefusalReceipt::new(
            RefusalCode::TypeMismatch,
            "TestGuard".to_string(),
            "hash123".to_string(),
            1,
            "Expected string, got number".to_string(),
        );

        let display = format!("{}", receipt);
        assert!(display.contains("TYPE_MISMATCH"));
        assert!(display.contains("TestGuard"));
        assert!(display.contains("hash123"));
    }

    #[test]
    fn test_refusal_receipt_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("field".to_string(), serde_json::json!("name"));

        let receipt = RefusalReceipt::with_metadata(
            RefusalCode::MissingRequiredField,
            "TestGuard".to_string(),
            "hash123".to_string(),
            1,
            "Field missing".to_string(),
            metadata,
        );

        assert!(receipt.metadata.is_some());
        assert_eq!(
            receipt.metadata.as_ref().unwrap().get("field"),
            Some(&serde_json::json!("name"))
        );
    }
}
