<p align="center">
  <img src="docs/assets/banner.png" alt="Fluidise Relay — emerald tidal currents flowing between coastal islands" width="100%">
</p>

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

`fluidise-relay` is a standalone Git hook manager written in Rust. V1 orchestrates `commit-msg` commands declared in `fluidise.toml`, with explicit read-only plan inspection before lifecycle mutations and structured JSON diagnostics by default. Relay enforces deterministic execution; external commands own commit policy.

## Table of Contents

- [Quick Start](#quick-start)
- [Key Features](#key-features)
- [Benchmarks at a Glance](#benchmarks-at-a-glance)
- [Installation](#installation)
  - [Prerequisites](#prerequisites)
  - [Building from Source](#building-from-source)
- [CLI Usage](#cli-usage)
  - [Syntax and Operations](#syntax-and-operations)
  - [Command Options](#command-options)
  - [Exit Codes](#exit-codes)
  - [JSON Output Schema](#json-output-schema)
  - [Human-Readable Output](#human-readable-output)
- [Configuration (`fluidise.toml`)](#configuration-fluidisetoml)
  - [Discovery and Precedence](#discovery-and-precedence)
  - [Schema and Parameters](#schema-and-parameters)
  - [Argument Substitution](#argument-substitution)
- [Operational Guarantees and Boundaries](#operational-guarantees-and-boundaries)
  - [POSIX Process Groups and Deadline Termination](#posix-process-groups-and-deadline-termination)
  - [Bounded Capture Budget (256 KiB)](#bounded-capture-budget-256-kib)
  - [Worktree Isolation and Safe Recovery](#worktree-isolation-and-safe-recovery)
  - [Two-Phase Approval Boundary](#two-phase-approval-boundary)
- [Local Development and Quality Gates](#local-development-and-quality-gates)
- [Architecture and Specifications](#architecture-and-specifications)
- [License](#license)

---

## Quick Start

### 1. Declare hooks in `fluidise.toml`

Create `fluidise.toml` at the root of the repository you want Relay to manage:

```toml
[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "commit-policy"
program = "fluidise-clint"
args = ["--file", "{message-file}", "--config", "fluidise.toml"]
modifies-message = false
```

### 2. Inspect and apply installation

Relay enforces an agent-first lifecycle: mutations require a plan ID obtained from an inspection:

```bash
RELAY="$(pwd)/target/release/fluidise-relay"

# 1. Prepare shared worktree configuration
PLAN=$( "$RELAY" inspect --operation prepare | jq -r '.plan.id' )
"$RELAY" prepare --plan "$PLAN"

# 2. Install Relay for the current worktree
PLAN=$( "$RELAY" inspect --operation install --binary "$RELAY" | jq -r '.plan.id' )
"$RELAY" install --binary "$RELAY" --plan "$PLAN"
```

### 3. Run against a commit-message file

Git triggers Relay automatically on subsequent commits. You can also run the chain directly:

```bash
fluidise-relay run --file .git/COMMIT_EDITMSG
```

Output (`stdout`, exit code `0`):

```json
{
  "schema_version": 1,
  "operation": "run",
  "status": "success",
  "diagnostics": [],
  "diagnostics_complete": true,
  "commands": [
    {
      "id": "commit-policy",
      "outcome": "success",
      "exit_code": 0,
      "stdout": {
        "data": "{\"schema_version\":1,\"status\":\"valid\",\"diagnostics\":[]}\n",
        "encoding": "utf8",
        "truncated": false,
        "incomplete": false
      },
      "stderr": {
        "data": "",
        "encoding": "utf8",
        "truncated": false,
        "incomplete": false
      }
    }
  ]
}
```

When an external hook command fails (exit code `1`):

```json
{
  "schema_version": 1,
  "operation": "run",
  "status": "failed",
  "diagnostics": [],
  "diagnostics_complete": true,
  "commands": [
    {
      "id": "commit-policy",
      "outcome": "failed",
      "exit_code": 1,
      "stdout": { "data": "...", "encoding": "utf8", "truncated": false, "incomplete": false },
      "stderr": { "data": "...", "encoding": "utf8", "truncated": false, "incomplete": false }
    }
  ]
}
```

---

## Key Features

- **Agent-First Inspection & Approval**: Read-only inspection returns a cryptographic SHA-256 plan ID. Lifecycle mutations require that ID and reject stale plans. Changing effective commands requires explicit re-approval.
- **Strict Process Control**: POSIX process groups (`process_group(0)`), bounded execution timeouts, and graceful (`SIGTERM`) followed by forced (`SIGKILL`) termination to eliminate orphan background processes.
- **Zero Foreign Runtimes**: Pure Rust implementation with zero shell dependencies, no Node.js/npm requirements, and no Python virtualenvs.
- **Deterministic Hook Execution**: Sequential execution with fail-fast semantics, bounded stdout/stderr capture budgets (256 KiB), and structured JSON diagnostics on standard output.
- **Worktree & Repository Safety**: Isolated hook installations via Git `extensions.worktreeConfig` with safe backups, conflict detection, and resumable recovery.

---

## Benchmarks at a Glance

Evaluated directly against the leading Git hook managers across Rust, Go, Node.js, and Python:

| Metric | `fluidise-relay` | `hk` (Rust) | `lefthook` (Go) | `husky` (Node.js) | `pre-commit` (Python) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Orchestration Latency** | **44.0 ms** | 38.5 ms | 63.4 ms *(1.4x slower)* | 15.5 ms *(raw shim)* | 207.4 ms *(4.7x slower)* |
| **Memory Footprint (RSS)** | **7.14 MB** | 15.98 MB *(2.2x RAM)* | 16.06 MB *(2.2x RAM)* | 7.09 MB *(raw shim)* | 35.20 MB *(4.9x RAM)* |
| **CPU Instructions (Retired)** | **31.0 M** | 64.9 M *(2.1x)* | 71.6 M *(2.3x)* | 23.7 M | 945.7 M *(30.5x)* |
| **Loop Throughput (50 commits)** | **24.1 /s** | 23.2 /s | 15.5 /s *(35% slower)* | 59.4 /s *(raw shim)* | 4.6 /s *(5.2x slower)* |
| **Process Group Deadline** | **POSIX killpg (0 orphan)** | None (hangs) | None (hangs) | None (hangs) | None (hangs) |
| **Bounded Capture Budget** | **256 KiB Guaranteed** | Unbounded | Unbounded | Unbounded | Unbounded |
| **Worktree Git Isolation** | **Strict (local)** | Global (`~/.gitconfig`) | Shared (`.git/hooks`) | Repo-wide | Shared (`.git/hooks`) |
| **Machine Output by Default** | **Strict JSON** | ANSI / Spinners | ANSI / Spinners | Plain text | Plain text |
| **Binary / Footprint on Disk** | **0.7 MB (0 dep)** | 28.0 MB | 12.0 MB | 139.0 MB (+ Node.js) | 74.0 MB (+ Python) |

See the complete 5-workload suite, CPU cycle profiling, and agent ergonomics analysis in [docs/benchmarks.md](docs/benchmarks.md).

---

## Installation

### Prerequisites

- Unix-based operating system (macOS or Linux; Windows is not supported).
- [Rust](https://www.rust-lang.org/) toolchain (MSRV 1.88, edition 2024; repository pins Rust 1.99.1 in `rust-toolchain.toml`).
- Git (2.20+) and standard `/bin/sh` for the installed launcher shim.

### Building from Source

```bash
cargo build --release
```

The resulting executable is located at `target/release/fluidise-relay` (~719 KiB). Keep the binary at a stable absolute path, as installed hooks reference it directly.

To verify the build:

```bash
cargo run -- --help
cargo run -- --version
```

---

## CLI Usage

### Syntax and Operations

```text
fluidise-relay inspect --operation <op> [--repo <path>] [--config <path>] [--binary <path>] [--human]
fluidise-relay <mutation> --plan <id> [--repo <path>] [--config <path>] [--binary <path>] [--human]
fluidise-relay run --file <path> [--repo <path>] [--config <path>] [--human]
```

Relay exposes seven explicit lifecycle operations:

| Operation | Type | Purpose | Required Flags |
| :--- | :---: | :--- | :--- |
| `inspect` | Read-only | Evaluates preconditions and computes a deterministic plan ID. | `--operation <name>` |
| `prepare` | Mutation | Enables `extensions.worktreeConfig` and migrates `core.worktree` when necessary. | `--plan <id>` |
| `install` | Mutation | Activates Relay for the worktree and approves its configuration. | `--plan <id>`, `--binary <path>` |
| `approve` | Mutation | Approves new effective execution settings after editing `fluidise.toml`. | `--plan <id>` |
| `run` | Execution | Sequentially executes the approved `commit-msg` command chain. | `--file <path>` |
| `uninstall`| Mutation | Restores previous hook setup, revokes approval, and retains backups. | `--plan <id>` |
| `unprepare`| Mutation | Restores common repository preparation after all worktrees are uninstalled. | `--plan <id>` |

### Command Options

| Option | Description |
| :--- | :--- |
| `--repo <path>` | Target repository directory (defaults to current repository). |
| `--config <path>` | Configuration file path (defaults to repository's `fluidise.toml`). Accepted for `install`, `approve`, and `run`. |
| `--file <path>` | Path to the commit-message file to validate (required for `run`). |
| `--binary <path>` | Absolute path of the Relay executable (required for `install`). |
| `--plan <id>` | Cryptographic SHA-256 plan identifier returned by `inspect`. |
| `--operation <name>`| Target mutation name for `inspect`. |
| `--human` | Opt-in plain-text output instead of JSON. |
| `--help`, `-h` | Print help information and exit. Must be used alone. |
| `--version`, `-v` | Print version information and exit. Must be used alone. |

### Exit Codes

| Exit Code | Meaning |
| :---: | :--- |
| `0` | **Success**: The operation succeeded, or the managed command chain passed without errors. |
| `1` | **Command Failure**: An executed external command returned a non-zero exit code or terminated via signal. |
| `2` | **Operational Error**: Invocation syntax error, configuration parse error, stale plan ID, state conflict, or execution deadline timeout. |

### JSON Output Schema

Direct invocations emit JSON conforming to `schema_version: 1` on standard output:

```json
{
  "schema_version": 1,
  "operation": "run",
  "status": "success",
  "diagnostics": [],
  "diagnostics_complete": true,
  "commands": [
    {
      "id": "commit-policy",
      "outcome": "success",
      "exit_code": 0,
      "stdout": { "data": "...", "encoding": "utf8", "truncated": false, "incomplete": false },
      "stderr": { "data": "", "encoding": "utf8", "truncated": false, "incomplete": false }
    }
  ]
}
```

- `diagnostics`: Array of structured error items with `code`, `message`, and optional `command_id`.
- `diagnostics_complete`: Boolean indicating whether all diagnostics were captured (capped at 20).
- `commands`: Detailed execution outcomes for each step in the chain. Non-UTF-8 output is encoded in `base64`.

### Human-Readable Output

Passing `--human` produces formatted plain text:

```bash
fluidise-relay inspect --operation prepare --human
# Output:
# Operation: prepare
# Target repository: /path/to/repo
# Plan ID: 1bc02b06...
# Actions:
#   - Prepare common worktree configuration, migrating core.worktree when necessary
```

---

## Configuration (`fluidise.toml`)

### Discovery and Precedence

By default, Relay looks for `fluidise.toml` at the root of the managed repository. Passing `--config <path>` overrides discovery. Unrelated top-level sections (e.g. `[commit]` used by `fluidise-clint`) are safely ignored.

### Schema and Parameters

```toml
[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "commit-policy"
program = "fluidise-clint"
args = ["--file", "{message-file}", "--config", "fluidise.toml"]
modifies-message = false
```

- `version` *(integer, required)*: Must be `1`.
- `timeout-seconds` *(positive integer, optional, default: `60`)*: Global deadline budget for the entire command chain.
- `[[hooks.commit-msg]]` *(array of tables, 1 to 64 commands)*:
  - `id` *(string, required)*: Unique ASCII identifier matching `[a-z][a-z0-9-]*` (maximum 64 characters).
  - `program` *(string, required)*: Program to execute. Names without `/` resolve against `PATH`; relative paths containing `/` resolve from the directory containing `fluidise.toml`.
  - `args` *(array of strings, required)*: Explicit argument list.
  - `modifies-message` *(boolean, optional, default: `false`)*: Declares whether the command is permitted to alter the commit-message file.

### Argument Substitution

- `{message-file}`: Replaced with the absolute native path to the commit-message file **only when it occupies an entire argument string**. Substring interpolation and shell expressions are not supported.
- Clint policy selection: `fluidise-clint` requires an explicit policy source: `--config <path>` (such as `fluidise.toml`) to read repository rules without Git discovery, or `--defaults` to use built-in conventional commit rules without a dedicated configuration section.

---

## Operational Guarantees and Boundaries

### POSIX Process Groups and Deadline Termination

- Managed commands run in an isolated foreground POSIX process group (`process_group(0)`).
- When the execution deadline expires, Relay sends `SIGTERM` to the entire process group, followed by a bounded wait (maximum 2 seconds) and `SIGKILL` if descendants fail to exit.
- Guarantees zero orphan child processes or detached background jobs.

### Bounded Capture Budget (256 KiB)

- External command output is subject to a global **256 KiB** raw capture budget across stdout and stderr.
- Excess output is drained continuously to prevent child processes from blocking on full pipes (`SIGPIPE`), but is not retained in memory. Truncation is explicitly reported with `"truncated": true`.
- Total Relay result JSON payload is capped at 2 MiB.

### Worktree Isolation and Safe Recovery

- Relay activates hooks strictly per worktree using Git's `extensions.worktreeConfig` and writes to `.git/config.worktree`.
- The global `~/.gitconfig` is never modified, preserving isolation across independent repositories and worktrees.
- Pre-existing hooks are preserved in durable backups and not chained automatically. Non-`commit-msg` events are delegated natively to their original targets.
- Interrupted lifecycle mutations leave verifiable journal state that can be safely resumed with `install` or `uninstall`.

### Two-Phase Approval Boundary

- Configuration is declarative: reading or inspecting `fluidise.toml` never spawns configured commands.
- Installing Relay approves the exact effective configuration. Editing commands, flags, or message permissions requires running `approve --plan <id>` with a fresh inspection plan.
- Comment-only changes or edits to unrelated sections do not invalidate the active approval.

---

## Local Development and Quality Gates

Run the local quality gates before committing or pushing changes:

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Linter check
cargo clippy --all-targets -- -D warnings

# 3. Test suite
cargo test --all-targets
```

> [!NOTE]
> Tests cover lifecycle plans, configuration approval, message changes, hook delegation, worktree isolation, output limits, timeouts, and recovery (including 456 failure injection points). The integration test exercising real `fluidise-clint` execution is ignored by default and requires `FLUIDISE_CLINT_BIN` pointing to the external binary.
>
> `./scripts/install-hooks.sh` is a development helper that writes `.git/hooks/commit-msg` directly; it is not Relay's production installation workflow.

---

## Architecture and Specifications

Detailed architectural decisions, contracts, and scoping records are maintained in [`Fluidise/core`](https://github.com/Fluidise/core):

- [`docs/hook-manager-v1.md`](https://github.com/Fluidise/core/blob/main/docs/hook-manager-v1.md): V1 scoping and technical contract.
- [`docs/dx-projects.md`](https://github.com/Fluidise/core/blob/main/docs/dx-projects.md): Fluidise DX tooling roadmap.
- Comprehensive competitor benchmarks: [docs/benchmarks.md](docs/benchmarks.md).
- Git delivery policy: [docs/git-workflow.md](docs/git-workflow.md).
- Security policy: [SECURITY.md](SECURITY.md).
- Contribution guide: [CONTRIBUTING.md](CONTRIBUTING.md).

---

## License

This project is licensed under the [MIT License](LICENSE). Copyright (c) 2026 Acrazie.
