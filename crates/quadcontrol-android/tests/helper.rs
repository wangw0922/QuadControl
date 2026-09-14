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
        audio_on_computer: true,
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
    // 串行化：配对测试每 250ms 起一个假 adb，进程翻涌下 Windows 会立刻复用
    // 刚释放的 PID，OpenProcess 打开的是别人，"进程已消失"断言就会误炸。
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        if handle.is_null() {
            return; // PID 已释放，进程确实没了
        }
        // 句柄能打开不等于目标进程还活着：PID 可能已被复用，或对象仍在
        // 内核清理中。看退出码——STILL_ACTIVE (259) 才算"仍在运行"。
        let mut code: u32 = 0;
        let ok = windows_sys::Win32::System::Threading::GetExitCodeProcess(handle, &mut code);
        windows_sys::Win32::Foundation::CloseHandle(handle);
        assert!(
            ok == 0 || code != 259,
            "进程 {pid} 仍在运行（exit code 查询 ok={ok}, code={code}）"
        );
    }
}

/// 主流程级测试：驱动**真实入口** `pair_qr_command_with_secret`——预检、信号安装、
/// 扫码轮次、统一 round-close、上线等待、去重全在里面。删掉其中任何一段接线它都会红。
/// 只有 CSPRNG 那一行被注入替代，其余没有任何测试替身。
///
/// 环境变量是进程级共享状态，必须串行化并复原，否则与并行测试互相踩。
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn pair_qr_command_spawns_every_competing_endpoint_and_reaps_the_loser() {
    use quadcontrol_android::wireless;

    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let log = std::env::temp_dir().join(format!("qc-pair-log-{}", std::process::id()));
    let _ = std::fs::remove_file(&log);

    let secret = wireless::generate_pairing_secret();
    let service_name = secret.service_name.clone();
    let keys = [
        "QUADCONTROL_FAKE_MODE",
        "QUADCONTROL_FAKE_SERVICE",
        "QUADCONTROL_FAKE_PAIR_LOG",
    ];
    let previous: Vec<(&str, Option<String>)> = keys
        .iter()
        .map(|key| (*key, std::env::var(key).ok()))
        .collect();
    unsafe {
        std::env::set_var("QUADCONTROL_FAKE_MODE", "pair-competition");
        std::env::set_var("QUADCONTROL_FAKE_SERVICE", &service_name);
        std::env::set_var("QUADCONTROL_FAKE_PAIR_LOG", &log);
    }

    let mut rendered = String::new();
    let result = wireless::pair_qr_command_with_secret(&helper(), secret, |qr| {
        rendered = qr.to_owned();
    });

    unsafe {
        for (key, value) in previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }

    let (serial, visible, reason) = result.expect("竞争中应有一个端点配对成功并上线");
    assert_eq!(serial, "adb-FAKEGUID-0001");
    assert_eq!(visible, 1);
    // 结束原因只能由统一 round_close 产出——删掉那次调用，这里就没得断言。
    assert_eq!(reason, quadcontrol_android::wireless::CloseReason::Success);
    // 渲染改用 ANSI 背景色后不再有块字符:黑底(40)才是深色模块。
    assert!(
        rendered.contains("\u{1b}[40m") && rendered.contains("\u{1b}[47m"),
        "二维码必须在等待之前就展示出来,且深浅两色都在"
    );

    let logged = std::fs::read_to_string(&log).unwrap_or_default();
    let mut endpoints = Vec::new();
    let mut loser_pid = None;
    for line in logged.lines().filter(|line| !line.is_empty()) {
        let (endpoint, pid) = line.split_once(' ').expect("日志应为 端点 PID");
        endpoints.push(endpoint.to_owned());
        if endpoint == "127.0.0.1:41002" {
            loser_pid = pid.parse::<u32>().ok();
        }
    }
    assert!(
        endpoints.iter().any(|e| e == "127.0.0.1:37145")
            && endpoints.iter().any(|e| e == "127.0.0.1:41002"),
        "同名竞争的每个端点都应各起过一个 pair，实际: {endpoints:?}"
    );

    // 败者是个永不返回的进程。用 PID 存活性证明 round-close 确实回收了它，
    // 而不是仅仅把它从 Vec 里移除——那样 Drop 也能让 is_empty 断言通过。
    // 用既有的跨平台辅助断言，Windows 上没有 libc。
    assert_process_gone(loser_pid.expect("应记录到败者 PID"));

    let _ = std::fs::remove_file(&log);
}

/// 有状态回归：服务消失（pair 仍在飞）→ 随后 mDNS 查询失败。
///
/// 守护两件事:
/// 1. **mDNS 失败不得提前杀掉仍在飞的 pair**——方案对 service-lost 定的规矩
///    同样适用于查询失败，那个进程仍可能配对成功。
/// 2. 该场景最终必须真正关闭轮次并归因为 ServiceLost。曾有一个静默失效:
///    `round_close` 回收后没有同步 `pair_in_flight`，`observe(ServiceLost)`
///    命中"还有 pair 在飞 → 等待"分支，轮次不关闭、round-id 不作废。
#[test]
fn service_lost_with_mdns_failure_waits_for_the_pair_then_closes() {
    use quadcontrol_android::wireless;

    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let base = std::env::temp_dir();
    let log = base.join(format!("qc-sl-log-{}", std::process::id()));
    let counter = base.join(format!("qc-sl-counter-{}", std::process::id()));
    let survived = base.join(format!("qc-sl-survived-{}", std::process::id()));
    for path in [&log, &counter, &survived] {
        let _ = std::fs::remove_file(path);
    }

    let secret = wireless::generate_pairing_secret();
    let keys = [
        "QUADCONTROL_FAKE_MODE",
        "QUADCONTROL_FAKE_SERVICE",
        "QUADCONTROL_FAKE_PAIR_LOG",
        "QUADCONTROL_FAKE_COUNTER",
        "QUADCONTROL_FAKE_SURVIVED",
    ];
    let previous: Vec<(&str, Option<String>)> = keys
        .iter()
        .map(|key| (*key, std::env::var(key).ok()))
        .collect();
    unsafe {
        std::env::set_var("QUADCONTROL_FAKE_MODE", "pair-service-lost");
        std::env::set_var("QUADCONTROL_FAKE_SERVICE", &secret.service_name);
        std::env::set_var("QUADCONTROL_FAKE_PAIR_LOG", &log);
        std::env::set_var("QUADCONTROL_FAKE_COUNTER", &counter);
        std::env::set_var("QUADCONTROL_FAKE_SURVIVED", &survived);
    }

    let mut round = wireless::RoundState::new(11);
    let original_id = round.id;
    let mut pairs = Vec::new();
    let started = std::time::Instant::now();
    let (event, result) = wireless::run_pairing_round(&helper(), &secret, &mut round, &mut pairs);
    let elapsed = started.elapsed();

    unsafe {
        for (key, value) in previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }

    // 1. 在飞的 pair 必须活到自己退出：它自己写下的标记就是"没被 SIGKILL"的证据。
    assert!(
        survived.exists(),
        "mDNS 失败不得提前杀掉仍在飞的 pair 子进程"
    );
    assert!(
        elapsed >= std::time::Duration::from_millis(1500),
        "轮次应等待在飞的 pair 退出，实际只用了 {elapsed:?}"
    );

    // 2. service-lost 压过随后的 mDNS 失败与 pair 失败。
    assert_eq!(event, wireless::RoundEvent::ServiceLost);
    assert!(matches!(result, Err(wireless::WirelessError::ServiceLost)));

    let mut secret = secret;
    let mut qr = String::from("placeholder");
    let (reason, closed) =
        wireless::round_close(&mut round, &mut pairs, &mut secret, &mut qr, event);
    closed.unwrap();

    assert_eq!(
        reason,
        wireless::CloseReason::ServiceLost,
        "原因不得退化成 Cancelled"
    );
    assert!(round.closed, "轮次必须真的关闭");
    assert_ne!(
        round.id, original_id,
        "round-id 必须作废，否则迟到事件不会被丢弃"
    );
    assert_eq!(
        round.observe_for_round(original_id, wireless::RoundEvent::PairFailed),
        wireless::RoundAction::IgnoredAfterClose
    );
    assert!(qr.is_empty());
    assert!(
        secret.password.is_empty() && secret.service_name.is_empty() && secret.payload.is_empty()
    );

    for path in [&log, &counter, &survived] {
        let _ = std::fs::remove_file(path);
    }
}
