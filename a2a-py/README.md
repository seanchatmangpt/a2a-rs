# a2a-construct (Python Bindings)

Python bindings for the A2A CONSTRUCT runtime - deterministic agent execution with cryptographic receipts.

## Overview

This package provides Python access to the CONSTRUCT layer of the A2A protocol, enabling:

- **Ontology State Management**: Store and query protocol entities (tasks, messages, agents)
- **Cryptographic Receipts**: Create tamper-proof audit trails of state transitions
- **Deterministic Execution**: Reproducible runtime behavior for agent operations

## Installation

### From PyPI (future)

```bash
pip install a2a-construct
```

### From Source

Requires Rust toolchain (1.85+) and Python 3.8+:

```bash
cd a2a-py
pip install maturin
maturin develop
```

Or build a wheel:

```bash
maturin build --release
pip install target/wheels/a2a_construct-*.whl
```

## Quick Start

### Ontology State

```python
from a2a_construct import OntologyState, StateBounds

# Create state with custom bounds
bounds = StateBounds(max_tasks=5000, max_messages_per_task=500, max_agents=100)
state = OntologyState(bounds=bounds)

# Add a task (from JSON)
task_json = '''
{
    "id": "task-1",
    "contextId": "ctx-1",
    "status": {
        "state": "pending",
        "reason": null
    }
}
'''
state.put_task_json(task_json)

# Query state
print(f"Task count: {state.task_count()}")
task = state.get_task_json("task-1")
print(f"Task: {task}")

# Add messages
message_json = '''
{
    "messageId": "msg-1",
    "role": "user",
    "content": [{"type": "text", "text": "Hello"}]
}
'''
state.add_message_json("task-1", message_json)

# Get statistics
stats = state.stats()
print(f"State: {stats}")
```

### Cryptographic Receipts

```python
from a2a_construct import Receipt, ReceiptChain

# Create individual receipts
receipt = Receipt.new(
    b"observation data",
    b"action taken",
    b"state delta"
)

print(f"Receipt hash: {receipt.receipt_hash()}")
print(f"Timestamp: {receipt.timestamp()}")

# Build a receipt chain
chain = ReceiptChain.new()
chain.add_transition(b"obs1", b"act1", b"delta1")
chain.add_transition(b"obs2", b"act2", b"delta2")
chain.add_transition(b"obs3", b"act3", b"delta3")

# Verify integrity
assert chain.verify_integrity()
print(f"Chain length: {chain.length()}")

# Get latest receipt
latest = chain.latest()
print(f"Latest: {latest}")

# Export/import
chain_json = chain.to_json()
restored = ReceiptChain.from_json(chain_json)
assert restored.verify_integrity()
```

### Serialization

All classes support JSON serialization for persistence and interop:

```python
# Export state
state_json = state.to_json()
with open("state.json", "w") as f:
    f.write(state_json)

# Import state
with open("state.json") as f:
    restored_state = OntologyState.from_json(f.read())

# Same for receipts
receipt_json = receipt.to_json()
restored_receipt = Receipt.from_json(receipt_json)
```

## API Reference

### OntologyState

Represents the complete protocol state including tasks, messages, agents, and notification configurations.

**Methods:**
- `OntologyState(bounds=None)` - Create new state with optional bounds
- `is_empty()` - Check if state contains no entities
- `task_count()` - Get number of tasks
- `agent_count()` - Get number of agents
- `stats()` - Get StateStats object
- `put_task_json(task_json)` - Add/update task from JSON
- `get_task_json(task_id)` - Get task as JSON (or None)
- `get_all_tasks_json()` - Get all tasks as JSON array
- `remove_task(task_id)` - Remove task, return as JSON (or None)
- `add_message_json(task_id, message_json)` - Add message to task
- `get_messages_json(task_id)` - Get messages as JSON array (or None)
- `message_count(task_id)` - Get message count for task
- `put_agent_json(agent_json)` - Register/update agent from JSON
- `get_agent_json(agent_name)` - Get agent as JSON (or None)
- `clear()` - Clear all state
- `to_json()` - Export state as JSON string
- `OntologyState.from_json(json)` - Import state from JSON string

### Receipt

Cryptographic receipt binding observation, action, and delta.

**Methods:**
- `Receipt.new(observation, action, delta)` - Create receipt from bytes
- `sequence` - Get sequence number (property)
- `timestamp()` - Get ISO 8601 timestamp string (property)
- `observation_hash()` - Get observation hash (property)
- `action_hash()` - Get action hash (property)
- `delta_hash()` - Get delta hash (property)
- `receipt_hash()` - Get combined receipt hash (property)
- `previous_hash()` - Get previous receipt hash or None (property)
- `verify_hashes()` - Verify internal hash consistency
- `to_json()` - Export as JSON string
- `Receipt.from_json(json)` - Import from JSON string

### ReceiptChain

Tamper-proof chain of receipts.

**Methods:**
- `ReceiptChain.new()` - Create empty chain
- `add_receipt(receipt)` - Add existing receipt to chain
- `add_transition(observation, action, delta)` - Create and add receipt
- `verify_integrity()` - Verify chain integrity (raises on failure)
- `length()` - Get number of receipts
- `is_empty()` - Check if chain is empty
- `get(sequence)` - Get receipt by sequence number (or None)
- `latest()` - Get most recent receipt (or None)
- `to_json()` - Export as JSON string
- `ReceiptChain.from_json(json)` - Import from JSON string

### StateBounds

Configuration for bounded state representation.

**Methods:**
- `StateBounds(max_tasks=10000, max_messages_per_task=1000, max_agents=1000)` - Create bounds
- `max_tasks` - Maximum tasks (property)
- `max_messages_per_task` - Maximum messages per task (property)
- `max_agents` - Maximum agents (property)

### StateStats

Statistics about ontology state.

**Properties:**
- `task_count` - Number of tasks
- `agent_count` - Number of agents
- `notification_config_count` - Number of notification configs
- `context_count` - Number of contexts
- `total_messages` - Total message count across all tasks

**Methods:**
- `to_json()` - Export as JSON string

## Architecture

### JSON Boundary Pattern

All complex Rust types (Task, Message, AgentCard, etc.) cross the FFI boundary as JSON strings. This approach:

- Simplifies the binding layer (no complex struct marshalling)
- Provides forward compatibility (new fields don't break the FFI)
- Enables easy debugging and inspection
- Matches the protocol's JSON-RPC foundation

### Memory Safety

PyO3 ensures safe memory management between Rust and Python:

- Rust objects are wrapped in Python objects with proper lifetime tracking
- No manual memory management required
- Thread-safe by default (GIL protection)

### Error Handling

Rust errors are automatically converted to Python `ConstructError` exceptions with descriptive messages.

## Development

### Running Tests

```bash
# Rust tests
cargo test

# Python tests (after maturin develop)
pytest tests/
```

### Building Documentation

```bash
# Rust docs
cargo doc --open

# Python docs (future)
pdoc a2a_construct
```

## Features

The package is built with the following Rust features enabled:

- `receipts` - Cryptographic receipt support
- `receipts-signing` - Ed25519 signature verification (future Python API)

## License

MIT OR Apache-2.0

## Links

- [A2A Protocol Specification](https://github.com/chrishayuk/a2a-rs)
- [CONSTRUCT Documentation](https://github.com/chrishayuk/a2a-rs/blob/master/CONSTRUCT.md)
- [PyO3 Documentation](https://pyo3.rs/)
