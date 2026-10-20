use crate::{
    Options,
    config::Config,
    repository::{Repo, inventory, locks, values},
    support::*,
};
use serde_json::{Value, json};
use std::ffi::CString;
use std::fs;
use std::io::{ErrorKind, Read};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
    incomplete: bool,
}
impl Capture {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            truncated: false,
            incomplete: false,
        }
    }
    fn value(&self) -> Value {
        let mut v = bytes_value(&self.bytes);
        v["truncated"] = json!(self.truncated);
        v["incomplete"] = json!(self.incomplete);
        v
    }
}
fn drain<R: Read>(pipe: &mut R, capture: &mut Capture, remaining: &mut usize) -> Result<bool> {
    let mut buffer = [0u8; 8192];
    // Bounded work per stream per iteration preserves fairness and deadline checks.
    for _ in 0..8 {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                let keep = n.min(*remaining);
                capture.bytes.extend_from_slice(&buffer[..keep]);
                *remaining -= keep;
                if keep < n {
                    capture.truncated = true;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(Failure::new("command.capture_failed", e)),
        }
    }
    Ok(false)
}
fn nonblocking(fd: &impl std::os::fd::AsFd) -> Result<()> {
    let flags =
        rustix::fs::fcntl_getfl(fd).map_err(|e| Failure::new("command.capture_failed", e))?;
    rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)
        .map_err(|e| Failure::new("command.capture_failed", e))
}
struct ManagedChild {
    pid: rustix::process::Pid,
    stdout: Option<fs::File>,
    stderr: Option<fs::File>,
}
impl ManagedChild {
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        rustix::process::waitpid(Some(self.pid), rustix::process::WaitOptions::NOHANG)
            .map(|s| s.map(|(_, status)| ExitStatus::from_raw(status.as_raw())))
            .map_err(std::io::Error::from)
    }
}
fn spawn_managed(command: &Command) -> std::io::Result<ManagedChild> {
    use nix::spawn::{PosixSpawnAttr, PosixSpawnFileActions, PosixSpawnFlags, posix_spawn};
    use nix::sys::signal::{SigSet, Signal};
    fn pair() -> std::io::Result<(fs::File, fs::File)> {
        let (read, write) = std::io::pipe()?;
        // Keep action sources above the standard descriptor range, even when
        // the caller has closed stdin/stdout/stderr.
        let read = fs::File::from(rustix::io::fcntl_dupfd_cloexec(&read, 3)?);
        let write = fs::File::from(rustix::io::fcntl_dupfd_cloexec(&write, 3)?);
        Ok((read, write))
    }
    fn cstring(bytes: &[u8]) -> std::io::Result<CString> {
        CString::new(bytes).map_err(|e| std::io::Error::new(ErrorKind::InvalidInput, e))
    }
    let (stdout, out_write) = pair()?;
    let (stderr, err_write) = pair()?;
    let program = cstring(command.get_program().as_bytes())?;
    let mut args = vec![program.clone()];
    for arg in command.get_args() {
        args.push(cstring(arg.as_bytes())?);
    }
    let mut env = Vec::new();
    for (key, value) in std::env::vars_os() {
        let mut entry = key.into_encoded_bytes();
        entry.push(b'=');
        entry.extend(value.as_bytes());
        env.push(cstring(&entry)?);
    }
    let mut actions = PosixSpawnFileActions::init()?;
    let input = fs::File::open("/dev/null")?;
    let input = fs::File::from(rustix::io::fcntl_dupfd_cloexec(&input, 3)?);
    actions.add_dup2(input.as_raw_fd(), 0)?;
    actions.add_dup2(out_write.as_raw_fd(), 1)?;
    actions.add_dup2(err_write.as_raw_fd(), 2)?;
    let mut attributes = PosixSpawnAttr::init()?;
    attributes.set_pgroup(nix::unistd::Pid::from_raw(0))?;
    let mut defaults = SigSet::empty();
    defaults.add(Signal::SIGPIPE);
    attributes.set_sigdefault(&defaults)?;
    attributes.set_flags(
        PosixSpawnFlags::POSIX_SPAWN_SETPGROUP | PosixSpawnFlags::POSIX_SPAWN_SETSIGDEF,
    )?;
    // This CLI has one execution thread and one operation per process. Resolve
    // invocation-relative inputs before entering the chain's required cwd.
    if let Some(dir) = command.get_current_dir() {
        std::env::set_current_dir(dir)?;
    }
    let pid = posix_spawn(program.as_c_str(), &actions, &attributes, &args, &env)?;
    let pid = rustix::process::Pid::from_raw(pid.as_raw()).expect("spawned positive pid");
    Ok(ManagedChild {
        pid,
        stdout: Some(stdout),
        stderr: Some(stderr),
    })
}
fn signal_group(child: &ManagedChild, signal: rustix::process::Signal) {
    let _ = rustix::process::kill_process_group(child.pid, signal);
}
struct Signals {
    value: Arc<AtomicUsize>,
    ids: Vec<signal_hook::SigId>,
}
impl Signals {
    fn new() -> Result<Self> {
        let value = Arc::new(AtomicUsize::new(0));
        let mut out = Self {
            value,
            ids: Vec::new(),
        };
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            out.ids.push(signal_hook::flag::register_usize(
                signal,
                Arc::clone(&out.value),
                signal as usize,
            )?);
        }
        Ok(out)
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        for id in self.ids.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

struct Outcome {
    record: Value,
    code: u8,
    diagnostic: Option<Value>,
}
fn execute(
    command: &mut Command,
    id: &str,
    deadline: Instant,
    remaining: &mut usize,
    signals: &Signals,
) -> Outcome {
    if signals.value.load(Ordering::Relaxed) != 0 || Instant::now() >= deadline {
        return Outcome {
            record: json!({"id":id,"outcome":"error"}),
            code: 2,
            diagnostic: Some(
                json!({"code":"execution.cancelled_before_spawn","message":"Execution was cancelled or its deadline expired before spawn","command_id":id}),
            ),
        };
    }
    let mut child = match spawn_managed(command) {
        Ok(c) => c,
        Err(e) => {
            return Outcome {
                record: json!({"id":id,"outcome":"error"}),
                code: 2,
                diagnostic: Some(
                    json!({"code":"command.spawn_failed","message":e.to_string(),"command_id":id}),
                ),
            };
        }
    };
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out = Capture::new();
    let mut err = Capture::new();
    let mut problem = None;
    let mut cleanup = None;
    let mut killed = false;
    let mut status = None;
    if let Err(e) = nonblocking(stdout.as_ref().expect("piped stdout"))
        .and_then(|_| nonblocking(stderr.as_ref().expect("piped stderr")))
    {
        problem = Some(e.diagnostics[0].clone());
    }
    loop {
        let interrupted = signals.value.load(Ordering::Relaxed);
        if cleanup.is_none()
            && (problem.is_some() || interrupted != 0 || Instant::now() >= deadline)
        {
            if problem.is_none() {
                problem = Some(
                    json!({"code":if interrupted!=0{"execution.interrupted"}else{"execution.timeout"},"message":if interrupted!=0{"Relay was interrupted"}else{"Command-chain deadline expired"},"command_id":id}),
                );
            }
            signal_group(&child, rustix::process::Signal::TERM);
            cleanup = Some(Instant::now());
        }
        if let Some(start) = cleanup {
            if start.elapsed() >= Duration::from_secs(1) && !killed {
                signal_group(&child, rustix::process::Signal::KILL);
                killed = true;
            }
            if start.elapsed() >= Duration::from_secs(2) {
                out.incomplete = stdout.is_some();
                err.incomplete = stderr.is_some();
                break;
            }
        }
        if stdout.is_some() || stderr.is_some() {
            let mut polls = Vec::new();
            if let Some(fd) = &stdout {
                polls.push(rustix::event::PollFd::new(fd, rustix::event::PollFlags::IN));
            }
            if let Some(fd) = &stderr {
                polls.push(rustix::event::PollFd::new(fd, rustix::event::PollFlags::IN));
            }
            let _ = rustix::event::poll(
                &mut polls,
                Some(&rustix::event::Timespec {
                    tv_sec: 0,
                    tv_nsec: 10_000_000,
                }),
            );
            drop(polls);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some(fd) = &mut stdout {
            match drain(fd, &mut out, remaining) {
                Ok(true) => stdout = None,
                Ok(false) => {}
                Err(e) => {
                    out.incomplete = true;
                    stdout = None;
                    problem.get_or_insert_with(|| e.diagnostics[0].clone());
                }
            }
        }
        if let Some(fd) = &mut stderr {
            match drain(fd, &mut err, remaining) {
                Ok(true) => stderr = None,
                Ok(false) => {}
                Err(e) => {
                    err.incomplete = true;
                    stderr = None;
                    problem.get_or_insert_with(|| e.diagnostics[0].clone());
                }
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(s) => status = s,
                Err(e) => {
                    problem.get_or_insert(json!({"code":"command.wait_failed","message":e.to_string(),"command_id":id}));
                }
            }
        }
        if status.is_some() && stdout.is_none() && stderr.is_none() && (cleanup.is_none() || killed)
        {
            // EOF and direct-child exit do not prove the process group is empty.
            // Once cleanup starts, always complete the forced group attempt.
            break;
        }
    }
    // Never perform an unbounded wait/read-to-end, including after direct-child exit.
    if status.is_none() {
        status = child.try_wait().ok().flatten();
    }
    if status.is_none() {
        problem.get_or_insert(json!({"code":"execution.cleanup_incomplete","message":"Termination could not be confirmed","command_id":id}));
    }
    let mut record = json!({"id":id,"stdout":out.value(),"stderr":err.value()});
    if let Some(s) = status {
        if let Some(code) = s.code() {
            record["exit_code"] = json!(code);
        }
        if let Some(signal) = s.signal() {
            record["signal"] = json!(signal);
        }
    }
    let code = if problem.is_some() {
        2
    } else if status.is_some_and(|s| s.success()) {
        0
    } else {
        1
    };
    record["outcome"] = json!(if code == 0 {
        "success"
    } else if code == 1 {
        "failed"
    } else {
        "error"
    });
    if code == 1 {
        problem = Some(
            json!({"code":if record["signal"].is_number(){"command.signalled"}else{"command.failed"},"message":"External command failed","command_id":id}),
        );
    }
    Outcome {
        record,
        code,
        diagnostic: problem,
    }
}
pub fn source_config(
    repo: &Repo,
    options: &Options,
    state: Option<&Value>,
) -> Result<std::path::PathBuf> {
    let path = if let Some(p) = &options.config {
        if p.is_absolute() {
            p.clone()
        } else {
            std::env::current_dir()?.join(p)
        }
    } else if let Some(s) = state {
        if s["phase"] != "inactive" {
            value_path(&s["config"])?
        } else {
            repo.root.join("fluidise.toml")
        }
    } else {
        repo.root.join("fluidise.toml")
    };
    let parent = fs::canonicalize(
        path.parent()
            .ok_or_else(|| Failure::new("config.path_invalid", "Missing configuration parent"))?,
    )?;
    Ok(parent.join(
        path.file_name().ok_or_else(|| {
            Failure::new("config.path_invalid", "Missing configuration file name")
        })?,
    ))
}
pub fn verify_owned(repo: &Repo, state: &Value) -> Result<()> {
    if state["identity"] != repo.identity() {
        return Err(Failure::new(
            "state.identity_mismatch",
            "Installation belongs to another worktree location",
        ));
    }
    let files = state["files"]
        .as_object()
        .ok_or_else(|| Failure::new("state.invalid", "Missing owned hook manifest"))?;
    let hooks = repo.local.join("hooks");
    for (name, expected) in files {
        if fingerprint(&hooks.join(name))? != *expected {
            return Err(Failure::new(
                "state.hook_modified",
                "An installed hook was modified; inspect before changing it",
            ));
        }
    }
    if hooks.exists() {
        for f in fs::read_dir(&hooks)? {
            let n = f?.file_name();
            if n.to_str().is_none_or(|s| !files.contains_key(s)) {
                return Err(Failure::new(
                    "state.hook_modified",
                    "Unexpected file in owned hooks directory",
                ));
            }
        }
    }
    if values(repo, &repo.work_config(), "core.hooksPath")? != vec![hooks.into_os_string()] {
        return Err(Failure::new(
            "state.activation_modified",
            "Worktree activation no longer matches Relay state",
        ));
    }
    let source = value_path(&state["source"])?;
    if inventory(&source)? != state["inventory"] {
        return Err(Failure::new(
            "state.source_changed",
            "Source hook inventory changed; inspect an installation update",
        ));
    }
    Ok(())
}
fn message_state(path: &std::path::Path, deadline: Instant, signals: &Signals) -> Result<Value> {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::MetadataExt;
    let mut file = regular_file(path)?;
    let link = fs::symlink_metadata(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 16384];
    loop {
        if signals.value.load(Ordering::Relaxed) != 0 {
            return Err(Failure::new(
                "execution.interrupted",
                "Interrupted while checking message state",
            ));
        }
        if Instant::now() >= deadline {
            return Err(Failure::new(
                "execution.timeout",
                "Command-chain deadline expired while checking message state",
            ));
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    let m = file.metadata()?;
    Ok(
        json!({"digest":hex(&hash.finalize()),"inode":m.ino(),"device":m.dev(),"size":m.len(),"mode":m.mode(),"mtime":m.mtime(),"mtime_nanos":m.mtime_nsec(),"link_inode":link.ino(),"link_target":if link.file_type().is_symlink(){path_value(&fs::read_link(path)?)}else{Value::Null}}),
    )
}
pub fn run(repo: &Repo, options: &Options) -> Result<(Value, u8)> {
    let _locks = locks(repo, false)?;
    let state = repo
        .state()?
        .ok_or_else(|| Failure::new("state.missing", "Relay is not installed"))?;
    if read_json(&repo.prep_path())?
        .is_some_and(|p| p["phase"] != "prepared" && p["phase"] != "inactive")
    {
        return Err(Failure::new(
            "state.common_inconsistent",
            "Common preparation is incomplete",
        ));
    }
    if state["phase"] != "active" {
        return Err(Failure::new(
            "state.inconsistent",
            "Installation is not active; inspect recovery",
        ));
    }
    verify_owned(repo, &state)?;
    let config = Config::load(&source_config(repo, options, Some(&state))?)?;
    if config.approval() != state["approval"] {
        return Err(Failure::new(
            "approval.required",
            "Effective execution configuration changed; inspect approve",
        ));
    }
    let file = options.file.as_ref().expect("validated --file");
    let file = if file.is_absolute() {
        file.clone()
    } else {
        std::env::current_dir()?.join(file)
    };
    let _ = regular_file(&file)?;
    let mut commands = Vec::new();
    let mut diagnostics = Vec::new();
    let mut remaining = CAPTURE_LIMIT;
    let mut code = 0;
    let deadline = Instant::now()
        .checked_add(config.timeout)
        .ok_or_else(|| Failure::new("config.invalid", "Timeout cannot be represented"))?;
    let signals = Signals::new()?;
    for spec in &config.commands {
        if Instant::now() >= deadline || signals.value.load(Ordering::Relaxed) != 0 {
            code = 2;
            diagnostics.push(json!({"code":"execution.timeout","message":"Execution budget expired before next command"}));
            break;
        }
        let before = if spec.modifies {
            None
        } else {
            match message_state(&file, deadline, &signals) {
                Ok(state) => Some(state),
                Err(mut error) => {
                    error.diagnostics[0]["command_id"] = json!(spec.id);
                    diagnostics.extend(error.diagnostics);
                    code = 2;
                    break;
                }
            }
        };
        let program = match resolve_program(
            &spec.program,
            config.source.parent().expect("config parent"),
            &repo.root,
        ) {
            Ok(p) => p,
            Err(mut e) => {
                e.diagnostics[0]["command_id"] = json!(spec.id);
                diagnostics.extend(e.diagnostics);
                code = 2;
                break;
            }
        };
        let mut cmd = Command::new(program);
        cmd.current_dir(&repo.root);
        for arg in &spec.args {
            if arg == "{message-file}" {
                cmd.arg(&file);
            } else {
                cmd.arg(arg);
            }
        }
        let outcome = execute(&mut cmd, &spec.id, deadline, &mut remaining, &signals);
        code = outcome.code;
        commands.push(outcome.record);
        if let Some(d) = outcome.diagnostic {
            diagnostics.push(d);
        }
        if let Some(before) = before {
            match message_state(&file, deadline, &signals) {
                Ok(after) if after == before => {}
                Err(error)
                    if error.diagnostics[0]["code"]
                        .as_str()
                        .is_some_and(|c| c.starts_with("execution.")) =>
                {
                    code = 2;
                    diagnostics.extend(error.diagnostics);
                }
                _ => {
                    code = 2;
                    diagnostics.push(json!({"code":"message.modified_unexpectedly","message":"Check-only command changed the message; file retained","command_id":spec.id}));
                }
            }
        }
        if code != 0 {
            break;
        }
    }
    let signal = signals.value.load(Ordering::Relaxed);
    drop(signals);
    let mut result = json!({"schema_version":1,"operation":"run","status":if code==0{"success"}else if code==1{"failed"}else{"error"},"diagnostics":diagnostics,"diagnostics_complete":true,"commands":commands});
    if signal != 0 {
        result["interrupted_signal"] = json!(signal);
    }
    Ok((result, code))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_hash_obeys_the_execution_deadline() {
        let path = std::env::temp_dir().join(format!("relay-hash-{}", std::process::id()));
        fs::write(&path, b"message").unwrap();
        let signals = Signals {
            value: Arc::new(AtomicUsize::new(0)),
            ids: Vec::new(),
        };
        let result = message_state(&path, Instant::now() - Duration::from_secs(1), &signals);
        fs::remove_file(path).unwrap();
        assert_eq!(
            result.unwrap_err().diagnostics[0]["code"],
            "execution.timeout"
        );
    }
}
