//! CLI flag tests for the `uteke-serve` binary (#1044).
//!
//! Uses Cargo's `CARGO_BIN_EXE_uteke-serve` env var, which points at the
//! built binary regardless of profile — no path assumptions.

use std::process::Command;

#[test]
fn version_flag_prints_version_and_exits_zero() {
    let out = Command::new(env!("CARGO_BIN_EXE_uteke-serve"))
        .arg("--version")
        .output()
        .expect("failed to spawn uteke-serve");
    assert!(out.status.success(), "--version must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.trim(),
        format!("uteke-serve {}", env!("CARGO_PKG_VERSION")),
        "--version output must be `uteke-serve <semver>`"
    );
}

#[test]
fn short_version_flag_matches_long() {
    let out = Command::new(env!("CARGO_BIN_EXE_uteke-serve"))
        .arg("-V")
        .output()
        .expect("failed to spawn uteke-serve");
    assert!(out.status.success(), "-V must exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        stdout.trim(),
        format!("uteke-serve {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unknown_flag_still_rejected_with_usage_hint() {
    let out = Command::new(env!("CARGO_BIN_EXE_uteke-serve"))
        .arg("--definitely-not-a-flag")
        .output()
        .expect("failed to spawn uteke-serve");
    assert!(!out.status.success(), "unknown flag must exit non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Unknown argument") && stderr.contains("--help"),
        "stderr should point at --help, got: {stderr}"
    );
}
