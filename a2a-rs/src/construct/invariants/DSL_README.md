# Invariants DSL

A domain-specific language for writing declarative invariants in the CONSTRUCT module.

## Overview

Instead of implementing the `Invariant` trait manually, you can write invariants using a simple expression language:

```rust
use a2a_rs::construct::invariants::{parse_invariant, InvariantRegistry};
use a2a_rs::domain::Task;

// Parse a declarative invariant
let expr = parse_invariant("INVARIANT task.artifacts.length <= 100").unwrap();

// Use it like any other invariant
let task = Task::new("task-1".to_string(), "ctx-1".to_string());
assert!(expr.evaluate(&task).is_ok());

// Or register it in a registry
let mut registry = InvariantRegistry::new();
registry.register("artifact_limit", Box::new(expr));
```

## Grammar

```ebnf
invariant    ::= "INVARIANT" expression
expression   ::= logical_or
logical_or   ::= logical_and ("OR" logical_and)*
logical_and  ::= comparison ("AND" comparison)*
comparison   ::= term (comp_op term)?
comp_op      ::= "==" | "!=" | "<=" | ">=" | "<" | ">"
term         ::= "NOT" term | "(" expression ")" | field_access | literal
field_access ::= identifier ("." identifier)*
literal      ::= number | string | boolean
identifier   ::= [a-zA-Z_][a-zA-Z0-9_]*
number       ::= "-"? [0-9]+
string       ::= "\"" [^"]* "\""
boolean      ::= "true" | "false"
```

## Operators

### Comparison Operators

| Operator | Description           | Example                    |
|----------|-----------------------|----------------------------|
| `==`     | Equal                 | `x == 42`                  |
| `!=`     | Not equal             | `status != "failed"`       |
| `<`      | Less than             | `count < 100`              |
| `<=`     | Less than or equal    | `artifacts.length <= 100`  |
| `>`      | Greater than          | `value > 0`                |
| `>=`     | Greater than or equal | `history.length >= 1`      |

### Logical Operators

| Operator | Description | Example                        |
|----------|-------------|--------------------------------|
| `AND`    | Logical AND | `x > 0 AND x < 100`            |
| `OR`     | Logical OR  | `state == "done" OR state == "error"` |
| `NOT`    | Logical NOT | `NOT disabled`                 |

### Field Access

Use dot notation to access nested fields:

```
task.id
task.status.state
message.content.text
```

### Special Accessors

| Accessor   | Description                  | Example                    |
|------------|------------------------------|----------------------------|
| `.length`  | Get array length             | `artifacts.length <= 100`  |
| `.exists`  | Check if value is not null   | `metadata.exists`          |

## Examples

### Simple Comparisons

```rust
// Numeric comparison
parse_invariant("INVARIANT count >= 10")

// String equality
parse_invariant(r#"INVARIANT id == "task-123""#)

// Field access
parse_invariant(r#"INVARIANT status.state == "completed""#)
```

### Array Length Checks

```rust
// Limit artifact count
parse_invariant("INVARIANT artifacts.length <= 100")

// Ensure history is not empty
parse_invariant("INVARIANT history.length > 0")

// Bounded range
parse_invariant("INVARIANT items.length >= 1 AND items.length <= 1000")
```

### Logical Combinations

```rust
// OR: either condition must be true
parse_invariant(r#"INVARIANT state == "completed" OR state == "failed""#)

// AND: both conditions must be true
parse_invariant("INVARIANT count > 0 AND count < 100")

// NOT: negation
parse_invariant("INVARIANT NOT disabled")

// Complex: combine with parentheses
parse_invariant("INVARIANT (x > 0 AND x < 10) OR x == 100")
```

### State Machine Invariants

```rust
// Task must be in valid terminal state when done
parse_invariant(
    r#"INVARIANT status.state == "completed" OR status.state == "failed" OR status.state == "canceled""#
)

// History must exist for completed tasks
parse_invariant(
    r#"INVARIANT status.state != "completed" OR history.length > 0"#
)
```

### Business Rule Invariants

```rust
// Artifact limits
parse_invariant("INVARIANT artifacts.length <= 100")

// History retention
parse_invariant("INVARIANT history.length <= 1000")

// Required fields
parse_invariant(r#"INVARIANT kind == "task""#)

// Combined rules
parse_invariant(
    r#"INVARIANT artifacts.length <= 100 AND history.length <= 1000 AND kind == "task""#
)
```

## Usage with InvariantRegistry

```rust
use a2a_rs::construct::invariants::{parse_invariant, InvariantRegistry};
use a2a_rs::domain::Task;

// Create registry
let mut registry = InvariantRegistry::new();

// Add DSL-based invariants
registry.register(
    "artifact_limit",
    Box::new(parse_invariant("INVARIANT artifacts.length <= 100").unwrap())
);

registry.register(
    "valid_kind",
    Box::new(parse_invariant(r#"INVARIANT kind == "task""#).unwrap())
);

registry.register(
    "terminal_state",
    Box::new(parse_invariant(
        r#"INVARIANT status.state == "completed" OR status.state == "failed""#
    ).unwrap())
);

// Check all invariants
let task = Task::new("task-1".to_string(), "ctx-1".to_string());
match registry.check_all(&task) {
    Ok(()) => println!("All invariants passed"),
    Err(e) => println!("Violation: {}", e),
}
```

## Implementation Details

### Evaluation

Invariants are evaluated by:

1. Serializing the value to JSON using `serde_json`
2. Traversing the JSON structure according to field paths
3. Evaluating comparisons and logical operators
4. Returning `Ok(())` if the expression evaluates to `true`, or an error otherwise

### Type System

The DSL supports three value types:

- **Numbers**: 64-bit signed integers (`i64`)
- **Strings**: UTF-8 strings
- **Booleans**: `true` or `false`

Comparisons are type-safe:
- Numbers can be compared with all operators
- Strings can be compared with all operators (lexicographic)
- Booleans can only use `==` and `!=`

### Error Handling

Parse errors:
```rust
match parse_invariant("INVALID") {
    Ok(expr) => { /* use expr */ },
    Err(e) => println!("Parse error: {}", e),
}
```

Evaluation errors:
```rust
let expr = parse_invariant("INVARIANT x > 0").unwrap();
match expr.evaluate(&value) {
    Ok(()) => println!("Invariant holds"),
    Err(e) => println!("Violation: {}", e),
}
```

## Performance Considerations

- **Parsing**: Parse once, evaluate many times. Cache parsed expressions.
- **Serialization**: Values are serialized to JSON for evaluation, which has some overhead.
- **Determinism**: All operations are deterministic and use `BTreeMap` for stable ordering.

## Limitations

- **No floating point**: Only integer arithmetic is supported
- **No functions**: Cannot call custom functions or methods
- **No mutation**: Expressions are pure and cannot modify state
- **No regex**: String matching is exact equality only

## Future Enhancements

Potential extensions to the DSL:

- Floating-point support
- Regular expression matching
- Custom function calls
- Quantifiers (forall, exists)
- Set operations
- Date/time comparisons
