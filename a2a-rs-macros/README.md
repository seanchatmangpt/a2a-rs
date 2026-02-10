# a2a-rs-macros

Procedural macros for the a2a-rs CONSTRUCT system.

## Overview

This crate provides two procedural macros that reduce boilerplate when implementing stations and guards:

- `#[station]` - Auto-implements the Station trait
- `#[guard]` - Auto-generates a Guard from a predicate function

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
a2a-rs-macros = { path = "../a2a-rs-macros" }
a2a-rs = { path = "../a2a-rs", features = ["construct"] }
```

## Usage

### Station Macro

The `#[station]` macro auto-implements the `Station` trait from a struct with `admit` and `step` methods:

```rust
use a2a_rs_macros::station;
use a2a_rs::construct::station::{Station, RefusalReceipt};
use a2a_rs::construct::ontology::OntologyState;

#[station(method = "custom/greet")]
struct GreetStation;

impl GreetStation {
    fn admit(ontology: &OntologyState, input: &GreetInput) -> Result<(), RefusalReceipt> {
        if input.name.is_empty() {
            return Err(RefusalReceipt::new(
                -32602,
                "Name cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    fn step(
        &mut self,
        ontology: &mut OntologyState,
        input: GreetInput,
    ) -> Result<GreetOutput, RefusalReceipt> {
        Ok(GreetOutput {
            greeting: format!("Hello, {}!", input.name),
        })
    }
}

// Input/Output types
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GreetInput {
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GreetOutput {
    greeting: String,
}
```

### Guard Macro

The `#[guard]` macro generates a Guard struct from a validation function:

```rust
use a2a_rs_macros::guard;
use a2a_rs::construct::guards::{Guard, RefusalReceipt};

#[guard(name = "PositiveNumber", code = "ValueOutOfRange")]
fn check_positive(input: &serde_json::Value) -> Result<(), String> {
    match input.as_f64() {
        Some(n) if n > 0.0 => Ok(()),
        _ => Err("Number must be positive".to_string()),
    }
}

// Usage:
let guard = PositiveNumber;
guard.check(&serde_json::json!(42), "value", 1)?;
```

#### Guard Attributes

- `name` (required) - The guard name for audit trails
- `code` (optional) - The `RefusalCode` variant (defaults to `PreconditionViolation`)

Available refusal codes:
- `TypeMismatch`
- `MissingRequiredField`
- `InvalidEnumVariant`
- `ValueOutOfRange`
- `InvalidStringLength`
- `PatternMismatch`
- `InvalidFormat`
- `InvalidStateTransition`
- `PreconditionViolation`
- `PostconditionViolation`
- `ResourceNotFound`
- `ResourceAlreadyExists`
- And more... (see `construct::guards::RefusalCode`)

## Design Philosophy

These macros follow the CONSTRUCT principle:

- **Domain logic is hand-written** - You write the actual `admit` and `step` logic
- **Structural code is generated** - The macro generates the trait implementation boilerplate
- **Type safety is preserved** - Input and Output types are inferred from your method signatures
- **Determinism is maintained** - Generated code is pure delegation, no hidden behavior

## License

MIT OR Apache-2.0
