<h1 align="center">fluidise-relay</h1>

<p align="center">
  <strong>Standalone, agent-first Git hook manager designed for deterministic orchestration, safety, and strict process control.</strong>
</p>

<p align="center">
    <a href="https://www.rust-lang.org">
        <img src="https://img.shields.io/badge/rustc-1.88%2B-informational?style=flat&labelColor=24141E&color=603840&logo=Rust&logoColor=white" alt="Rust 1.88+"></a>
    <a href="https://github.com/Fluidise/relay/actions/workflows/ci.yml">
        <img src="https://img.shields.io/github/actions/workflow/status/Fluidise/relay/ci.yml?branch=main&style=flat&labelColor=24141E&color=603840&logo=GitHub%20Actions&logoColor=white" alt="CI Status"></a>
    <a href="LICENSE">
        <img src="https://img.shields.io/badge/license-MIT-informational?style=flat&labelColor=24141E&color=603840" alt="MIT License"></a>
</p>

`fluidise-relay` is an agent-first Git hook manager written in Rust. It safely orchestrates Git hooks (focusing on `commit-msg` in V1), enforces explicit declarative configuration in `fluidise.toml`, provides structured JSON diagnostics by default, and guarantees process group lifecycle cleanup.

---

## Key Features

- **Agent-First Inspection & Approval**: Every mutation or execution plan is inspected and explicitly approved before activation.
- **Strict Process Control**: POSIX process groups, bounded timeouts (default 60s), and graceful/forced cleanup preventing orphan child processes.
- **Zero Foreign Runtimes**: Pure Rust implementation with zero shell dependencies and zero Node.js/npm requirements.
- **Deterministic Hook Execution**: Sequential execution with fail-fast semantics, bounded stdout/stderr capture budgets (256 KiB), and structured JSON diagnostics.
- **Worktree & Repository Safety**: Isolated hook installations with safe backup and conflict resolution.

---

## Quick Start

### Building from Source

```bash
cargo build --release
```

### Checking the CLI

```bash
cargo run -- --help
cargo run -- --version
```

---

## Local Development & Quality Gates

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Linter check
cargo clippy --all-targets -- -D warnings

# 3. Test suite
cargo test --all-targets

# 4. Install local commit-msg hook
./scripts/install-hooks.sh
```

---

## Architecture & Specifications

Detailed architectural decisions, contracts, and scoping records are maintained in [`Fluidise/core`](https://github.com/Fluidise/core):

- [`docs/hook-manager-v1.md`](https://github.com/Fluidise/core/blob/main/docs/hook-manager-v1.md): V1 scoping and technical contract.
- [`docs/dx-projects.md`](https://github.com/Fluidise/core/blob/main/docs/dx-projects.md): Fluidise DX tooling roadmap.

---

## License

This project is licensed under the [MIT License](LICENSE).
