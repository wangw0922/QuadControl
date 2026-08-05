#![cfg(unix)]

mod common;

use std::process::Command;

use common::{successful_adb, FakeAdb};

const CLI: &str = env!("CARGO_BIN_EXE_quadcontrol-adb-probe");

#[test]
fn cli_returns_zero_and_json_for_a_successful_probe() {
    let fake = successful_adb();
    let output = Command::new(CLI)
        .env("PATH", fake.directory())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"adb_version\":\"1.0.41\""));
    assert!(stdout.contains("\"wireless_connected\":true"));
}

#[test]
fn cli_returns_one_and_stderr_when_adb_fails() {
    let fake = FakeAdb::new("printf 'version failed' >&2; exit 7");
    let output = Command::new(CLI)
        .env("PATH", fake.directory())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("version failed"));
}

#[test]
fn cli_returns_one_when_adb_is_missing() {
    let empty_path = FakeAdb::new("");
    std::fs::remove_file(empty_path.executable()).unwrap();
    let output = Command::new(CLI)
        .env("PATH", empty_path.directory())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("adb executable was not found"));
}

#[test]
fn cli_returns_usage_code_for_invalid_arguments() {
    let output = Command::new(CLI).arg("--unexpected").output().unwrap();

    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("usage: quadcontrol-adb-probe"));
}
