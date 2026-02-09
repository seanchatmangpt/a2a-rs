#!/bin/bash
# Auto-format Rust files after Write/Edit
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ "$FILE_PATH" == *.rs ]]; then
  cargo fmt --all -- --quiet 2>/dev/null
fi

exit 0
