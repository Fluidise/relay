#!/usr/bin/env bash
set -euo pipefail

# worktree.sh - Helper script for managing situational Git worktrees

REPO_ROOT="$(git rev-parse --show-toplevel)"
WORKTREES_DIR="$REPO_ROOT/.worktrees"
DEFAULT_BASE="main"

usage() {
    echo "Usage: $0 {add <branch-name> [base-branch]|remove <branch-name>|list}"
    exit 1
}

command_add() {
    local branch="$1"
    local base="${2:-$DEFAULT_BASE}"
    local target_dir="$WORKTREES_DIR/$branch"

    mkdir -p "$WORKTREES_DIR"

    if [ -d "$target_dir" ]; then
        echo "Error: Worktree directory already exists: $target_dir" >&2
        exit 1
    fi

    echo "Creating worktree for branch '$branch' based on '$base'..."
    git worktree add -b "$branch" "$target_dir" "$base"
    echo "✅ Worktree ready at: $target_dir"
}

command_remove() {
    local branch="$1"
    local target_dir="$WORKTREES_DIR/$branch"

    if [ ! -d "$target_dir" ]; then
        echo "Error: Worktree directory not found: $target_dir" >&2
        exit 1
    fi

    echo "Removing worktree: $target_dir..."
    git worktree remove "$target_dir"
    echo "✅ Worktree removed."
}

command_list() {
    git worktree list
}

case "${1:-}" in
    add)
        [ -z "${2:-}" ] && usage
        command_add "$2" "${3:-}"
        ;;
    remove)
        [ -z "${2:-}" ] && usage
        command_remove "$2"
        ;;
    list)
        command_list
        ;;
    *)
        usage
        ;;
esac
