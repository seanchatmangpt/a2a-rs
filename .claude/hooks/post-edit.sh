#!/bin/bash
# Async: format + check after edits, report back via systemMessage
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ "$FILE_PATH" != *.rs ]]; then
  exit 0
fi

cd "$CLAUDE_PROJECT_DIR" || exit 0

# Format
cargo fmt --all -- --quiet 2>/dev/null

# Check compilation
CHECK_OUTPUT=$(cargo check --workspace 2>&1)
CHECK_EXIT=$?

if [ $CHECK_EXIT -ne 0 ]; then
  # Extract just the error lines, not the full output
  ERRORS=$(echo "$CHECK_OUTPUT" | grep -E "^error" | head -5)
  jq -n --arg errors "$ERRORS" '{
    systemMessage: ("Compilation errors after editing: " + $errors)
  }'
else
  exit 0
fi
