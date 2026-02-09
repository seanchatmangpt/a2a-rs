#!/bin/bash
# Block dangerous commands
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

if echo "$COMMAND" | grep -qE 'rm -rf|cargo publish|git push.*--force|git reset --hard'; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: "Blocked by safety hook. Use explicit permission for destructive operations."
    }
  }'
else
  exit 0
fi
