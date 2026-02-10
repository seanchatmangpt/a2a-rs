#!/bin/bash
# Example usage of a2a-construct CLI

set -e

echo "=== A2A CONSTRUCT CLI Example ==="
echo ""

# Build the CLI first
echo "Building CLI..."
cargo build --release -p a2a-construct-cli
CLI="./target/release/a2a-construct"

# Clean up any previous state
rm -f example-state.json example-receipts.json

echo ""
echo "1. Create a new task with an initial message"
$CLI run \
  --operation create-task \
  --task-id task-example-001 \
  --context-id workflow-demo \
  --message "Start processing user request" \
  --priority high \
  --save-state example-state.json \
  --save-receipts example-receipts.json

echo ""
echo "2. Inspect the state"
$CLI inspect --state example-state.json --detailed

echo ""
echo "3. Send additional message"
$CLI run \
  --operation send-message \
  --task-id task-example-001 \
  --message "Processing complete" \
  --state-file example-state.json \
  --save-state example-state.json

echo ""
echo "4. Update task state to completed"
$CLI run \
  --operation update-state \
  --task-id task-example-001 \
  --state completed \
  --state-file example-state.json \
  --save-state example-state.json

echo ""
echo "5. Show final state statistics"
$CLI inspect --state example-state.json --stats-only

echo ""
echo "6. Validate invariants"
$CLI validate --state example-state.json

echo ""
echo "7. Show state as JSON"
$CLI inspect --state example-state.json --json | jq '.stats'

echo ""
echo "=== Example complete ==="
echo "State saved to: example-state.json"
echo "Receipts saved to: example-receipts.json"
