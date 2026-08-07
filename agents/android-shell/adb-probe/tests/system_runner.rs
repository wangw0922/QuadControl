#![cfg(unix)]

mod common;

use std::time::{Duration, Instant};

use adb_probe::{run_probe, AdbCommand, CommandRunner, ExecutionError, SystemRunner, Transport};
use common::{successful_adb, FakeAdb, RetryTextFileBusy};

#[test]
fn system_runner_executes_only_the_expected_parameterized_commands() {
    let fake = successful_adb();
    let runner = RetryTextFileBusy(SystemRunner::new(fake.executable()));
    let report = run_probe(&runner).unwrap();

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
    let runner = RetryTextFileBusy(SystemRunner::with_limits(
        fake.executable(),
        Duration::from_millis(20),
        1024,
    ));
    let started = Instant::now();

    let result = runner.run(AdbCommand::Version);

    assert_eq!(result, Err(ExecutionError::TimedOut(AdbCommand::Version)));
    // The bound is loose because the first execution of a freshly written script is
    // security-scanned on macOS and that cost lands inside this measurement. It still
    // fails a genuine hang, which is unbounded — the original two-second bound instead
    // failed intermittently on a developer machine while Linux CI stayed green. This
    // script never exits on its own, so it cannot be warmed up first.
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "timeout path did not return promptly"
    );
}

/// These two tests are about the output bound, not about timing, so the timeout is
/// deliberately generous. macOS security-scans a freshly written executable on its
/// first run — measured here at 0.5 s idle and up to 3 s under load, against 0.03 s
/// for every later run. A one-second timeout turned that into a spurious
/// `TimedOut`. Linux CI never saw it, so the failure only ever reproduced on a
/// developer's Mac. Timeout behaviour itself is covered by
/// `system_runner_kills_and_reaps_a_timed_out_process`, which uses a script that
/// genuinely never exits.
const OUTPUT_LIMIT_TEST_TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn system_runner_rejects_stdout_over_the_limit() {
    let fake = FakeAdb::new("printf '12345'");
    let runner = RetryTextFileBusy(SystemRunner::with_limits(
        fake.executable(),
        OUTPUT_LIMIT_TEST_TIMEOUT,
        4,
    ));

    assert_eq!(
        runner.run(AdbCommand::Version),
        Err(ExecutionError::OutputTooLarge(AdbCommand::Version))
    );
}

#[test]
fn system_runner_rejects_stderr_over_the_limit() {
    let fake = FakeAdb::new("printf '12345' >&2");
    let runner = RetryTextFileBusy(SystemRunner::with_limits(
        fake.executable(),
        OUTPUT_LIMIT_TEST_TIMEOUT,
        4,
    ));

    assert_eq!(
        runner.run(AdbCommand::Version),
        Err(ExecutionError::OutputTooLarge(AdbCommand::Version))
    );
}
