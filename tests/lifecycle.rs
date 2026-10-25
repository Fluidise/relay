use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "relay-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let f = Self {
            root: fs::canonicalize(root).unwrap(),
        };
        assert!(f.git(&["init", "--quiet", "--template="]).status.success());
        f
    }
    fn git(&self, args: &[&str]) -> Output {
        Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap()
    }
    fn relay(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
            .args(args)
            .arg("--repo")
            .arg(&self.root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> (Value, i32) {
        let out = self.relay(args);
        let value = serde_json::from_slice(&out.stdout)
            .unwrap_or_else(|_| panic!("not JSON: {:?} {:?}", out.stdout, out.stderr));
        (value, out.status.code().unwrap_or(-1))
    }
    fn apply(&self, operation: &str) -> Value {
        let mut args = vec!["inspect", "--operation", operation];
        if operation == "install" {
            args.extend(["--binary", env!("CARGO_BIN_EXE_fluidise-relay")]);
        }
        let (p, code) = self.json(&args);
        assert_eq!(code, 0, "{p}");
        let id = p["plan"]["id"].as_str().unwrap();
        let mut args = vec![operation, "--plan", id];
        if operation == "install" {
            args.extend(["--binary", env!("CARGO_BIN_EXE_fluidise-relay")]);
        }
        let (v, code) = self.json(&args);
        assert_eq!(code, 0, "{v}");
        v
    }
    fn config(&self, script: &str, modifies: bool) {
        let args = json!(["-c", script, "relay-helper", "{message-file}"]);
        fs::write(self.root.join("fluidise.toml"),format!("[hooks]\nversion=1\ntimeout-seconds=2\n[[hooks.commit-msg]]\nid='check'\nprogram='/bin/sh'\nargs={args}\nmodifies-message={modifies}\n")).unwrap();
    }
    fn ready(&self, script: &str, modifies: bool) {
        self.config(script, modifies);
        self.apply("prepare");
        self.apply("install");
        fs::write(self.root.join("message"), "feat: test relay\n").unwrap();
    }
    fn run(&self) -> (Value, i32) {
        self.json(&["run", "--file", self.root.join("message").to_str().unwrap()])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn executable(path: &Path, script: &str) {
    fs::write(path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn install_run_noop_uninstall_and_unprepare() {
    let f = Fixture::new();
    f.ready("printf 'ok'; printf 'err' >&2", false);
    let (v, c) = f.run();
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["commands"][0]["stdout"]["data"], "ok");
    assert_eq!(v["commands"][0]["stderr"]["data"], "err");
    assert_eq!(f.apply("install")["changed"], false);
    assert_eq!(f.apply("approve")["changed"], false);
    f.apply("uninstall");
    assert!(f.root.join(".git/fluidise-relay/backups").exists());
    assert_eq!(f.apply("uninstall")["changed"], false);
    f.apply("unprepare");
    assert!(
        !f.git(&["config", "--get", "extensions.worktreeConfig"])
            .status
            .success()
    );
}
#[test]
fn inspection_is_read_only_and_stale_plan_is_refused() {
    let f = Fixture::new();
    let (p, c) = f.json(&["inspect", "--operation", "prepare"]);
    assert_eq!(c, 0, "{p}");
    assert!(!f.root.join(".git/fluidise-relay").exists());
    assert!(!f.root.join(".git/fluidise-relay-common").exists());
    f.git(&["config", "user.name", "changed"]);
    let (v, c) = f.json(&["prepare", "--plan", p["plan"]["id"].as_str().unwrap()]);
    assert_eq!(c, 2);
    assert_eq!(v["diagnostics"][0]["code"], "plan.stale");
    assert!(!f.root.join(".git/fluidise-relay").exists());
}
#[test]
fn approval_is_effective_and_single_active() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let path = f.root.join("fluidise.toml");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        format!("# harmless\n{original}\n[foreign]\nvalue=42\n"),
    )
    .unwrap();
    assert_eq!(f.run().1, 0);
    f.config("exit 2", false);
    assert_eq!(f.run().0["diagnostics"][0]["code"], "approval.required");
    f.apply("approve");
    let (v, c) = f.run();
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["commands"][0]["exit_code"], 2);
    fs::write(&path, original).unwrap();
    assert_eq!(f.run().0["diagnostics"][0]["code"], "approval.required");
}
#[test]
fn check_only_mutation_blocks_and_retains_the_message() {
    let f = Fixture::new();
    f.ready("printf 'broken' > \"$1\"", false);
    let (v, c) = f.run();
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "message.modified_unexpectedly");
    assert_eq!(
        fs::read_to_string(f.root.join("message")).unwrap(),
        "broken"
    );
}
#[test]
fn declared_mutation_is_allowed() {
    let f = Fixture::new();
    f.ready("printf 'feat: changed' > \"$1\"", true);
    assert_eq!(f.run().1, 0);
}
#[test]
fn external_signal_is_not_an_exit_code() {
    let f = Fixture::new();
    f.ready("kill -TERM $$", false);
    let (v, c) = f.run();
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["commands"][0]["signal"], 15);
    assert!(v["commands"][0].get("exit_code").is_none());
}
#[test]
fn invalid_utf8_is_base64() {
    let f = Fixture::new();
    f.ready("printf '\\377'", false);
    let (v, c) = f.run();
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["commands"][0]["stdout"]["encoding"], "base64");
    assert_eq!(v["commands"][0]["stdout"]["data"], "/w==");
}
#[test]
fn timeout_covers_descendants_holding_a_pipe() {
    let f = Fixture::new();
    f.ready("sleep 20 & exit 0", false);
    let start = Instant::now();
    let (v, c) = f.run();
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "execution.timeout");
    assert!(start.elapsed() < Duration::from_secs(6));
}
#[test]
fn both_streams_are_drained_beyond_the_capture_budget() {
    let f = Fixture::new();
    f.ready("i=0; while [ $i -lt 30000 ]; do printf '1234567890'; printf 'abcdefghij' >&2; i=$((i+1)); done",false);
    let (v, c) = f.run();
    assert_eq!(c, 0, "{v}");
    let command = &v["commands"][0];
    let total = command["stdout"]["data"].as_str().unwrap().len()
        + command["stderr"]["data"].as_str().unwrap().len();
    assert_eq!(total, 262144);
    assert!(command["stdout"]["truncated"] == true || command["stderr"]["truncated"] == true);
}
#[test]
fn missing_config_blocks_but_does_not_prevent_uninstall() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    fs::remove_file(f.root.join("fluidise.toml")).unwrap();
    assert_eq!(f.run().1, 2);
    f.apply("uninstall");
}
#[test]
fn external_owned_hook_edit_is_preserved() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let hook = f.root.join(".git/fluidise-relay/hooks/commit-msg");
    fs::write(&hook, "external edit").unwrap();
    assert_eq!(f.json(&["inspect", "--operation", "uninstall"]).1, 2);
    assert_eq!(fs::read_to_string(hook).unwrap(), "external edit");
}
#[test]
fn preexisting_hook_is_backed_up_and_other_event_delegates() {
    let f = Fixture::new();
    let source = f.root.join(".git/hooks");
    fs::create_dir(&source).unwrap();
    executable(&source.join("commit-msg"), "exit 99");
    executable(
        &source.join("pre-commit"),
        "printf '%s' \"$PWD\" > delegated",
    );
    f.ready("exit 0", false);
    let out = f.git(&[
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: actual commit",
    ]);
    assert!(out.status.success(), "{:?}", out.stderr);
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(f.root.join("delegated")).unwrap(),
        f.root.to_str().unwrap()
    );
    f.apply("uninstall");
    assert!(
        fs::read_to_string(source.join("commit-msg"))
            .unwrap()
            .contains("exit 99")
    );
}
#[test]
fn new_source_event_requires_explicit_update() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let dir = f.root.join(".git/hooks");
    fs::create_dir_all(&dir).unwrap();
    executable(&dir.join("pre-commit"), "exit 0");
    assert_eq!(f.run().0["diagnostics"][0]["code"], "state.source_changed");
    f.apply("install");
    assert_eq!(f.run().1, 0);
}
#[test]
fn owned_preparation_cannot_be_removed_while_installed() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    assert_eq!(
        f.json(&["inspect", "--operation", "unprepare"]).0["diagnostics"][0]["code"],
        "git.preparation_in_use"
    );
}
#[test]
fn clint_codes_are_not_reinterpreted() {
    let f = Fixture::new();
    f.ready("exit 1", false);
    assert_eq!(f.run().1, 1);
    f.config("exit 2", false);
    f.apply("approve");
    let (v, c) = f.run();
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["commands"][0]["exit_code"], 2);
}

#[test]
fn linked_worktrees_remain_isolated() {
    let f = Fixture::new();
    assert!(
        f.git(&[
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "initial"
        ])
        .status
        .success()
    );
    let path = f.root.join("linked");
    assert!(
        f.git(&[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            path.to_str().unwrap(),
            "HEAD"
        ])
        .status
        .success()
    );
    let linked = Fixture { root: path };
    f.ready("printf main", false);
    assert!(
        !linked
            .git(&["config", "--get", "core.hooksPath"])
            .status
            .success()
    );
    linked.ready("printf linked", false);
    assert_eq!(f.run().0["commands"][0]["stdout"]["data"], "main");
    assert_eq!(linked.run().0["commands"][0]["stdout"]["data"], "linked");
    f.apply("uninstall");
    assert_eq!(linked.run().1, 0);
    assert_eq!(f.json(&["inspect", "--operation", "unprepare"]).1, 2);
    linked.apply("uninstall");
    f.apply("unprepare");
}

#[test]
fn approval_can_change_the_recorded_configuration_source() {
    let f = Fixture::new();
    f.ready("exit 1", false);
    let second = f.root.join("alternate.toml");
    f.config("exit 0", false);
    fs::rename(f.root.join("fluidise.toml"), &second).unwrap();
    let path = second.to_str().unwrap();
    let (p, c) = f.json(&["inspect", "--operation", "approve", "--config", path]);
    assert_eq!(c, 0, "{p}");
    let (v, c) = f.json(&[
        "approve",
        "--plan",
        p["plan"]["id"].as_str().unwrap(),
        "--config",
        path,
    ]);
    assert_eq!(c, 0, "{v}");
    assert_eq!(f.run().1, 0);
    let output = f.git(&[
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: recorded config",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn pending_install_and_uninstall_are_resumable() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let path = f.root.join(".git/fluidise-relay/state.json");
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["phase"] = json!("installing");
    state["previous_files"] = Value::Null;
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    let hook = f.root.join(".git/fluidise-relay/hooks/commit-msg");
    fs::remove_file(&hook).unwrap();
    let stage = hook.with_file_name(".relay-stage-commit-msg");
    fs::write(&stage, "partial generated launcher").unwrap();
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(f.run().1, 2);
    f.apply("install");
    assert_eq!(f.run().1, 0);
    let mut state: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["phase"] = json!("uninstalling");
    state["approval"] = Value::Null;
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    fs::remove_file(hook).unwrap();
    f.apply("uninstall");
    assert_eq!(f.apply("uninstall")["changed"], false);
}

#[test]
fn common_core_worktree_is_migrated_and_restored() {
    let f = Fixture::new();
    f.git(&["config", "core.worktree", f.root.to_str().unwrap()]);
    f.config("exit 0", false);
    f.apply("prepare");
    assert!(
        !f.git(&[
            "config",
            "--file",
            f.root.join(".git/config").to_str().unwrap(),
            "--get",
            "core.worktree"
        ])
        .status
        .success()
    );
    assert_eq!(
        String::from_utf8(
            f.git(&["config", "--worktree", "--get", "core.worktree"])
                .stdout
        )
        .unwrap()
        .trim(),
        f.root.to_str().unwrap()
    );
    f.apply("install");
    f.apply("uninstall");
    f.apply("unprepare");
    assert_eq!(
        String::from_utf8(f.git(&["config", "--get", "core.worktree"]).stdout)
            .unwrap()
            .trim(),
        f.root.to_str().unwrap()
    );
}

#[test]
fn unknown_state_version_blocks_without_overwriting() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let path = f.root.join(".git/fluidise-relay/state.json");
    let original = fs::read(&path).unwrap();
    let mut state: Value = serde_json::from_slice(&original).unwrap();
    state["version"] = json!(999);
    let changed = serde_json::to_vec(&state).unwrap();
    fs::write(&path, &changed).unwrap();
    assert_eq!(f.json(&["inspect", "--operation", "uninstall"]).1, 2);
    assert_eq!(fs::read(path).unwrap(), changed);
}

#[test]
fn unavailable_programs_are_never_run_during_inspection() {
    let f = Fixture::new();
    f.apply("prepare");
    f.config("printf unexpected > side-effect", false);
    let (v, c) = f.json(&[
        "inspect",
        "--operation",
        "install",
        "--binary",
        env!("CARGO_BIN_EXE_fluidise-relay"),
    ]);
    assert_eq!(c, 0, "{v}");
    assert!(!f.root.join("side-effect").exists());
    let path = f.root.join("fluidise.toml");
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("/bin/sh", "/missing/program");
    fs::write(path, text).unwrap();
    assert_eq!(
        f.json(&[
            "inspect",
            "--operation",
            "install",
            "--binary",
            env!("CARGO_BIN_EXE_fluidise-relay")
        ])
        .0["diagnostics"][0]["code"],
        "command.unavailable"
    );
}

#[test]
#[ignore = "requires FLUIDISE_CLINT_BIN pointing to the real external clint executable"]
fn real_clint_through_git_and_direct_execution() {
    let f = Fixture::new();
    let binary = std::env::var("FLUIDISE_CLINT_BIN").expect("real clint executable");
    fs::write(f.root.join("fluidise.toml"),format!("[hooks]\nversion=1\n[[hooks.commit-msg]]\nid='commit-policy'\nprogram={}\nargs=['--file','{{message-file}}','--config','fluidise.toml']\n",json!(binary))).unwrap();
    f.apply("prepare");
    f.apply("install");
    let output = f.git(&[
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: exercise relay",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(f.root.join("message"), "invalid message").unwrap();
    let (v, c) = f.run();
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["commands"][0]["exit_code"], 1);
    let path = f.root.join("fluidise.toml");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n[commit]\nversion=999\n");
    fs::write(path, text).unwrap();
    let (v, c) = f.run();
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["commands"][0]["exit_code"], 2);
}

#[test]
fn delegated_protocol_is_byte_for_byte_and_keeps_the_original_script_path() {
    use std::io::Write;
    use std::process::Stdio;
    let f = Fixture::new();
    let source = f.root.join(".git/hooks");
    fs::create_dir(&source).unwrap();
    let hook = source.join("pre-push");
    executable(
        &hook,
        "printf '%s\\n' \"$0|$PWD|$1|$2|$RELAY_TEST_ENV\"; cat; printf 'protocol error' >&2; exit 37",
    );
    let invoke = |path: &Path| {
        let mut child = Command::new(path)
            .current_dir(&f.root)
            .args(["origin", "destination"])
            .env("RELAY_TEST_ENV", "preserved")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"refs data\0native\n")
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let before = invoke(Path::new(".git/hooks/pre-push"));
    f.ready("exit 0", false);
    let after = invoke(&f.root.join(".git/fluidise-relay/hooks/pre-push"));
    assert_eq!(before.stdout, after.stdout);
    assert_eq!(before.stderr, after.stderr);
    assert_eq!(before.status, after.status);
}

#[test]
fn absent_push_to_checkout_keeps_git_update_instead_behavior() {
    let target = Fixture::new();
    assert!(
        target
            .git(&[
                "-c",
                "user.name=Relay",
                "-c",
                "user.email=relay@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "initial"
            ])
            .status
            .success()
    );
    target.git(&["config", "receive.denyCurrentBranch", "updateInstead"]);
    target.ready("exit 0", false);
    assert!(
        !target
            .root
            .join(".git/fluidise-relay/hooks/push-to-checkout")
            .exists()
    );
    let branch = String::from_utf8(target.git(&["symbolic-ref", "HEAD"]).stdout).unwrap();
    let branch = branch.trim();
    let sender = Fixture::new();
    assert!(
        sender
            .git(&["fetch", "--quiet", target.root.to_str().unwrap(), "HEAD"])
            .status
            .success()
    );
    assert!(
        sender
            .git(&["checkout", "--quiet", "--detach", "FETCH_HEAD"])
            .status
            .success()
    );
    fs::write(
        sender.root.join("tracked"),
        "updated by Git, not a no-op hook",
    )
    .unwrap();
    sender.git(&["add", "tracked"]);
    assert!(
        sender
            .git(&[
                "-c",
                "user.name=Relay",
                "-c",
                "user.email=relay@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "feat: update target"
            ])
            .status
            .success()
    );
    let refspec = format!("HEAD:{branch}");
    let out = sender.git(&["push", "--quiet", target.root.to_str().unwrap(), &refspec]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(target.root.join("tracked")).unwrap(),
        "updated by Git, not a no-op hook"
    );
}

#[test]
fn symlink_hooks_are_delegated_without_rewriting_the_target() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let source = f.root.join(".git/hooks");
    fs::create_dir(&source).unwrap();
    let target = f.root.join("original-hook");
    executable(&target, "exit 0");
    symlink(&target, source.join("commit-msg")).unwrap();
    symlink(&target, source.join("pre-commit")).unwrap();
    f.ready("exit 0", false);
    f.apply("uninstall");
    assert_eq!(fs::read_link(source.join("commit-msg")).unwrap(), target);
    assert_eq!(fs::read_to_string(target).unwrap(), "#!/bin/sh\nexit 0\n");
}

#[test]
#[cfg(target_os = "linux")]
fn non_utf8_worktree_paths_are_lossless() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let mut f = Fixture::new();
    let mut bytes = f.root.as_os_str().as_encoded_bytes().to_vec();
    bytes.extend_from_slice(b"-\xff");
    let new = PathBuf::from(OsString::from_vec(bytes));
    fs::rename(&f.root, &new).unwrap();
    f.root = new;
    f.ready("printf '%s' \"$1\"", false);
    let out = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .args(["run", "--repo"])
        .arg(&f.root)
        .arg("--file")
        .arg(f.root.join("message"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(out.status.success(), "{v}");
    assert_eq!(v["commands"][0]["stdout"]["encoding"], "base64");
}

#[test]
fn spaces_quotes_unicode_and_newlines_in_paths_are_literal() {
    let mut f = Fixture::new();
    let new = f.root.with_file_name(format!(
        "{} - ' quote\n é",
        f.root.file_name().unwrap().to_str().unwrap()
    ));
    fs::rename(&f.root, &new).unwrap();
    f.root = fs::canonicalize(new).unwrap();
    f.ready("printf '%s' \"$1\"", false);
    let (v, c) = f.run();
    assert_eq!(c, 0, "{v}");
    assert_eq!(
        v["commands"][0]["stdout"]["data"],
        f.root.join("message").to_str().unwrap()
    );
}

#[test]
fn fifo_messages_fail_without_blocking_on_open() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let fifo = f.root.join("fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let start = Instant::now();
    let (v, c) = f.json(&["run", "--file", fifo.to_str().unwrap()]);
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "input.unsupported");
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn configuration_limits_and_strict_types_are_enforced() {
    let f = Fixture::new();
    f.apply("prepare");
    let args = [
        "inspect",
        "--operation",
        "install",
        "--binary",
        env!("CARGO_BIN_EXE_fluidise-relay"),
    ];
    for text in [
        "[hooks]\nversion='1'\ncommit-msg=[]\n",
        "[hooks]\nversion=1\ncommit-msg=[]\n",
        "[hooks]\nversion=1\ntimeout-seconds=0\n[[hooks.commit-msg]]\nid='Bad'\nprogram='/bin/sh'\nargs=[]\nmodifies-message='false'\n",
    ] {
        fs::write(f.root.join("fluidise.toml"), text).unwrap();
        assert_eq!(f.json(&args).1, 2);
    }
    f.config("exit 0", false);
    let path = f.root.join("fluidise.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("{text}\n#{}", "a".repeat(1_048_576))).unwrap();
    assert_eq!(
        f.json(&args).0["diagnostics"][0]["code"],
        "input.limit_exceeded"
    );
    let mut text = String::from("[hooks]\nversion=1\n");
    for i in 0..65 {
        text.push_str(&format!(
            "[[hooks.commit-msg]]\nid='command-{i}'\nprogram='/bin/sh'\nargs=[]\n"
        ));
    }
    fs::write(path, text).unwrap();
    assert_eq!(f.json(&args).1, 2);
}

#[test]
fn native_interruption_is_preserved_after_child_cleanup() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    let f = Fixture::new();
    f.ready("printf started > started; sleep 20", false);
    let child = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .args(["run", "--repo"])
        .arg(&f.root)
        .arg("--file")
        .arg(f.root.join("message"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !f.root.join("started").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    rustix::process::kill_process(
        rustix::process::Pid::from_raw(child.id() as i32).unwrap(),
        rustix::process::Signal::INT,
    )
    .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.signal(), Some(2));
    assert!(serde_json::from_slice::<Value>(&out.stdout).is_ok());
}

#[test]
fn lock_contention_fails_without_waiting() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let lock = fs::File::open(f.root.join(".git")).unwrap();
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive).unwrap();
    let start = Instant::now();
    let (v, c) = f.run();
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "lock.contended");
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(f.json(&["inspect", "--operation", "uninstall"]).1, 0);
}

#[test]
fn failed_command_stops_the_chain_and_keeps_prior_modifications() {
    let f = Fixture::new();
    f.config("printf changed > \"$1\"", true);
    let path = f.root.join("fluidise.toml");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n[[hooks.commit-msg]]\nid='reject'\nprogram='/bin/sh'\nargs=['-c','exit 9']\n[[hooks.commit-msg]]\nid='never-run'\nprogram='/bin/sh'\nargs=['-c','touch unexpected']\n");
    fs::write(path, text).unwrap();
    f.apply("prepare");
    f.apply("install");
    fs::write(f.root.join("message"), "original").unwrap();
    let (v, c) = f.run();
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["commands"].as_array().unwrap().len(), 2);
    assert_eq!(v["commands"][1]["exit_code"], 9);
    assert!(!f.root.join("unexpected").exists());
    assert_eq!(
        fs::read_to_string(f.root.join("message")).unwrap(),
        "changed"
    );
}

#[test]
fn an_update_keeps_the_original_backup() {
    let f = Fixture::new();
    let source = f.root.join(".git/hooks");
    fs::create_dir(&source).unwrap();
    executable(&source.join("commit-msg"), "exit 42");
    f.ready("exit 0", false);
    let directory = f.root.join(".git/fluidise-relay/backups");
    let before: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before.len(), 1);
    let bytes = fs::read(directory.join(&before[0])).unwrap();
    f.config("printf updated", false);
    f.apply("install");
    let after: Vec<_> = fs::read_dir(&directory)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
    assert_eq!(fs::read(directory.join(&before[0])).unwrap(), bytes);
}

#[test]
fn preexisting_preparation_and_target_override_are_not_removed() {
    let f = Fixture::new();
    let source = f.root.join("custom hooks");
    fs::create_dir(&source).unwrap();
    f.git(&["config", "extensions.worktreeConfig", "true"]);
    f.git(&[
        "config",
        "--worktree",
        "core.hooksPath",
        source.to_str().unwrap(),
    ]);
    f.ready("exit 0", false);
    f.apply("uninstall");
    assert_eq!(
        String::from_utf8(
            f.git(&["config", "--worktree", "--get", "core.hooksPath"])
                .stdout
        )
        .unwrap()
        .trim(),
        source.to_str().unwrap()
    );
    assert_eq!(f.apply("unprepare")["changed"], false);
    assert_eq!(
        f.git(&["config", "--get", "extensions.worktreeConfig"])
            .stdout,
        b"true\n"
    );
}

#[test]
fn restoration_does_not_silently_activate_a_new_inherited_source() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    let external = f.root.join("external");
    fs::create_dir(&external).unwrap();
    f.git(&[
        "config",
        "--local",
        "core.hooksPath",
        external.to_str().unwrap(),
    ]);
    let (v, c) = f.json(&["inspect", "--operation", "uninstall"]);
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "state.restoration_conflict");
    assert_eq!(f.run().1, 0);
}

#[test]
fn missing_state_never_installs_recursive_delegation() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    fs::remove_file(f.root.join(".git/fluidise-relay/state.json")).unwrap();
    let (v, c) = f.json(&[
        "inspect",
        "--operation",
        "install",
        "--binary",
        env!("CARGO_BIN_EXE_fluidise-relay"),
    ]);
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "state.missing");
}

#[test]
fn escaped_json_logs_still_fit_the_transport_budget() {
    let f = Fixture::new();
    f.ready("dd if=/dev/zero bs=262144 count=1 2>/dev/null", false);
    let output = f.relay(&["run", "--file", f.root.join("message").to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.len() <= 2_097_152);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["commands"][0]["stdout"]["data"]
            .as_str()
            .unwrap()
            .len(),
        262144
    );
}

#[test]
fn broken_output_transport_is_not_success() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    let (writer, reader) = UnixStream::pair().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .arg("--version")
        .stdout(std::process::Stdio::from(OwnedFd::from(writer)))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn another_executable_cannot_be_installed_as_relay() {
    let f = Fixture::new();
    f.config("exit 0", false);
    f.apply("prepare");
    let fake = f.root.join("not-relay");
    executable(&fake, "exit 0");
    let (v, c) = f.json(&[
        "inspect",
        "--operation",
        "install",
        "--binary",
        fake.to_str().unwrap(),
    ]);
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "install.binary_mismatch");
    assert!(!f.root.join(".git/fluidise-relay/state.json").exists());
}

#[test]
fn quoted_binary_copy_is_a_valid_stable_reference() {
    let f = Fixture::new();
    f.config("exit 0", false);
    f.apply("prepare");
    let copy = f.root.join("Relay's copy");
    fs::copy(env!("CARGO_BIN_EXE_fluidise-relay"), &copy).unwrap();
    fs::set_permissions(&copy, fs::Permissions::from_mode(0o700)).unwrap();
    let binary = copy.to_str().unwrap();
    let (p, c) = f.json(&["inspect", "--operation", "install", "--binary", binary]);
    assert_eq!(c, 0, "{p}");
    let (v, c) = f.json(&[
        "install",
        "--binary",
        binary,
        "--plan",
        p["plan"]["id"].as_str().unwrap(),
    ]);
    assert_eq!(c, 0, "{v}");
    let out = f.git(&[
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: quoted binary",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    assert!(out.stderr.is_empty());
}

#[test]
fn native_git_locks_are_reported_and_never_deleted() {
    let f = Fixture::new();
    let lock = f.root.join(".git/config.lock");
    fs::write(&lock, "foreign lock").unwrap();
    let (v, c) = f.json(&["inspect", "--operation", "prepare"]);
    assert_eq!(c, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "git.lock_present");
    assert_eq!(fs::read_to_string(lock).unwrap(), "foreign lock");
    assert!(!f.root.join(".git/fluidise-relay").exists());
}

#[test]
fn timeout_forces_same_group_descendant_after_parent_and_pipes_exit() {
    let f = Fixture::new();
    f.ready(
        "/bin/sh -c 'trap \"\" TERM; exec sleep 30' >/dev/null 2>&1 & echo $! > survivor; wait",
        false,
    );
    let start = Instant::now();
    let (v, code) = f.run();
    let pid = fs::read_to_string(f.root.join("survivor")).unwrap();
    let state = Command::new("ps")
        .args(["-p", pid.trim(), "-o", "stat="])
        .output()
        .unwrap();
    let alive = state.status.success()
        && !String::from_utf8_lossy(&state.stdout)
            .trim()
            .starts_with('Z');
    // Always clean the disposable probe before reporting a failed assertion.
    let _ = Command::new("kill").args(["-KILL", pid.trim()]).output();
    assert_eq!(code, 2, "{v}");
    assert!(start.elapsed() < Duration::from_secs(6));
    assert!(!alive, "same-group descendant survived cleanup: {v}");
}

#[test]
fn installation_reactivates_an_inspected_missing_override() {
    let f = Fixture::new();
    f.ready("exit 0", false);
    assert!(
        f.git(&["config", "--worktree", "--unset-all", "core.hooksPath"])
            .status
            .success()
    );
    let applied = f.apply("install");
    assert_eq!(applied["changed"], true);
    assert_eq!(f.run().1, 0);
    assert_eq!(f.apply("install")["changed"], false);
}

#[test]
fn dormant_worktree_configuration_is_not_silently_activated() {
    for linked_target in [false, true] {
        let f = Fixture::new();
        assert!(
            f.git(&[
                "-c",
                "user.name=Relay",
                "-c",
                "user.email=relay@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "initial"
            ])
            .status
            .success()
        );
        let linked = f.root.join("linked");
        assert!(
            f.git(&[
                "worktree",
                "add",
                "--detach",
                "--quiet",
                linked.to_str().unwrap(),
                "HEAD"
            ])
            .status
            .success()
        );
        let admin = if linked_target {
            f.root.join(".git/worktrees/linked")
        } else {
            f.root.join(".git")
        };
        fs::write(
            admin.join("config.worktree"),
            "[core]\n hooksPath=/dormant-unapproved-hooks\n",
        )
        .unwrap();
        let (v, code) = f.json(&["inspect", "--operation", "prepare"]);
        assert_eq!(code, 2, "{v}");
        assert_eq!(v["diagnostics"][0]["code"], "git.migration_unsupported");
        assert!(!f.root.join(".git/fluidise-relay-common").exists());
        assert!(
            !f.git(&["config", "--get", "extensions.worktreeConfig"])
                .status
                .success()
        );
    }
}

#[test]
fn preparation_plan_binds_dormant_worktree_configurations() {
    let f = Fixture::new();
    assert!(
        f.git(&[
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "initial"
        ])
        .status
        .success()
    );
    let linked = f.root.join("linked");
    assert!(
        f.git(&[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            linked.to_str().unwrap(),
            "HEAD"
        ])
        .status
        .success()
    );
    let (plan, code) = f.json(&["inspect", "--operation", "prepare"]);
    assert_eq!(code, 0, "{plan}");
    // Even harmless dormant-file changes in a different worktree stale the plan.
    fs::write(
        f.root.join(".git/worktrees/linked/config.worktree"),
        "# reviewed separately\n",
    )
    .unwrap();
    let (v, code) = f.json(&["prepare", "--plan", plan["plan"]["id"].as_str().unwrap()]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "plan.stale");
}

#[test]
fn commit_msg_symlinks_must_resolve_to_regular_files() {
    use std::os::unix::fs::symlink;
    for target_kind in ["directory", "missing", "fifo"] {
        let f = Fixture::new();
        f.config("exit 0", false);
        f.apply("prepare");
        let source = f.root.join(".git/hooks");
        fs::create_dir(&source).unwrap();
        let target = f.root.join("target");
        match target_kind {
            "directory" => fs::create_dir(&target).unwrap(),
            "fifo" => assert!(
                Command::new("mkfifo")
                    .arg(&target)
                    .status()
                    .unwrap()
                    .success()
            ),
            _ => {}
        }
        symlink(target, source.join("commit-msg")).unwrap();
        let (v, code) = f.json(&[
            "inspect",
            "--operation",
            "install",
            "--binary",
            env!("CARGO_BIN_EXE_fluidise-relay"),
        ]);
        assert_eq!(code, 2, "{target_kind}: {v}");
        assert_eq!(v["diagnostics"][0]["code"], "install.source_invalid");
    }
}

#[test]
fn executable_checks_use_effective_access_not_other_classes_bits() {
    let f = Fixture::new();
    let program = f.root.join("program");
    executable(&program, "exit 0");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o401)).unwrap();
    // A privileged test identity may legitimately have executable access.
    if Command::new("/bin/sh")
        .arg("-c")
        .arg("test -x \"$1\"")
        .arg("probe")
        .arg(&program)
        .status()
        .unwrap()
        .success()
    {
        return;
    }
    fs::write(
        f.root.join("fluidise.toml"),
        "[hooks]\nversion=1\n[[hooks.commit-msg]]\nid='check'\nprogram='./program'\nargs=[]\n",
    )
    .unwrap();
    f.apply("prepare");
    let (v, code) = f.json(&[
        "inspect",
        "--operation",
        "install",
        "--binary",
        env!("CARGO_BIN_EXE_fluidise-relay"),
    ]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "command.unavailable");
    f.config("exit 0", false);
    let hooks = f.root.join(".git/hooks");
    fs::create_dir(&hooks).unwrap();
    fs::rename(program, hooks.join("pre-commit")).unwrap();
    f.apply("install");
    assert!(!f.root.join(".git/fluidise-relay/hooks/pre-commit").exists());
    assert!(
        f.git(&[
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "feat: permission check"
        ])
        .status
        .success()
    );
}

#[test]
fn included_hook_source_drift_blocks_old_uninstall_plan() {
    let f = Fixture::new();
    let a = f.root.join("A");
    let b = f.root.join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    executable(&a.join("commit-msg"), "exit 0");
    executable(&b.join("commit-msg"), "exit 0");
    let included = f.root.join("hooks.inc");
    fs::write(&included, format!("[core]\nhooksPath={}\n", a.display())).unwrap();
    assert!(
        f.git(&["config", "include.path", included.to_str().unwrap()])
            .status
            .success()
    );
    f.ready("exit 0", false);
    let (plan, code) = f.json(&["inspect", "--operation", "uninstall"]);
    assert_eq!(code, 0, "{plan}");
    fs::write(&included, format!("[core]\nhooksPath={}\n", b.display())).unwrap();
    let (v, code) = f.json(&["uninstall", "--plan", plan["plan"]["id"].as_str().unwrap()]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["changed"], false);
    assert_eq!(v["diagnostics"][0]["code"], "state.restoration_conflict");
    assert!(f.root.join(".git/fluidise-relay/hooks/commit-msg").exists());
}

#[test]
fn delegated_native_environment_is_not_rewritten_by_a_shell() {
    let f = Fixture::new();
    let source = f.root.join(".git/hooks");
    fs::create_dir(&source).unwrap();
    // env is a native executable: no source-hook interpreter can hide mutations.
    std::os::unix::fs::symlink("/usr/bin/env", source.join("pre-commit")).unwrap();
    let invoke = |path: &Path| {
        Command::new(path)
            .current_dir(&f.root)
            .env("OLDPWD", "/fixture/sentinel")
            .env("PWD", "/fixture/unchanged-pwd")
            .env("RELAY_TEST_ENV", "native\nvalue")
            .output()
            .unwrap()
    };
    let before = invoke(&source.join("pre-commit"));
    assert!(before.status.success());
    f.ready("exit 0", false);
    let after = invoke(&f.root.join(".git/fluidise-relay/hooks/pre-commit"));
    assert_eq!(after.status, before.status);
    let entries = |bytes: Vec<u8>| {
        let mut entries: Vec<_> = String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
        entries.sort();
        entries
    };
    assert!(
        entries(after.stdout) == entries(before.stdout),
        "delegation changed the inherited environment"
    );
    assert_eq!(after.stderr, before.stderr);
}

#[test]
fn native_delegation_preserves_signal_and_removes_owned_copy_when_selected_binary_missing() {
    use std::os::unix::process::ExitStatusExt;
    let f = Fixture::new();
    let source = f.root.join(".git/hooks");
    fs::create_dir(&source).unwrap();
    executable(&source.join("pre-commit"), "kill -TERM $$");
    f.config("exit 0", false);
    f.apply("prepare");
    let copy = f.root.join("relay copy");
    fs::copy(env!("CARGO_BIN_EXE_fluidise-relay"), &copy).unwrap();
    let args = [
        "inspect",
        "--operation",
        "install",
        "--binary",
        copy.to_str().unwrap(),
    ];
    let (plan, code) = f.json(&args);
    assert_eq!(code, 0, "{plan}");
    assert_eq!(
        f.json(&[
            "install",
            "--binary",
            copy.to_str().unwrap(),
            "--plan",
            plan["plan"]["id"].as_str().unwrap()
        ])
        .1,
        0
    );
    let owned = f.root.join(".git/fluidise-relay/hooks/pre-commit");
    let status = Command::new(&owned)
        .current_dir(&f.root)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(15));
    fs::remove_file(copy).unwrap();
    assert!(fs::symlink_metadata(&owned).unwrap().is_file());
    f.apply("uninstall");
    assert!(fs::symlink_metadata(owned).is_err());
}

#[test]
fn included_worktree_hook_settings_are_refused_before_activation() {
    let f = Fixture::new();
    f.config("exit 0", false);
    f.apply("prepare");
    let source = f.root.join("hooks");
    fs::create_dir(&source).unwrap();
    let include = f.root.join("worktree.inc");
    fs::write(
        &include,
        format!("[core]\nhooksPath={}\n", source.display()),
    )
    .unwrap();
    assert!(
        f.git(&[
            "config",
            "--worktree",
            "include.path",
            include.to_str().unwrap()
        ])
        .status
        .success()
    );
    let (v, code) = f.json(&[
        "inspect",
        "--operation",
        "install",
        "--binary",
        env!("CARGO_BIN_EXE_fluidise-relay"),
    ]);
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "git.config_unsupported");
    assert!(!f.root.join(".git/fluidise-relay/state.json").exists());
}

#[test]
fn git_push_blocks_when_selected_relay_is_missing_or_not_executable() {
    use std::os::unix::fs::symlink;
    for unavailable in ["missing", "not-executable", "dangling-stable-link"] {
        let f = Fixture::new();
        let source = f.root.join(".git/hooks");
        fs::create_dir(&source).unwrap();
        executable(
            &source.join("pre-push"),
            "printf ran > policy-invoked; exit 37",
        );
        assert!(
            f.git(&[
                "-c",
                "user.name=Relay",
                "-c",
                "user.email=relay@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "feat: fixture"
            ])
            .status
            .success()
        );
        let destination = f.root.join("destination.git");
        assert!(
            f.git(&[
                "init",
                "--bare",
                "--quiet",
                "--template=",
                destination.to_str().unwrap()
            ])
            .status
            .success()
        );
        assert!(
            f.git(&["remote", "add", "origin", destination.to_str().unwrap()])
                .status
                .success()
        );
        f.config("exit 0", false);
        f.apply("prepare");
        let target = f.root.join("relay-target");
        let stable = f.root.join("relay-stable");
        fs::copy(env!("CARGO_BIN_EXE_fluidise-relay"), &target).unwrap();
        symlink(&target, &stable).unwrap();
        let binary = stable.to_str().unwrap();
        let (plan, code) = f.json(&["inspect", "--operation", "install", "--binary", binary]);
        assert_eq!(code, 0, "{plan}");
        let (applied, code) = f.json(&[
            "install",
            "--binary",
            binary,
            "--plan",
            plan["plan"]["id"].as_str().unwrap(),
        ]);
        assert_eq!(code, 0, "{applied}");
        let push = ["push", "--quiet", "origin", "HEAD:refs/heads/test"];
        assert_eq!(f.git(&push).status.code(), Some(1));
        let marker = f.root.join("policy-invoked");
        assert!(marker.exists());
        fs::remove_file(&marker).unwrap();
        match unavailable {
            "missing" => fs::remove_file(&stable).unwrap(),
            "not-executable" => {
                fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap()
            }
            _ => fs::remove_file(&target).unwrap(),
        }
        let result = f.git(&push);
        assert_eq!(
            result.status.code(),
            Some(1),
            "{unavailable}: push must remain blocked"
        );
        let diagnostic: Value = result
            .stderr
            .split(|b| *b == b'\n')
            .find_map(|line| serde_json::from_slice::<Value>(line).ok())
            .expect("explicit Relay failure on stderr");
        assert_eq!(
            diagnostic["diagnostics"][0]["code"], "relay.unavailable",
            "{diagnostic}"
        );
        assert!(
            !marker.exists(),
            "unavailable Relay must not execute the original policy"
        );
        assert!(
            !f.git(&[
                "-C",
                destination.to_str().unwrap(),
                "show-ref",
                "--verify",
                "refs/heads/test"
            ])
            .status
            .success()
        );
        if fs::symlink_metadata(&stable).is_ok() {
            fs::remove_file(&stable).unwrap();
        }
        fs::copy(env!("CARGO_BIN_EXE_fluidise-relay"), &stable).unwrap();
        assert_eq!(f.git(&push).status.code(), Some(1));
        assert!(
            marker.exists(),
            "restored selected Relay must preserve the original policy"
        );
    }
}

#[test]
fn uninstall_plan_binds_restored_event_presence_and_executability() {
    for change in ["added", "removed", "enabled", "disabled"] {
        let f = Fixture::new();
        let source = f.root.join(".git/hooks");
        fs::create_dir(&source).unwrap();
        let event = source.join("pre-commit");
        if change != "added" {
            executable(&event, "printf ran > restored-event; exit 77");
            if change == "enabled" {
                fs::set_permissions(&event, fs::Permissions::from_mode(0o600)).unwrap();
            }
        }
        f.ready("exit 0", false);
        let (old, code) = f.json(&["inspect", "--operation", "uninstall"]);
        assert_eq!(code, 0, "{old}");
        match change {
            "added" => executable(&event, "printf ran > restored-event; exit 77"),
            "removed" => fs::remove_file(&event).unwrap(),
            "enabled" => fs::set_permissions(&event, fs::Permissions::from_mode(0o700)).unwrap(),
            _ => fs::set_permissions(&event, fs::Permissions::from_mode(0o600)).unwrap(),
        }
        let state = f.root.join(".git/fluidise-relay/state.json");
        let work_config = f.root.join(".git/config.worktree");
        let before_state = fs::read(&state).unwrap();
        let before_config = fs::read(&work_config).unwrap();
        let (v, code) = f.json(&["uninstall", "--plan", old["plan"]["id"].as_str().unwrap()]);
        assert_eq!(code, 2, "{change}: {v}");
        assert_eq!(v["diagnostics"][0]["code"], "plan.stale");
        assert_eq!(v["changed"], false);
        assert_eq!(fs::read(&state).unwrap(), before_state);
        assert_eq!(fs::read(&work_config).unwrap(), before_config);
        assert!(f.root.join(".git/fluidise-relay/hooks/commit-msg").exists());
        let (fresh, code) = f.json(&["inspect", "--operation", "uninstall"]);
        assert_eq!(code, 0, "{fresh}");
        assert_ne!(fresh["plan"]["id"], old["plan"]["id"], "{change}");
        let active_event = change == "added" || change == "enabled";
        assert_eq!(
            fresh["plan"]["restoration"]["inventory"]["pre-commit"].is_object(),
            active_event,
            "{fresh}"
        );
        let (removed, code) =
            f.json(&["uninstall", "--plan", fresh["plan"]["id"].as_str().unwrap()]);
        assert_eq!(code, 0, "{removed}");
        let commit = f.git(&[
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "feat: inspected restoration",
        ]);
        assert_eq!(
            commit.status.success(),
            !active_event,
            "{change}: {:?}",
            commit.stderr
        );
        assert_eq!(f.root.join("restored-event").exists(), active_event);
    }
}

#[test]
fn saved_tilde_hook_override_cannot_restore_into_a_different_home() {
    let f = Fixture::new();
    let home_a = f.root.join("home-a");
    let home_b = f.root.join("home-b");
    fs::create_dir_all(home_a.join("hooks")).unwrap();
    fs::create_dir_all(home_b.join("hooks")).unwrap();
    executable(
        &home_b.join("hooks/pre-commit"),
        "printf ran > uninspected-event; exit 77",
    );
    f.config("exit 0", false);
    f.apply("prepare");
    assert!(
        f.git(&["config", "--worktree", "core.hooksPath", "~/hooks"])
            .status
            .success()
    );
    let invoke = |home: &Path, args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
            .args(args)
            .arg("--repo")
            .arg(&f.root)
            .env("HOME", home)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        (value, out.status.code().unwrap())
    };
    let binary = env!("CARGO_BIN_EXE_fluidise-relay");
    let (install, code) = invoke(
        &home_a,
        &["inspect", "--operation", "install", "--binary", binary],
    );
    assert_eq!(code, 0, "{install}");
    let (v, code) = invoke(
        &home_a,
        &[
            "install",
            "--binary",
            binary,
            "--plan",
            install["plan"]["id"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{v}");
    let (removal, code) = invoke(&home_a, &["inspect", "--operation", "uninstall"]);
    assert_eq!(code, 0, "{removal}");
    let state_path = f.root.join(".git/fluidise-relay/state.json");
    let config_path = f.root.join(".git/config.worktree");
    let before = (
        fs::read(&state_path).unwrap(),
        fs::read(&config_path).unwrap(),
    );
    let (v, code) = invoke(
        &home_b,
        &[
            "uninstall",
            "--plan",
            removal["plan"]["id"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["changed"], false);
    assert_eq!(v["diagnostics"][0]["code"], "state.restoration_conflict");
    assert_eq!(
        (
            fs::read(&state_path).unwrap(),
            fs::read(&config_path).unwrap()
        ),
        before
    );
    assert_eq!(
        invoke(&home_b, &["inspect", "--operation", "uninstall"]).1,
        2
    );
    assert!(!f.root.join("uninspected-event").exists());
    let (v, code) = invoke(
        &home_a,
        &[
            "uninstall",
            "--plan",
            removal["plan"]["id"].as_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        String::from_utf8(
            f.git(&["config", "--worktree", "--get", "core.hooksPath"])
                .stdout
        )
        .unwrap()
        .trim(),
        "~/hooks"
    );
}

#[test]
fn global_extension_value_is_not_repository_worktree_capability() {
    let f = Fixture::new();
    let global = f.root.join("global.gitconfig");
    let contents = "[extensions]\nworktreeConfig=true\n";
    fs::write(&global, contents).unwrap();
    f.config("exit 0", false);
    let invoke = |args: &[&str]| {
        let out = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
            .args(args)
            .arg("--repo")
            .arg(&f.root)
            .env("GIT_CONFIG_GLOBAL", &global)
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        (value, out.status.code().unwrap())
    };
    let (plan, code) = invoke(&["inspect", "--operation", "prepare"]);
    assert_eq!(code, 0, "{plan}");
    assert!(
        !plan["plan"]["actions"].as_array().unwrap().is_empty(),
        "{plan}"
    );
    let (v, code) = invoke(&["prepare", "--plan", plan["plan"]["id"].as_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["changed"], true);
    assert_eq!(
        f.git(&[
            "config",
            "--local",
            "--bool",
            "--get",
            "extensions.worktreeConfig"
        ])
        .stdout,
        b"true\n"
    );
    let binary = env!("CARGO_BIN_EXE_fluidise-relay");
    let (plan, code) = invoke(&["inspect", "--operation", "install", "--binary", binary]);
    assert_eq!(code, 0, "{plan}");
    let (v, code) = invoke(&[
        "install",
        "--binary",
        binary,
        "--plan",
        plan["plan"]["id"].as_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{v}");
    let actual = f.git(&["config", "--path", "--get", "core.hooksPath"]);
    assert!(actual.status.success());
    assert_eq!(
        String::from_utf8(actual.stdout).unwrap().trim(),
        f.root.join(".git/fluidise-relay/hooks").to_str().unwrap()
    );
    for operation in ["uninstall", "unprepare"] {
        let (plan, code) = invoke(&["inspect", "--operation", operation]);
        assert_eq!(code, 0, "{plan}");
        let (v, code) = invoke(&[operation, "--plan", plan["plan"]["id"].as_str().unwrap()]);
        assert_eq!(code, 0, "{v}");
    }
    assert!(
        !f.git(&["config", "--local", "--get", "extensions.worktreeConfig"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(global).unwrap(), contents);
}

#[test]
fn included_extension_value_is_not_repository_worktree_capability() {
    let f = Fixture::new();
    let included = f.root.join("extension.inc");
    fs::write(&included, "[extensions]\nworktreeConfig=true\n").unwrap();
    assert!(
        f.git(&["config", "include.path", included.to_str().unwrap()])
            .status
            .success()
    );
    f.config("exit 0", false);
    assert_eq!(f.apply("prepare")["changed"], true);
    f.apply("install");
    assert!(
        f.git(&["config", "--get", "core.hooksPath"])
            .status
            .success()
    );
    f.apply("uninstall");
    f.apply("unprepare");
    assert!(
        !f.git(&[
            "config",
            "--local",
            "--no-includes",
            "--get",
            "extensions.worktreeConfig"
        ])
        .status
        .success()
    );
    assert_eq!(
        fs::read_to_string(included).unwrap(),
        "[extensions]\nworktreeConfig=true\n"
    );
}

#[test]
fn all_native_true_bare_spellings_refuse_preparation_without_worktree_changes() {
    for spelling in [
        "true", "yes", "on", "1", "-1", "TRUE", "YeS", "2", "implicit",
    ] {
        let f = Fixture::new();
        assert!(
            f.git(&[
                "-c",
                "user.name=Relay",
                "-c",
                "user.email=relay@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "feat: fixture"
            ])
            .status
            .success()
        );
        let target = f.root.join("target");
        let other = f.root.join("other");
        for path in [&target, &other] {
            assert!(
                f.git(&[
                    "worktree",
                    "add",
                    "--detach",
                    "--quiet",
                    path.to_str().unwrap(),
                    "HEAD"
                ])
                .status
                .success()
            );
        }
        let common_config = f.root.join(".git/config");
        if spelling == "implicit" {
            assert!(f.git(&["config", "--unset", "core.bare"]).status.success());
            let mut contents = fs::read_to_string(&common_config).unwrap();
            contents.push_str("\n[core]\n bare\n");
            fs::write(&common_config, contents).unwrap();
        } else {
            assert!(f.git(&["config", "core.bare", spelling]).status.success());
        }
        let target = Fixture { root: target };
        let other = Fixture { root: other };
        assert_eq!(
            target.git(&["rev-parse", "--is-bare-repository"]).stdout,
            b"false\n"
        );
        assert_eq!(
            other.git(&["rev-parse", "--is-bare-repository"]).stdout,
            b"false\n"
        );
        let before = fs::read(&common_config).unwrap();
        let (result, code) = target.json(&["inspect", "--operation", "prepare"]);
        assert_eq!(code, 2, "{spelling}: {result}");
        assert_eq!(
            result["diagnostics"][0]["code"],
            "git.migration_unsupported"
        );
        assert_eq!(fs::read(&common_config).unwrap(), before);
        assert!(!f.root.join(".git/fluidise-relay-common").exists());
        assert!(!f.root.join(".git/worktrees/target/fluidise-relay").exists());
        assert!(
            target.git(&["status", "--porcelain"]).status.success(),
            "target {spelling}"
        );
        assert!(
            other.git(&["status", "--porcelain"]).status.success(),
            "other {spelling}"
        );
    }
}

#[test]
fn native_false_bare_spellings_remain_supported_and_unchanged() {
    for spelling in ["false", "no", "off", "0", "FALSE", ""] {
        let f = Fixture::new();
        assert!(f.git(&["config", "core.bare", spelling]).status.success());
        assert_eq!(f.apply("prepare")["changed"], true);
        f.apply("unprepare");
        assert_eq!(
            f.git(&[
                "config",
                "--file",
                f.root.join(".git/config").to_str().unwrap(),
                "--get",
                "core.bare"
            ])
            .stdout,
            format!("{spelling}\n").as_bytes()
        );
        assert!(f.git(&["status", "--porcelain"]).status.success());
    }
}

#[test]
fn managed_shebangless_files_never_get_an_implicit_shell() {
    let f = Fixture::new();
    let program = f.root.join("not-a-native-executable");
    fs::write(
        &program,
        "printf ran > hidden-shell-marker; printf unexpected; exit 0\n",
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(f.root.join("fluidise.toml"), "[hooks]\nversion=1\n[[hooks.commit-msg]]\nid='check'\nprogram='./not-a-native-executable'\nargs=[]\n").unwrap();
    f.apply("prepare");
    f.apply("install");
    fs::write(f.root.join("message"), "feat: strict launch\n").unwrap();
    let (v, code) = f.run();
    assert_eq!(code, 2, "{v}");
    assert_eq!(v["diagnostics"][0]["code"], "command.spawn_failed");
    assert!(v["commands"][0]["exit_code"].is_null());
    assert!(!f.root.join("hidden-shell-marker").exists());
    fs::write(f.root.join("fluidise.toml"), "[hooks]\nversion=1\n[[hooks.commit-msg]]\nid='check'\nprogram='/bin/sh'\nargs=['./not-a-native-executable']\n").unwrap();
    f.apply("approve");
    assert_eq!(f.run().1, 0);
    assert!(f.root.join("hidden-shell-marker").exists());
}

#[test]
fn actual_git_default_hook_invocation_path_is_preserved() {
    let f = Fixture::new();
    let hooks = f.root.join(".git/hooks");
    fs::create_dir(&hooks).unwrap();
    executable(
        &hooks.join("pre-commit"),
        "printf '%s\\n' \"$0\" >&2; case \"$0\" in .git/hooks/pre-commit) exit 0;; *) exit 77;; esac",
    );
    let commit = [
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: path fixture",
    ];
    let before = f.git(&commit);
    assert!(before.status.success());
    f.ready("exit 0", false);
    let after = f.git(&commit);
    assert_eq!(after.status, before.status);
    assert_eq!(after.stderr, before.stderr);
}

#[test]
fn actual_receive_event_preserves_default_invocation_path() {
    let receiver = Fixture::new();
    let hooks = receiver.root.join(".git/hooks");
    fs::create_dir(&hooks).unwrap();
    executable(
        &hooks.join("pre-receive"),
        "printf '%s' \"$0\" > delegate-path; cat >/dev/null",
    );
    let sender = Fixture::new();
    assert!(
        sender
            .git(&[
                "-c",
                "user.name=Relay",
                "-c",
                "user.email=relay@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "feat: receive fixture"
            ])
            .status
            .success()
    );
    assert!(
        sender
            .git(&[
                "push",
                "--quiet",
                receiver.root.to_str().unwrap(),
                "HEAD:refs/heads/before"
            ])
            .status
            .success()
    );
    let before = fs::read(receiver.root.join(".git/delegate-path")).unwrap();
    receiver.ready("exit 0", false);
    let result = sender.git(&[
        "push",
        "--quiet",
        receiver.root.to_str().unwrap(),
        "HEAD:refs/heads/after",
    ]);
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(
        fs::read(receiver.root.join(".git/delegate-path")).unwrap(),
        before
    );
}

#[test]
fn actual_git_explicit_administrative_path_is_preserved() {
    let f = Fixture::new();
    let hooks = f.root.join(".git/hooks");
    fs::create_dir(&hooks).unwrap();
    executable(&hooks.join("pre-commit"), "printf '%s\\n' \"$0\" >&2");
    let admin = f.root.join(".git");
    let commit = [
        "--git-dir",
        admin.to_str().unwrap(),
        "--work-tree",
        f.root.to_str().unwrap(),
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: explicit admin fixture",
    ];
    let before = f.git(&commit);
    assert!(before.status.success());
    f.ready("exit 0", false);
    let after = f.git(&commit);
    assert_eq!(after.status, before.status);
    assert_eq!(after.stderr, before.stderr);
}

#[test]
fn actual_linked_worktree_default_hook_path_is_preserved() {
    let main = Fixture::new();
    assert!(
        main.git(&[
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "feat: linked fixture"
        ])
        .status
        .success()
    );
    let linked = main.root.join("linked");
    assert!(
        main.git(&[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            linked.to_str().unwrap(),
            "HEAD"
        ])
        .status
        .success()
    );
    let hooks = main.root.join(".git/hooks");
    fs::create_dir(&hooks).unwrap();
    executable(&hooks.join("pre-commit"), "printf '%s\\n' \"$0\" >&2");
    let f = Fixture { root: linked };
    let commit = [
        "-c",
        "user.name=Relay",
        "-c",
        "user.email=relay@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--quiet",
        "--allow-empty",
        "-m",
        "feat: linked path",
    ];
    let before = f.git(&commit);
    assert!(before.status.success());
    f.ready("exit 0", false);
    let after = f.git(&commit);
    assert_eq!(after.status, before.status);
    assert_eq!(after.stderr, before.stderr);
}

#[test]
fn managed_commit_msg_unavailable_binary_has_structured_failure() {
    use std::os::unix::fs::symlink;
    for unavailable in [
        "missing",
        "not-executable",
        "dangling-link",
        "directory",
        "directory-link",
    ] {
        let f = Fixture::new();
        f.config("printf ran > policy-invoked", false);
        f.apply("prepare");
        let target = f.root.join("relay target");
        let stable = f.root.join("relay 'stable");
        fs::copy(env!("CARGO_BIN_EXE_fluidise-relay"), &target).unwrap();
        symlink(&target, &stable).unwrap();
        let binary = stable.to_str().unwrap();
        let (plan, code) = f.json(&["inspect", "--operation", "install", "--binary", binary]);
        assert_eq!(code, 0, "{plan}");
        let (applied, code) = f.json(&[
            "install",
            "--binary",
            binary,
            "--plan",
            plan["plan"]["id"].as_str().unwrap(),
        ]);
        assert_eq!(code, 0, "{applied}");
        let message = f.root.join("message");
        fs::write(&message, "feat: fixture\n").unwrap();
        let hook = f.root.join(".git/fluidise-relay/hooks/commit-msg");
        let invoke = || {
            Command::new(&hook)
                .arg(&message)
                .current_dir(&f.root)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .output()
                .unwrap()
        };
        let before = invoke();
        assert!(before.status.success(), "{before:?}");
        let marker = f.root.join("policy-invoked");
        assert!(marker.exists());
        fs::remove_file(&marker).unwrap();
        match unavailable {
            "missing" => fs::remove_file(&stable).unwrap(),
            "not-executable" => {
                fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap()
            }
            "dangling-link" => fs::remove_file(&target).unwrap(),
            "directory" => {
                fs::remove_file(&stable).unwrap();
                fs::create_dir(&stable).unwrap();
                fs::set_permissions(&stable, fs::Permissions::from_mode(0o700)).unwrap();
            }
            "directory-link" => {
                fs::remove_file(&target).unwrap();
                fs::create_dir(&target).unwrap();
                fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
            }
            _ => unreachable!(),
        }
        let direct = invoke();
        assert_eq!(direct.status.code(), Some(2), "{unavailable}: {direct:?}");
        assert!(direct.stdout.is_empty());
        let diagnostic: Value =
            serde_json::from_slice(&direct.stderr).expect("only JSON on stderr");
        assert_eq!(diagnostic["diagnostics"][0]["code"], "relay.unavailable");
        assert_eq!(diagnostic["status"], "error");
        let commit = f.git(&[
            "-c",
            "user.name=Relay",
            "-c",
            "user.email=relay@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "feat: blocked",
        ]);
        assert_eq!(commit.status.code(), Some(1), "{unavailable}: {commit:?}");
        let diagnostic: Value =
            serde_json::from_slice(&commit.stderr).expect("Git forwards only JSON on stderr");
        assert_eq!(diagnostic["diagnostics"][0]["code"], "relay.unavailable");
        assert!(
            !marker.exists(),
            "unavailable Relay must not run the policy"
        );
        assert!(!f.git(&["rev-parse", "--verify", "HEAD"]).status.success());
    }
}
