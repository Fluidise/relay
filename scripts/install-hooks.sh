#!/usr/bin/env bash
set -euo pipefail

# install-hooks.sh - Installs local Git hooks dogfooding fluidise-clint

REPO_ROOT="$(git rev-parse --show-toplevel)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

if [ ! -d "$HOOKS_DIR" ]; then
    echo "Error: .git/hooks directory not found at $HOOKS_DIR" >&2
    exit 1
fi

COMMIT_MSG_HOOK="$HOOKS_DIR/commit-msg"

cat > "$COMMIT_MSG_HOOK" << 'EOF'
#!/usr/bin/env bash
# Git commit-msg hook dogfooding fluidise-clint
set -euo pipefail

COMMIT_MSG_FILE="$1"
REPO_ROOT="$(git rev-parse --show-toplevel)"

LINTER=""
if command -v fluidise-clint >/dev/null 2>&1; then
    LINTER="fluidise-clint"
elif [ -x "$REPO_ROOT/../lint/target/release/fluidise-clint" ]; then
    LINTER="$REPO_ROOT/../lint/target/release/fluidise-clint"
elif [ -x "$REPO_ROOT/../lint/target/debug/fluidise-clint" ]; then
    LINTER="$REPO_ROOT/../lint/target/debug/fluidise-clint"
fi

if [ -n "$LINTER" ]; then
    if ! "$LINTER" --file "$COMMIT_MSG_FILE" --human; then
        echo "" >&2
        echo "❌ Commit rejected by fluidise-clint." >&2
        echo "Please update your commit message to adhere to Conventional Commits." >&2
        exit 1
    fi
else
    # Fallback advisory when fluidise-clint binary is not yet compiled or installed
    FIRST_LINE="$(head -n 1 "$COMMIT_MSG_FILE")"
    CONVENTIONAL_REGEX="^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-zA-Z0-9_-]+\))?: .+"
    if ! echo "$FIRST_LINE" | grep -qE "$CONVENTIONAL_REGEX"; then
        echo "⚠️  fluidise-clint binary not found in PATH or ../lint/target/." >&2
        echo "❌ Commit header does not follow Conventional Commits format:" >&2
        echo "   '$FIRST_LINE'" >&2
        echo "Expected: <type>(<scope>): <description>" >&2
        exit 1
    fi
fi
EOF

chmod +x "$COMMIT_MSG_HOOK"
echo "✅ Git commit-msg hook installed successfully at $COMMIT_MSG_HOOK"
