use std::process::Command;

#[test]
fn test_version_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .arg("--version")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("fluidise-relay"));
}

#[test]
fn test_help_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .arg("--help")
        .output()
        .expect("failed to execute process");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Standalone agent-first Git hook manager"));
}
