# AI Agent Instructions for `fluidise-relay`

## Project Overview

`fluidise-relay` is a standalone, agent-first Git hook manager written in Rust. It safely orchestrates Git hooks (beginning with `commit-msg` in V1), enforces explicit declarative configuration, provides structured JSON output by default, and guarantees process safety and clean cleanup.

- **Stack**: Rust 2024 edition (MSRV 1.99.1), Cargo.
- **Remote & Delivery**: All development and branches go directly to **GitHub** (`Fluidise/relay`).
- **Specifications**: See cross-cutting architecture records in `Fluidise/core` (`docs/hook-manager-v1.md`).

## Authoritative Git Policy

Follow the rules in [docs/git-workflow.md](docs/git-workflow.md):
- **Branching**: Trunk-Based (`main` base; short-lived branches `feat/*`, `fix/*`, `chore/*`, `docs/*`).
- **Commits**: Conventional Commits convention enforced. Validated locally with `fluidise-clint`.
- **Co-authorship**: Never add `Co-authored-by: Claude` or any similar co-author mention in commit messages.
- **Verification before commit/push**:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all-targets`

## Developer Tooling

- Local Git hook setup: `./scripts/install-hooks.sh`
- Situational worktrees: `./scripts/worktree.sh {add|remove|list}`
