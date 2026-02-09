---
name: ontology
description: Manage the A2A Protocol RDF ontology - add entities, properties, validate against spec
context: fork
agent: Explore
---

Manage and validate the A2A Protocol RDF ontology for ggen code generation, focused on $ARGUMENTS.

## Current ontology files

!`wc -l ggen/ontology/*.ttl`

## Reference schemas

Read the relevant spec files based on $ARGUMENTS:
- `spec/agent.json` - AgentCard, AgentCapabilities, AgentSkill, AgentProvider
- `spec/message.json` - Message, Part, TextPart, FilePart, DataPart
- `spec/task.json` - Task, TaskState, TaskStatus, Artifact
- `spec/requests.json` - All JSON-RPC method definitions
- `spec/errors.json` - Error codes and formats
- `spec/events.json` - TaskStatusUpdateEvent, TaskArtifactUpdateEvent
- `spec/notifications.json` - PushNotificationConfig
- `spec/jsonrpc.json` - JSON-RPC 2.0 base types

## Steps

### Step 1: Compare spec against ontology
Read the spec JSON schemas related to $ARGUMENTS (from `spec/*.json`) and the corresponding Turtle files (from `ggen/ontology/*.ttl`). Build a mapping of every type, property, and constraint defined in the spec.

### Step 2: Identify gaps
For each spec type related to $ARGUMENTS, check whether the ontology contains:
- A corresponding RDF class declaration
- All properties with correct domains and ranges
- Cardinality constraints (required vs optional fields)
- Enum value definitions (e.g., TaskState variants)
- Relationships between types (e.g., Task has Messages)

### Step 3: Validate RDF syntax
Check the Turtle files for:
- Valid prefix declarations
- Correct triple termination (periods, semicolons, commas)
- Consistent use of namespaces
- Proper datatype annotations (xsd:string, xsd:boolean, etc.)

### Step 4: Report coverage
Produce a structured report with:
- **Covered**: Spec types fully represented in the ontology
- **Partial**: Spec types present but missing properties or constraints
- **Missing**: Spec types with no ontology representation
- **Extra**: Ontology entries not found in the spec (potential drift)

### Step 5: Suggest additions
For each gap found, suggest the specific Turtle triples needed to bring the ontology into alignment with the spec. Use the existing ontology conventions (prefixes, naming patterns, annotation style) for consistency.
