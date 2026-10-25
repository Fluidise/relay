use std::env;
use std::fs;

// Trailer / non-empty check: validates commit content is non-empty.
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
        eprintln!("Commit message cannot be empty");
        std::process::exit(1);
    }
}
