use quadcontrol_android::{shutdown_all, spawn_supervised, LaunchOptions, SessionStatus};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_scrcpy-test-helper"))
}

fn options(mode: &str, pid_file: Option<&PathBuf>) -> LaunchOptions {
    let mut scrcpy_env = vec![("QUADCONTROL_FAKE_MODE".into(), mode.into())];
    if let Some(pid_file) = pid_file {
        scrcpy_env.push((
            "QUADCONTROL_FAKE_PID".into(),
            pid_file.to_string_lossy().into_owned(),
        ));
    }
    LaunchOptions {
        adb: PathBuf::from("REDACTEDSERIAL-adb"),
        scrcpy: helper(),
        serial: "REDACTEDSERIAL".into(),
        bit_rate: "8M".into(),
        screen_off: false,
        passthrough: Vec::new(),
        scrcpy_env,
    }
}

fn wait_for_status(handle: &quadcontrol_android::SessionHandle) -> SessionStatus {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let status = handle.status();
        if !matches!(status, SessionStatus::Running | SessionStatus::Stopping) {
            return status;
        }
        assert!(Instant::now() < deadline, "监督会话未在测试预算内结束");
        thread::sleep(Duration::from_millis(10));
    }
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
        if handle.is_null() {
            return;
        }
        let mut code = 0;
        let ok = windows_sys::Win32::System::Threading::GetExitCodeProcess(handle, &mut code);
        windows_sys::Win32::Foundation::CloseHandle(handle);
        assert!(ok == 0 || code != 259);
    }
}

/// P2 要把 Handle 放进 Tauri 的共享 state，那里要求 `Send + Sync`。
/// 编译期钉住，免得到接线时才发现结构选错。
#[test]
fn handle_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<quadcontrol_android::SessionHandle>();
}

#[test]
fn request_stop_reaps_supervised_helper() {
    let pid_file = std::env::temp_dir().join(format!(
        "quadcontrol-supervised-{}-{}.pid",
        std::process::id(),
        1
    ));
    let _ = std::fs::remove_file(&pid_file);
    let handle = spawn_supervised(&options("wait", Some(&pid_file)), 1).unwrap();
    for _ in 0..200 {
        if pid_file.exists() {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    handle.request_stop().unwrap();
    assert!(matches!(
        wait_for_status(&handle),
        SessionStatus::Exited { .. }
    ));
    let pid: u32 = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
    assert_process_gone(pid);
    let _ = std::fs::remove_file(pid_file);
}

#[test]
fn natural_exit_is_reported_with_code_and_stderr_tail() {
    let handle = spawn_supervised(&options("exit-7", None), 2).unwrap();
    assert_eq!(
        wait_for_status(&handle),
        SessionStatus::Exited {
            code: 7,
            stderr_tail: String::new(),
        }
    );
}

#[test]
fn repeated_stop_requests_are_idempotent() {
    let handle = spawn_supervised(&options("wait", None), 3).unwrap();
    handle.request_stop().unwrap();
    handle.request_stop().unwrap();
    handle.request_stop().unwrap();
    assert!(matches!(
        wait_for_status(&handle),
        SessionStatus::Exited { .. }
    ));
}

#[test]
fn shutdown_all_broadcasts_and_waits_for_all_sessions() {
    let handles: Vec<_> = (10..13)
        .map(|id| spawn_supervised(&options("wait", None), id).unwrap())
        .collect();
    let report = shutdown_all(&handles, Duration::from_secs(7));
    assert!(
        report.timed_out.is_empty(),
        "timed out: {:?}",
        report.timed_out
    );
    for handle in &handles {
        assert!(matches!(handle.status(), SessionStatus::Exited { .. }));
    }
}

#[test]
fn running_status_is_visible_before_stop() {
    let handle = spawn_supervised(&options("wait", None), 20).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !matches!(handle.status(), SessionStatus::Running) {
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    handle.request_stop().unwrap();
    assert!(matches!(
        wait_for_status(&handle),
        SessionStatus::Exited { .. }
    ));
}
