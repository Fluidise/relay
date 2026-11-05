use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;

fn main() -> io::Result<()> {
    let mut out = io::stdout().lock();
    for (key, value) in std::env::vars_os() {
        out.write_all(key.as_bytes())?;
        out.write_all(b"=")?;
        out.write_all(value.as_bytes())?;
        out.write_all(b"\0")?;
    }
    out.flush()
}
