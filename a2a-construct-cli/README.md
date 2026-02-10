# a2a-construct-cli

Command-line interface for A2A CONSTRUCT runtime operations.

## Overview

The `a2a-construct` CLI provides tools for executing, replaying, validating, and inspecting the CONSTRUCT runtime and its ontology state. It enables deterministic runtime execution with typed packets, cryptographic receipt chains, and invariant checking.

## Installation

```bash
cargo install --path a2a-construct-cli
```

Or build from source:

```bash
cargo build --release -p a2a-construct-cli
```

The binary will be available at `target/release/a2a-construct`.

## Commands

### `run` - Execute Operations

Execute operations on the runtime with typed packets.

```bash
# Create a new task
a2a-construct run \
  --operation create-task \
  --task-id task-1 \
  --context-id ctx-1 \
  --message "Initial query" \
  --priority high \
  --save-state state.json \
  --save-receipts receipts.json

# Send a message to an existing task
a2a-construct run \
  --operation send-message \
  --task-id task-1 \
  --message "Follow-up message" \
  --state-file state.json \
  --save-state state.json

# Update task state
a2a-construct run \
  --operation update-state \
  --task-id task-1 \
  --state completed \
  --state-file state.json \
  --save-state state.json

# Complete a task
a2a-construct run \
  --operation complete-task \
  --task-id task-1 \
  --station-id default-station \
  --state-file state.json

# Cancel a task
a2a-construct run \
  --operation cancel-task \
  --task-id task-1 \
  --state-file state.json
```

**Operations:**
- `create-task` - Create a new task (requires: `--task-id`, optional: `--context-id`, `--message`, `--priority`)
- `send-message` - Send a message to a task (requires: `--task-id`, `--message`)
- `update-state` - Update task state (requires: `--task-id`, `--state`)
- `complete-task` - Mark task as complete (requires: `--task-id`, `--station-id`)
- `cancel-task` - Cancel a pending task (requires: `--task-id`)

**Options:**
- `--state-file` - Load existing state before execution
- `--save-state` - Save state after execution
- `--save-receipts` - Save cryptographic receipt chain
- `--json` - Output results in JSON format

### `replay` - Replay from Receipt Chain

Replay operations from a cryptographic receipt chain.

```bash
# Replay with integrity verification
a2a-construct replay \
  --receipts receipts.json \
  --verify \
  --save-state replayed-state.json

# Replay from initial state
a2a-construct replay \
  --receipts receipts.json \
  --state initial-state.json \
  --save-state final-state.json
```

**Options:**
- `--receipts` - Path to receipt chain file (required)
- `--state` - Path to initial state file (optional)
- `--verify` - Verify chain integrity before replay (default: true)
- `--save-state` - Save final state after replay

### `validate` - Check Invariants

Validate invariants on a state file.

```bash
# Validate all invariants
a2a-construct validate --state state.json

# Validate specific invariant type
a2a-construct validate \
  --state state.json \
  --invariant task-state \
  --verbose

# JSON output
a2a-construct validate --state state.json --json
```

**Invariant Types:**
- `task-state` - Task state machine invariants
- `artifact-immutability` - Artifact immutability checks
- `event-ordering` - Event ordering consistency

**Options:**
- `--state` - Path to state file (required)
- `--invariant` - Check specific invariant type (optional)
- `--verbose` - Show detailed validation output

### `inspect` - Display Ontology State

Inspect and display ontology state.

```bash
# Show state statistics
a2a-construct inspect --state state.json --stats-only

# Show all tasks and agents
a2a-construct inspect --state state.json

# Show detailed task information
a2a-construct inspect --state state.json --detailed

# Filter by task ID
a2a-construct inspect --state state.json --task-id task-1

# Filter by context ID
a2a-construct inspect --state state.json --context-id ctx-1

# JSON output
a2a-construct inspect --state state.json --json
```

**Options:**
- `--state` - Path to state file (required)
- `--detailed` - Show detailed information including messages
- `--task-id` - Filter by specific task ID
- `--context-id` - Filter by context ID
- `--stats-only` - Show only statistics

## Global Options

- `--verbose` / `-v` - Enable verbose logging (debug level)
- `--json` - Output results in JSON format (all commands)

## Examples

### Complete Workflow

```bash
# 1. Create a task
a2a-construct run \
  --operation create-task \
  --task-id task-001 \
  --context-id workflow-1 \
  --message "Start processing" \
  --priority high \
  --save-state state.json \
  --save-receipts receipts.json

# 2. Inspect the state
a2a-construct inspect --state state.json --detailed

# 3. Send additional messages
a2a-construct run \
  --operation send-message \
  --task-id task-001 \
  --message "Update progress" \
  --state-file state.json \
  --save-state state.json

# 4. Update task state
a2a-construct run \
  --operation update-state \
  --task-id task-001 \
  --state completed \
  --state-file state.json \
  --save-state state.json

# 5. Validate final state
a2a-construct validate --state state.json --verbose

# 6. Replay from receipts
a2a-construct replay \
  --receipts receipts.json \
  --verify \
  --save-state replayed-state.json
```

### JSON Output Integration

```bash
# Create task and capture output
OUTPUT=$(a2a-construct run \
  --operation create-task \
  --task-id task-002 \
  --context-id api-call \
  --save-state state.json \
  --json)

# Parse execution ID
EXEC_ID=$(echo "$OUTPUT" | jq -r '.executionId')
echo "Execution ID: $EXEC_ID"

# Get task count
TASK_COUNT=$(a2a-construct inspect --state state.json --stats-only --json | jq -r '.task_count')
echo "Total tasks: $TASK_COUNT"
```

## Architecture

The CLI is built on the CONSTRUCT runtime, which implements:

- **μ(O)** - The total compiler/runtime function
- **Λ** - Scheduler for deterministic task ordering
- **G** - Guards for admission control
- **Q** - Invariants for correctness checking
- **Δ** - State deltas for bounded updates

All operations produce cryptographic receipts that form a tamper-proof audit trail.

## State File Format

State files are JSON representations of `OntologyState`:

```json
{
  "tasks": {},
  "taskMessages": {},
  "agents": {},
  "notificationConfigs": {},
  "contextToTasks": {},
  "bounds": {
    "maxTasks": 10000,
    "maxMessagesPerTask": 1000,
    "maxAgents": 1000
  }
}
```

## Receipt Chain Format

Receipt chains are JSON representations of `ReceiptChain`:

```json
{
  "receipts": [
    {
      "sequence": 0,
      "timestamp": "2026-02-10T12:00:00Z",
      "observationHash": "abc123...",
      "actionHash": "def456...",
      "deltaHash": "ghi789...",
      "receiptHash": "jkl012...",
      "previousHash": null
    }
  ]
}
```

## Development

```bash
# Run tests
cargo test -p a2a-construct-cli

# Check formatting
cargo fmt -p a2a-construct-cli

# Run clippy
cargo clippy -p a2a-construct-cli

# Build documentation
cargo doc -p a2a-construct-cli --open
```

## License

MIT OR Apache-2.0
