# Relay V1 — implementation evidence

Status: local implementation and correction checks completed; independent re-review is pending after an initial rejected snapshot. This is not a release or a claim of complete four-platform compatibility.

## Authorization and scope

The user approved beginning the implementation after the `Fluidise/core` Q1–Q66 scoping and implementation-contract draft, confirmed that `Fluidise/relay` already existed, and selected option 2 authorizing Rust 1.99.1 setup, Cargo dependencies, local compilation and tests. Repository creation, Git shipping and real-repository activation remain excluded.

The source checkout had concurrent README/banner work. Implementation uses an isolated `feat/commit-msg-manager` worktree from verified `main` commit `ee5cadd599a72f80833bc47f9f94fecc9956f0a6`; no README/banner changes were moved, staged or overwritten. The exact final worktree location and final checks are recorded at handoff.

## Implemented responsibilities

- `config`: native TOML reader with strict owned-section validation and effective configuration digests.
- `support`: lossless native bytes, bounded input, canonical typed SHA-256 encoding and private synchronized state writes.
- `repository`: Git discovery, scoped configuration access, atomic single-key Git updates, private locking and hook inventory.
- `lifecycle`: read-only plans, separate common preparation, target activation, explicit delegation, approval, backup retention and journaled recovery.
- `runner`: sequential commands, message-change checks, concurrent nonblocking captures, process groups, bounded cleanup and native interruption.
- `main`: strict CLI, JSON/human rendering, managed-hook display and transport failure handling.

No public library API, extra production build target, unapproved dependency, project-owned unsafe block, CI pipeline change or clint-engine dependency was added. The later explicitly approved nix amendment is recorded below.

## Criterion map

| Criterion | Local tests/evidence | Remaining limits |
| --- | --- | --- |
| C1 | `tests/cli.rs`; strict configuration cases, native-byte unit round trip, spaces/quotes/newlines/Unicode integration paths | Actual non-UTF-8 directory names require Linux; APFS rejects the fixture itself |
| C2 | Read-only/stale-plan tests, effective single approval, no configured execution during inspection, canonical domain/length/order tests | Plan ID is not authentication; arbitrary external writers remain outside exclusive locking |
| C3 | Full lifecycle/no-op/update/removal, linked-worktree isolation, common `core.worktree` migration/restoration, inherited-source drift and pre-existing preparation | Relative hook paths and multi-valued managed keys explicitly refused |
| C4 | Byte-for-byte delegated stdin/streams/status/cwd/script path; symlink source; real local push proves absent `push-to-checkout` retains Git checkout behavior | No continuous observation of newly added source events |
| C5 | Saturated stdout/stderr, raw cap, Base64, external signal, native Relay interruption, zero-exit child retaining descendant pipes | Detached descendants/forced manager death not universally contained |
| C6 | Check-only modification rejection, declared modification, failure preserves earlier changes, remaining commands do not run, FIFO rejection | No inferred Git cleanup or arbitrary external-effect rollback |
| C7 | Nonblocking contention; pending-state/stage recovery; deterministic unit-only failures at prepare/install/uninstall/unprepare persistence phases; interrupted update retires prior delegation safely | No power-cut certification; native Git stale locks require explicit owner resolution |
| C8 | Exact child exit/signal records, boundary configuration/chain cases, escaped NUL output within JSON budget, broken output transport | Diagnostic catalogue and schema are a local V1 surface, not an already published compatibility guarantee |
| C9 | Opt-in `real_clint_through_git_and_direct_execution` uses the actual external clint executable, including real Git success and invalid/operational exits | Deliberately ignored by the ordinary suite unless a real binary is supplied; report its separate execution, not a mocked substitute |
| C10 | Pinned reference-toolchain formatting, Clippy, tests and release build | Rust 1.88 minimum, other native targets, CI, packaging and publication remain unproved/unexecuted |

Fault injection is compiled only into unit tests, not exposed as a production environment variable or CLI feature. It exercises recoverable errors at logical durable phase boundaries, not every possible OS or hardware failure.

## Reproduction

```sh
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 clippy --locked --offline --all-targets -- -D warnings
cargo +1.99.1 test --locked --offline --all-targets
FLUIDISE_CLINT_BIN=/absolute/path/to/fluidise-clint cargo +1.99.1 test --locked --offline --test lifecycle real_clint_through_git_and_direct_execution -- --ignored
cargo +1.99.1 build --locked --offline --release
```

Offline commands require dependencies already cached. Tests create disposable local repositories and deliberately configured fixture commands; no user hook or actual user repository is activated. Git fixture configuration excludes personal/system configuration. Required developer fixture utilities are Git and standard Unix tools such as `/bin/sh`, `sleep`, `cat`, `dd` and `mkfifo`.

## Platform and review status

Observed local platform: macOS 26.7.1, `arm64`, Git 2.53.0, Rust 1.99.1. Rust 1.99.1 was installed only after explicit option-2 authorization. The source manifest's candidate minimum remains 1.88, not yet an executed minimum-toolchain claim.

An initial non-UTF-8-directory fixture failed in the sandbox, then was rerun outside it: the filesystem returned `Illegal byte sequence` at directory creation, before Relay ran. The physical-path test is therefore Linux-specific; lossless native-byte conversion is still unit-tested on macOS, and representable Unicode/quoted/newline paths have integration coverage. This is an environmental limitation, not a reported passing Linux proof.

macOS `x86_64`, Linux `arm64` and Linux `x86_64`, minimum OS/libc/kernel floors, remote CI and distribution remain pending. Do not infer them from the local release build. Review recommendation: run an independent review of trust, installation recovery, filesystem ownership and process lifetime boundaries before considering the local batch complete. The initial independent review rejected its snapshot; no acceptance is claimed for the corrected snapshot.


## Initial local validation — October 20, 2026

Worktree: `/Users/acrazie/Fluidise/relay/.worktrees/commit-msg-manager`, branch `feat/commit-msg-manager`, base `ee5cadd599a72f80833bc47f9f94fecc9956f0a6`. All commands in Reproduction were rerun on the final source snapshot with Rust 1.99.1 and cached dependencies.

- Formatting check and Clippy (`--all-targets -- -D warnings`): passed.
- Ordinary suite: 5 unit tests, 5 CLI tests and 39 lifecycle tests passed; the real-clint test was intentionally ignored in that invocation.
- Separate opt-in integration using `/Users/acrazie/Fluidise/lint/target/debug/fluidise-clint`: passed. This includes actual Git hook execution and direct execution, not only test doubles.
- Total distinct validated tests: 50; no failed tests.
- Locked offline release build: passed; output is a Mach-O `arm64` executable, 702784 bytes. Size identifies this local build only, not a distribution guarantee.
- `git diff --check`: passed. No files were staged and no commit, push, PR, publication or real-repository activation was performed.

Locking uses existing common/private Git administrative directory inodes in that order, without bootstrap lockfile writes. Mutations hold exclusive locks and runtime execution holds shared locks. Native Git config locks remain separate and are never automatically deleted.

C1–C10 have the local evidence mapped above, subject to their explicit limits. Other native targets, the candidate Rust 1.88 minimum and remote CI remain unproved. Independent review is recommended for installation recovery, ownership, concurrency and process boundaries; user choice remains pending.


## First correction batch — October 20, 2026

An independent reviewer demonstrated six defects despite the initial passing checks. Corrections remain inside the approved behavior: force the process-group termination attempt even after direct-child exit and pipe EOF; inspect and bind all registered dormant worktree configurations before enabling the extension; include actual activation in installation no-op eligibility; remove the union of current and previous owned hooks during interrupted-update removal; reject hook symlinks not resolving to regular files; and use the OS effective-identity executable-access check.

Seven regression tests were added, including an actual fault-injected interrupted update followed by removal and harmless linked-worktree configuration changes invalidating a plan. Before correction, the lifecycle regression run demonstrated five failures; the original fault reproduction is also preserved in the independent report. The permission regression is conditional on an unprivileged identity; on this host the denied-access branch ran and passed after correction.

Corrected snapshot checks passed: Rust 1.99.1 formatting, locked offline Clippy with warnings denied, 6 unit tests, 5 CLI tests, 45 ordinary lifecycle tests, and the separately enabled real-clint integration test. Total distinct passing tests: 57. The ordinary suite intentionally ignores only the opt-in clint test. The locked offline release build passed (Mach-O arm64, 719360 bytes); `git diff --check` passed. No dependencies or unsafe code were added. Independent re-review remains a completion gate; its report and final snapshot identity will be recorded in the core implementation contract without changing frozen Relay task files.


## Second correction batch — October 20, 2026

The first re-review rejected its snapshot for two newly demonstrated boundaries: ignored Git includes during restoration checks, and shell delegation altering the inherited environment. Both targeted regressions failed before correction and passed afterward.

Inherited common/global/system hook settings now use Git's include-aware scoped reads and enter installation/removal plan preconditions. Worktree-local included hook settings are refused rather than guessed. Effective target activation is checked before finalizing installation. Delegated entries now link to the same Relay binary and enter its private native ownership-checked dispatch, which directly replaces itself with the original hook; no extra public operation, production binary, shell interpreter, timeout or capture layer was added to delegation. The selected binary must remain available for delegated events. Native symlink publication, pending-stage recovery and removal retain the existing ownership journal.

Five additional tests cover included-source drift without writes, unchanged native environment, delegated native signal and dangling-owned-link removal, refusal of worktree-local hook includes, and staged native-delegate recovery/removal. The environment comparison does not print inherited environment values on assertion failure.

Final local checks passed: reference Rust 1.99.1 formatting and Clippy with warnings denied; 7 unit, 5 CLI and 49 ordinary lifecycle tests; separate real-clint integration; locked offline release build; diff whitespace check. Total distinct passing tests: 62. Ordinary suite ignores only opt-in clint, which was separately executed successfully. Local release: Mach-O arm64, 735968 bytes. No dependencies, project unsafe code, Git delivery or real-repository activation were added. Second independent re-review remains pending and is the final bounded re-review cycle; the authoritative final verdict will be recorded in the core contract without editing this frozen snapshot.


## Additional authorized correction — October 20, 2026

The user explicitly authorized one more targeted correction/re-review cycle after the second re-review demonstrated silent Git-hook absence when the selected Relay executable disappeared. The actual local-push regression failed before correction (push unexpectedly succeeded) and passed afterward for three cases: missing selected path, executable permission removed from its target, and dangling selected stable symlink. All cases now block the real disposable push, emit `relay.unavailable`, leave the destination ref absent and avoid running the original policy; restoring the selected executable restores the original policy's exit-37 rejection.

Delegated events are now independent regular-file copies of the same executable image, published through the existing private atomic-write path and tracked by owned content digests. They are not hard links or separately built binaries. Their native entry validates local ownership and checks executable access to the selected logical Relay path before original-hook execution. Removing or disabling that selected path no longer makes Git skip the installed delegated entry. Native dispatch still preserves the original environment, cwd, arguments, streams and native status. Each event consumes one executable image on disk; an inspected installation update refreshes the copied native entry code. There is no extra public command, production build target, dependency, project-owned unsafe block or shell workaround. The unused symlink-publication helper and launcher variant were removed rather than retained as extension points.

Final corrected-source checks passed with Rust 1.99.1: formatting, locked offline Clippy with warnings denied, all-target tests with the real-clint opt-in enabled (7 unit, 5 CLI, 51 lifecycle; 63 passed, none ignored or failed), locked offline release build and whitespace checks. Local release size: 735904 bytes, Mach-O arm64. Stage recovery, removal while the selected binary is absent, byte-preserving environment/protocol and signal regressions remain passing. Other platforms and the candidate Rust 1.88 minimum remain unproved. Additional independent re-review is pending; its verdict will be recorded in the core contract without editing the frozen Relay task snapshot. No Git shipping or real-repository activation was performed.


## Second additional authorized correction — October 20, 2026

After explicit user authorization for one more targeted correction/re-review, uninstall now presents and hashes the current source event inventory inside its restoration plan. It reuses the same inventory representation as installation; no new layer, state format or dependency was introduced. An old uninstall plan is refused without writes after source-event addition, removal, executable enablement or disablement. A new explicit inspection describes the changed event set and its own approved removal can restore that set. Script-content trust for existing delegated events is unchanged.

The new regression failed before correction because an old plan succeeded after event addition. After correction all four event transitions pass: stale plan returns code 2/`plan.stale` with `changed=false`, saved installation/configuration bytes remain identical, fresh plan identifiers differ and show the current event inventory, and actual disposable Git commits after freshly inspected removal execute only the newly approved executable events.

Final local checks: Rust 1.99.1 formatting, strict locked offline Clippy, all-target tests with actual clint enabled (7 unit + 5 CLI + 52 lifecycle = 64 passed, none failed or ignored), locked offline release build and whitespace checks passed. Native release remains Mach-O arm64, 735904 bytes. Independent re-review is pending; the final verdict belongs in the core contract without editing this frozen snapshot. No shipping or real-repository activation was performed.


## Third additional authorized correction — October 20, 2026

The user authorized one more correction/re-review for two Git semantic gaps. A saved explicit hook-path override is now re-expanded through Git's native path rules during removal inspection/revalidation and compared with the recorded restoration source; HOME-dependent target drift fails with `state.restoration_conflict` before writes. Capability detection reads only the common repository configuration file without includes, matching Git's actual worktree-extension activation rather than an effective global value. A disposable native Git probe also confirmed that an included extension value alone does not load `config.worktree` on Git 2.53.0.

Both reported regressions failed before correction and pass afterward. HOME drift is rejected without changing installation/configuration bytes; restoring the original HOME permits the original inspected removal and preserves its raw `~/hooks` setting. A global extension value alone now yields a real preparation action, followed by successful isolated activation/removal/unpreparation without altering that global fixture file. An additional include-only extension case verifies the same capability boundary and retained included configuration.

Final local checks passed with Rust 1.99.1: formatting, locked offline strict Clippy, all-target tests including actual clint (7 unit + 5 CLI + 55 lifecycle = 67 passed, none failed or ignored), locked offline release and diff whitespace checks. Local release remains Mach-O arm64, 735904 bytes. No new dependency, toolchain, public operation or production target was added. Independent review is pending and will be recorded in the core contract without editing the frozen task files. No Git shipping or real-repository activation was performed.


## Fourth additional authorized correction — October 20, 2026

The user authorized one more targeted correction/re-review for the unsupported-bare migration guard. Common Git booleans now use one native typed reader shared by `core.bare` safety checks and repository worktree-configuration capability. Git owns value interpretation; there is no custom true/false spelling parser. Existing single-value/file-type validation and common-file/no-includes scope remain enforced.

The regression failed before correction for `core.bare=yes`. After correction nine true representations (`true`, `yes`, `on`, `1`, `-1`, `TRUE`, `YeS`, `2`, and valueless implicit true) all reject preparation before writes while both targeted and non-target linked worktrees remain usable. Six false representations (`false`, `no`, `off`, `0`, `FALSE`, and an explicit empty value) retain supported prepare/unprepare behavior and their original raw spelling.

Final host validation passed with Rust 1.99.1: formatting, locked offline strict Clippy, all-target tests with actual clint enabled (7 unit + 5 CLI + 57 lifecycle = 69 passed, none failed or ignored), locked offline release build and whitespace checks. Local release remains Mach-O arm64, 735904 bytes. No dependency, toolchain, public operation, production target or real-repository activation was added. Independent re-review is pending; its verdict will be recorded in the core contract without changing the frozen task files. No Git shipping was performed.


## Fifth authorized correction batch and approved dependency amendment

The user authorized correction of implicit-shell managed execution, Git invocation-path fidelity and the C7 publication evidence gap, then separately approved exact nix 0.31.3 retrieval/build/tests. Cargo retrieved and verified that exact package; its source declares Rust 1.69 and exposes safe POSIX spawn APIs. Only `process` and `signal` are needed/enabled; approved-but-unused `fs` remains off. The lockfile adds only nix 0.31.3 and cfg_aliases 0.2.2, retaining all existing versions. No project unsafe code or runtime tool dependency was added.

Managed execution now calls strict `posix_spawn`, with native byte arguments/environment, an empty stdin, separate captured pipes, its own process group and native wait status. File descriptors are owned and close-on-exec, with action sources duplicated above standard descriptor numbers. The single-operation/single-execution-thread CLI enters the chain's required root cwd only after resolving invocation-relative configuration/message inputs. An executable file without a native header or shebang fails startup, with no fabricated external exit; an explicitly selected shell remains supported. The new regression failed before correction and passed afterward.

Default delegated hook invocation spelling is derived in Git's original routing/cwd context rather than canonicalized into an absolute script path. Explicit source paths retain their inspected absolute form. Original-source provenance is recorded in private state/plans; native path resolution must still identify the inspected directory. Actual Git regressions pass for ordinary commit, explicit administrative path, linked worktree and receive-side events. The prior direct protocol test now compares the default Git invocation spelling, rather than an unrelated absolute direct call. Native environment/streams/status/signal and selected-binary-loss tests remain passing. The default-path behavioral regression failed before correction.

### C7 publication-boundary evidence

Unit-only fault hooks now surround every application-owned persistent publication operation: private directory creation/permissions; staging creation/truncation/permissions; write/copy; file sync; atomic rename; parent sync; file/directory removal and parent sync; and native Git configuration transaction/file-sync/parent-sync boundaries. Directories are private from creation and exact owner permissions are restored despite umask. Incomplete private stages are retained for inspected recovery, not published or silently treated as complete. There is no production fault flag/environment switch.

The complete enumerated matrix successfully injected and recovered **456 before/after failures**:

| Case | Injected failures recovered |
| --- | ---: |
| Prepare | 50 |
| Initial install | 62 |
| Install with native delegation | 74 |
| Install with symlink backup metadata | 62 |
| Update retiring a delegation | 28 |
| Approve | 12 |
| Uninstall | 38 |
| Uninstall with interrupted staged delegation | 42 |
| Uninstall removing newly introduced worktree config | 42 |
| Unprepare | 46 |

Each case asserts an explicit failure at the selected operation, no execution of configured marker commands, unchanged original hook contents, and successful fresh inspected recovery with valid owned active state where applicable. These are subcases within one unit test, not 456 extra top-level tests. Git's internal syscall implementation is not instrumented: its native atomic config operation is the application boundary, with separate real Git-lock preservation tests. Hardware power-loss, arbitrary kernel failures and universal external-writer exclusion are not certified.

### Final candidate checks

On the final source candidate: Rust 1.99.1 formatting and locked offline strict Clippy passed; actual Git invocation-path tests passed; locked offline release passed (Mach-O arm64, **736032 bytes**); whitespace checks passed. Full all-target tests with actual clint enabled passed: **8 unit + 5 CLI + 62 lifecycle = 75 passed**, none failed or ignored. The eight-unit suite, including the complete 456-failure matrix, completed in 361.48 seconds on this run; this is an observation, not a performance promise. Earlier logical-phase tests remain in place as additional evidence.

Independent review remains pending on this candidate. Its final snapshot identity and verdict belong in the core contract without editing these frozen task files. Cross-platform runtime, candidate Rust 1.88, CI and distribution remain unproved. No real-repository activation or Git shipping was performed.

## Managed commit-msg selected-path eligibility correction

The generated managed launcher now requires the selected path to resolve to a regular file as well as be executable before invoking it. Both tests follow symlinks, preserving stable links to regular executables. Availability failures retain the existing JSON stderr and exit-2 protocol. No new dependency or persistence mechanism is introduced.

The new five-case regression covers a missing selected link, non-executable target, dangling link, direct searchable directory and link to a searchable directory. Every case first executes successfully through a stable link whose path contains spaces and an apostrophe. After invalidation it verifies direct-hook exit 2 and JSON-only stderr, an actual Git commit blocked with the same diagnostic, no configured policy execution, and no created commit. The test failed before correction on the directory case (exit 126 and shell stderr), then passed.

Rust 1.99.1 formatting, locked offline strict Clippy, all 63 lifecycle tests with actual clint enabled, locked offline release, and diff checks passed. A full all-target rerun including the unchanged publication fault matrix was started separately; its eventual result and this candidate's independent review verdict are recorded in the core contract. No shipping or real-repository activation was performed. Independent review is pending on this frozen candidate.
