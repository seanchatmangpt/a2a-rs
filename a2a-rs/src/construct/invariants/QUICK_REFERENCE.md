# Invariants DSL - Quick Reference

## Basic Syntax

```rust
INVARIANT <expression>
```

## Operators

```text
Comparison:  ==  !=  <  <=  >  >=
Logical:     AND  OR  NOT
Grouping:    ( )
```

## Examples

```rust
// Simple comparison
INVARIANT count > 0
INVARIANT id == "task-123"

// Array length
INVARIANT artifacts.length <= 100

// Logical AND
INVARIANT x > 0 AND x < 100

// Logical OR
INVARIANT state == "done" OR state == "error"

// NOT operator
INVARIANT NOT disabled

// Complex with grouping
INVARIANT (x > 0 AND x < 10) OR x == 100

// Field access
INVARIANT status.state == "completed"
INVARIANT message.content.text == "hello"

// Special accessors
INVARIANT artifacts.length > 0      // .length for arrays
INVARIANT metadata.exists            // .exists for null check
```

## Usage

```rust
use a2a_rs::construct::invariants::{parse_invariant, InvariantRegistry};

// Parse
let inv = parse_invariant("INVARIANT artifacts.length <= 100")?;

// Evaluate
inv.evaluate(&task)?;

// Register
let mut registry = InvariantRegistry::new();
registry.register("artifact_limit", Box::new(inv));
registry.check_all(&task)?;
```

## Types

- **Numbers**: `42`, `-10`, `0` (i64)
- **Strings**: `"completed"`, `"task-123"` (quoted)
- **Booleans**: `true`, `false`

## Common Patterns

```rust
// Bounds checking
INVARIANT count >= 1 AND count <= 100

// State validation
INVARIANT state == "completed" OR state == "failed" OR state == "canceled"

// Required field
INVARIANT kind == "task"

// Non-empty check
INVARIANT history.length > 0

// Combined constraints
INVARIANT artifacts.length <= 100 AND history.length <= 1000 AND kind == "task"
```

## Error Handling

```rust
match parse_invariant("INVARIANT x > 0") {
    Ok(expr) => {
        match expr.evaluate(&value) {
            Ok(()) => println!("✓ Invariant holds"),
            Err(e) => println!("✗ Violation: {}", e),
        }
    }
    Err(e) => println!("✗ Parse error: {}", e),
}
```
