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

`fluidise-relay` is a standalone Git hook manager written in Rust. V1 orchestrates `commit-msg` commands from `fluidise.toml`, with explicit inspection before lifecycle changes and structured JSON diagnostics by default. Relay manages execution; external commands own commit-message policy.

---

## Key Features

- **Inspect Before Applying**: Read-only inspection returns a plan ID. Lifecycle mutations require that ID and reject stale plans. Installation approves the effective hook configuration; subsequent execution changes require renewed approval.
- **Bounded Process Control**: Sequential, fail-fast execution with a shared timeout (60 seconds by default), POSIX process groups, and bounded termination attempts on timeout or interruption.
- **Native Command Execution**: Configured programs run directly, without implicit shell expansion. No Node.js/npm runtime is required; the installed `commit-msg` launcher requires `/bin/sh`.
- **Structured Diagnostics**: JSON results include command exit codes or signals. Captured stdout/stderr share a 256 KiB budget across the chain; excess output is drained without retaining it. Results are limited to 2 MiB.
- **Worktree Isolation & Recovery**: Worktree-local activation, preservation of the previous hook setup, retained backups, conflict detection, and resumable interrupted lifecycle operations.

---

## Quick Start

### Building from Source

The implementation uses Unix-specific APIs; Windows is not supported. Git and `/bin/sh` must be available. The repository pins Rust 1.99.1 in `rust-toolchain.toml`; `Cargo.toml` declares Rust 1.88 as the minimum version, not a validation result for this checkout.

```bash
cargo build --release
```

The default build output is `target/release/fluidise-relay`. Keep the executable at a stable absolute path: installed hooks reference it.

### Checking the CLI

```bash
cargo run -- --help
cargo run -- --version
```

### Configuring commit-msg

Create `fluidise.toml` at the root of the repository you want Relay to manage. This example requires `fluidise-clint` to be installed separately and available on `PATH`:

```toml
[hooks]
version = 1
timeout-seconds = 60

[[hooks.commit-msg]]
id = "commit-policy"
program = "fluidise-clint"
args = ["--file", "{message-file}"]
modifies-message = false
```

Define 1–64 commands with unique IDs matching `[a-z][a-z0-9-]*` (maximum 64 characters). Each command requires an explicit `args` array. Unknown keys inside `[hooks]` or command tables are rejected; unrelated top-level sections are ignored.

`{message-file}` is replaced only when it occupies an entire argument. Programs without `/` resolve through `PATH`; relative program paths containing `/` resolve from the configuration directory. Commands run from the managed repository root.

### Inspecting and Installing

From Relay's source directory, select the built executable and target repository:

```bash
RELAY="$(pwd)/target/release/fluidise-relay"
printf 'Absolute path of the repository to manage: '
read -r REPO

"$RELAY" inspect --operation prepare --repo "$REPO" --human
```

Review the plan before applying it. Enter the displayed plan ID when prompted below:

```bash
read -r PLAN
"$RELAY" prepare --repo "$REPO" --plan "$PLAN"

"$RELAY" inspect --operation install --repo "$REPO" --binary "$RELAY" --human
read -r PLAN
"$RELAY" install --repo "$REPO" --binary "$RELAY" --plan "$PLAN"
```

`prepare` enables common worktree configuration, migrating `core.worktree` when necessary. `install` activates Relay for the selected worktree and approves its effective configuration. The selected binary must be the running Relay executable or a byte-identical copy. If state changes between inspection and application, inspect again rather than reusing a stale ID.

The previous `commit-msg` hook is preserved, not automatically chained. Put required checks explicitly in `fluidise.toml`. Other existing hook events are delegated to their original source; newly added source events require a new installation inspection and update.

### Running, Updating, and Removing

Git invokes Relay on subsequent commits. To run against an existing message file directly:

```bash
printf 'Absolute path of an existing commit-message file: '
read -r MESSAGE
"$RELAY" run --repo "$REPO" --file "$MESSAGE"
```

For each lifecycle change, inspect with `inspect --operation NAME`, review the returned plan, then invoke `NAME --plan ID` with the same applicable options:

| Operation | Purpose |
| --- | --- |
| `prepare` | Prepare shared worktree configuration. |
| `install` | Install or update the selected worktree; requires `--binary` with an absolute path. |
| `approve` | Replace the active execution approval after reviewing configuration changes. |
| `uninstall` | Restore prior activation, revoke approval, and retain backups. |
| `unprepare` | Restore Relay-owned common preparation after installations have been removed. |

`--repo` defaults to the current repository. `--config` applies only to `install`, `approve`, and `run`, including inspection of the first two. Its default is the recorded configuration for an active installation, otherwise the repository's `fluidise.toml`. Relative `--config` and `--file` paths resolve from the invocation directory.

### Output and Safety Limits

- Direct invocations emit JSON on stdout by default; `--human` selects human-readable output. Installed hook execution is silent on success and reports failures on stderr.
- Exit code `0` means success, `1` means an external command failed, and `2` means an invocation, configuration, state, or execution error. External exit codes and signals remain in command records; interruptions received by Relay are re-raised after cleanup.
- Commands are check-only unless `modifies-message = true`. An unexpected message change stops execution, but Relay retains the changed file: it does not roll back message edits or other command side effects.
- Approval covers effective execution settings, not executable contents or a sandbox. Trust configured programs and their environment. Comment-only or unrelated-section changes do not require renewed approval.
- Cleanup targets the command's process group; it is not a guarantee against descendants that escape that group. The timeout covers the command chain, with additional bounded cleanup time.
- External edits to owned hooks or activation settings can block operations. Inspect and resolve conflicts instead of deleting state or forcing replacement.

---

## Local Development & Quality Gates

```bash
# 1. Format check
cargo fmt --all -- --check

# 2. Linter check
cargo clippy --all-targets -- -D warnings

# 3. Test suite
cargo test --all-targets
```

Tests cover lifecycle plans, configuration approval, message changes, hook delegation, worktree isolation, output limits, timeouts, and recovery. The real-Clint integration test is ignored by default and requires `FLUIDISE_CLINT_BIN` pointing to that external executable.

`./scripts/install-hooks.sh` is a separate development helper that writes `.git/hooks/commit-msg` directly; it is not Relay's installation workflow and does not handle linked-worktree `.git` files. Review it before use. See [Contributing](CONTRIBUTING.md), [Git workflow](docs/git-workflow.md), and [Security policy](SECURITY.md).

---

## Architecture & Specifications

Detailed architectural decisions, contracts, and scoping records are maintained in [`Fluidise/core`](https://github.com/Fluidise/core):

- [`docs/hook-manager-v1.md`](https://github.com/Fluidise/core/blob/main/docs/hook-manager-v1.md): V1 scoping and technical contract.
- [`docs/dx-projects.md`](https://github.com/Fluidise/core/blob/main/docs/dx-projects.md): Fluidise DX tooling roadmap.

---

## License

This project is licensed under the [MIT License](LICENSE).
