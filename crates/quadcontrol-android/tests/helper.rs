use adb_probe::parse_devices;
use quadcontrol_android::{validate_scrcpy_with_env, LaunchOptions, Session};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_scrcpy-test-helper"))
}

fn helper_env(mode: &str) -> Vec<(String, String)> {
    vec![("QUADCONTROL_FAKE_MODE".into(), mode.into())]
}

fn options(mode: &str) -> LaunchOptions {
    LaunchOptions {
        adb: PathBuf::from("REDACTEDSERIAL-adb"),
        scrcpy: helper(),
        serial: "REDACTEDSERIAL".into(),
        bit_rate: "8M".into(),
        screen_off: false,
        passthrough: Vec::new(),
        scrcpy_env: helper_env(mode),
    }
}

#[test]
fn helper_version_validation_covers_malformed_old_prerelease_and_nonzero_exit() {
    validate_scrcpy_with_env(&helper(), &helper_env("normal")).unwrap();
    assert!(validate_scrcpy_with_env(&helper(), &helper_env("prerelease")).is_err());
    assert!(validate_scrcpy_with_env(&helper(), &helper_env("invalid")).is_err());
    assert!(validate_scrcpy_with_env(&helper(), &helper_env("old")).is_err());
    assert!(validate_scrcpy_with_env(&helper(), &helper_env("version-fail")).is_err());
}

#[test]
fn fake_adb_reports_ready_and_unauthorized_device_states() {
    for (mode, expected_ready) in [("adb-ready", true), ("adb-unauthorized", false)] {
        let output = Command::new(helper())
            .env("QUADCONTROL_FAKE_MODE", mode)
            .args(["devices", "-l"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let devices = parse_devices(&String::from_utf8(output.stdout).unwrap()).unwrap();
        assert_eq!(
            matches!(devices[0].state, adb_probe::DeviceState::Device),
            expected_ready
        );
    }
}

#[test]
fn stderr_flood_is_drained_into_a_bounded_tail() {
    let session = Session::start(&options("stderr-flood")).unwrap();
    thread::sleep(Duration::from_millis(100));
    assert!(session.stderr_tail().len() <= 64 * 1024);
    let _ = session.wait().unwrap();
}

#[test]
fn scrcpy_exit_code_is_propagated() {
    let session = Session::start(&options("exit-23")).unwrap();
    assert_eq!(session.wait().unwrap(), 23);
}

#[test]
fn stop_reaps_the_helper_without_an_orphan() {
    let pid_file =
        std::env::temp_dir().join(format!("quadcontrol-helper-{}.pid", std::process::id()));
    let mut options = options("wait");
    options.scrcpy_env.push((
        "QUADCONTROL_FAKE_PID".into(),
        pid_file.to_string_lossy().into_owned(),
    ));
    let mut session = Session::start(&options).unwrap();
    for _ in 0..20 {
        if pid_file.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    session.stop().unwrap();
    let _ = session.wait().unwrap();
    let pid: u32 = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
    assert_process_gone(pid);
    let _ = std::fs::remove_file(pid_file);
}

#[cfg(unix)]
fn assert_process_gone(pid: u32) {
    unsafe {
        assert_eq!(libc::kill(pid as libc::pid_t, 0), -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
}

#[cfg(windows)]
fn assert_process_gone(pid: u32) {
    unsafe {
        let handle = windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        assert!(handle.is_null());
    }
}
