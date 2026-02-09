---
name: spec-check
description: Validate implementation against A2A Protocol v0.3.0 JSON schemas and RDF ontology
context: fork
agent: Explore
---

Validate that $ARGUMENTS conforms to the A2A Protocol v0.3.0 specification across all representation layers: spec JSON schemas, RDF ontology, and generated Rust code.

## Reference schemas

Read these spec files to understand the expected structure:
- `spec/agent.json` - AgentCard, AgentCapabilities, AgentSkill
- `spec/message.json` - Message, Part, TextPart, FilePart, DataPart
- `spec/task.json` - Task, TaskState, TaskStatus, Artifact
- `spec/requests.json` - All JSON-RPC method definitions
- `spec/errors.json` - Error codes and formats
- `spec/events.json` - TaskStatusUpdateEvent, TaskArtifactUpdateEvent
- `spec/notifications.json` - PushNotificationConfig
- `spec/jsonrpc.json` - JSON-RPC 2.0 base types

## RDF ontology files

Read these ontology files to understand the RDF representation:
- `ggen/ontology/*.ttl` - Turtle files modeling A2A types as RDF classes and properties

## Code generation pipeline

Read the generation configuration:
- `ggen.toml` - CONSTRUCT queries and generation rules that transform RDF ontology into Rust code

## Validation checklist

### Phase 1: Spec JSON to Rust implementation

For the implementation files related to $ARGUMENTS:
1. Read the relevant spec JSON schema
2. Read the Rust implementation
3. Compare field names, types, and optionality
4. Check enum variants match spec exactly
5. Verify serde rename attributes match JSON field names
6. Check for missing fields or extra fields not in spec
7. Verify error codes match `spec/errors.json`

### Phase 2: Ontology-to-spec validation

Validate that `ggen/ontology/*.ttl` accurately represents `spec/*.json`:
1. Read the relevant spec JSON schema and the corresponding `.ttl` ontology file
2. Verify every JSON schema type has a corresponding `rdfs:Class` or `owl:Class` in the ontology
3. Verify every JSON schema property has a corresponding `rdf:Property` / `owl:DatatypeProperty` / `owl:ObjectProperty`
4. Check that property domains and ranges in the ontology match the parent object and value types in the JSON schema
5. Confirm required vs optional cardinality constraints (`owl:minCardinality`, `owl:maxCardinality`, or `sh:minCount`/`sh:maxCount`) match the JSON schema `required` array
6. Verify enum values in the ontology (e.g., `owl:oneOf` or named individuals) match the JSON schema `enum` arrays exactly
7. Check that inheritance relationships (`rdfs:subClassOf`) correspond to JSON schema `allOf` / composition patterns

### Phase 3: CONSTRUCT query validation

Validate that CONSTRUCT queries in `ggen.toml` produce correct intermediate graphs:
1. Read each CONSTRUCT query defined in `ggen.toml`
2. Verify the WHERE clause references classes and properties that exist in the ontology `.ttl` files
3. Verify the CONSTRUCT template produces triples that contain all the information needed for Rust code generation (struct fields, types, derives, serde attributes)
4. Check that the query correctly handles optional properties (OPTIONAL blocks) vs required properties
5. Confirm the query does not silently drop properties or introduce properties absent from the ontology

### Phase 4: Three-way comparison (spec JSON <-> RDF ontology <-> generated Rust code)

Perform a three-way consistency check across all layers:
1. For each type related to $ARGUMENTS, collect:
   - The JSON schema definition from `spec/*.json`
   - The RDF class/property definitions from `ggen/ontology/*.ttl`
   - The generated Rust struct/enum from the implementation source
2. Build a comparison table with columns: field name, JSON schema type, RDF property + range, Rust field type
3. Flag any row where the three representations disagree on:
   - Field name or naming convention mapping (camelCase in JSON, snake_case in Rust, prefixed URI in RDF)
   - Type mapping (e.g., JSON `string` vs `xsd:string` vs Rust `String`)
   - Optionality (JSON `required` vs RDF cardinality vs Rust `Option<T>`)
   - Enum variants (JSON `enum` vs RDF individuals vs Rust enum variants)
   - Nested/referenced types (JSON `$ref` vs RDF `owl:ObjectProperty` range vs Rust nested struct)
4. Verify that the code generation pipeline preserves all spec semantics end-to-end: spec -> ontology -> CONSTRUCT -> Rust

## Output format

Report deviations as a structured list organized by phase, with:
- The spec JSON location (file + JSON path)
- The ontology location (file + class/property URI) when applicable
- The Rust implementation location (file + line) when applicable
- A description of the mismatch
- Severity: **breaking** (semantic mismatch), **warning** (cosmetic or naming), **info** (suggestion)
