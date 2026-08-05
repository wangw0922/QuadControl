#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub struct FakeAdb {
    directory: PathBuf,
    executable: PathBuf,
}

impl FakeAdb {
    pub fn new(script_body: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "quadcontrol-adb-probe-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create fake-adb directory");
        let executable = directory.join("adb");
        fs::write(&executable, format!("#!/bin/sh\n{script_body}\n"))
            .expect("write fake adb");
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
