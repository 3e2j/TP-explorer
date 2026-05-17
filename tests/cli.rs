// Verifies the binary prints usage when launched without a command.
#[test]
fn binary_prints_usage_without_arguments() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tpmt"))
        .output()
        .expect("run binary");
    assert!(output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}

// Verifies unknown commands still fall back to the general usage text.
#[test]
fn binary_prints_usage_for_unknown_command() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_tpmt"))
        .arg("unknown")
        .output()
        .expect("run binary");
    assert!(output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Usage:"));
}
