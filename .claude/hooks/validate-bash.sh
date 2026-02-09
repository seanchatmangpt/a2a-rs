#!/bin/bash
# Gate dangerous bash commands with structured PreToolUse output
INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

# Block destructive operations
if echo "$COMMAND" | grep -qE 'rm -rf|cargo publish|git push.*--force|git reset --hard|git clean -f'; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: "Blocked: destructive operation. Run manually if intentional."
    }
  }'
  exit 0
fi

# Flag database migrations for review
if echo "$COMMAND" | grep -qE 'sqlx migrate|diesel migration'; then
  jq -n '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "ask",
      permissionDecisionReason: "Database migration detected. Confirm before executing.",
      additionalContext: "This is a database migration. Ensure you have reviewed the migration SQL and have a rollback plan."
    }
  }'
  exit 0
fi

exit 0
