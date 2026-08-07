#![cfg(unix)]
// This module is compiled separately into each integration-test binary, and neither
// uses all of it, so unused-item warnings here are an artifact of that duplication
// rather than real dead code.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use adb_probe::{AdbCommand, CommandOutput, CommandRunner, ExecutionError};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Wraps a runner and retries only `ETXTBSY`.
///
/// Tests execute in parallel. While one thread still holds a write handle to a
/// freshly created script, another thread's `fork` inherits that descriptor, and
/// Linux refuses to `exec` a file that is open for writing — `Text file busy`. The
/// window is tiny and clears on its own, so a bounded retry is the fix. It is a
/// property of writing-then-executing in a multi-threaded test, not of the code under
/// test; macOS does not enforce this, so it only ever appeared on Linux CI.
///
/// Every other outcome is passed through untouched — this must not paper over real
/// failures, including the timeout and output-limit errors these tests assert on.
pub struct RetryTextFileBusy<R>(pub R);

impl<R: CommandRunner> CommandRunner for RetryTextFileBusy<R> {
    fn run(&self, command: AdbCommand) -> Result<CommandOutput, ExecutionError> {
        for attempt in 0..50u32 {
            let result = self.0.run(command);
            match &result {
                Err(ExecutionError::Io(message)) if message.contains("Text file busy") => {
                    thread::sleep(Duration::from_millis(u64::from(10 * (attempt + 1).min(5))));
                }
                _ => return result,
            }
        }
        self.0.run(command)
    }
}

pub struct FakeAdb {
    directory: PathBuf,
    executable: PathBuf,
}

impl FakeAdb {
    pub fn new(script_body: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("quadcontrol-adb-probe-{}-{id}", std::process::id()));
        fs::create_dir(&directory).expect("create fake-adb directory");
        let executable = directory.join("adb");
        fs::write(&executable, format!("#!/bin/sh\n{script_body}\n")).expect("write fake adb");
        let mut permissions = fs::metadata(&executable)
            .expect("read fake-adb metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&executable, permissions).expect("make fake adb executable");

        Self {
            directory,
            executable,
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }
}

impl Drop for FakeAdb {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.executable);
        let _ = fs::remove_dir(&self.directory);
    }
}

/// Same `ETXTBSY` retry, applied to a whole CLI invocation.
///
/// The CLI execs the fake adb inside its own process, so the runner-level wrapper
/// cannot reach it; the race surfaces as a spurious `Text file busy` on stderr and a
/// non-zero exit. Only that signature is retried.
pub fn cli_output_retrying_text_file_busy(
    build: impl Fn() -> std::process::Command,
) -> std::process::Output {
    for attempt in 0..50u32 {
        let output = build().output().expect("run CLI");
        if !String::from_utf8_lossy(&output.stderr).contains("Text file busy") {
            return output;
        }
        thread::sleep(Duration::from_millis(u64::from(10 * (attempt + 1).min(5))));
    }
    build().output().expect("run CLI")
}

pub fn successful_adb() -> FakeAdb {
    FakeAdb::new(
        r#"
case "$1 $2" in
  "version ")
    printf 'Android Debug Bridge version 1.0.41\n'
    ;;
  "devices -l")
    printf 'List of devices attached\n192.0.2.1:5555 device product:pixel\n'
    ;;
  "mdns services")
    printf 'phone _adb-tls-connect._tcp. 192.0.2.1:37001\n'
    ;;
  *)
    printf 'unexpected arguments: %s %s\n' "$1" "$2" >&2
    exit 9
    ;;
esac
"#,
    )
}
