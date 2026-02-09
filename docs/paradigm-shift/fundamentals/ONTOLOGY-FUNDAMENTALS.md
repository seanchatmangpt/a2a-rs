# Ontology Fundamentals

**Target Audience:** Beginner developers new to semantic web concepts
**Estimated Reading Time:** 15-20 minutes
**Prerequisites:** Basic understanding of data structures (structs, enums)
**Phase:** 1 (Fundamentals)
**Priority:** P0

---

## Overview

This document introduces ontologies from a practical, code-first perspective. You'll learn what ontologies are, how they work, and why they're useful for code generation in the a2a-rs project—all without needing a computer science degree.

By the end of this guide, you'll be able to read and understand the RDF files in `ggen/ontology/` and see how they serve as the single source of truth for generating Rust types.

---

## What is an Ontology? (No Academic Jargon)

Think of an ontology as a **structured knowledge graph** that describes the concepts in your domain and their relationships. In simpler terms:

- **Database schema** tells you how data is stored (tables, columns, types)
- **Ontology** tells you what data means and how concepts relate to each other

For a2a-rs, our ontology describes the Agent-to-Agent Protocol: what an `AgentCard` is, what properties it has, how it relates to `AgentSkill`, and what constraints apply to each field.

### Why Use an Ontology for Code Generation?

Traditional approach:
1. Write JSON Schema spec
2. Manually implement Rust types
3. Keep them in sync (manually)
4. Write documentation (separately)

Ontology-driven approach:
1. Define concepts once in RDF (the ontology)
2. Generate Rust types automatically
3. Generate validation rules automatically
4. Generate documentation automatically
5. Always in sync by design

The ontology becomes your **single source of truth**. Change the ontology, regenerate the code, and everything updates consistently.

---

## RDF Triples: The Building Blocks

RDF (Resource Description Framework) expresses knowledge as **triples**—simple statements with three parts:

```
Subject  Predicate  Object
```

Think of it as: **"Subject has Predicate Object"** or **"Subject Predicate Object"**

### Example 1: Describing an Agent Card

From `ggen/ontology/a2a-agent.ttl`:

```turtle
a2a:AgentCard a a2a:Entity ;
    a2a:name "AgentCard" ;
    rdfs:comment "The AgentCard is a self-describing manifest for an agent." ;
    a2a:hasProperty a2a:AgentCard_name .
```

Let's break this into triples:

1. `a2a:AgentCard` **a** `a2a:Entity`
   → "AgentCard is-a Entity"

2. `a2a:AgentCard` **a2a:name** `"AgentCard"`
   → "AgentCard has name 'AgentCard'"

3. `a2a:AgentCard` **rdfs:comment** `"The AgentCard is a self-describing manifest..."`
   → "AgentCard has comment '...'"

4. `a2a:AgentCard` **a2a:hasProperty** `a2a:AgentCard_name`
   → "AgentCard has property AgentCard_name"

The semicolons (`;`) are syntactic sugar meaning "continue with the same subject." It's like saying:

```
AgentCard is-a Entity.
AgentCard has name "AgentCard".
AgentCard has comment "...".
AgentCard has property AgentCard_name.
```

### Example 2: Describing a Property

From the same file:

```turtle
a2a:AgentCard_name a a2a:Property ;
    a2a:name "name" ;
    a2a:type "string"^^xsd:string ;
    a2a:required true ;
    rdfs:comment "A human-readable name for the agent." ;
    a2a:example "Recipe Agent" .
```

Triples:

1. `a2a:AgentCard_name` **a** `a2a:Property` → "AgentCard_name is-a Property"
2. `a2a:AgentCard_name` **a2a:name** `"name"` → Field name in JSON/Rust
3. `a2a:AgentCard_name` **a2a:type** `"string"` → Type is string
4. `a2a:AgentCard_name` **a2a:required** `true` → Field is required
5. `a2a:AgentCard_name` **rdfs:comment** `"A human-readable..."` → Documentation
6. `a2a:AgentCard_name` **a2a:example** `"Recipe Agent"` → Example value

This gets transformed into Rust code like:

```rust
/// The AgentCard is a self-describing manifest for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// A human-readable name for the agent.
    /// Example: "Recipe Agent"
    pub name: String,
    // ... other fields
}
```

---

## Reading RDF Turtle Syntax

Turtle (`.ttl`) is a human-readable RDF format. Here are the key syntax patterns:

### 1. Prefixes (Namespaces)

```turtle
@prefix a2a: <https://ggen.io/ontology/a2a/> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
```

Think of prefixes like Rust's `use` statements—they're shortcuts:

- `a2a:AgentCard` expands to `<https://ggen.io/ontology/a2a/AgentCard>`
- `rdf:type` expands to `<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>`

### 2. The `a` Shortcut

```turtle
a2a:AgentCard a a2a:Entity .
```

`a` is a special shortcut for `rdf:type`. The above means:

```turtle
a2a:AgentCard rdf:type a2a:Entity .
```

In plain English: "AgentCard is of type Entity"

### 3. Semicolons: Same Subject

```turtle
a2a:AgentCard a a2a:Entity ;
    a2a:name "AgentCard" ;
    a2a:hasProperty a2a:AgentCard_name .
```

All three statements share the same subject (`a2a:AgentCard`). This is equivalent to:

```turtle
a2a:AgentCard a a2a:Entity .
a2a:AgentCard a2a:name "AgentCard" .
a2a:AgentCard a2a:hasProperty a2a:AgentCard_name .
```

### 4. Commas: Same Subject and Predicate

```turtle
a2a:TaskState a2a:hasValue a2a:TaskState.submitted ,
                           a2a:TaskState.working ,
                           a2a:TaskState.completed .
```

All three values share the same subject and predicate. Equivalent to:

```turtle
a2a:TaskState a2a:hasValue a2a:TaskState.submitted .
a2a:TaskState a2a:hasValue a2a:TaskState.working .
a2a:TaskState a2a:hasValue a2a:TaskState.completed .
```

### 5. Typed Literals

```turtle
a2a:required true .
a2a:type "string"^^xsd:string .
a2a:defaultValue "0.3.0" .
```

- `true` is a boolean literal
- `"string"^^xsd:string` is a string with explicit type annotation
- `"0.3.0"` is a plain string literal

---

## Ontologies vs. Database Schemas

Let's compare how you'd model the same concept:

### Database Schema Approach

```sql
CREATE TABLE agent_cards (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    url VARCHAR(2048) NOT NULL,
    version VARCHAR(50) NOT NULL,
    protocol_version VARCHAR(20) NOT NULL DEFAULT '0.3.0',
    -- ... more columns
);

CREATE TABLE agent_skills (
    id UUID PRIMARY KEY,
    agent_card_id UUID REFERENCES agent_cards(id),
    skill_id VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    -- ... more columns
);
```

**What this tells you:**
- Storage structure (tables)
- Data types (VARCHAR, UUID)
- Constraints (NOT NULL, foreign keys)

**What it doesn't tell you:**
- What an AgentCard conceptually represents
- Why skills are separate from the card
- What the difference between `name` and `skill_id` means semantically
- How to generate API types from this schema

### Ontology Approach

```turtle
a2a:AgentCard a a2a:Entity ;
    rdfs:comment "The AgentCard is a self-describing manifest for an agent." ;
    a2a:hasProperty a2a:AgentCard_name ;
    a2a:hasProperty a2a:AgentCard_skills .

a2a:AgentCard_skills a a2a:Property ;
    a2a:name "skills" ;
    a2a:type "reference"^^xsd:string ;
    a2a:refEntity "AgentSkill" ;
    a2a:isArray true ;
    rdfs:comment "The set of skills, or distinct capabilities, that the agent can perform." .

a2a:AgentSkill a a2a:Entity ;
    rdfs:comment "Represents a distinct capability or function that an agent can perform." ;
    a2a:hasProperty a2a:AgentSkill_id ;
    a2a:hasProperty a2a:AgentSkill_name .
```

**What this tells you:**
- Semantic meaning (what things represent)
- Relationships (AgentCard has-many Skills)
- Documentation (embedded in the model)
- Constraints (array, required, references)
- Generation rules (this is a reference type, generate accordingly)

**Key Differences:**

| Aspect | Database Schema | Ontology |
|--------|----------------|----------|
| Focus | Storage & retrieval | Meaning & relationships |
| Portability | Database-specific | Platform-agnostic |
| Documentation | Separate (comments/wiki) | Embedded (rdfs:comment) |
| Code generation | Limited (types only) | Rich (types, validation, docs) |
| Evolution | Schema migrations | Versioned concepts |
| Reasoning | None | Can infer relationships |

---

## Real Examples from a2a-rs Ontology

Let's examine five real examples from the project and see what they teach us.

### Example 1: Enum Definitions (from a2a-message.ttl)

```turtle
a2a:Role a a2a:Enum ;
    a2a:enumName "Role" ;
    a2a:description "Identifies the sender of the message." ;
    a2a:hasEnumValue a2a:Role_User, a2a:Role_Agent .

a2a:Role_User a a2a:EnumValue ;
    a2a:valueName "user" ;
    a2a:description "The client/user role." .

a2a:Role_Agent a a2a:EnumValue ;
    a2a:valueName "agent" ;
    a2a:description "The agent/service role." .
```

**Generated Rust:**
```rust
/// Identifies the sender of the message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The client/user role.
    User,
    /// The agent/service role.
    Agent,
}
```

**What we learn:** Enums are first-class ontology concepts with their own values and documentation.

### Example 2: Complex Entity with References (from a2a-message.ttl)

```turtle
a2a:Message a a2a:Entity ;
    a2a:entityName "Message" ;
    a2a:description "Represents a single message in the conversation between a user and an agent." ;
    a2a:hasProperty a2a:Message_kind,
                    a2a:Message_messageId,
                    a2a:Message_role,
                    a2a:Message_parts .

a2a:Message_role a a2a:Property ;
    a2a:propertyName "role" ;
    a2a:propertyType "string" ;
    a2a:isRequired "true"^^xsd:boolean ;
    a2a:description "Identifies the sender of the message. 'user' for the client, 'agent' for the service." ;
    a2a:referencesEnum a2a:Role .
```

**What we learn:**
- Properties can reference other types (enums, entities)
- The `referencesEnum` predicate creates a typed relationship
- Code generator uses this to emit `role: Role` instead of `role: String`

### Example 3: Array Properties (from a2a-agent.ttl)

```turtle
a2a:AgentCard_skills a a2a:Property ;
    a2a:name "skills" ;
    a2a:type "reference"^^xsd:string ;
    a2a:required true ;
    a2a:isArray true ;
    a2a:refEntity "AgentSkill" ;
    rdfs:comment "The set of skills, or distinct capabilities, that the agent can perform." .
```

**Generated Rust:**
```rust
/// The set of skills, or distinct capabilities, that the agent can perform.
pub skills: Vec<AgentSkill>,
```

**What we learn:** The ontology captures collection semantics (`isArray`) and generates appropriate container types.

### Example 4: State Machine Enum (from a2a-task.ttl)

```turtle
a2a:TaskState a a2a:Enum ;
    a2a:name "TaskState" ;
    a2a:description "Represents the possible states of a Task." ;
    a2a:type "string" ;
    a2a:hasValue a2a:TaskState.submitted ,
                 a2a:TaskState.working ,
                 a2a:TaskState.input-required ,
                 a2a:TaskState.completed ,
                 a2a:TaskState.canceled ,
                 a2a:TaskState.failed .

a2a:TaskState.submitted a a2a:EnumValue ;
    a2a:name "submitted" ;
    a2a:belongsTo a2a:TaskState ;
    a2a:description "The task has been submitted and is awaiting processing." .
```

**What we learn:**
- Enums can model state machines with semantic meaning
- Each state has documentation explaining when it applies
- The ontology captures domain knowledge (what states are valid)

### Example 5: Union Types with Discriminators (from a2a-message.ttl)

```turtle
a2a:Part a a2a:Entity ;
    a2a:entityName "Part" ;
    a2a:description "Represents a part of a message, which can be text, a file, or structured data." ;
    a2a:isAbstract "true"^^xsd:boolean ;
    a2a:hasVariant a2a:TextPart, a2a:FilePart, a2a:DataPart .

a2a:TextPart a a2a:Entity ;
    a2a:entityName "TextPart" ;
    a2a:extendsEntity a2a:PartBase ;
    a2a:hasProperty a2a:TextPart_kind,
                    a2a:TextPart_text .

a2a:TextPart_kind a a2a:Property ;
    a2a:propertyName "kind" ;
    a2a:propertyType "string" ;
    a2a:isRequired "true"^^xsd:boolean ;
    a2a:formatConstraint "const: \"text\"" .
```

**Generated Rust:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Part {
    #[serde(rename = "text")]
    Text(TextPart),
    #[serde(rename = "file")]
    File(FilePart),
    #[serde(rename = "data")]
    Data(DataPart),
}
```

**What we learn:**
- Abstract entities with variants become Rust enums
- The `kind` field becomes the serde discriminator
- Inheritance (`extendsEntity`) captures shared base properties

---

## Hands-On Exercises

### Exercise 1: Read a Simple Entity

Open `ggen/ontology/a2a-agent.ttl` and find the `AgentProvider` entity (around line 286).

**Questions:**
1. What properties does `AgentProvider` have?
2. Which properties are required?
3. What are the property types?
4. What example values are given?

<details>
<summary>Answers</summary>

1. It has two properties: `organization` and `url`
2. Both are required (`a2a:required true`)
3. Both are strings (`a2a:type "string"`)
4. Examples: "Google" for organization, "https://ai.google.dev" for url
</details>

### Exercise 2: Follow a Reference

In `ggen/ontology/a2a-agent.ttl`, look at the `AgentCard_capabilities` property (line 76).

**Questions:**
1. What is the `refEntity` value?
2. Find that entity in the file. What properties does it have?
3. Are any of those properties optional?

<details>
<summary>Answers</summary>

1. `refEntity` is `"AgentCapabilities"`
2. It has properties: `streaming`, `pushNotifications`, `stateTransitionHistory`, `extensions`
3. All four properties have `a2a:required false`, making them optional
</details>

### Exercise 3: Understand an Enum

Open `ggen/ontology/a2a-task.ttl` and examine the `TaskState` enum (starts at line 15).

**Questions:**
1. How many values does this enum have?
2. What is the description of the `working` state?
3. Which state indicates the task needs user input?

<details>
<summary>Answers</summary>

1. Nine values: submitted, working, input-required, completed, canceled, failed, rejected, auth-required, unknown
2. "The agent is actively working on the task."
3. `TaskState.input-required`
</details>

### Exercise 4: Count Triples

Look at this snippet from `a2a-message.ttl` (line 36-47):

```turtle
a2a:Message a a2a:Entity ;
    a2a:entityName "Message" ;
    a2a:description "Represents a single message..." ;
    a2a:hasProperty a2a:Message_kind,
                    a2a:Message_messageId,
                    a2a:Message_role,
                    a2a:Message_parts .
```

**Questions:**
1. How many triples are expressed here?
2. What is the subject of all triples?
3. What predicate is used for the type declaration?

<details>
<summary>Answers</summary>

1. Seven triples (1 for type, 1 for entityName, 1 for description, 4 for hasProperty)
2. `a2a:Message`
3. `a` (which is shorthand for `rdf:type`)
</details>

### Exercise 5: Trace Code Generation

Pick the `AgentCard_version` property (line 62 in `a2a-agent.ttl`).

**Challenge:** Based on the RDF properties, predict what Rust code will be generated for this field.

<details>
<summary>Expected Generated Code</summary>

```rust
/// The agent's own version number. The format is defined by the provider.
/// Example: "1.0.0"
pub version: String,
```

The generator reads:
- `rdfs:comment` → doc comment
- `a2a:example` → example in doc comment
- `a2a:type "string"` → Rust type `String`
- `a2a:required true` → not wrapped in `Option<>`
</details>

---

## Next Steps

Now that you understand RDF triples and how to read the ontology, you're ready to learn how to **query and transform** this knowledge using SPARQL CONSTRUCT queries.

Continue to: **[SPARQL CONSTRUCT Introduction](../transformation/SPARQL-CONSTRUCT.md)**

In that document, you'll learn:
- How SPARQL queries extract information from the ontology
- What makes CONSTRUCT queries different from SELECT
- How ggen uses CONSTRUCT to transform ontology → intermediate graphs → Rust code
- How to write your own queries to extend the code generation pipeline

---

## Summary

You've learned:

1. **Ontologies** are structured knowledge graphs that capture meaning, not just structure
2. **RDF triples** express knowledge as Subject-Predicate-Object statements
3. **Turtle syntax** provides a readable way to write RDF with prefixes, semicolons, and shortcuts
4. **Ontologies differ from schemas** by focusing on semantics and enabling richer code generation
5. **a2a-rs ontology** defines entities, properties, enums, and relationships that generate Rust types
6. **Reading RDF** is a learnable skill with hands-on practice

The key insight: by modeling domain knowledge formally in RDF, we get a machine-readable, queryable, single source of truth that can generate consistent code, documentation, and validation—all while maintaining semantic clarity.

---

## Additional Resources

- [ggen/ontology/](../../../ggen/ontology/) - Project ontology files
- [ggen/ggen.toml](../../../ggen/ggen.toml) - Code generation configuration
- [CLAUDE.md](../../../CLAUDE.md) - Project development guide
- [RDF Primer (W3C)](https://www.w3.org/TR/rdf11-primer/) - Official RDF specification (more technical)
- [Turtle Specification](https://www.w3.org/TR/turtle/) - Complete Turtle syntax reference
