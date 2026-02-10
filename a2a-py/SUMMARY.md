# a2a-py Package Summary

## Overview

Python bindings for the A2A CONSTRUCT runtime, enabling deterministic agent execution with cryptographic receipts in Python.

## What Was Created

### Core Files

1. **`Cargo.toml`** - Rust package manifest with PyO3 dependencies
2. **`pyproject.toml`** - Python packaging configuration for maturin
3. **`src/lib.rs`** - PyO3 bindings implementation

### Python Support

4. **`python/a2a_construct/__init__.py`** - Python module stub
5. **`examples/basic_usage.py`** - Comprehensive usage example
6. **`tests/test_basic.py`** - Pytest test suite

### Documentation

7. **`README.md`** - API reference and quick start guide
8. **`INSTALL.md`** - Detailed installation instructions
9. **`Makefile`** - Development task automation
10. **`.gitignore`** - Python/Rust artifact exclusions

## Exposed Classes

### OntologyState

Manages protocol state (tasks, messages, agents, notification configs).

**Key Methods:**
- `OntologyState(bounds=None)` - Create new state
- `put_task_json(json)` / `get_task_json(id)` - Task operations
- `add_message_json(task_id, json)` / `get_messages_json(id)` - Message operations
- `put_agent_json(json)` / `get_agent_json(name)` - Agent operations
- `stats()` - Get StateStats
- `to_json()` / `from_json(json)` - Serialization

### Receipt

Cryptographic receipt for a single state transition.

**Key Methods:**
- `Receipt.new(observation, action, delta)` - Create from bytes
- Properties: `sequence()`, `timestamp()`, `observation_hash()`, `action_hash()`, `delta_hash()`, `receipt_hash()`, `previous_hash()`
- `verify_hashes()` - Verify internal consistency
- `to_json()` / `from_json(json)` - Serialization

### ReceiptChain

Tamper-proof chain of receipts.

**Key Methods:**
- `ReceiptChain()` - Create empty chain
- `add_transition(obs, act, delta)` - Create and add receipt
- `add_receipt(receipt)` - Add existing receipt
- `verify_integrity()` - Verify chain (raises on failure)
- `get(seq)` / `latest()` - Retrieve receipts
- `length()` / `is_empty()` - Chain info
- `to_json()` / `from_json(json)` - Serialization

### StateBounds

Configuration for bounded state.

**Constructor:**
- `StateBounds(max_tasks=10000, max_messages_per_task=1000, max_agents=1000)`

**Properties:**
- `max_tasks`, `max_messages_per_task`, `max_agents`

### StateStats

State statistics.

**Properties:**
- `task_count`, `agent_count`, `notification_config_count`, `context_count`, `total_messages`

**Methods:**
- `to_json()` - Export as JSON

### ConstructError

Exception raised for CONSTRUCT errors.

## Design Decisions

### JSON Boundary Pattern

All complex Rust types cross the FFI boundary as JSON strings:
- **Simplicity**: No complex struct marshalling
- **Forward compatibility**: New fields don't break FFI
- **Debuggability**: Easy to inspect data
- **Protocol alignment**: Matches JSON-RPC foundation

### Memory Safety

PyO3 ensures safe memory management:
- Automatic reference counting
- No manual memory management
- Thread-safe (GIL protection)

### Error Handling

Rust errors convert to Python `ConstructError` exceptions with descriptive messages.

## Usage Example

```python
from a2a_construct import OntologyState, ReceiptChain
import json

# Create state
state = OntologyState()

# Add task
task_json = json.dumps({
    "id": "task-1",
    "contextId": "ctx-1",
    "status": {"state": "pending", "reason": None},
    "agent": {"name": "Agent", "url": "http://localhost", "publicKey": None}
})
state.put_task_json(task_json)

# Add message
msg_json = json.dumps({
    "messageId": "msg-1",
    "role": "user",
    "content": [{"type": "text", "text": "Hello"}]
})
state.add_message_json("task-1", msg_json)

# Create receipt chain
chain = ReceiptChain()
chain.add_transition(b"observation", b"action", b"delta")
assert chain.verify_integrity()

# Serialize
state_json = state.to_json()
chain_json = chain.to_json()
```

## Installation

```bash
cd a2a-py
pip install maturin
maturin develop
```

See [INSTALL.md](INSTALL.md) for detailed instructions.

## Testing

```bash
cd a2a-py
maturin develop
pytest tests/
```

## Building Wheels

```bash
cd a2a-py
maturin build --release
pip install target/wheels/a2a_construct-*.whl
```

## Features

Built with Rust features:
- `receipts` - Cryptographic receipt support
- `receipts-signing` - Ed25519 signatures (future Python API)

## Architecture Notes

### Not Exposed (Yet)

The following CONSTRUCT components are not yet exposed to Python:
- **Station** trait and implementations - Future addition for method handlers
- **Runtime** executor - Future addition for complete execution pipeline
- **Scheduler** - Future addition for task scheduling
- **Guards** and **Invariants** - Future additions for validation

These can be added incrementally as needed.

### Type Mapping

| Rust Type | Python Representation |
|-----------|----------------------|
| `OntologyState` | JSON serialization |
| `Task` | JSON string |
| `Message` | JSON string |
| `AgentCard` | JSON string |
| `Receipt` | PyReceipt class |
| `ReceiptChain` | PyReceiptChain class |
| `Vec<u8>` (bytes) | Python bytes |
| `String` | Python str |
| `Option<T>` | Python None or T |

## Known Issues

1. **a2a-rs compilation errors**: The parent `a2a-rs` crate currently has compilation errors in `construct/replay/debugger.rs` and some DSL parsers. These are pre-existing issues not related to the Python bindings.

2. **Station/Runtime not exposed**: These higher-level components are not yet wrapped. The bindings currently focus on state management and receipts.

## Future Enhancements

1. **Station bindings**: Expose station trait for custom method handlers
2. **Runtime bindings**: Expose the execution pipeline
3. **Async support**: Add async Python APIs using `pyo3-asyncio`
4. **Signature verification**: Expose Ed25519 signing/verification APIs
5. **Type stubs**: Generate `.pyi` files for better IDE support
6. **Wheels for PyPI**: Build and publish wheels for multiple platforms

## File Structure

```
a2a-py/
├── Cargo.toml              # Rust package manifest
├── pyproject.toml          # Python package config
├── Makefile                # Development tasks
├── README.md               # API documentation
├── INSTALL.md              # Installation guide
├── SUMMARY.md              # This file
├── .gitignore              # Artifact exclusions
├── src/
│   └── lib.rs              # PyO3 bindings
├── python/
│   └── a2a_construct/
│       └── __init__.py     # Python module
├── examples/
│   └── basic_usage.py      # Usage example
└── tests/
    └── test_basic.py       # Pytest tests
```

## Integration with Workspace

The package is added to the workspace in `/home/user/a2a-rs/Cargo.toml`:

```toml
members = ["a2a-rs", "a2a-agents", "a2a-client", "a2a-ap2", "a2a-py", ...]
```

## License

MIT OR Apache-2.0 (matches parent workspace)
