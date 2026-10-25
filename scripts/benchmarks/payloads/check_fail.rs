// Simulated failure: prints error diagnostic to stderr and exits with code 1.
fn main() {
    eprintln!("simulated check failure");
    std::process::exit(1);
}
