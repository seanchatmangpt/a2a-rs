# Property-Based Tests for Construct Module

This directory contains property-based tests using `proptest` for the construct module, particularly focused on the `TaskStateMachine`.

## Overview

Property-based testing validates invariants that should hold for **all** possible inputs, not just specific test cases. We use `proptest` to generate thousands of test cases automatically.

## Test Coverage

### 1. Determinism (`prop_deterministic_transitions`)

**Property**: Same sequence of transitions always produces the same final state.

**Invariant**: Given the same sequence of valid transitions applied to two identical FSMs, both should end up in the same state with identical histories (excluding timestamps).

**Tested**: 1000 cases

### 2. Idempotence (`prop_transition_idempotence`)

**Property**: Attempting the same transition twice has no additional effect.

**Invariant**: After a successful transition, attempting the same transition again should either fail or have no additional effect on the state machine.

**Tested**: 1000 cases

### 3. Termination (`prop_all_paths_terminate`, `prop_terminal_states_are_terminal`)

**Property**: All valid transition sequences eventually reach a terminal state.

**Invariants**:
- Following any valid sequence of transitions should either reach a terminal state or exhaust all possible transitions
- Terminal states have no outgoing transitions
- No infinite loops: transition count is bounded

**Tested**: 2000 cases (1000 each)

### 4. Invariant Preservation (`prop_only_valid_transitions_succeed`, `prop_allowed_transitions_are_valid`, `prop_non_terminal_states_have_transitions`)

**Property**: Only valid transitions according to FSM rules are allowed.

**Invariants**:
- The FSM rejects any transition that violates its rules
- Allowed transitions from any state are actually valid according to the FSM rules
- Non-terminal states always have at least one valid transition
- FSM maintains consistency between current state and allowed transitions

**Tested**: 3000 cases (1000 each)

### 5. Receipt Chain Integrity (`prop_history_chain_is_valid`, `prop_history_timestamps_monotonic`, `prop_artifacts_preserved_in_history`)

**Property**: Transition history forms a valid chain with integrity guarantees.

**Invariants**:
- Each transition's 'to' state matches the next transition's 'from' state
- The first transition always starts from `Submitted`
- The last transition's 'to' state matches the current state
- Timestamps are monotonically increasing
- Artifacts attached to transitions are preserved in history

**Tested**: 3000 cases (1000 each)

### 6. Refusal Correctness (`prop_invalid_transitions_rejected`, `prop_guards_can_reject_transitions`, `prop_transition_count_monotonic`)

**Property**: Invalid transitions are rejected with appropriate errors.

**Invariants**:
- Invalid transitions return appropriate error types (`InvalidTransition`, `TransitionFromTerminalState`)
- Guard functions can reject otherwise valid transitions
- Transition count never decreases (monotonic)
- Rejected transitions don't modify state

**Tested**: 3000 cases (1000 each)

### Additional Properties

#### Message Preservation (`prop_messages_preserved_in_transitions`)
- Messages attached to transitions are preserved in the history

#### Custom Transition Rules (`prop_custom_transitions_respected`)
- Custom transition rules provided at FSM creation are respected

#### Task ID Immutability (`prop_task_id_immutable`)
- Task ID remains unchanged throughout the FSM lifecycle

**Tested**: 3000 cases (1000 each)

## Running the Tests

```bash
# Run all property tests
cargo test --lib construct::tests::proptest --all-features

# Run a specific property test
cargo test --lib construct::tests::proptest::prop_deterministic_transitions --all-features

# Run with verbose output
cargo test --lib construct::tests::proptest --all-features -- --nocapture

# Run with custom case count (default is 1000)
PROPTEST_CASES=10000 cargo test --lib construct::tests::proptest --all-features
```

## Generators

The test suite includes generators for:

- **TaskState**: All valid task states (submitted, working, completed, etc.)
- **Message**: Valid A2A protocol messages with random IDs and parts
- **Artifact**: Valid artifacts with parts and metadata
- **Part**: Text and file parts with valid content
- **Transition Sequences**: Valid sequences of state transitions

## Test Configuration

- **Default cases per test**: 1000
- **Total test cases**: 15,000+ across all properties
- **Timeout**: Default proptest timeout (no custom timeout set)

## Key Insights

1. **FSM Rules are Sound**: The state machine correctly enforces all transition rules
2. **History is Reliable**: The transition history forms a valid, unbroken chain
3. **Guards Work**: Custom guard functions can override default behavior
4. **Deterministic**: Same inputs always produce same outputs (modulo timestamps)
5. **No Leaks**: State and history are properly maintained across all operations

## Future Enhancements

- [ ] Test concurrent transitions (if FSM becomes async)
- [ ] Test serialization/deserialization round-trips
- [ ] Test with larger artifact payloads
- [ ] Test memory usage properties
- [ ] Test performance invariants (bounded operation time)
