# Mental Model Shift: From Code-First to Ontology-First

**Status**: Phase 1, P0 Priority
**Target Audience**: Developers new to RDF/ontology concepts
**Prerequisites**: Basic understanding of Rust and data modeling
**Related Docs**: [Ontology Basics](./ONTOLOGY-BASICS.md) | [CONSTRUCT Pipeline](./CONSTRUCT-PIPELINE.md) | [RDF for Developers](./RDF-FOR-DEVELOPERS.md)

---

## Table of Contents

1. [Overview](#overview)
2. [The Fundamental Shift](#the-fundamental-shift)
3. [Side-by-Side Comparison](#side-by-side-comparison)
4. [Common Mental Blockers](#common-mental-blockers)
5. [Visual Model](#visual-model)
6. [Key Principles](#key-principles)
7. [Practical Implications](#practical-implications)
8. [Next Steps](#next-steps)

---

## Overview

If you're a developer coming to the a2a-rs project, you may notice something unusual: instead of defining types directly in Rust code, we define them in RDF ontology files (`.ttl` files), then use SPARQL CONSTRUCT queries to generate the Rust code. This isn't just a different workflow—it represents a **fundamental shift in how we think about types and their relationships**.

This document explains that shift and helps you develop the mental model needed to work effectively with ontology-driven development.

### What This Document Is NOT

This is not a tutorial on RDF syntax or SPARQL queries. Those are covered in other foundational documents. Instead, this focuses on the **conceptual leap** from traditional code-first thinking to ontology-first thinking.

---

## The Fundamental Shift

### Code-First Thinking (Traditional)

In traditional software development, you think in this sequence:

1. **Understand the requirements**: "I need an agent with a name, description, and capabilities"
2. **Write the code**: Define a struct with those fields
3. **Write tests**: Ensure the struct behaves correctly
4. **Write documentation**: Explain what the struct does
5. **Handle validation**: Add validation logic
6. **Maintain consistency**: Manually keep JSON schemas, API docs, and code in sync

**Your source of truth**: The code itself

### Ontology-First Thinking (Our Approach)

With ontology-driven development, you think differently:

1. **Model the domain**: "What is an Agent? What are its essential properties?"
2. **Express relationships**: "An Agent has Skills. Skills have required parameters."
3. **Capture constraints**: "A URL must be valid. A name must be between 1-200 characters."
4. **Define transformations**: "How should this ontology map to Rust? To JSON? To documentation?"
5. **Generate artifacts**: Run the generator to produce code, tests, schemas, and docs

**Your source of truth**: The ontology (`.ttl` files)

### Why This Matters

The ontology-first approach makes explicit what code-first approaches leave implicit:

- **Relationships between types** are first-class citizens, not just comments or conventions
- **Constraints and validation rules** are defined once, enforced everywhere
- **Multiple representations** (Rust, JSON Schema, documentation) are guaranteed consistent
- **Evolution and versioning** are trackable through semantic relationships
- **Cross-language consistency** emerges naturally from shared semantics

---

## Side-by-Side Comparison

Let's examine how the `AgentCard` type is defined in both approaches.

### Example 1: Defining AgentCard

#### Code-First Approach

```rust
// src/domain/core/agent.rs

use serde::{Deserialize, Serialize};
use bon::Builder;

/// The AgentCard is a self-describing manifest for an agent.
/// It provides essential metadata including the agent's identity,
/// capabilities, skills, and communication methods.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// A human-readable name for the agent
    pub name: String,

    /// A human-readable description of the agent
    pub description: String,

    /// The preferred endpoint URL for interacting with the agent
    pub url: String,

    /// The agent's own version number
    pub version: String,

    /// The version of the A2A protocol this agent supports
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// A declaration of optional capabilities supported by the agent
    pub capabilities: AgentCapabilities,

    /// The set of skills that the agent can perform
    pub skills: Vec<AgentSkill>,

    // ... more fields ...
}

fn default_protocol_version() -> String {
    "0.3.0".to_string()
}

// Separately: write JSON Schema validation
// Separately: write API documentation
// Separately: ensure spec compliance
// Separately: maintain these in sync across changes
```

**What you're thinking**: "I need these fields. Let me write a struct."

**What's implicit**:
- Why does `capabilities` reference `AgentCapabilities`?
- What's the relationship between `AgentCard` and `AgentSkill`?
- Why is `protocol_version` defaulted to "0.3.0"?
- How do I ensure this matches the JSON Schema spec?

#### Ontology-First Approach

```turtle
# ggen/ontology/a2a-agent.ttl

@prefix a2a: <https://ggen.io/ontology/a2a/> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

# Define what an AgentCard IS
a2a:AgentCard a a2a:Entity ;
    a2a:name "AgentCard" ;
    rdfs:comment "The AgentCard is a self-describing manifest for an agent. It provides essential metadata including the agent's identity, capabilities, skills, supported communication methods, and security requirements." ;
    a2a:hasProperty a2a:AgentCard_name ;
    a2a:hasProperty a2a:AgentCard_description ;
    a2a:hasProperty a2a:AgentCard_url ;
    a2a:hasProperty a2a:AgentCard_version ;
    a2a:hasProperty a2a:AgentCard_protocolVersion ;
    a2a:hasProperty a2a:AgentCard_capabilities ;
    a2a:hasProperty a2a:AgentCard_skills .

# Define what each property MEANS
a2a:AgentCard_name a a2a:Property ;
    a2a:name "name" ;
    a2a:type xsd:string ;
    a2a:required true ;
    rdfs:comment "A human-readable name for the agent." ;
    a2a:example "Recipe Agent" .

a2a:AgentCard_capabilities a a2a:Property ;
    a2a:name "capabilities" ;
    a2a:type a2a:AgentCapabilities ;  # Semantic relationship!
    a2a:required true ;
    rdfs:comment "A declaration of optional capabilities supported by the agent." .

a2a:AgentCard_skills a a2a:Property ;
    a2a:name "skills" ;
    a2a:type a2a:AgentSkill ;  # Semantic relationship!
    a2a:required true ;
    a2a:isArray true ;
    rdfs:comment "The set of skills, or distinct capabilities, that the agent can perform." .

a2a:AgentCard_protocolVersion a a2a:Property ;
    a2a:name "protocolVersion" ;
    a2a:type xsd:string ;
    a2a:required true ;
    a2a:defaultValue "0.3.0" ;
    rdfs:comment "The version of the A2A protocol this agent supports." .
```

**What you're thinking**: "What is an AgentCard in the domain? What are its essential properties and relationships?"

**What's explicit**:
- `AgentCard` is an `Entity` (a first-class domain concept)
- `capabilities` is a semantic relationship to `AgentCapabilities` (not just a field)
- `skills` is an array relationship to `AgentSkill` (composition relationship)
- `protocolVersion` has a default value of "0.3.0" with semantic meaning
- All documentation is attached to the ontology, not scattered in comments

### Example 2: Generating Code from Ontology

The ontology above doesn't execute—it describes. To get executable code, we use SPARQL CONSTRUCT:

```sparql
# ggen.toml - domain-structs rule

CONSTRUCT {
    ?entity a a2a:GeneratedStruct ;
        a2a:structName ?name ;
        a2a:structDoc ?entityDoc ;
        a2a:hasField ?prop .

    ?prop a a2a:StructField ;
        a2a:fieldName ?propName ;
        a2a:fieldType ?rustType ;
        a2a:isRequired ?required ;
        a2a:description ?desc ;
        a2a:hasDefault ?defaultVal ;
        a2a:serdeRename ?jsonName .
}
WHERE {
    ?entity a a2a:Entity ;
        a2a:name ?name .

    OPTIONAL { ?entity rdfs:comment ?entityDoc }

    ?entity a2a:hasProperty ?prop .
    ?prop a2a:name ?propName ;
        a2a:type ?type .

    # Map XSD types to Rust types
    BIND(
        IF(?type = xsd:string, "String",
        IF(?type = xsd:boolean, "bool",
        IF(?type = xsd:integer, "i64",
        str(?type))))
        AS ?rustType
    )

    OPTIONAL { ?prop a2a:required ?required }
    OPTIONAL { ?prop rdfs:comment ?desc }
    OPTIONAL { ?prop a2a:default ?defaultVal }
}
```

This CONSTRUCT query **transforms** the ontology into an intermediate RDF graph that describes Rust structs. A Tera template then renders that graph into actual Rust code:

```rust
// Generated by ggen from ontology

use serde::{Deserialize, Serialize};
use bon::Builder;

/// The AgentCard is a self-describing manifest for an agent.
/// It provides essential metadata including the agent's identity,
/// capabilities, skills, supported communication methods, and security requirements.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// A human-readable name for the agent.
    pub name: String,

    /// A human-readable description of the agent.
    pub description: String,

    /// The preferred endpoint URL for interacting with the agent.
    pub url: String,

    /// The agent's own version number.
    pub version: String,

    /// The version of the A2A protocol this agent supports.
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// A declaration of optional capabilities supported by the agent.
    pub capabilities: AgentCapabilities,

    /// The set of skills that the agent can perform.
    pub skills: Vec<AgentSkill>,
}

fn default_protocol_version() -> String {
    "0.3.0".to_string()
}
```

**The key insight**: The CONSTRUCT query is itself **queryable and composable**. You can:
- Query the intermediate RDF to validate mappings
- Chain multiple CONSTRUCT queries to build transformation pipelines
- Generate multiple artifacts (Rust, TypeScript, JSON Schema) from the same ontology
- Validate three-way consistency: ontology ↔ spec ↔ generated code

---

## Common Mental Blockers

### Blocker 1: "Why not just write code?"

**The impulse**: "I can write that struct in 2 minutes. Why spend time on ontology?"

**The reality**: You're not writing one struct. You're defining:
- The Rust struct
- The JSON Schema for validation
- The TypeScript types for the client
- The API documentation
- The test fixtures
- The migration logic when fields change

With ontology-first, you define the domain **once**, and all artifacts are generated consistently. When you change the ontology, all representations update together. No manual synchronization.

**When code-first breaks down**: Imagine you need to add a new optional field `iconUrl` to `AgentCard`. In code-first:
1. Update the Rust struct
2. Update the JSON Schema
3. Update the TypeScript types
4. Update the API docs
5. Update test fixtures
6. Update migration logic
7. Hope you didn't miss any place where this type is referenced

In ontology-first:
1. Add `a2a:AgentCard_iconUrl` property to ontology
2. Run `ggen generate`
3. Done. All artifacts are regenerated consistently.

### Blocker 2: "I don't understand RDF/SPARQL"

**The impulse**: "This looks complicated. I just want to build features."

**The reality**: You don't need to understand RDF deeply to **use** ontology-driven development. You need to understand:
1. How to read `.ttl` files (simpler than it looks)
2. That CONSTRUCT queries transform ontology to intermediate representations
3. That templates render those representations as code

Think of it like CSS: you don't need to understand browser rendering engines to write CSS. You need to understand the syntax and what it controls.

**Practical approach**: Start by reading existing ontology definitions. Copy the patterns. As you gain familiarity, the semantics become clearer. The structure is designed to be readable.

### Blocker 3: "What if I need custom logic?"

**The impulse**: "Generated code can't handle my specific use case."

**The reality**: Ontology-first doesn't mean **everything** is generated. The architecture is:

```
┌─────────────────────────────────────────┐
│ GENERATED (from ontology)               │
│ - Domain structs (data types)           │
│ - Port traits (interfaces)              │
│ - Validation rules                      │
│ - Error types                            │
└─────────────────────────────────────────┘
              ↓
┌─────────────────────────────────────────┐
│ HAND-WRITTEN (custom logic)             │
│ - Adapter implementations                │
│ - Business logic handlers                │
│ - Application routing                    │
│ - Service wrappers                       │
└─────────────────────────────────────────┘
```

**Generate the ontology-derivable, write the implementation-specific.**

Domain types and their relationships are derivable from ontology. Business logic is not. The hexagonal architecture enforces this separation.

### Blocker 4: "How do I debug generated code?"

**The impulse**: "If the code is generated, I can't trace issues."

**The reality**: Generated code is **readable Rust**. You can:
- Read the generated `.rs` files (they're in `a2a-rs/src/generated/`)
- Debug them like any Rust code
- Trace through with a debugger
- Run `cargo expand` to see macro expansion

If the generated code has a bug, you fix it in the **ontology or template**, not the generated code. Think of it like fixing a macro definition vs. fixing macro output.

**Debugging workflow**:
1. Issue in generated code? Look at the intermediate CONSTRUCT results
2. Issue in CONSTRUCT results? Look at the ontology definitions
3. Issue in how CONSTRUCT maps to Rust? Look at the template

Each layer is inspectable and debuggable.

### Blocker 5: "This feels like over-engineering"

**The impulse**: "We're building a protocol implementation, not a PhD thesis."

**The reality**: The A2A Protocol is a **specification**. We're not inventing types; we're implementing a defined standard. Ontology-first gives us:

1. **Spec compliance by construction**: The ontology is validated against the protocol JSON Schemas
2. **Three-way consistency**: Ontology ↔ Spec ↔ Generated Code
3. **Evolvability**: When the spec changes (v0.3.0 → v0.4.0), we update the ontology once
4. **Multi-language support**: Generate Rust, TypeScript, Python from the same ontology

This isn't over-engineering; it's **engineering for the problem domain**. Protocol implementations demand this level of rigor.

---

## Visual Model

### Traditional Code-First Flow

```
┌─────────────┐
│   Developer │
│   Writes    │
│    Code     │
└──────┬──────┘
       │
       ↓
┌─────────────────────────────────────┐
│      Rust Source Code               │
│  (structs, enums, traits)           │
└──────┬──────────────────────────────┘
       │
       ↓
┌─────────────────┐    ┌──────────────────┐    ┌────────────────┐
│  Manually write │    │  Manually write  │    │ Manually write │
│  JSON Schema    │    │  Documentation   │    │  TypeScript    │
└─────────────────┘    └──────────────────┘    └────────────────┘
       │                       │                        │
       ↓                       ↓                        ↓
    ❌ Drift              ❌ Out of sync           ❌ Inconsistent
```

**Problem**: Each artifact maintained separately. Consistency is manual and fragile.

### Ontology-First Flow (CONSTRUCT Pipeline)

```
                    ┌────────────────────────────────┐
                    │    RDF Ontology (.ttl)         │
                    │  - Entities (AgentCard, Task)  │
                    │  - Properties (name, url)      │
                    │  - Relationships (hasSkill)    │
                    │  - Constraints (required, min) │
                    └────────────┬───────────────────┘
                                 │
                                 │ SPARQL CONSTRUCT
                                 ↓
                    ┌────────────────────────────────┐
                    │  Intermediate RDF Graphs       │
                    │  - GeneratedStruct entities    │
                    │  - StructField relationships   │
                    │  - Type mappings (xsd → Rust)  │
                    └────────────┬───────────────────┘
                                 │
                    ┌────────────┼───────────────┬─────────────┐
                    │            │               │             │
              Tera Template  Tera Template  Tera Template  Tera Template
                    │            │               │             │
                    ↓            ↓               ↓             ↓
            ┌──────────────┐ ┌─────────────┐ ┌──────────────┐ ┌────────────┐
            │  Rust Structs│ │Port Traits  │ │ Validation   │ │JSON Schema │
            └──────────────┘ └─────────────┘ └──────────────┘ └────────────┘
                    │            │               │             │
                    └────────────┼───────────────┴─────────────┘
                                 │
                                 ↓
                    ┌────────────────────────────────┐
                    │   ✅ Consistent Artifacts      │
                    │   ✅ Single Source of Truth    │
                    │   ✅ Composable Pipeline       │
                    └────────────────────────────────┘
```

**Key insight**: CONSTRUCT queries produce RDF (not tables), enabling:
- **Graph-to-graph transformations** (not query-to-template)
- **Composable pipelines** (chain CONSTRUCT queries)
- **Queryable intermediate results** (validate at each stage)

---

## Key Principles

### Principle 1: Think in Relationships, Not Fields

**Code-first thinking**: "AgentCard has a field `skills` of type `Vec<AgentSkill>`"

**Ontology-first thinking**: "AgentCard has a **composition relationship** with AgentSkill, where an AgentCard owns multiple Skills"

The ontology makes the **nature of the relationship** explicit:
- Is it composition or reference?
- Is it one-to-many or many-to-many?
- Is it required or optional?
- What constraints govern it?

This semantic information drives validation, serialization, and evolution.

### Principle 2: Define Constraints Once, Enforce Everywhere

**Code-first approach**: Write validation logic in multiple places:
- Rust struct builder methods
- JSON Schema `minLength`/`maxLength`
- API input validation
- Database constraints

**Ontology-first approach**: Define constraints in the ontology:

```turtle
a2a:AgentCard_name a a2a:Property ;
    a2a:name "name" ;
    a2a:type xsd:string ;
    a2a:required true ;
    a2a:hasConstraint [
        a2a:kind "length" ;
        a2a:minLength 1 ;
        a2a:maxLength 200 ;
        a2a:errorMessage "Agent name must be 1-200 characters"
    ] .
```

The generator produces:
- Rust validation functions
- JSON Schema constraints
- Runtime validation in builders
- Error messages

**Define once, enforce everywhere.**

### Principle 3: Generate the Derivable, Write the Specific

**What to generate**:
- Domain structs (pure data)
- Port traits (interfaces)
- Error enums
- Validation rules
- Protocol types

**What to hand-write**:
- Adapter implementations (HTTP client, database storage)
- Business logic (task orchestration, message routing)
- Application routing (JSON-RPC dispatcher)
- Service wrappers (high-level client/server APIs)

The line is clear: if it's **derivable from domain semantics**, generate it. If it's **implementation-specific**, write it.

### Principle 4: Validate at Three Levels

Ontology-first enables three-way validation:

1. **Ontology ↔ Spec**: Validate ontology against JSON Schema spec
2. **Spec ↔ Generated Code**: Validate generated types serialize correctly
3. **Ontology ↔ Generated Code**: Validate CONSTRUCT mappings are correct

This three-way validation catches inconsistencies that single-source approaches miss.

### Principle 5: Embrace Iterative Refinement

You don't need to get the ontology perfect on the first pass. The workflow is:

1. **Draft ontology**: Define entities and properties
2. **Generate code**: Run `ggen generate`
3. **Review output**: Look at generated Rust
4. **Refine ontology**: Adjust types, constraints, relationships
5. **Regenerate**: Run `ggen generate` again
6. **Iterate**: Repeat until the generated code matches your intent

The generator is **fast** (<1 second for a2a-rs). You can iterate rapidly.

---

## Practical Implications

### For New Features

**Code-first workflow**:
1. Write Rust structs
2. Add serde attributes
3. Write validation
4. Write JSON Schema
5. Write tests
6. Hope everything stays in sync

**Ontology-first workflow**:
1. Add entity to ontology
2. Run `ggen generate`
3. Implement adapters/handlers (hand-written)
4. Tests are generated or guided by generated types

### For Protocol Changes

When A2A Protocol updates from v0.3.0 → v0.4.0:

**Code-first**:
- Manually update all Rust types
- Update all JSON Schemas
- Update all tests
- Risk missing some updates
- Risk inconsistent behavior

**Ontology-first**:
- Update ontology definitions
- Run `ggen generate`
- All artifacts regenerate consistently
- Type errors guide adapter updates

### For Multi-Language Support

**Code-first**:
- Maintain separate type definitions per language
- Manually sync changes
- Hope for consistency

**Ontology-first**:
- Single ontology
- Multiple CONSTRUCT queries + templates
- Generate Rust, TypeScript, Python, Go from same source
- Guaranteed consistent semantics

---

## Next Steps

### For Learners

1. **Read existing ontologies**: Start with `/ggen/ontology/a2a-agent.ttl`
2. **Examine CONSTRUCT queries**: Look at `/ggen/ggen.toml` rules
3. **Inspect generated code**: Check `/a2a-rs/src/generated/` (after running generator)
4. **Run the generator**: `ggen generate --config ggen/ggen.toml`
5. **Make a small change**: Add a property, regenerate, observe

### For Contributors

1. **Understand the architecture**: Domain → Port → Adapter → Application
2. **Know what to generate**: Domain types, port traits
3. **Know what to hand-write**: Adapters, business logic
4. **Follow the patterns**: Copy existing ontology structures
5. **Validate your changes**: Run `cargo test --all-features`

### Deep Dives

- **[Ontology Basics](./ONTOLOGY-BASICS.md)**: Learn RDF/Turtle syntax
- **[CONSTRUCT Pipeline](./CONSTRUCT-PIPELINE.md)**: Understand SPARQL CONSTRUCT in depth
- **[RDF for Developers](./RDF-FOR-DEVELOPERS.md)**: Practical RDF patterns for software engineers
- **[Template Development](../how-to/TEMPLATE-DEVELOPMENT.md)**: Write Tera templates for code generation

---

## Summary

The mental model shift from code-first to ontology-first is not just a workflow change—it's a **conceptual reframing**:

| Code-First | Ontology-First |
|------------|----------------|
| "What fields do I need?" | "What is this entity in the domain?" |
| "How do I implement this?" | "What are the essential relationships?" |
| Code is source of truth | Ontology is source of truth |
| Manual consistency | Generated consistency |
| Single artifact | Multiple consistent artifacts |
| Implicit semantics | Explicit semantics |
| Fragile to change | Evolvable by design |

The key insight: **stop thinking about code, start thinking about domain semantics**. The code follows naturally once the domain is modeled correctly.

When you embrace this shift, you gain:
- **Consistency** across all artifacts
- **Evolvability** when requirements change
- **Multi-language support** from a single source
- **Spec compliance** by construction
- **Composable transformations** via CONSTRUCT pipelines

The initial investment in learning RDF/SPARQL pays dividends in maintainability, correctness, and agility.

---

**Next**: [Ontology Basics](./ONTOLOGY-BASICS.md) — Learn to read and write RDF/Turtle syntax
