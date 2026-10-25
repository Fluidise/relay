use std::env;
use std::fs;

// Micro-check: reads the commit message file, ensures it is non-empty, and exits 0.
fn main() {
    let path = match env::args().nth(1) {
        Some(p) => p,
        None => std::process::exit(1),
    };
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => std::process::exit(1),
    };
    if content.trim().is_empty() {
        std::process::exit(1);
    }
}
