use crate::{Options, config::Config, repository::*, runner, support::*};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Plan {
    body: Value,
    state: Option<Value>,
    config: Option<Config>,
    source: Option<PathBuf>,
}
fn source_state(state: &Option<Value>) -> Option<&Value> {
    state.as_ref().filter(|s| s["phase"] != "inactive")
}
fn launcher(repo: &Repo, _config: &Path, binary: &Path) -> Vec<u8> {
    let mut out = b"#!/bin/sh\n# Owned by fluidise-relay; inspect before editing.\n".to_vec();
    out.extend_from_slice(b"if [ ! -f ");
    out.extend(shell_quote(binary.as_os_str()));
    out.extend_from_slice(b" ] || [ ! -x ");
    out.extend(shell_quote(binary.as_os_str()));
    out.extend_from_slice(b" ]; then\n  printf '%s\\n' '{\"schema_version\":1,\"operation\":\"run\",\"status\":\"error\",\"diagnostics\":[{\"code\":\"relay.unavailable\",\"message\":\"Installed Relay executable is unavailable\"}],\"diagnostics_complete\":true}' >&2\n  exit 2\nfi\nFLUIDISE_RELAY_HOOK=1\nexport FLUIDISE_RELAY_HOOK\nexec ");
    out.extend(shell_quote(binary.as_os_str()));
    for arg in [
        OsStr::new("run"),
        OsStr::new("--repo"),
        repo.root.as_os_str(),
        OsStr::new("--file"),
    ] {
        out.push(b' ');
        out.extend(shell_quote(arg));
    }
    out.extend_from_slice(b" \"$1\"\n");
    out
}
fn launchers(
    repo: &Repo,
    config: &Path,
    binary: &Path,
    _source: &Path,
    inventory: &Value,
) -> Result<BTreeMap<String, Arc<[u8]>>> {
    let mut files = BTreeMap::new();
    files.insert(
        "commit-msg".into(),
        Arc::from(launcher(repo, config, binary)),
    );
    let map = inventory
        .as_object()
        .ok_or_else(|| Failure::new("state.invalid", "Invalid inventory"))?;
    let mut native: Option<Arc<[u8]>> = None;
    for name in map.keys().filter(|n| n.as_str() != "commit-msg") {
        if name.contains('/') || name.is_empty() || name == ".." {
            return Err(Failure::new("state.invalid", "Invalid hook name"));
        }
        let bytes = match &native {
            Some(bytes) => Arc::clone(bytes),
            None => {
                let bytes: Arc<[u8]> = Arc::from(fs::read(std::env::current_exe()?)?);
                native = Some(Arc::clone(&bytes));
                bytes
            }
        };
        files.insert(name.clone(), bytes);
    }
    Ok(files)
}
fn desired_manifest(files: &BTreeMap<String, Arc<[u8]>>) -> Value {
    use sha2::{Digest, Sha256};
    Value::Object(
        files
            .iter()
            .map(|(name, bytes)| {
                (
                    name.clone(),
                    json!({"kind":"file","mode":448,"digest":hex(&Sha256::digest(bytes))}),
                )
            })
            .collect(),
    )
}

// Each delegated event is an independent executable copy of this same binary.
// Git still enters it if the selected binary disappears; no shell changes the
// original hook's environment before native process replacement.
pub fn delegated_hook() -> Option<Result<()>> {
    use std::os::unix::process::CommandExt;
    let invoked = PathBuf::from(std::env::args_os().next()?);
    let hooks = invoked.parent()?;
    let local = hooks.parent()?;
    if hooks.file_name()? != "hooks" || local.file_name()? != "fluidise-relay" {
        return None;
    }
    Some((|| -> Result<()> {
        let local = fs::canonicalize(local)?;
        check_private_dir(&local)?;
        let initial = read_json(&local.join("state.json"))?
            .ok_or_else(|| Failure::new("state.missing", "Delegation state missing"))?;
        let repo = Repo::discover(&value_path(&initial["identity"]["root"])?)?;
        if repo.local != local {
            return Err(Failure::new(
                "state.identity_mismatch",
                "Delegation belongs to another worktree",
            ));
        }
        let _locks = locks(&repo, false)?;
        check_private_dir(&repo.local.join("hooks"))?;
        let state = repo
            .state()?
            .ok_or_else(|| Failure::new("state.missing", "Delegation state missing"))?;
        let name = invoked
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Failure::new("state.invalid", "Invalid delegated hook name"))?;
        if name == "commit-msg"
            || state["phase"] != "active"
            || state["inventory"]
                .as_object()
                .is_none_or(|m| !m.contains_key(name))
            || state["files"][name].is_null()
            || fingerprint(&repo.local.join("hooks").join(name))? != state["files"][name]
        {
            return Err(Failure::new(
                "state.inconsistent",
                "Delegated hook is not active or owned",
            ));
        }
        if !executable(&value_path(&state["binary"])?) {
            return Err(Failure::new(
                "relay.unavailable",
                "Installed Relay executable is unavailable",
            ));
        }
        let source = value_path(&state["source"])?.join(name);
        if !executable(&source) {
            return Err(Failure::new(
                "command.unavailable",
                "Original delegated hook is not executable",
            ));
        }
        let invocation = match state["source_default"].as_bool() {
            Some(true) => repo.default_hook_invocation(name)?,
            Some(false) => source.clone(),
            None => {
                return Err(Failure::new(
                    "state.invalid",
                    "Missing original hook-path provenance",
                ));
            }
        };
        if !same_directory(
            invocation.parent().expect("hook parent"),
            source.parent().expect("source parent"),
        ) {
            return Err(Failure::new(
                "state.identity_mismatch",
                "Original hook invocation resolves outside the inspected source",
            ));
        }
        let error = std::process::Command::new(invocation)
            .args(std::env::args_os().skip(1))
            .exec();
        Err(Failure::new("command.exec_failed", error))
    })())
}
fn check_files(repo: &Repo, state: &Value, pending: bool) -> Result<()> {
    let directory = repo.local.join("hooks");
    let expected = state["files"]
        .as_object()
        .ok_or_else(|| Failure::new("state.invalid", "Invalid manifest"))?;
    let old = state["previous_files"].as_object();
    if directory.exists() {
        if !fs::symlink_metadata(&directory)?.is_dir() {
            return Err(Failure::new(
                "state.hook_modified",
                "Hook directory was replaced",
            ));
        }
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| Failure::new("state.hook_modified", "Unrecognized hook file"))?;
            if pending
                && name.strip_prefix(".relay-stage-").is_some_and(|n| {
                    expected.contains_key(n) || old.is_some_and(|m| m.contains_key(n))
                })
            {
                let metadata = fs::symlink_metadata(entry.path())?;
                if metadata.is_file() && metadata.permissions().mode() & 0o077 == 0 {
                    continue;
                }
            }
            let actual = fingerprint(&entry.path())?;
            let allowed = expected.get(name).is_some_and(|v| *v == actual)
                || (pending && old.and_then(|m| m.get(name)).is_some_and(|v| *v == actual));
            if !allowed {
                return Err(Failure::new(
                    "state.hook_modified",
                    "Owned hook content has external drift",
                ));
            }
        }
    }
    if !pending {
        for (name, wanted) in expected {
            if fingerprint(&directory.join(name))? != *wanted {
                return Err(Failure::new(
                    "state.hook_modified",
                    "Owned hook is missing or modified",
                ));
            }
        }
    }
    Ok(())
}
fn build(repo: &Repo, options: &Options, operation: &str) -> Result<Plan> {
    if ["prepare", "install", "uninstall", "unprepare"].contains(&operation) {
        for config in [
            repo.config(),
            repo.work_config(),
            repo.common.join("config.worktree"),
        ] {
            let mut path = config.into_os_string();
            path.push(".lock");
            let path = PathBuf::from(path);
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let mut error = Failure::new(
                        "git.lock_present",
                        "Git configuration is locked; verify its owner and resolve separately before inspecting again",
                    );
                    error.diagnostics[0]["location"] = path_value(&path);
                    return Err(error);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
    }
    let state = repo.state()?;
    if ["install", "approve"].contains(&operation)
        && read_json(&repo.prep_path())?
            .is_some_and(|p| p["phase"] != "prepared" && p["phase"] != "inactive")
    {
        return Err(Failure::new(
            "state.common_inconsistent",
            "Resume common preparation before activation",
        ));
    }
    if let Some(s) = &state
        && (!["inactive", "active", "installing", "uninstalling"]
            .iter()
            .any(|p| s["phase"] == *p)
            || s["generation"].as_u64().is_none()
            || s["identity"] != repo.identity())
    {
        return Err(Failure::new(
            "state.invalid",
            "State identity, version or phase is invalid",
        ));
    }
    let mut body = json!({"version":1,"operation":operation,"target":repo.identity(),"preconditions":repo.fingerprints()?,"actions":[]});
    if ["install", "uninstall"].contains(&operation) {
        body["preconditions"]["inherited_hooks"] = inherited_hooks(repo)?;
        if included_values(repo, &repo.work_config(), "core.hooksPath")?
            != values(repo, &repo.work_config(), "core.hooksPath")?
        {
            return Err(Failure::new(
                "git.config_unsupported",
                "Worktree hook configuration from includes requires a separately reviewed migration",
            ));
        }
    }
    let owned = repo.local.join("hooks");
    let mut owned_files = Vec::new();
    if owned.exists() {
        if !fs::symlink_metadata(&owned)?.is_dir() {
            return Err(Failure::new(
                "state.hook_modified",
                "Owned hook directory was replaced",
            ));
        }
        let mut names = fs::read_dir(&owned)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        names.sort();
        for name in names {
            owned_files.push(json!({"name":bytes_value(std::os::unix::ffi::OsStrExt::as_bytes(name.as_os_str())),"fingerprint":fingerprint(&owned.join(&name))?}));
        }
    }
    body["preconditions"]["owned_hooks"] = json!(owned_files);
    let mut config = None;
    let mut source = None;
    let mut actions = Vec::new();
    match operation {
        "prepare" => {
            let prep = read_json(&repo.prep_path())?;
            if let Some(p) = &prep {
                if p["common"] != path_value(&repo.common)
                    || !["inactive", "prepared", "preparing", "unpreparing"]
                        .iter()
                        .any(|s| p["phase"] == *s)
                {
                    return Err(Failure::new(
                        "state.invalid",
                        "Invalid preparation identity or phase",
                    ));
                }
                if p["phase"] == "unpreparing" {
                    return Err(Failure::new(
                        "state.preparation_incomplete",
                        "Resume unprepare before preparing again",
                    ));
                }
                if p["phase"] == "preparing" {
                    let original = decode_values(&p["before_common_worktree"])?;
                    let current = values(repo, &repo.config(), "core.worktree")?;
                    let ext = values(repo, &repo.config(), "extensions.worktreeConfig")?;
                    let main = values(repo, &repo.common.join("config.worktree"), "core.worktree")?;
                    if !(current.is_empty() || current == original)
                        || !(ext == vec![OsString::from("true")]
                            || ext == decode_values(&p["before_extension"])?)
                        || !(main == decode_values(&p["before_main_worktree"])?
                            || main == decode_values(&p["after_main_worktree"])?)
                    {
                        return Err(Failure::new(
                            "git.preparation_modified",
                            "Interrupted preparation has external drift",
                        ));
                    }
                }
            }
            if !repo.extension()? || prep.as_ref().is_some_and(|p| p["phase"] == "preparing") {
                let main = repo.common.join("config.worktree");
                let original = if let Some(p) = prep.as_ref().filter(|p| p["phase"] == "preparing")
                {
                    decode_values(&p["before_common_worktree"])?
                } else {
                    values(repo, &repo.config(), "core.worktree")?
                };
                let mut dormant = Vec::new();
                for admin in repo.administrative_dirs()? {
                    let path = admin.join("config.worktree");
                    let stamp = fingerprint(&path)?;
                    if !stamp.is_null() {
                        if stamp["kind"] != "file" {
                            return Err(Failure::new(
                                "git.migration_unsupported",
                                "Worktree configuration must be a regular file",
                            ));
                        }
                        let output = git_at(
                            &repo.root,
                            &[
                                "config".as_ref(),
                                "--file".as_ref(),
                                path.as_os_str(),
                                "--no-includes".as_ref(),
                                "--null".as_ref(),
                                "--name-only".as_ref(),
                                "--list".as_ref(),
                            ],
                        )?;
                        if !output.status.success() {
                            return Err(Failure::new(
                                "git.config_failed",
                                "Cannot inspect dormant worktree configuration",
                            ));
                        }
                        let keys: Vec<_> = output
                            .stdout
                            .split(|b| *b == 0)
                            .filter(|b| !b.is_empty())
                            .collect();
                        if !keys.is_empty()
                            && (admin != repo.common
                                || original.is_empty()
                                || keys != vec![b"core.worktree".as_slice()]
                                || values(repo, &path, "core.worktree")? != original)
                        {
                            return Err(Failure::new(
                                "git.migration_unsupported",
                                "Dormant worktree configuration would activate unapproved settings; review it separately before preparation",
                            ));
                        }
                    }
                    dormant.push(json!({"path":path_value(&path),"fingerprint":stamp}));
                }
                body["preconditions"]["worktree_configurations"] = json!(dormant);
                let old = values(repo, &repo.config(), "core.worktree")?;
                let existing = values(repo, &main, "core.worktree")?;
                if !old.is_empty() && !existing.is_empty() && old != existing {
                    return Err(Failure::new(
                        "git.migration_conflict",
                        "Main-worktree core.worktree conflicts with common configuration",
                    ));
                }
                if config_boolean(repo, &repo.config(), "core.bare")? == Some(true) {
                    return Err(Failure::new(
                        "git.migration_unsupported",
                        "core.bare=true requires a separately reviewed migration",
                    ));
                }
                body["migration"] = json!({"core_worktree":values_json(&old),"main_worktree_config":path_value(&main),"main_values":values_json(&existing)});
                actions.push(
                    "Prepare common worktree configuration, migrating core.worktree when necessary",
                );
            }
        }
        "install" | "approve" => {
            if !repo.extension()? {
                return Err(Failure::new(
                    "git.preparation_required",
                    "Inspect and apply prepare, then inspect installation again",
                ));
            }
            if operation == "approve" && source_state(&state).is_none_or(|s| s["phase"] != "active")
            {
                return Err(Failure::new(
                    "state.not_active",
                    "approve requires an active installation",
                ));
            }
            let path = runner::source_config(repo, options, source_state(&state))?;
            let c = Config::load(&path)?;
            c.check_programs(&repo.root)?;
            let src = if let Some(s) = source_state(&state) {
                check_files(repo, s, s["phase"] != "active")?;
                value_path(&s["source"])?
            } else {
                repo.source()?
            };
            if src == repo.local.join("hooks") || same_directory(&src, &repo.local.join("hooks")) {
                return Err(Failure::new(
                    "state.missing",
                    "Owned activation exists without recoverable installation state; refusing recursive delegation",
                ));
            }
            let inv = inventory(&src)?;
            body["source"] = path_value(&src);
            body["source_default"] = if let Some(s) = source_state(&state) {
                json!(s["source_default"].as_bool().ok_or_else(|| Failure::new(
                    "state.invalid",
                    "Missing original hook-path provenance"
                ))?)
            } else {
                json!(repo.source_is_default()?)
            };
            body["inventory"] = inv.clone();
            body["configuration"] = json!({"source":path_value(&path),"effective":c.effective(),"sections":c.sections,"used_sections":["hooks"],"foreign_sections":c.sections.iter().filter(|s|s.as_str()!="hooks").collect::<Vec<_>>()});
            if operation == "install" {
                let binary = options.binary.as_ref().expect("validated --binary");
                if !executable(binary) {
                    return Err(Failure::new(
                        "command.unavailable",
                        "Relay binary is not executable",
                    ));
                }
                let current = std::env::current_exe()?;
                use std::os::unix::fs::MetadataExt;
                let selected = fs::metadata(binary)?;
                let running = fs::metadata(&current)?;
                if (selected.dev(), selected.ino()) != (running.dev(), running.ino())
                    && file_digest(binary)? != file_digest(&current)?
                {
                    return Err(Failure::new(
                        "install.binary_mismatch",
                        "The selected Relay path must identify this executable or a byte-identical copy; invoke the intended installed Relay to install it",
                    ));
                }
                let files = launchers(repo, &path, binary, &src, &inv)?;
                let manifest = desired_manifest(&files);
                body["binary"] = path_value(binary);
                body["files"] = manifest.clone();
                let actual = values(repo, &repo.work_config(), "core.hooksPath")?;
                let expected = vec![repo.local.join("hooks").into_os_string()];
                let unchanged = actual == expected
                    && source_state(&state).is_some_and(|s| {
                        s["phase"] == "active"
                            && s["files"] == manifest
                            && s["approval"] == c.approval()
                            && s["config"] == path_value(&path)
                            && s["inventory"] == inv
                    });
                if !unchanged {
                    actions.push("Preserve original state, publish owned hooks, activate target and approve execution configuration");
                }
                if let Some(s) = source_state(&state) {
                    let before = decode_values(&s["before_override"])?;
                    if actual != expected && actual != before {
                        return Err(Failure::new(
                            "state.activation_modified",
                            "Activation configuration changed externally",
                        ));
                    }
                }
            } else {
                runner::verify_owned(repo, source_state(&state).expect("active state"))?;
                if state.as_ref().is_some_and(|s| {
                    s["approval"] != c.approval() || s["config"] != path_value(&c.source)
                }) {
                    actions.push("Replace the single active execution approval");
                }
            }
            config = Some(c);
            source = Some(src);
        }
        "uninstall" => {
            if let Some(s) = source_state(&state) {
                check_files(repo, s, s["phase"] != "active")?;
                let src = value_path(&s["source"])?;
                let now = fingerprint(&src.join("commit-msg"))?;
                if now != s["original_commit"] {
                    return Err(Failure::new(
                        "state.restoration_conflict",
                        "Original commit-msg source changed; restoration requires manual resolution",
                    ));
                }
                let actual = values(repo, &repo.work_config(), "core.hooksPath")?;
                let before = decode_values(&s["before_override"])?;
                if let Some(raw) = before.first()
                    && repo.source_with_override(Some(raw.as_os_str()))? != src
                {
                    return Err(Failure::new(
                        "state.restoration_conflict",
                        "Saved hook override resolves to a different source in the current environment",
                    ));
                }
                if before.is_empty() && inherited_hooks(repo)? != s["inherited_hooks"] {
                    return Err(Failure::new(
                        "state.restoration_conflict",
                        "Inherited hook configuration changed; refusing to silently activate a different source",
                    ));
                }
                body["restoration"] = json!({"override":s["before_override"],"source":s["source"],"inventory":inventory(&src)?,"retained_backup":s["backup"]});
                if actual != vec![repo.local.join("hooks").into_os_string()] && actual != before {
                    return Err(Failure::new(
                        "state.activation_modified",
                        "Refusing to overwrite externally changed activation",
                    ));
                }
                actions.push(
                    "Restore prior target activation, revoke execution approval and retain backups",
                );
            }
        }
        "unprepare" => {
            if let Some(p) = read_json(&repo.prep_path())?.filter(|p| p["phase"] != "inactive") {
                if p["phase"] != "prepared" && p["phase"] != "unpreparing" {
                    return Err(Failure::new(
                        "state.preparation_incomplete",
                        "Resume prepare before inspecting its removal",
                    ));
                }
                if p["common"] != path_value(&repo.common) {
                    return Err(Failure::new(
                        "state.identity_mismatch",
                        "Preparation belongs to another repository",
                    ));
                }
                for admin in repo.administrative_dirs()? {
                    if read_json(&admin.join("fluidise-relay/state.json"))?
                        .is_some_and(|s| s["phase"] != "inactive")
                    {
                        return Err(Failure::new(
                            "git.preparation_in_use",
                            "A Relay installation still depends on common preparation",
                        ));
                    }
                }
                let current_extension = values(repo, &repo.config(), "extensions.worktreeConfig")?;
                let current_common = values(repo, &repo.config(), "core.worktree")?;
                let current_main =
                    values(repo, &repo.common.join("config.worktree"), "core.worktree")?;
                let recovering = p["phase"] == "unpreparing";
                if !(current_extension == vec![OsString::from("true")]
                    || recovering && current_extension == decode_values(&p["before_extension"])?)
                    || !(current_common.is_empty()
                        || recovering
                            && current_common == decode_values(&p["before_common_worktree"])?)
                    || !(current_main == decode_values(&p["after_main_worktree"])?
                        || recovering && current_main == decode_values(&p["before_main_worktree"])?)
                {
                    return Err(Failure::new(
                        "git.preparation_modified",
                        "Common preparation changed externally",
                    ));
                }
                actions.push("Undo Relay-owned common preparation; external consumers must be reviewed by the approver");
            }
        }
        _ => {
            return Err(Failure::new(
                "invocation.invalid",
                "Unknown mutation operation",
            ));
        }
    }
    body["actions"] = json!(actions);
    body["id"] = json!(digest("fluidise-relay.plan.v1", &body));
    if serde_json::to_vec(&body).expect("plan JSON").len() > OUTPUT_LIMIT - 8192 {
        return Err(Failure::new(
            "output.limit_exceeded",
            "Inspection plan exceeds its representational budget",
        ));
    }
    Ok(Plan {
        body,
        state,
        config,
        source,
    })
}
fn backup(repo: &Repo, source: &Path, id: &str) -> Result<Value> {
    let directory = repo.local.join("backups");
    private_dir(&directory)?;
    let commit = source.join("commit-msg");
    let state = fingerprint(&commit)?;
    let path = directory.join(id);
    if state["kind"] == "file" {
        if !path.exists() {
            let mut input = regular_file(&commit)?;
            let stage = directory.join(format!(".relay-stage-{id}"));
            if let Ok(m) = fs::symlink_metadata(&stage)
                && (!m.is_file() || m.permissions().mode() & 0o077 != 0)
            {
                return Err(Failure::new(
                    "install.backup_changed",
                    "Unsafe backup staging file",
                ));
            }

            let mut output = publication("backup.open", || {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
                    .open(&stage)
            })?;
            publication("backup.permissions", || {
                output.set_permissions(fs::Permissions::from_mode(0o600))
            })?;
            publication("backup.write", || std::io::copy(&mut input, &mut output))?;
            publication("backup.file_sync", || output.sync_all())?;
            if file_digest(&stage)? != state["digest"].as_str().unwrap_or("") {
                return Err(Failure::new(
                    "install.backup_changed",
                    "Source changed during backup",
                ));
            }
            publication("backup.rename", || fs::rename(stage, &path))?;
            publication("backup.parent_sync", || File::open(&directory)?.sync_all())?;
        }
        if file_digest(&path)? != state["digest"].as_str().unwrap_or("") {
            return Err(Failure::new(
                "install.backup_changed",
                "Backup is incomplete or changed",
            ));
        }
    } else if state["kind"] == "symlink" {
        let meta = directory.join(format!("{id}.json"));
        if !meta.exists() {
            write_json(&meta, &json!({"version":1,"link":state}))?;
        }
    }
    Ok(json!({"path":path_value(&path),"original":state}))
}
fn prepare(repo: &Repo) -> Result<()> {
    let mut p = if let Some(p) = read_json(&repo.prep_path())?.filter(|p| p["phase"] == "preparing")
    {
        p
    } else {
        let common = values(repo, &repo.config(), "core.worktree")?;
        let main = values(repo, &repo.common.join("config.worktree"), "core.worktree")?;
        json!({"version":1,"phase":"preparing","common":path_value(&repo.common),"before_main_exists":repo.common.join("config.worktree").exists(),"before_extension":values_json(&values(repo,&repo.config(),"extensions.worktreeConfig")?),"before_common_worktree":values_json(&common),"before_main_worktree":values_json(&main),"after_main_worktree":values_json(if common.is_empty(){&main}else{&common})})
    };
    write_json(&repo.prep_path(), &p)?;
    #[cfg(test)]
    checkpoint("prepare.after_journal")?;
    let migration = decode_values(&p["before_common_worktree"])?;
    if !migration.is_empty() {
        replace_values(
            repo,
            &repo.common.join("config.worktree"),
            "core.worktree",
            &migration,
        )?;
    }
    #[cfg(test)]
    checkpoint("prepare.after_main")?;
    replace_values(
        repo,
        &repo.config(),
        "extensions.worktreeConfig",
        &[OsString::from("true")],
    )?;
    #[cfg(test)]
    checkpoint("prepare.after_extension")?;
    if !migration.is_empty() {
        replace_values(repo, &repo.config(), "core.worktree", &[])?;
    }
    #[cfg(test)]
    checkpoint("prepare.after_migration")?;
    p["phase"] = json!("prepared");
    write_json(&repo.prep_path(), &p)?;
    #[cfg(test)]
    checkpoint("prepare.after_finalize")?;
    Ok(())
}
fn install(repo: &Repo, options: &Options, plan: &Plan) -> Result<()> {
    let config = plan.config.as_ref().expect("install config");
    let source = plan.source.as_ref().expect("install source");
    let id = plan.body["id"].as_str().expect("plan id");
    let previous = source_state(&plan.state);
    let mut state = if let Some(s) = previous {
        s.clone()
    } else {
        json!({"version":1,"identity":repo.identity(),"generation":plan.state.as_ref().and_then(|s|s["generation"].as_u64()).unwrap_or(0),"source":path_value(source),"source_default":plan.body["source_default"],"original_commit":fingerprint(&source.join("commit-msg"))?,"before_work_config_exists":repo.work_config().exists(),"before_override":values_json(&values(repo,&repo.work_config(),"core.hooksPath")?),"inherited_hooks":inherited_hooks(repo)?,"backup":backup(repo,source,id)?})
    };
    #[cfg(test)]
    checkpoint("install.after_backup")?;
    // Retain ownership across interrupted updates, including retired hook names.
    let mut previous_files = state["previous_files"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(files) = state["files"].as_object() {
        for (name, value) in files {
            previous_files
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
    }
    let hooks = repo.local.join("hooks");
    if hooks.exists() {
        for entry in fs::read_dir(&hooks)? {
            let entry = entry?;
            let name = entry.file_name();
            if let Some(name) = name.to_str().filter(|s| !s.starts_with(".relay-stage-")) {
                previous_files.insert(name.into(), fingerprint(&entry.path())?);
            }
        }
    }
    state["previous_files"] = Value::Object(previous_files);
    state["files"] = plan.body["files"].clone();
    state["phase"] = json!("installing");
    state["config"] = path_value(&config.source);
    state["inventory"] = plan.body["inventory"].clone();
    state["approval"] = json!(config.approval());
    state["binary"] = plan.body["binary"].clone();
    write_json(&repo.state_path(), &state)?;
    #[cfg(test)]
    checkpoint("install.after_journal")?;
    private_dir(&hooks)?;
    let files = launchers(
        repo,
        &config.source,
        options.binary.as_ref().expect("binary"),
        source,
        &state["inventory"],
    )?;
    if desired_manifest(&files) != state["files"] {
        return Err(Failure::new(
            "plan.stale",
            "Launcher executable changed before publication; inspect recovery",
        ));
    }
    for (name, bytes) in &files {
        if fingerprint(&hooks.join(name))? != state["files"][name] {
            atomic_write(&hooks.join(name), bytes, 0o700)?;
            #[cfg(test)]
            checkpoint("install.after_hook")?;
        }
    }
    if let Some(old) = state["previous_files"].as_object() {
        for name in old.keys() {
            if !files.contains_key(name) && !fingerprint(&hooks.join(name))?.is_null() {
                remove_owned_file(hooks.join(name))?;
            }
        }
    }
    for entry in fs::read_dir(&hooks)? {
        let entry = entry?;
        let n = entry.file_name();
        if n.to_str().is_some_and(|s| s.starts_with(".relay-stage-")) {
            remove_owned_file(entry.path())?;
        }
    }
    replace_values(
        repo,
        &repo.work_config(),
        "core.hooksPath",
        &[hooks.into_os_string()],
    )?;
    #[cfg(test)]
    checkpoint("install.after_activation")?;
    if repo.source()? != repo.local.join("hooks") {
        return Err(Failure::new(
            "state.activation_modified",
            "Effective Git hook source does not match the inspected activation",
        ));
    }
    check_files(repo, &state, false)?;
    state["phase"] = json!("active");
    state["generation"] = json!(
        state["generation"]
            .as_u64()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Failure::new("state.limit_exceeded", "State generation overflow"))?
    );
    state
        .as_object_mut()
        .expect("state object")
        .remove("previous_files");
    write_json(&repo.state_path(), &state)?;
    #[cfg(test)]
    checkpoint("install.after_finalize")?;
    Ok(())
}
fn uninstall(repo: &Repo, plan: &Plan) -> Result<()> {
    let mut state = plan.state.clone().expect("installed state");
    state["phase"] = json!("uninstalling");
    state["approval"] = Value::Null;
    write_json(&repo.state_path(), &state)?;
    #[cfg(test)]
    checkpoint("uninstall.after_journal")?;
    replace_values(
        repo,
        &repo.work_config(),
        "core.hooksPath",
        &decode_values(&state["before_override"])?,
    )?;
    #[cfg(test)]
    checkpoint("uninstall.after_activation")?;
    if state["before_work_config_exists"] == false
        && repo.work_config().exists()
        && fs::metadata(repo.work_config())?.len() == 0
    {
        remove_owned_file(repo.work_config())?;
    }
    let hooks = repo.local.join("hooks");
    let mut names = std::collections::BTreeSet::new();
    for key in ["files", "previous_files"] {
        if let Some(files) = state[key].as_object() {
            names.extend(files.keys());
        }
    }
    for name in names {
        let path = hooks.join(name);
        if !fingerprint(&path)?.is_null() {
            remove_owned_file(path)?;
            #[cfg(test)]
            checkpoint("uninstall.after_hook")?;
        }
        let stage = hooks.join(format!(".relay-stage-{name}"));
        if !fingerprint(&stage)?.is_null() {
            remove_owned_file(stage)?;
        }
    }
    if hooks.exists() {
        remove_owned_dir(&hooks)?;
    }
    state["phase"] = json!("inactive");
    state["generation"] = json!(
        state["generation"]
            .as_u64()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| Failure::new("state.limit_exceeded", "State generation overflow"))?
    );
    write_json(&repo.state_path(), &state)?;
    #[cfg(test)]
    checkpoint("uninstall.after_finalize")?;
    Ok(())
}
fn unprepare(repo: &Repo) -> Result<()> {
    let mut p = read_json(&repo.prep_path())?
        .ok_or_else(|| Failure::new("state.missing", "Preparation record missing"))?;
    p["phase"] = json!("unpreparing");
    write_json(&repo.prep_path(), &p)?;
    #[cfg(test)]
    checkpoint("unprepare.after_journal")?;
    replace_values(
        repo,
        &repo.config(),
        "core.worktree",
        &decode_values(&p["before_common_worktree"])?,
    )?;
    #[cfg(test)]
    checkpoint("unprepare.after_common")?;
    replace_values(
        repo,
        &repo.common.join("config.worktree"),
        "core.worktree",
        &decode_values(&p["before_main_worktree"])?,
    )?;
    #[cfg(test)]
    checkpoint("unprepare.after_main")?;
    replace_values(
        repo,
        &repo.config(),
        "extensions.worktreeConfig",
        &decode_values(&p["before_extension"])?,
    )?;
    #[cfg(test)]
    checkpoint("unprepare.after_extension")?;
    let main = repo.common.join("config.worktree");
    if p["before_main_exists"] == false && main.exists() && fs::metadata(&main)?.len() == 0 {
        remove_owned_file(main)?;
    }
    p["phase"] = json!("inactive");
    write_json(&repo.prep_path(), &p)?;
    #[cfg(test)]
    checkpoint("unprepare.after_finalize")?;
    Ok(())
}
fn effects(repo: &Repo) -> Result<Value> {
    use std::os::unix::fs::MetadataExt;
    fn namespace(path: &Path, depth: u8) -> Result<Value> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
            Err(e) => return Err(e.into()),
        };
        if metadata.is_dir() {
            let mut entries = Vec::new();
            if depth > 0 {
                let mut names = fs::read_dir(path)?
                    .map(|e| e.map(|e| e.file_name()))
                    .collect::<std::io::Result<Vec<_>>>()?;
                names.sort();
                for name in names {
                    entries.push(json!({"name":bytes_value(std::os::unix::ffi::OsStrExt::as_bytes(name.as_os_str())),"state":namespace(&path.join(&name),depth-1)?}));
                }
            }
            Ok(json!({"kind":"directory","mode":metadata.mode(),"entries":entries}))
        } else if metadata.file_type().is_symlink() {
            Ok(json!({"kind":"symlink","target":path_value(&fs::read_link(path)?)}))
        } else {
            Ok(
                json!({"kind":"file","inode":metadata.ino(),"device":metadata.dev(),"size":metadata.len(),"mode":metadata.mode(),"modified_seconds":metadata.mtime(),"modified_nanos":metadata.mtime_nsec()}),
            )
        }
    }
    Ok(
        json!({"git":repo.fingerprints()?,"local":namespace(&repo.local,2)?,"common":namespace(&repo.shared,2)?}),
    )
}
pub fn dispatch(repo: &Repo, options: &Options) -> Result<(Value, u8)> {
    let operation = if options.operation == "inspect" {
        options.target.as_deref().expect("inspection target")
    } else {
        &options.operation
    };
    let before = build(repo, options, operation)?;
    if options.operation == "inspect" {
        return Ok((
            json!({"schema_version":1,"operation":"inspect","status":"success","diagnostics":[],"diagnostics_complete":true,"plan":before.body}),
            0,
        ));
    }
    if before.body["id"].as_str() != options.plan.as_deref() {
        return Err(Failure::new(
            "plan.stale",
            "Plan no longer matches current state; inspect again",
        ));
    }
    if before.body["actions"]
        .as_array()
        .is_some_and(|a| a.is_empty())
    {
        return Ok((
            json!({"schema_version":1,"operation":operation,"status":"success","changed":false,"diagnostics":[],"diagnostics_complete":true}),
            0,
        ));
    }
    let _locks = locks(repo, true)?;
    let plan = build(repo, options, operation)?;
    if plan.body["id"].as_str() != options.plan.as_deref() {
        return Err(Failure::new(
            "plan.stale",
            "Plan changed while acquiring locks; inspect again",
        ));
    }
    let before_effects = effects(repo)?;
    let outcome = (|| -> Result<()> {
        private_dir(&repo.shared)?;
        private_dir(&repo.local)?;
        match operation {
            "prepare" => prepare(repo),
            "install" => install(repo, options, &plan),
            "uninstall" => uninstall(repo, &plan),
            "unprepare" => unprepare(repo),
            "approve" => {
                let mut state = plan.state.clone().expect("active state");
                state["approval"] =
                    json!(plan.config.as_ref().expect("approval config").approval());
                state["config"] = path_value(&plan.config.as_ref().expect("config").source);
                state["generation"] = json!(
                    state["generation"]
                        .as_u64()
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or_else(|| Failure::new(
                            "state.limit_exceeded",
                            "State generation overflow"
                        ))?
                );
                write_json(&repo.state_path(), &state)
            }
            _ => Err(Failure::new("invocation.invalid", "Unknown operation")),
        }
    })();
    match outcome {
        Ok(()) => {
            let mut result = json!({"schema_version":1,"operation":operation,"status":"success","changed":true,"diagnostics":[],"diagnostics_complete":true});
            if operation == "uninstall" {
                result["retained_backups"] = path_value(&repo.local.join("backups"));
            }
            Ok((result, 0))
        }
        Err(e) => {
            let mut result = e.result(operation);
            match effects(repo) {
                Ok(after) => result["changed"] = json!(after != before_effects),
                Err(_) => {
                    result["changed"] = json!(true);
                    result["diagnostics"].as_array_mut().expect("diagnostics").push(json!({"code":"state.changes_unconfirmed","message":"Could not verify persisted effects after failure; inspect before retrying"}));
                }
            }
            Ok((result, 2))
        }
    }
}

#[cfg(test)]
thread_local! { static FAULT:std::cell::Cell<Option<&'static str>>=const {std::cell::Cell::new(None)}; }
#[cfg(test)]
fn checkpoint(name: &str) -> Result<()> {
    FAULT.with(|point| {
        if point.get() == Some(name) {
            point.set(None);
            Err(Failure::new("test.injected", name))
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        root: PathBuf,
        repo: Repo,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "relay-fault-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            assert!(
                git_at(
                    &root,
                    &["init".as_ref(), "--quiet".as_ref(), "--template=".as_ref()]
                )
                .unwrap()
                .status
                .success()
            );
            let repo = Repo::discover(&root).unwrap();
            fs::create_dir(repo.common.join("hooks")).unwrap();
            fs::write(repo.common.join("hooks/commit-msg"), "#!/bin/sh\nexit 99\n").unwrap();
            replace_values(
                &repo,
                &repo.config(),
                "core.worktree",
                &[repo.root.clone().into_os_string()],
            )
            .unwrap();
            fs::write(repo.root.join("fluidise.toml"),"[hooks]\nversion=1\n[[hooks.commit-msg]]\nid='check'\nprogram='/bin/sh'\nargs=['-c','exit 0']\n").unwrap();
            Self { root, repo }
        }
        fn apply(&self, operation: &str) -> (Value, u8) {
            let mut options = Options {
                operation: operation.into(),
                binary: if operation == "install" {
                    Some(std::env::current_exe().unwrap())
                } else {
                    None
                },
                ..Options::default()
            };
            let plan = build(&self.repo, &options, operation).unwrap();
            options.plan = Some(plan.body["id"].as_str().unwrap().into());
            dispatch(&self.repo, &options).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn interrupted_update_keeps_ownership_of_retired_delegation() {
        let f = Fixture::new();
        f.apply("prepare");
        let source = f.repo.common.join("hooks/pre-commit");
        fs::write(&source, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(f.apply("install").1, 0);
        fs::remove_file(source).unwrap();
        FAULT.with(|p| p.set(Some("install.after_journal")));
        assert_eq!(f.apply("install").1, 2);
        assert_eq!(f.apply("install").1, 0);
        assert!(!f.repo.local.join("hooks/pre-commit").exists());
        runner::verify_owned(&f.repo, &f.repo.state().unwrap().unwrap()).unwrap();
    }
    #[test]
    fn interrupted_update_can_be_uninstalled_with_retired_delegation() {
        let f = Fixture::new();
        f.apply("prepare");
        let source = f.repo.common.join("hooks/pre-commit");
        fs::write(&source, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(f.apply("install").1, 0);
        fs::remove_file(source).unwrap();
        FAULT.with(|p| p.set(Some("install.after_journal")));
        assert_eq!(f.apply("install").1, 2);
        assert_eq!(f.apply("uninstall").1, 0);
        assert_eq!(f.repo.state().unwrap().unwrap()["phase"], "inactive");
        assert!(!f.repo.local.join("hooks").exists());
        assert!(f.repo.local.join("backups").exists());
    }
    #[test]
    fn native_delegate_staging_resumes_or_removes_safely() {
        for operation in ["install", "uninstall"] {
            let f = Fixture::new();
            f.apply("prepare");
            let source = f.repo.common.join("hooks/pre-commit");
            fs::write(&source, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
            assert_eq!(f.apply("install").1, 0);
            let hooks = f.repo.local.join("hooks");
            fs::rename(
                hooks.join("pre-commit"),
                hooks.join(".relay-stage-pre-commit"),
            )
            .unwrap();
            let mut state = f.repo.state().unwrap().unwrap();
            state["phase"] = json!("installing");
            write_json(&f.repo.state_path(), &state).unwrap();
            assert_eq!(f.apply(operation).1, 0);
            assert!(!hooks.join(".relay-stage-pre-commit").exists());
            if operation == "install" {
                runner::verify_owned(&f.repo, &f.repo.state().unwrap().unwrap()).unwrap();
            } else {
                assert!(!hooks.exists());
            }
        }
    }
    #[test]
    fn every_persistent_publication_boundary_recovers() {
        fn operation(case: &str) -> &str {
            match case {
                "install-delegate" | "install-symlink" | "update-retired" => "install",
                "uninstall-staged" | "uninstall-new-config" => "uninstall",
                other => other,
            }
        }
        fn fixture(case: &str) -> Fixture {
            let f = Fixture::new();
            if case == "uninstall-new-config" {
                replace_values(&f.repo, &f.repo.config(), "core.worktree", &[]).unwrap();
            }
            if case == "install-symlink" {
                let original = f.repo.common.join("hooks/commit-msg");
                let target = f.root.join("original-hook");
                fs::rename(&original, &target).unwrap();
                std::os::unix::fs::symlink(target, original).unwrap();
            }
            if ["install-delegate", "update-retired", "uninstall-staged"].contains(&case) {
                let hook = f.repo.common.join("hooks/pre-commit");
                fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
                fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let config = f.repo.root.join("fluidise.toml");
            fs::write(
                &config,
                fs::read_to_string(&config)
                    .unwrap()
                    .replace("exit 0", "printf unexpected > unexpected-execution; exit 0"),
            )
            .unwrap();
            if case != "prepare" {
                assert_eq!(f.apply("prepare").1, 0);
            }
            if operation(case) == "uninstall" || ["approve", "update-retired"].contains(&case) {
                assert_eq!(f.apply("install").1, 0);
            }
            if case == "approve" {
                fs::write(
                    &config,
                    fs::read_to_string(&config)
                        .unwrap()
                        .replace("exit 0", "exit 1"),
                )
                .unwrap();
            }
            if case == "update-retired" {
                fs::remove_file(f.repo.common.join("hooks/pre-commit")).unwrap();
            }
            if case == "uninstall-staged" {
                let hooks = f.repo.local.join("hooks");
                fs::rename(
                    hooks.join("pre-commit"),
                    hooks.join(".relay-stage-pre-commit"),
                )
                .unwrap();
                let mut state = f.repo.state().unwrap().unwrap();
                state["phase"] = json!("installing");
                write_json(&f.repo.state_path(), &state).unwrap();
            }
            f
        }
        let mut total = 0;
        for case in [
            "prepare",
            "install",
            "install-delegate",
            "install-symlink",
            "update-retired",
            "approve",
            "uninstall",
            "uninstall-staged",
            "uninstall-new-config",
            "unprepare",
        ] {
            let baseline = fixture(case);
            persistence_fault::arm(None);
            assert_eq!(baseline.apply(operation(case)).1, 0);
            let trace = persistence_fault::trace();
            assert!(!trace.is_empty(), "{case}");
            for (index, point) in trace.iter().enumerate() {
                persistence_fault::arm(None);
                let f = fixture(case);
                persistence_fault::arm(Some(index));
                let (result, code) = f.apply(operation(case));
                let recorded = persistence_fault::trace();
                persistence_fault::arm(None);
                assert_eq!(code, 2, "{case} {index} {point:?}: {result}");
                assert_eq!(
                    result["diagnostics"][0]["code"], "test.persistence_failure",
                    "{result}"
                );
                assert_eq!(recorded.last(), Some(point), "{case} {index}");
                assert!(!f.root.join("unexpected-execution").exists());
                assert_eq!(
                    f.apply(operation(case)).1,
                    0,
                    "resume {case} {index} {point:?}"
                );
                if let Some(state) = f.repo.state().unwrap()
                    && state["phase"] == "active"
                {
                    runner::verify_owned(&f.repo, &state).unwrap();
                }
                assert!(!f.root.join("unexpected-execution").exists());
                assert_eq!(
                    fs::read_to_string(f.repo.common.join("hooks/commit-msg")).unwrap(),
                    "#!/bin/sh\nexit 99\n"
                );
            }
            total += trace.len();
            eprintln!(
                "{case}: {} before/after publication failures recovered",
                trace.len()
            );
        }
        eprintln!("total publication boundaries exercised: {total}");
    }
    #[test]
    fn persistent_phase_failures_resume_without_implicit_execution() {
        let points = [
            (
                "prepare",
                vec![
                    "prepare.after_journal",
                    "prepare.after_main",
                    "prepare.after_extension",
                    "prepare.after_migration",
                    "prepare.after_finalize",
                ],
            ),
            (
                "install",
                vec![
                    "install.after_backup",
                    "install.after_journal",
                    "install.after_hook",
                    "install.after_activation",
                    "install.after_finalize",
                ],
            ),
            (
                "uninstall",
                vec![
                    "uninstall.after_journal",
                    "uninstall.after_activation",
                    "uninstall.after_hook",
                    "uninstall.after_finalize",
                ],
            ),
            (
                "unprepare",
                vec![
                    "unprepare.after_journal",
                    "unprepare.after_common",
                    "unprepare.after_main",
                    "unprepare.after_extension",
                    "unprepare.after_finalize",
                ],
            ),
        ];
        for (operation, points) in points {
            for point in points {
                let f = Fixture::new();
                if operation != "prepare" {
                    assert_eq!(f.apply("prepare").1, 0);
                }
                if operation == "uninstall" {
                    assert_eq!(f.apply("install").1, 0);
                }
                FAULT.with(|p| p.set(Some(point)));
                let (result, code) = f.apply(operation);
                assert_eq!(code, 2, "{point}: {result}");
                assert_eq!(result["diagnostics"][0]["code"], "test.injected", "{point}");
                assert_eq!(f.apply(operation).1, 0, "resume {point}");
                if operation == "install" {
                    runner::verify_owned(&f.repo, &f.repo.state().unwrap().unwrap()).unwrap();
                }
                if operation == "uninstall" {
                    assert_eq!(f.repo.state().unwrap().unwrap()["phase"], "inactive");
                }
                if operation == "prepare" {
                    assert!(f.repo.extension().unwrap());
                }
                if operation == "unprepare" {
                    assert!(!f.repo.extension().unwrap());
                }
            }
        }
    }
}
