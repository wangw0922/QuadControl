#![cfg(unix)]

mod common;

use std::time::{Duration, Instant};

use adb_probe::{
    run_probe, AdbCommand, CommandRunner, ExecutionError, SystemRunner, Transport,
};
use common::{successful_adb, FakeAdb};

#[test]
fn system_runner_executes_only_the_expected_parameterized_commands() {
    let fake = successful_adb();
    let report = run_probe(&SystemRunner::new(fake.executable())).unwrap();

    assert_eq!(report.adb_version, "1.0.41");
    assert_eq!(report.devices[0].transport, Transport::Tcp);
    assert!(report.wireless_connected);
    assert!(report.mdns.connect_service_advertised);
}

#[test]
fn system_runner_reports_a_missing_executable() {
    let fake = successful_adb();
    let missing = fake.directory().join("does-not-exist");
    let result = SystemRunner::new(missing).run(AdbCommand::Version);

    assert_eq!(result, Err(ExecutionError::AdbNotFound));
}

#[test]
fn system_runner_kills_and_reaps_a_timed_out_process() {
    let fake = FakeAdb::new("while :; do :; done");
    let runner = SystemRunner::with_limits(fake.executable(), Duration::from_millis(20), 1024);
    let started = Instant::now();

    let result = runner.run(AdbCommand::Version);

    assert_eq!(result, Err(ExecutionError::TimedOut(AdbCommand::Version)));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "timeout path did not return promptly"
    );
}

#[test]
fn system_runner_rejects_stdout_over_the_limit() {
    let fake = FakeAdb::new("printf '12345'");
    let runner = SystemRunner::with_limits(fake.executable(), Duration::from_secs(1), 4);

    assert_eq!(
        runner.run(AdbCommand::Version),
        Err(ExecutionError::OutputTooLarge(AdbCommand::Version))
    );
}

#[test]
fn system_runner_rejects_stderr_over_the_limit() {
    let fake = FakeAdb::new("printf '12345' >&2");
    let runner = SystemRunner::with_limits(fake.executable(), Duration::from_secs(1), 4);

    assert_eq!(
        runner.run(AdbCommand::Version),
        Err(ExecutionError::OutputTooLarge(AdbCommand::Version))
    );
}
