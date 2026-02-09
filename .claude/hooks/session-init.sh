#!/bin/bash
# Inject live project state into Claude's context at session start
INPUT=$(cat)

BRANCH=$(git -C "$CLAUDE_PROJECT_DIR" branch --show-current 2>/dev/null)
DIRTY=$(git -C "$CLAUDE_PROJECT_DIR" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
RECENT=$(git -C "$CLAUDE_PROJECT_DIR" log --oneline -5 2>/dev/null)
FAILING_TESTS=$(cd "$CLAUDE_PROJECT_DIR" && cargo test --workspace --no-run 2>&1 | grep -c "error" || true)

cat <<EOF
Project state:
- Branch: $BRANCH
- Uncommitted changes: $DIRTY files
- Recent commits:
$RECENT
- Compilation errors: $FAILING_TESTS
EOF
