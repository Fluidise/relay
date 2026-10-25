use std::env;
use std::fs;

// Length check: ensures subject line does not exceed 72 characters.
fn main() {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => std::process::exit(1),
    };
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => std::process::exit(1),
    };
    let first_line = content.lines().next().unwrap_or("");
    if first_line.len() > 72 {
        eprintln!("Commit subject too long (max 72 chars)");
        std::process::exit(1);
    }
}
