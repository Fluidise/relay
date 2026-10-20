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

#[test]
fn missing_operation_is_a_structured_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["status"], "error");
}

#[test]
fn help_cannot_activate_an_operation() {
    let output = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
        .args(["install", "--help"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
}

#[test]
fn all_mixed_display_spellings_are_rejected() {
    for args in [
        ["-h", "--version"],
        ["--help", "-v"],
        ["--version", "--help"],
        ["-v", "--version"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_fluidise-relay"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
    }
}
