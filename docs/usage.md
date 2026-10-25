# Relay V1 — local usage and boundaries

Relay orchestrates explicitly configured commands for `commit-msg`. It does not contain clint's policy engine, modify `fluidise.toml`, stage changes or provide a server-side contribution gate. Each configured tool remains independently usable.

This implementation is not a published release or a demonstrated four-platform distribution. Build with the pinned Rust 1.99.1 toolchain using `cargo build --locked --release`; users of an eventual distributed binary will not need Cargo. Do not activate actual repository hooks until the separate installation scope is approved.

## Configure the chain

Create the optional suite file in the repository root before asking Relay to activate its own section:

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

Once Relay is installed, missing or invalid configuration blocks execution rather than disabling it. An empty command list is invalid. Foreign tool sections do not participate in execution approval, but the complete file must be valid TOML. Inspection exposes the used and foreign sections.

Commands run in array order from the target worktree root, with empty stdin. A bare program name is searched on inherited `PATH`; a relative program containing `/` is resolved from the configuration directory. Other arguments remain literal. Only the whole `{message-file}` argument is replaced, with an absolute native path; the marker has no escape syntax in V1. No shell is implicit. Invoke `/bin/sh` explicitly if the project genuinely needs shell semantics.

Command identifiers are unique ASCII `[a-z][a-z0-9-]*` of at most 64 characters. The program and argument array are required; the array may be empty. Modification permission defaults to false. A declared modifier can precede clint; clint should be last. Unexpected message modification by a check-only command blocks the remaining chain and retains the modified file. There is no rollback of arbitrary external effects.

Clint reads the supplied file without reproducing Git's final message cleanup. Clint requires an explicit policy source: `--config <path>` (such as `fluidise.toml`) to read repository rules without Git discovery, or `--defaults` to enforce standard conventional commits. Relay does not promise identical validation of the final cleaned commit text across all Git modes. Local hooks can also be bypassed by Git mechanisms such as `--no-verify`; this is not server-side enforcement.

## Inspect before each mutation

All operations use JSON by default. `--human` is an explicit alternate display. `--help` and `--version` must be used alone. A relative `--config` or `--file` path is resolved from the invocation directory, not the `--repo` directory.

The following commands describe the workflow; they are not permission to install hooks in an existing user repository. Substitute the inspected plan identifier rather than reusing a stale value:

```sh
fluidise-relay inspect --operation prepare --repo /path/to/worktree
fluidise-relay prepare --repo /path/to/worktree --plan <prepare-id>

fluidise-relay inspect --operation install --repo /path/to/worktree --binary /stable/path/fluidise-relay
fluidise-relay install --repo /path/to/worktree --binary /stable/path/fluidise-relay --plan <install-id>
```

`<prepare-id>` and `<install-id>` are placeholders, not literal shell input. Preparation and installation are distinct; inspect again after preparation changes repository state. The selected binary path must be absolute and point to the running Relay executable or a byte-identical copy. A stable symlink to it is supported without rewriting that logical reference to a versioned target. An unrelated successful executable cannot masquerade as Relay during installation inspection. Subsequent executable changes remain within the documented environment trust boundary, not cryptographically pinned runtime approval.

Inspection writes no state, backup, lock or plan file and never launches configured programs. It checks that they are present and executable. Initial installation records approval of the inspected chain. Plans bind the operation, target, relevant configuration/state and owned-hook fingerprints. They are preconditions, not authentication or human signatures. A changed plan is refused; there is no force bypass.

Git's default hook directory is shared among worktrees. `prepare` explicitly enables worktree-specific configuration and migrates supported common `core.worktree` settings. `install` selects a private hook directory only for the targeted worktree. Other executable hook events present during inspection are delegated to their original scripts with their original cwd, environment, arguments, streams and status; they do not become Relay-managed command chains. Absent hooks remain absent, including `push-to-checkout`.

Adding/removing a source event or changing its executable status requires an inspected `install` update. Newly added events are not automatically active, and Relay cannot continuously observe an event it does not intercept. Changes to the contents of already trusted external scripts are not integrity-pinned.

## Approve a changed configuration

```sh
fluidise-relay inspect --operation approve --repo /path/to/worktree
fluidise-relay approve --repo /path/to/worktree --plan <approve-id>
fluidise-relay run --repo /path/to/worktree --file /path/to/message
```

`approve` requires an active installation. Use `--config /path/to/alternate.toml` consistently on inspection and approval to select another source. Installed hooks read the source recorded in local state, not an obsolete hardcoded source. Only one approval is active per worktree: after approving B, returning to A requires approval again. Comments, formatting and foreign sections do not change the effective approval digest.

Neither approval nor its digest is stored in versioned configuration. They are local to the worktree's Git metadata; cloning does not inherit authorization. A run reads and validates one snapshot and does not introduce commands through intermediate configuration rereads. Scripts and binaries remain trusted repository/environment code, not sandboxed or runtime-integrity-pinned.

## Results and execution budgets

- Exit `0`: success or no change needed.
- Exit `1`: an external command exited nonzero or was terminated by a signal. Its exact `exit_code` or `signal` is preserved without inventing universal tool semantics.
- Exit `2`: invocation, configuration, approval, state, installation, spawn/capture or timeout error.
- Interruption of Relay itself attempts cleanup and native signal termination; forced death may prevent cleanup or diagnostics.

Direct CLI results are JSON on stdout. Successful managed hooks are silent; failures emit JSON on stderr. Command captures are separate records with explicit UTF-8 or Base64 encoding, truncation and incompleteness flags. They are not raw text mixed into the JSON. Delegated hooks preserve their original protocol streams instead.

Default execution time is 60 seconds for the whole chain, configurable as a positive integer. Timeout or cancellation initiates process-group termination with at most two additional seconds of cleanup waiting: graceful termination, then forced termination after one second when needed. Capturing output is part of the execution budget; a descendant retaining a pipe cannot force an unbounded read-to-end. Detached descendants and OS scheduling/uninterruptible I/O are outside a universal termination or hard real-time guarantee. Commands are foreground work, not a background-job facility.

Fixed limits: 1 MiB configuration, 64 commands, 20 diagnostics, 256 KiB raw captured external output across the whole chain, and 2 MiB JSON. Excess external output is drained without retaining it within the active deadline. Truncation alone does not make a successful command fail. Logs can contain secrets; no automatic redaction is claimed.

## Removal and interrupted operations

Inspect `uninstall`, then pass its identifier to `uninstall`. This restores the prior targeted activation, revokes approval and retains backups. It works without valid TOML. External changes to owned hooks or restoration provenance are conflicts, not permission to overwrite. Inspect `install` or `uninstall` to resume a recognizable interrupted installation. Inconsistent state blocks execution; commits do not repair it automatically.

Ordinary uninstall keeps common preparation in place. Inspect and approve `unprepare` separately only after reviewing other consumers; Relay refuses while a known Relay installation remains. Pre-existing preparation is not owned and is not removed. There is no automatic backup purge or cleanup command in V1.

Relay takes nonblocking advisory locks on existing Git administrative directory descriptors: common directory first, then the private worktree directory, skipping the second lock when both are identical. Mutations use exclusive locks; execution uses shared locks. No bootstrap lockfile is created. These locks coordinate Relay operations only and do not constrain unrelated writers that ignore them. Native Git configuration lockfiles are never deleted automatically. If one is present, inspection reports its path: verify its owner and resolve it separately, then inspect again. A forced process death may leave such a lock even when Relay's phase journal is recognizable.

## Explicit capability limits

- macOS/Linux are targets; Windows is outside V1. Consult actual evidence before claiming any OS/architecture or minimum OS support.
- Bare repositories are not supported by this batch.
- Relative `core.hooksPath` is refused rather than guessing event-specific cwd semantics. A separately approved migration to an absolute source is required.
- Git keys Relay modifies must have at most one value in their managed file. Multiple values, `core.bare=true`, conflicting common/main-worktree settings and unsupported file layouts require separate review rather than heuristic migration.
- Hook sources may be regular files or links resolving to regular executable files for delegated events. Special files are rejected. Message regular-file checks use nonblocking probes so a FIFO does not hang inspection of its type.
- Unknown metadata versions, missing state behind existing Relay activation, replaced hooks and unresolvable restoration drift fail closed.

These are safety refusals, not evidence of transparent compatibility with every existing hook manager or Git configuration.


Preparation inspects and fingerprints every registered worktree's `config.worktree`, including missing files. When enabling the extension, dormant settings are refused rather than silently activated; the supported exception is the main worktree's `core.worktree` value matching the explicitly inspected common migration. Empty or comment-only dormant files are harmless but still bind the plan. Existing enabled worktree configuration is not treated as dormant. Executability checks use the OS effective-identity access check, not a union of permission bits. Hook symlinks must resolve to regular files.


Non-`commit-msg` delegation entries are independent regular-file copies of the same Relay executable, not symlinks, hard links, shell wrappers or separately built binaries. Git can still enter them when the selected Relay path is missing, is a dangling symlink or lacks executable access; they then emit `relay.unavailable` and fail without executing the original hook. The private native entry validates local ownership and selected-path availability before replacing itself with the original hook without changing its environment, cwd, arguments or streams. Missing or inactive ownership fails closed. The managed `commit-msg` entry remains the silent-success launcher described above.

Each delegated event stores one executable image; installation updates refresh these copies and their ownership digests. This storage cost avoids silently turning a required hook into an absent Git event when the selected executable disappears. A stable selected symlink retains its logical path; refreshing the native entry code after a Relay upgrade requires an explicit inspected installation update. Deleting or disabling the owned hook entries themselves remains ordinary local Git-hook tampering, not a supported enforcement boundary.

Inherited common/global/system hook configuration is queried with Git includes enabled and bound into installation/removal plans. A changed inherited restoration source blocks removal without writes. Worktree-local included `core.hooksPath` settings are explicitly unsupported and require separately reviewed migration; Relay does not silently let included values override its target activation.


Removal inspection also presents the current inventory of the source hooks it will reactivate. Adding, removing or changing executable eligibility of a source event invalidates an older uninstall plan without writes. Inspect removal again and explicitly apply that new plan to restore the newly inspected event set. Existing delegated script contents remain inside the documented trust boundary; event presence and executable eligibility are not treated as mere script-content changes.


Worktree-configuration capability is checked in the common repository configuration file with includes disabled, matching Git's repository-extension activation. A global or included extension value alone is not preparation. Removal re-expands any saved explicit hook override through Git's native pathname rules in the current environment; if that would select a different source (for example after `HOME` changes for `~/hooks`), removal refuses the restoration conflict without writes. Restore the original environment or review that migration separately; Relay does not guess a new restoration target.


Managed Git booleans are interpreted by Git's native boolean reader in their owning common configuration file. Unsupported `core.bare=true` migrations are refused for every true spelling Git accepts, including case variants, `yes`, `on`, numeric true values and valueless settings. False spellings remain supported and are not rewritten. The same reader validates repository worktree-configuration capability without consulting global/include-only extension values.


Managed commands use strict native POSIX spawning, not an execvp-family implicit shell fallback. An executable text file without a native header or shebang fails startup; select `/bin/sh` explicitly if shell interpretation is intended. Native delegated Git events remain separate from this managed-chain policy: they retain Git's original invocation path spelling and event protocol. Default hook paths are derived in the event's original Git routing/cwd context, while explicit configured source paths retain their inspected absolute form.

Fault-injection hooks are compiled only into unit tests. They surround owned directory creation/permissions, file creation/truncation/permissions, writes/copies, file/directory synchronization, atomic renames, removal, and the native Git configuration transaction boundary. The test matrix enumerates before/after errors for preparation, initial/update installation, approval and removal/reversal, including copied delegates, symlink backup metadata and interrupted stages. It verifies explicit errors, no implicit configured execution, retained original hooks and successful fresh inspected recovery. This is application-boundary error testing, not power-cut certification or instrumentation of Git's internal implementation.
