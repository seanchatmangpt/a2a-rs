---
name: spec-check
description: Validate implementation against A2A Protocol v0.3.0 JSON schemas
context: fork
agent: Explore
---

Validate that $ARGUMENTS conforms to the A2A Protocol v0.3.0 specification.

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

## Validation checklist

For the implementation files related to $ARGUMENTS:
1. Read the relevant spec JSON schema
2. Read the Rust implementation
3. Compare field names, types, and optionality
4. Check enum variants match spec exactly
5. Verify serde rename attributes match JSON field names
6. Check for missing fields or extra fields not in spec
7. Verify error codes match `spec/errors.json`

Report deviations as a structured list with spec reference and impl location.
