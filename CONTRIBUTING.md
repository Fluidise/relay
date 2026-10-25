# Contributing to fluidise-relay

Thank you for contributing to `fluidise-relay`!

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/) toolchain (MSRV 1.99.1, edition 2024).
- [GitHub CLI (`gh`)](https://cli.github.com/) (recommended for managing PRs).

### Setup Local Environment

1. Clone the repository directly from GitHub:
   ```bash
   git clone https://github.com/Fluidise/relay.git
   cd relay
   ```
2. Build the project:
   ```bash
   cargo build
   ```
3. Install the commit-message hook:
   ```bash
   ./scripts/install-hooks.sh
   ```

## Development Workflow

We follow Trunk-Based Development directly on GitHub.

1. **Create a branch**:
   ```bash
   git checkout -b feat/my-improvement main
   ```
   *Or use a worktree for isolated work*:
   ```bash
   ./scripts/worktree.sh add feat/my-improvement
   ```

2. **Validate your code locally**:
   Ensure all checks pass before committing:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets
   ```

3. **Commit your changes**:
   Write commit messages following the Conventional Commits specification. The local hook will validate your message.
   *(Note: Do not include co-author mentions such as `Co-authored-by: Claude`).*

4. **Submit a Pull Request**:
   Push your branch directly to GitHub and open a Pull Request targeting `main`.
