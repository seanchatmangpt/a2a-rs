#!/bin/bash
# Enforce hexagonal architecture layer boundaries on writes
INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
NEW_CONTENT=$(echo "$INPUT" | jq -r '.tool_input.content // .tool_input.new_string // empty')

# Only check a2a-rs library files
if [[ "$FILE_PATH" != *"a2a-rs/src/"* ]]; then
  exit 0
fi

# Domain layer must not import from adapter or application
if [[ "$FILE_PATH" == *"/domain/"* ]]; then
  if echo "$NEW_CONTENT" | grep -qE 'use crate::(adapter|application|services)'; then
    jq -n '{
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "Architecture violation: domain/ cannot import from adapter/, application/, or services/. Domain must remain dependency-free."
      }
    }'
    exit 0
  fi
fi

# Port layer must only depend on domain
if [[ "$FILE_PATH" == *"/port/"* ]]; then
  if echo "$NEW_CONTENT" | grep -qE 'use crate::(adapter|application|services)'; then
    jq -n '{
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "Architecture violation: port/ can only depend on domain/. Ports are trait definitions, not implementations."
      }
    }'
    exit 0
  fi
fi

exit 0
