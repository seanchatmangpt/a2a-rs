# Fuzz Testing for CONSTRUCT Station System

This directory contains fuzz targets for testing the robustness of the CONSTRUCT station implementation using libFuzzer.

## Overview

The fuzz targets validate that:
1. **Stations never panic** on invalid inputs
2. **Guards properly reject** malformed packets with typed refusals
3. **Admission control** is deterministic and consistent
4. **Step operations** maintain state consistency
5. **All error paths** return `RefusalReceipt` instead of panicking

## Fuzz Targets

### `station.rs`

Comprehensive fuzz testing of all station implementations:

- `SendMessageStation` - Message sending with task creation
- `GetTaskStation` - Task retrieval
- `CancelTaskStation` - Task cancellation
- `ListTasksStation` - Task listing with filters
- `SendStreamingMessageStation` - Streaming message handling
- `TaskResubscribeStation` - Task resubscription
- `SetPushNotificationConfigStation` - Push notification configuration
- `GetPushNotificationConfigStation` - Config retrieval
- `ListPushNotificationConfigsStation` - Config listing
- `DeletePushNotificationConfigStation` - Config deletion
- `StationRegistry` - Dynamic dispatch testing

The fuzzer generates random typed packets with varying:
- Message IDs, task IDs, context IDs
- Message content (including empty, very long, special characters)
- Task states and transitions
- Notification URLs (malformed, empty, etc.)

## Prerequisites

Fuzzing requires nightly Rust:

```bash
rustup install nightly
```

The `rust-toolchain.toml` file in this directory automatically selects nightly when building fuzz targets.

## Running Fuzz Tests

From the `a2a-rs` directory:

```bash
# Build the fuzz target
cargo fuzz build station

# Run with default options (runs indefinitely until crash found)
cargo fuzz run station

# Run for specific duration
cargo fuzz run station -- -max_total_time=300  # 5 minutes

# Run with corpus
cargo fuzz run station corpus/station/

# Generate coverage report
cargo fuzz coverage station
```

## Corpus Management

The fuzzer maintains a corpus of interesting inputs:

- `corpus/station/` - Inputs that increase coverage
- `artifacts/station/` - Inputs that caused crashes/failures

To use a saved corpus:

```bash
cargo fuzz run station corpus/station/
```

## Invariants Tested

The fuzz target validates these invariants:

1. **No Panics**: All operations return Result, never panic
2. **Valid Refusals**: Failed operations return typed `RefusalReceipt`
3. **State Consistency**:
   - Task count never negative
   - All tasks have non-empty IDs
   - Context IDs are always set
4. **Admission Determinism**: Same input → same admission decision
5. **Guard Coverage**: All guard types properly reject invalid inputs

## Expected Behavior

### Successful Fuzzing

The fuzzer should:
- Generate millions of inputs without crashes
- Exercise all station code paths
- Find edge cases in parsing/validation
- Build a diverse corpus of valid/invalid inputs

### Finding Bugs

If the fuzzer finds an issue:

1. Crash artifact saved to `artifacts/station/`
2. Minimized test case created
3. Review the crashing input
4. Add regression test to `src/construct/tests/`
5. Fix the underlying issue

## Integration with CI

To run fuzzing in CI (GitHub Actions):

```yaml
- name: Fuzz Testing
  run: |
    cd a2a-rs
    cargo +nightly fuzz run station -- -max_total_time=60 -max_len=4096
```

## Troubleshooting

### Compilation Errors

If you see compilation errors, ensure:
1. The main `a2a-rs` crate builds: `cargo build --all-features`
2. Nightly toolchain is installed: `rustup install nightly`
3. You're in the correct directory

### Slow Fuzzing

Optimize fuzzing speed:

```bash
# Increase number of jobs
cargo fuzz run station -- -jobs=8

# Reduce input length
cargo fuzz run station -- -max_len=1024
```

### Memory Issues

If fuzzing runs out of memory:

```bash
# Limit RSS (resident set size)
cargo fuzz run station -- -rss_limit_mb=2048
```

## Architecture Notes

### Why Fuzzing Matters for CONSTRUCT

CONSTRUCT stations are deterministic finite state machines that must NEVER panic. Fuzzing validates this critical property:

1. **No unwrap()**: Library code uses `?` for all fallible operations
2. **Typed Errors**: All failures return `RefusalReceipt`, not opaque panics
3. **Guard Validation**: Every input constraint has a guard that can be fuzzed
4. **State Machine Safety**: Invalid transitions rejected deterministically

### Fuzz Input Design

The fuzzer generates structured inputs representing valid A2A protocol operations. This is more effective than completely random bytes because:

1. Exercises real code paths (not just early validation)
2. Finds semantic bugs (not just crashes)
3. Builds useful corpus for regression testing
4. Tests guard logic at the semantic level

## References

- [libFuzzer Documentation](https://llvm.org/docs/LibFuzzer.html)
- [cargo-fuzz Guide](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [CONSTRUCT Station Architecture](/home/user/a2a-rs/a2a-rs/src/construct/station.rs)
- [Guards System](/home/user/a2a-rs/a2a-rs/src/construct/guards/mod.rs)
