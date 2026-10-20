use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub const CONFIG_LIMIT: u64 = 1_048_576;
pub const OUTPUT_LIMIT: usize = 2_097_152;
pub const CAPTURE_LIMIT: usize = 262_144;
pub type Result<T> = std::result::Result<T, Failure>;

#[derive(Debug)]
pub struct Failure {
    pub diagnostics: Vec<Value>,
    pub complete: bool,
}
impl Failure {
    pub fn new(code: &str, message: impl ToString) -> Self {
        Self {
            diagnostics: vec![
                json!({"code":code,"message":message.to_string().chars().take(512).collect::<String>()}),
            ],
            complete: true,
        }
    }
    pub fn result(&self, operation: &str) -> Value {
        json!({"schema_version":1,"operation":operation,"status":"error","diagnostics":self.diagnostics,"diagnostics_complete":self.complete})
    }
}
impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        Self::new("io.failed", e)
    }
}
pub fn bytes_value(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(s) => json!({"encoding":"utf8","data":s}),
        Err(_) => json!({"encoding":"base64","data":STANDARD.encode(bytes)}),
    }
}
pub fn value_bytes(v: &Value) -> Result<Vec<u8>> {
    let data = v["data"]
        .as_str()
        .ok_or_else(|| Failure::new("state.invalid", "Missing byte data"))?;
    match v["encoding"].as_str() {
        Some("utf8") => Ok(data.as_bytes().to_vec()),
        Some("base64") => STANDARD
            .decode(data)
            .map_err(|_| Failure::new("state.invalid", "Invalid Base64")),
        _ => Err(Failure::new("state.invalid", "Unknown byte encoding")),
    }
}
pub fn path_value(p: &Path) -> Value {
    bytes_value(p.as_os_str().as_bytes())
}
pub fn value_path(v: &Value) -> Result<PathBuf> {
    Ok(PathBuf::from(OsString::from_vec(value_bytes(v)?)))
}

// Typed, length-delimited canonical encoding; JSON serialization order is irrelevant.
pub fn digest(domain: &str, value: &Value) -> String {
    fn field(h: &mut Sha256, bytes: &[u8]) {
        h.update((bytes.len() as u64).to_be_bytes());
        h.update(bytes);
    }
    fn encode(h: &mut Sha256, v: &Value) {
        match v {
            Value::Null => h.update([0]),
            Value::Bool(b) => h.update([1, u8::from(*b)]),
            Value::Number(n) => {
                h.update([2]);
                field(h, n.to_string().as_bytes());
            }
            Value::String(s) => {
                h.update([3]);
                field(h, s.as_bytes());
            }
            Value::Array(a) => {
                h.update([4]);
                h.update((a.len() as u64).to_be_bytes());
                for v in a {
                    encode(h, v);
                }
            }
            Value::Object(o) => {
                h.update([5]);
                h.update((o.len() as u64).to_be_bytes());
                let mut keys: Vec<_> = o.keys().collect();
                keys.sort();
                for k in keys {
                    field(h, k.as_bytes());
                    encode(h, &o[k]);
                }
            }
        }
    }
    let mut h = Sha256::new();
    field(&mut h, domain.as_bytes());
    encode(&mut h, value);
    hex(&h.finalize())
}
pub fn file_digest(path: &Path) -> Result<String> {
    let mut file = regular_file(path)?;
    let mut h = Sha256::new();
    let mut buffer = [0u8; 16384];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        h.update(&buffer[..n]);
    }
    Ok(hex(&h.finalize()))
}
pub fn regular_file(path: &Path) -> Result<File> {
    // O_NONBLOCK prevents FIFO opens from hanging before the descriptor type check.
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|e| Failure::new("input.open_failed", e))?;
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(Failure::new("input.unsupported", "Expected a regular file"));
    }
    Ok(file)
}
pub fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    regular_file(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(Failure::new(
            "input.limit_exceeded",
            "Input exceeds its byte limit",
        ));
    }
    Ok(bytes)
}
pub fn fingerprint(path: &Path) -> Result<Value> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(e.into()),
        Ok(m) if m.file_type().is_symlink() => {
            Ok(json!({"kind":"symlink","target":path_value(&fs::read_link(path)?)}))
        }
        Ok(m) if m.is_file() => Ok(
            json!({"kind":"file","mode":m.permissions().mode() & 0o777,"digest":file_digest(path)?}),
        ),
        Ok(_) => Err(Failure::new(
            "input.unsupported",
            "Special files are not supported",
        )),
    }
}
pub fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file())
        && rustix::fs::accessat(
            rustix::fs::CWD,
            path,
            rustix::fs::Access::EXEC_OK,
            rustix::fs::AtFlags::EACCESS,
        )
        .is_ok()
}
pub fn same_directory(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a)
        .ok()
        .zip(fs::canonicalize(b).ok())
        .is_some_and(|(a, b)| a == b)
}
pub fn resolve_program(program: &str, config_dir: &Path, work_dir: &Path) -> Result<PathBuf> {
    let found = if program.contains('/') {
        let path = PathBuf::from(program);
        if path.is_absolute() {
            path
        } else {
            config_dir.join(path)
        }
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| {
                if dir.is_absolute() {
                    dir.join(program)
                } else {
                    work_dir.join(dir).join(program)
                }
            })
            .find(|p| executable(p))
            .ok_or_else(|| {
                Failure::new(
                    "command.unavailable",
                    format!("Program is unavailable: {program}"),
                )
            })?
    };
    if !executable(&found) {
        return Err(Failure::new(
            "command.unavailable",
            format!("Program is not executable: {program}"),
        ));
    }
    Ok(found)
}
pub fn publication<T, E>(
    label: &'static str,
    action: impl FnOnce() -> std::result::Result<T, E>,
) -> Result<T>
where
    Failure: From<E>,
{
    #[cfg(test)]
    persistence_fault::boundary(label, "before")?;
    let value = action().map_err(Failure::from)?;
    #[cfg(test)]
    persistence_fault::boundary(label, "after")?;
    #[cfg(not(test))]
    let _ = label;
    Ok(value)
}
pub fn remove_owned_file(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    publication("file.remove", || fs::remove_file(path))?;
    publication("file.remove.parent_sync", || {
        File::open(path.parent().expect("owned parent"))?.sync_all()
    })
}
pub fn remove_owned_dir(path: &Path) -> Result<()> {
    publication("directory.remove", || fs::remove_dir(path))?;
    publication("directory.remove.parent_sync", || {
        File::open(path.parent().expect("owned parent"))?.sync_all()
    })
}
#[cfg(test)]
pub mod persistence_fault {
    use super::*;
    #[derive(Default)]
    struct Fault {
        target: Option<usize>,
        trace: Vec<(&'static str, &'static str)>,
    }
    thread_local! { static FAULT: std::cell::RefCell<Fault> = std::cell::RefCell::new(Fault::default()); }
    pub fn arm(target: Option<usize>) {
        FAULT.with(|f| {
            *f.borrow_mut() = Fault {
                target,
                trace: Vec::new(),
            }
        });
    }
    pub fn trace() -> Vec<(&'static str, &'static str)> {
        FAULT.with(|f| f.borrow().trace.clone())
    }
    pub fn boundary(label: &'static str, side: &'static str) -> Result<()> {
        FAULT.with(|f| {
            let mut f = f.borrow_mut();
            f.trace.push((label, side));
            if f.target == Some(f.trace.len() - 1) {
                f.target = None;
                Err(Failure::new(
                    "test.persistence_failure",
                    format!("{side} {label}"),
                ))
            } else {
                Ok(())
            }
        })
    }
}
pub fn private_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        publication("directory.create", || {
            fs::DirBuilder::new().mode(0o700).create(path)
        })?;
        publication("directory.permissions", || {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        })?;
    }
    check_private_dir(path)
}
pub fn check_private_dir(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() || m.permissions().mode() & 0o077 != 0 {
        return Err(Failure::new(
            "state.permissions",
            "Relay state directory must be private and not a symlink",
        ));
    }
    Ok(())
}
pub fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Failure::new("state.invalid", "Missing parent directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| Failure::new("state.invalid", "Missing file name"))?;
    let mut temp_name = OsString::from(".relay-stage-");
    temp_name.push(name);
    let temp = parent.join(temp_name);
    if let Ok(m) = fs::symlink_metadata(&temp)
        && (!m.is_file() || m.permissions().mode() & 0o077 != 0)
    {
        return Err(Failure::new("state.stage_conflict", "Unsafe staging file"));
    }

    let mut file = publication("atomic.open", || {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .mode(mode)
            .open(&temp)
    })?;
    publication("atomic.permissions", || {
        file.set_permissions(fs::Permissions::from_mode(mode))
    })?;
    publication("atomic.write", || file.write_all(bytes))?;
    publication("atomic.file_sync", || file.sync_all())?;
    publication("atomic.rename", || fs::rename(&temp, path))?;
    publication("atomic.parent_sync", || File::open(parent)?.sync_all())?;
    Ok(())
}

pub fn read_json(path: &Path) -> Result<Option<Value>> {
    let m = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !m.is_file() || m.permissions().mode() & 0o077 != 0 {
        return Err(Failure::new(
            "state.permissions",
            "State must be a private regular file",
        ));
    }
    let v: Value = serde_json::from_slice(&read_bounded(path, OUTPUT_LIMIT as u64)?)
        .map_err(|e| Failure::new("state.invalid", e))?;
    if v["version"] != 1 {
        return Err(Failure::new(
            "state.version_unsupported",
            "Unsupported state version",
        ));
    }
    Ok(Some(v))
}
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Failure::new("state.invalid", e))?;
    if bytes.len() > OUTPUT_LIMIT {
        return Err(Failure::new("state.limit_exceeded", "State exceeds limit"));
    }
    atomic_write(path, &bytes, 0o600)
}
pub fn shell_quote(s: &OsStr) -> Vec<u8> {
    let mut out = vec![b'\''];
    for b in s.as_bytes() {
        if *b == b'\'' {
            out.extend_from_slice(b"'\\''");
        } else {
            out.push(*b);
        }
    }
    out.push(b'\'');
    out
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_bytes_round_trip_without_loss() {
        let path = PathBuf::from(OsString::from_vec(b"/native/\xff\n' path".to_vec()));
        let value = path_value(&path);
        assert_eq!(value["encoding"], "base64");
        assert_eq!(value_path(&value).unwrap(), path);
    }
    #[test]
    fn canonical_digest_is_domain_separated_and_length_delimited() {
        assert_ne!(
            digest("plan", &json!(["ab", "c"])),
            digest("plan", &json!(["a", "bc"]))
        );
        assert_ne!(digest("plan", &json!(1)), digest("approval", &json!(1)));
        assert_ne!(digest("plan", &json!(1)), digest("plan", &json!("1")));
        assert_eq!(
            digest("plan", &json!({"b":2,"a":1})),
            digest("plan", &json!({"a":1,"b":2}))
        );
        assert_ne!(
            digest("plan", &json!([1, 2])),
            digest("plan", &json!([2, 1]))
        );
    }
}
