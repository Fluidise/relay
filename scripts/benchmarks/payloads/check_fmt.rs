use std::env;
use std::fs;

// Format check: verifies conventional commit prefix (feat, fix, docs, chore, etc.)
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
    let prefixes = [
        "feat", "fix", "docs", "chore", "refactor", "test", "ci", "perf", "style", "build",
    ];
    let mut valid = false;
    for prefix in prefixes {
        if first_line.starts_with(prefix) {
            let rest = &first_line[prefix.len()..];
            if rest.starts_with(':') || rest.starts_with('(') {
                valid = true;
                break;
            }
        }
    }
    if !valid {
        eprintln!("Invalid commit format");
        std::process::exit(1);
    }
}
