use crate::support::*;
use serde_json::{Value, json};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub struct Repo {
    pub root: PathBuf,
    pub git: PathBuf,
    pub common: PathBuf,
    pub local: PathBuf,
    pub shared: PathBuf,
}
pub fn git_at(dir: &Path, args: &[&OsStr]) -> Result<Output> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir);
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        cmd.env_remove(key);
    }
    #[cfg(test)]
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT");
    cmd.args(args).stdin(std::process::Stdio::null());
    cmd.output().map_err(|e| Failure::new("git.unavailable", e))
}
fn git_line(dir: &Path, args: &[&OsStr]) -> Result<OsString> {
    git_output_line(git_at(dir, args)?)
}
fn git_output_line(out: Output) -> Result<OsString> {
    if !out.status.success() {
        return Err(Failure::new(
            "git.failed",
            String::from_utf8_lossy(&out.stderr),
        ));
    }
    let mut bytes = out.stdout;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(OsString::from_vec(bytes))
}
fn split_config_key(key: &str) -> Option<(&str, Option<&str>, &str)> {
    let dot1 = key.find('.')?;
    let section = &key[..dot1];
    let remainder = &key[dot1 + 1..];
    if let Some(dot2) = remainder.rfind('.') {
        let subsection = &remainder[..dot2];
        let var = &remainder[dot2 + 1..];
        Some((section, Some(subsection), var))
    } else {
        Some((section, None, remainder))
    }
}

fn unescape_git_config_value(mut v: &[u8]) -> Vec<u8> {
    if v.len() >= 2 && v.first() == Some(&b'"') && v.last() == Some(&b'"') {
        v = &v[1..v.len() - 1];
    }
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        if v[i] == b'\\' && i + 1 < v.len() {
            match v[i + 1] {
                b'n' => out.push(b'\n'),
                b't' => out.push(b'\t'),
                b'\\' => out.push(b'\\'),
                b'"' => out.push(b'"'),
                b'b' => out.push(0x08),
                other => {
                    out.push(other);
                }
            }
            i += 2;
        } else {
            out.push(v[i]);
            i += 1;
        }
    }
    out
}

fn fast_config_values(file: &Path, key: &str) -> Result<Option<Vec<OsString>>> {
    let (target_sec, target_sub, target_var) = match split_config_key(key) {
        Some(parts) => parts,
        None => return Ok(None),
    };
    let content = match fs::read(file) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let mut current_sec: Option<String> = None;
    let mut current_sub: Option<String> = None;
    let mut values = Vec::new();

    for line in content.split(|&b| b == b'\n') {
        let mut trimmed = line;
        while let Some(&b) = trimmed.first() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                trimmed = &trimmed[1..];
            } else {
                break;
            }
        }
        while let Some(&b) = trimmed.last() {
            if b == b' ' || b == b'\t' || b == b'\r' {
                trimmed = &trimmed[..trimmed.len() - 1];
            } else {
                break;
            }
        }
        if trimmed.is_empty() || trimmed[0] == b'#' || trimmed[0] == b';' {
            continue;
        }
        if trimmed[0] == b'[' {
            let Some(end) = trimmed.iter().rposition(|&b| b == b']') else {
                return Ok(None);
            };
            let inside = match std::str::from_utf8(&trimmed[1..end]) {
                Ok(s) => s.trim(),
                Err(_) => return Ok(None),
            };
            if let Some(space_idx) = inside.find(|c: char| c.is_whitespace()) {
                let sec = inside[..space_idx].trim();
                let mut sub = inside[space_idx..].trim();
                if sub.starts_with('"') && sub.ends_with('"') && sub.len() >= 2 {
                    sub = &sub[1..sub.len() - 1];
                }
                current_sec = Some(sec.to_ascii_lowercase());
                current_sub = Some(sub.to_string());
            } else {
                current_sec = Some(inside.to_ascii_lowercase());
                current_sub = None;
            }
            continue;
        }
        let Some(eq_idx) = trimmed.iter().position(|&b| b == b'=') else {
            let k_str = match std::str::from_utf8(trimmed) {
                Ok(s) => s.trim(),
                Err(_) => return Ok(None),
            };
            if let Some(sec) = &current_sec
                && sec.eq_ignore_ascii_case(target_sec)
                && current_sub.as_deref() == target_sub
                && k_str.eq_ignore_ascii_case(target_var)
            {
                values.push(OsString::from("true"));
            }
            continue;
        };
        let k_bytes = &trimmed[..eq_idx];
        let v_bytes = &trimmed[eq_idx + 1..];
        let k_str = match std::str::from_utf8(k_bytes) {
            Ok(s) => s.trim(),
            Err(_) => return Ok(None),
        };
        if let Some(sec) = &current_sec
            && sec.eq_ignore_ascii_case(target_sec)
            && current_sub.as_deref() == target_sub
            && k_str.eq_ignore_ascii_case(target_var)
        {
            let mut v = v_bytes;
            while let Some(&b) = v.first() {
                if b == b' ' || b == b'\t' {
                    v = &v[1..];
                } else {
                    break;
                }
            }
            while let Some(&b) = v.last() {
                if b == b' ' || b == b'\t' {
                    v = &v[..v.len() - 1];
                } else {
                    break;
                }
            }
            let unescaped = unescape_git_config_value(v);
            values.push(OsString::from_vec(unescaped));
        }
    }
    Ok(Some(values))
}

fn discover_in_process(path: &Path) -> Option<Repo> {
    let canonical = fs::canonicalize(path).ok()?;
    let mut current = if canonical.is_file() {
        canonical.parent()?
    } else {
        canonical.as_path()
    };
    loop {
        let dotgit = current.join(".git");
        if dotgit.is_dir() {
            let git = fs::canonicalize(&dotgit).ok()?;
            let common = if git.join("commondir").is_file() {
                let content = fs::read_to_string(git.join("commondir")).ok()?;
                fs::canonicalize(git.join(content.trim())).ok()?
            } else {
                git.clone()
            };
            let root =
                if let Ok(Some(vals)) = fast_config_values(&git.join("config"), "core.worktree") {
                    if let Some(wt) = vals.first() {
                        let wt_path = PathBuf::from(wt);
                        if wt_path.is_absolute() {
                            fs::canonicalize(wt_path).ok()?
                        } else {
                            fs::canonicalize(git.join(wt_path)).ok()?
                        }
                    } else {
                        current.to_path_buf()
                    }
                } else {
                    current.to_path_buf()
                };
            return Some(Repo {
                local: git.join("fluidise-relay"),
                shared: common.join("fluidise-relay-common"),
                root,
                git,
                common,
            });
        } else if dotgit.is_file() {
            let content = fs::read_to_string(&dotgit).ok()?;
            for line in content.lines() {
                let trimmed = line.trim();
                if let Some(rest) = trimmed.strip_prefix("gitdir:") {
                    let gitdir_path = rest.trim();
                    let git = fs::canonicalize(current.join(gitdir_path)).ok()?;
                    let root = current.to_path_buf();
                    let common = if git.join("commondir").is_file() {
                        let c = fs::read_to_string(git.join("commondir")).ok()?;
                        fs::canonicalize(git.join(c.trim())).ok()?
                    } else {
                        git.clone()
                    };
                    return Some(Repo {
                        local: git.join("fluidise-relay"),
                        shared: common.join("fluidise-relay-common"),
                        root,
                        git,
                        common,
                    });
                }
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    None
}

impl Repo {
    pub fn discover(path: &Path) -> Result<Self> {
        if let Some(repo) = discover_in_process(path) {
            return Ok(repo);
        }
        let root = PathBuf::from(git_line(
            path,
            &["rev-parse".as_ref(), "--show-toplevel".as_ref()],
        )?);
        let git = PathBuf::from(git_line(
            &root,
            &["rev-parse".as_ref(), "--absolute-git-dir".as_ref()],
        )?);
        let common = PathBuf::from(git_line(
            &root,
            &[
                "rev-parse".as_ref(),
                "--path-format=absolute".as_ref(),
                "--git-common-dir".as_ref(),
            ],
        )?);
        let root = fs::canonicalize(root)?;
        let git = fs::canonicalize(git)?;
        let common = fs::canonicalize(common)?;
        Ok(Self {
            local: git.join("fluidise-relay"),
            shared: common.join("fluidise-relay-common"),
            root,
            git,
            common,
        })
    }
    pub fn identity(&self) -> Value {
        json!({"root":path_value(&self.root),"git":path_value(&self.git),"common":path_value(&self.common)})
    }
    pub fn config(&self) -> PathBuf {
        self.common.join("config")
    }
    pub fn work_config(&self) -> PathBuf {
        self.git.join("config.worktree")
    }
    pub fn state_path(&self) -> PathBuf {
        self.local.join("state.json")
    }
    pub fn prep_path(&self) -> PathBuf {
        self.shared.join("preparation.json")
    }
    pub fn state(&self) -> Result<Option<Value>> {
        let state = read_json(&self.state_path())?;
        if let Some(s) = &state {
            if s["identity"] != self.identity()
                || s["generation"].as_u64().is_none()
                || !["active", "inactive", "installing", "uninstalling"]
                    .iter()
                    .any(|phase| s["phase"] == *phase)
            {
                return Err(Failure::new(
                    "state.invalid",
                    "Invalid state identity, phase or generation",
                ));
            }
            if s["phase"] != "inactive" {
                for key in ["source", "config", "binary"] {
                    if !value_path(&s[key])?.is_absolute() {
                        return Err(Failure::new(
                            "state.invalid",
                            "State paths must be absolute",
                        ));
                    }
                }
                if value_path(&s["source"])? == self.local.join("hooks")
                    || same_directory(&value_path(&s["source"])?, &self.local.join("hooks"))
                {
                    return Err(Failure::new(
                        "state.invalid",
                        "Owned hooks cannot be their own delegation source",
                    ));
                }
                let _ = decode_values(&s["before_override"])?;
                for key in ["files", "inventory", "previous_files"] {
                    if key == "previous_files" && s[key].is_null() {
                        continue;
                    }
                    let map = s[key].as_object().ok_or_else(|| {
                        Failure::new("state.invalid", "Invalid state hook manifest")
                    })?;
                    for name in map.keys() {
                        if name.is_empty()
                            || name.starts_with('.')
                            || name.contains('/')
                            || name.contains('\0')
                        {
                            return Err(Failure::new(
                                "state.invalid",
                                "State hook names must be plain basenames",
                            ));
                        }
                    }
                }
            }
        }
        Ok(state)
    }
    pub fn extension(&self) -> Result<bool> {
        Ok(config_boolean(self, &self.config(), "extensions.worktreeConfig")?.unwrap_or(false))
    }
    pub fn source(&self) -> Result<PathBuf> {
        self.source_with_override(None)
    }
    pub fn source_with_override(&self, raw: Option<&OsStr>) -> Result<PathBuf> {
        let setting = raw.map(|value| {
            let mut setting = OsString::from("core.hooksPath=");
            setting.push(value);
            setting
        });
        let mut args = Vec::new();
        if let Some(setting) = &setting {
            args.extend([OsStr::new("-c"), setting.as_os_str()]);
        }
        args.extend(["config", "--path", "--null", "--get", "core.hooksPath"].map(OsStr::new));
        let out = git_at(&self.root, &args)?;
        match out.status.code() {
            Some(1) => Ok(self.common.join("hooks")),
            Some(0) => {
                let mut b = out.stdout;
                if b.pop() != Some(0) {
                    return Err(Failure::new("git.config_failed", "Invalid Git path output"));
                }
                let p = PathBuf::from(OsString::from_vec(b));
                if p.is_relative() {
                    return Err(Failure::new(
                        "git.relative_hooks_path",
                        "Relative core.hooksPath requires an explicit absolute-path migration; Relay will not guess event-specific directories",
                    ));
                }
                Ok(p)
            }
            _ => Err(Failure::new(
                "git.config_failed",
                "Cannot discover hook source",
            )),
        }
    }
    pub fn source_is_default(&self) -> Result<bool> {
        let out = git_at(
            &self.root,
            &[
                "config".as_ref(),
                "--get".as_ref(),
                "core.hooksPath".as_ref(),
            ],
        )?;
        match out.status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => Err(Failure::new(
                "git.config_failed",
                "Cannot determine hook-path provenance",
            )),
        }
    }
    pub fn default_hook_invocation(&self, name: &str) -> Result<PathBuf> {
        // Unlike explicit --repo discovery, a native delegated event must keep
        // Git's own routing context to preserve its original path spelling.
        let common = git_output_line(
            Command::new("git")
                .args(["rev-parse", "--git-common-dir"])
                .stdin(std::process::Stdio::null())
                .output()
                .map_err(|e| Failure::new("git.unavailable", e))?,
        )?;
        let base = if common == "." {
            PathBuf::new()
        } else {
            PathBuf::from(common)
        };
        Ok(base.join("hooks").join(name))
    }
    pub fn administrative_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut admins = vec![self.common.clone()];
        let directory = self.common.join("worktrees");
        if directory.exists() {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    return Err(Failure::new(
                        "git.migration_unsupported",
                        "Worktree administrative entries must be directories",
                    ));
                }
                admins.push(entry.path());
            }
        }
        admins.sort();
        Ok(admins)
    }
    pub fn fingerprints(&self) -> Result<Value> {
        Ok(
            json!({"common_config":fingerprint(&self.config())?,"worktree_config":fingerprint(&self.work_config())?,"state":fingerprint(&self.state_path())?,"preparation":fingerprint(&self.prep_path())?}),
        )
    }
}
pub fn config_boolean(repo: &Repo, file: &Path, key: &str) -> Result<Option<bool>> {
    if values(repo, file, key)?.is_empty() {
        return Ok(None);
    }
    let out = git_at(
        &repo.root,
        &[
            "config".as_ref(),
            "--file".as_ref(),
            file.as_os_str(),
            "--no-includes".as_ref(),
            "--bool".as_ref(),
            "--get".as_ref(),
            key.as_ref(),
        ],
    )?;
    match (out.status.code(), out.stdout.as_slice()) {
        (Some(0), b"true\n") => Ok(Some(true)),
        (Some(0), b"false\n") => Ok(Some(false)),
        (Some(1), _) => Ok(None),
        _ => Err(Failure::new(
            "git.config_failed",
            "Cannot read a managed Git boolean",
        )),
    }
}
pub fn values(repo: &Repo, file: &Path, key: &str) -> Result<Vec<OsString>> {
    config_values(repo, file, key, false)
}
pub fn included_values(repo: &Repo, file: &Path, key: &str) -> Result<Vec<OsString>> {
    config_values(repo, file, key, true)
}
fn config_values(repo: &Repo, file: &Path, key: &str, includes: bool) -> Result<Vec<OsString>> {
    if !file.exists() {
        return Ok(Vec::new());
    }
    if !fs::symlink_metadata(file)?.is_file() {
        return Err(Failure::new(
            "git.config_unsupported",
            "Git configuration must be a regular file",
        ));
    }
    if !includes && let Ok(Some(parsed)) = fast_config_values(file, key) {
        if parsed.len() > 1 {
            return Err(Failure::new(
                "git.config_unsupported",
                "Managed Git keys must have at most one local value",
            ));
        }
        return Ok(parsed);
    }
    let out = git_at(
        &repo.root,
        &[
            "config".as_ref(),
            "--file".as_ref(),
            file.as_os_str(),
            if includes {
                "--includes".as_ref()
            } else {
                "--no-includes".as_ref()
            },
            "--null".as_ref(),
            "--get-all".as_ref(),
            key.as_ref(),
        ],
    )?;
    match out.status.code() {
        Some(1) => Ok(Vec::new()),
        Some(0) => {
            let values: Vec<_> = out
                .stdout
                .strip_suffix(&[0])
                .unwrap_or(&out.stdout)
                .split(|b| *b == 0)
                .map(|b| OsString::from_vec(b.to_vec()))
                .collect();
            if values.len() > 1 {
                return Err(Failure::new(
                    "git.config_unsupported",
                    "Managed Git keys must have at most one local value",
                ));
            }
            Ok(values)
        }
        _ => Err(Failure::new(
            "git.config_failed",
            "Cannot inspect configuration values",
        )),
    }
}
pub fn values_json(list: &[OsString]) -> Value {
    Value::Array(list.iter().map(|s| bytes_value(s.as_bytes())).collect())
}
pub fn inherited_hooks(repo: &Repo) -> Result<Value> {
    let mut scopes = serde_json::Map::new();
    for scope in ["--system", "--global", "--local"] {
        let output = git_at(
            &repo.root,
            &[
                "config".as_ref(),
                scope.as_ref(),
                "--includes".as_ref(),
                "--null".as_ref(),
                "--path".as_ref(),
                "--get-all".as_ref(),
                "core.hooksPath".as_ref(),
            ],
        )?;
        match output.status.code() {
            Some(0) => {
                scopes.insert(
                    scope.to_owned(),
                    Value::Array(
                        output
                            .stdout
                            .strip_suffix(&[0])
                            .unwrap_or(&output.stdout)
                            .split(|b| *b == 0)
                            .map(bytes_value)
                            .collect(),
                    ),
                );
            }
            Some(1) => {
                scopes.insert(scope.to_owned(), json!([]));
            }
            _ => {
                return Err(Failure::new(
                    "git.config_failed",
                    "Cannot inspect inherited hook configuration",
                ));
            }
        }
    }
    Ok(Value::Object(scopes))
}
pub fn decode_values(v: &Value) -> Result<Vec<OsString>> {
    v.as_array()
        .ok_or_else(|| Failure::new("state.invalid", "Expected saved configuration values"))?
        .iter()
        .map(|v| value_bytes(v).map(OsString::from_vec))
        .collect()
}
pub fn replace_values(repo: &Repo, file: &Path, key: &str, list: &[OsString]) -> Result<()> {
    if list.len() > 1 {
        return Err(Failure::new(
            "git.config_unsupported",
            "Multiple managed values require a separately reviewed migration",
        ));
    }
    if values(repo, file, key)?.as_slice() == list {
        return Ok(());
    }
    let mut args = vec![
        OsStr::new("config"),
        OsStr::new("--file"),
        file.as_os_str(),
        OsStr::new(if list.is_empty() {
            "--unset-all"
        } else {
            "--replace-all"
        }),
        OsStr::new(key),
    ];
    if let Some(value) = list.first() {
        args.push(value.as_os_str());
    }
    let out = publication("git.config.publish", || git_at(&repo.root, &args))?;
    if !out.status.success() {
        return Err(Failure::new(
            "git.config_write_failed",
            String::from_utf8_lossy(&out.stderr),
        ));
    }
    publication("git.config.file_sync", || File::open(file)?.sync_all())?;
    let parent = file
        .parent()
        .ok_or_else(|| Failure::new("git.config_failed", "Missing configuration directory"))?;
    publication("git.config.parent_sync", || File::open(parent)?.sync_all())?;
    Ok(())
}
pub struct Locks {
    _common: File,
    _local: Option<File>,
}
impl Drop for Locks {
    fn drop(&mut self) {
        let _ = rustix::fs::flock(&self._common, rustix::fs::FlockOperation::Unlock);
        if let Some(local) = &self._local {
            let _ = rustix::fs::flock(local, rustix::fs::FlockOperation::Unlock);
        }
    }
}
pub fn locks(repo: &Repo, write: bool) -> Result<Locks> {
    // Administrative directory inodes already exist and remain stable across
    // atomic config writes. Locking them needs no bootstrap writes or unlinking.
    fn lock(path: &Path, write: bool) -> Result<File> {
        let file = File::open(path).map_err(|e| Failure::new("lock.unavailable", e))?;
        if !file.metadata()?.is_dir() {
            return Err(Failure::new(
                "lock.invalid",
                "Git administrative lock target must be a directory",
            ));
        }
        let op = if write {
            rustix::fs::FlockOperation::NonBlockingLockExclusive
        } else {
            rustix::fs::FlockOperation::NonBlockingLockShared
        };
        let mut attempts = 0;
        loop {
            match rustix::fs::flock(&file, op) {
                Ok(()) => return Ok(file),
                Err(e)
                    if (e == rustix::io::Errno::WOULDBLOCK || e == rustix::io::Errno::AGAIN)
                        && attempts < 10 =>
                {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) => return Err(Failure::new("lock.contended", e)),
            }
        }
    }
    use std::os::unix::fs::MetadataExt;
    let common = lock(&repo.common, write)?;
    let same_target = repo.git == repo.common
        || (if let (Ok(m1), Ok(m2)) = (repo.common.metadata(), repo.git.metadata()) {
            m1.dev() == m2.dev() && m1.ino() == m2.ino()
        } else {
            false
        });
    let local = if same_target {
        None
    } else {
        Some(lock(&repo.git, write)?)
    };
    if !write {
        check_private_dir(&repo.shared)?;
        check_private_dir(&repo.local)?;
    }
    Ok(Locks {
        _common: common,
        _local: local,
    })
}
pub fn inventory(source: &Path) -> Result<Value> {
    let mut result = serde_json::Map::new();
    if !source.exists() {
        return Ok(Value::Object(result));
    }
    if !fs::metadata(source)?.is_dir() {
        return Err(Failure::new(
            "install.source_invalid",
            "Hook source must be a directory",
        ));
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(s) = name.to_str() else {
            continue;
        };
        if s.ends_with(".sample") || s.starts_with('.') {
            continue;
        }
        let p = entry.path();
        let kind = fingerprint(&p)?;
        if kind["kind"] == "symlink" && !fs::metadata(&p).is_ok_and(|m| m.is_file()) {
            return Err(Failure::new(
                "install.source_invalid",
                "Hook symlinks must resolve to regular files",
            ));
        }
        if s == "commit-msg" {
            result.insert(s.into(), kind);
        } else if executable(&p) {
            result.insert(
                s.into(),
                json!({"kind":if kind["kind"]=="symlink"{"symlink"}else{"file"},"executable":true}),
            );
        }
    }
    Ok(Value::Object(result))
}
