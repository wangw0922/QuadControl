//! 会话监督的生命周期测试：假 `ios` 二进制 + 测试内假 WDA 服务，不碰真机。
//!
//! 「无孤儿」的判据是**可证伪的**：记下子进程 PID，停止后 `kill -0` 必须得
//! `ESRCH`（与 `quadcontrol-android/tests/supervised.rs` 同一方法）。

mod fake_wda;

use fake_wda::{FakeWda, FakeWdaConfig};
use quadcontrol_ios::session::{
    shutdown_all, spawn_supervised, ExitReason, IosLaunchOptions, IosSessionHandle,
    IosSessionStatus, LaunchMode, IOS_SHUTDOWN_DEADLINE,
};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

// 只有 Launch 模式的用例（`cfg(unix)`）用得到这些；windows 上它们会被判成未使用。
#[cfg(unix)]
use quadcontrol_ios::session::WdaIds;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::atomic::{AtomicU16, Ordering};

/// 假 UDID：**绝不出现真实设备标识**（红线）。
const FAKE_UDID: &str = "REDACTEDSERIAL-0001";

/// re-exec 辅助进程的环境变量标记，值是 PID 落盘目录。
// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
const CHILD_HARNESS_MARKER: &str = "QUADCONTROL_IOS_PDEATHSIG_PID_DIR";

// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
fn helper() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_ios-test-helper"))
}

/// 给每个用例分配互不冲突的假隧道信息端口。
///
/// 真实默认端口是 28100，但集成测试并发跑：若所有用例都探测同一个端口，
/// 「端口被占」这一条会把其它用例全带崩。所以端口是 `IosLaunchOptions` 的字段。
///
/// **不能**用「bind 0 拿号再释放」：那个号会立刻落回临时端口池，被并发用例里的
/// 合成上游抢走，于是本该正常启动的会话报 `TunnelPortBusy`——实测就是这样偶发
/// 失败的。这里改用一段固定的高位私有区间 + 原子计数，每个用例拿到互不相同、
/// 且不在临时端口池里的号，再确认它确实空闲。
///
/// 这个端口现在有**两重**作用：启动前的「是否被占」预检要求它空闲，而假
/// `tunnel start` 随后会**真的绑上它**——会话正是靠「绑不上了」判定隧道就绪。
/// 所以这里返回的号必须此刻空闲、且不会被别人抢走，两个条件缺一不可。
// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
fn unique_tunnel_port() -> u16 {
    /// 低于 macOS / Linux 的临时端口范围，避开被 bind 0 分配到的可能。
    const BASE: u16 = 28200;
    static NEXT: AtomicU16 = AtomicU16::new(0);
    for _ in 0..200 {
        let port = BASE + NEXT.fetch_add(1, Ordering::Relaxed);
        // bind 成功即释放：证明此刻没人占，且这个号不会被临时端口池再分配出去。
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("找不到空闲的假隧道信息端口");
}

/// 起会话，遇到 `tunnel_port_busy` 就换个端口重试。
///
/// 为什么需要：`unique_tunnel_port` 只能保证「取号那一刻空闲」，而假 `tunnel start`
/// 现在会**真的长期持有**这个端口（S3）。取号与会话内部的占用预检之间存在窗口——
/// 上一轮测试残留的隧道进程恰好在这段时间里退出/仍在，都会让预检判成被占。
/// 实测 8 轮里复现 1 次，所以这里直接重试，而不是把偶发失败留给 CI。
///
/// 只对 `tunnel_port_busy` 重试；其它错误原样抛出，免得把真故障掩盖掉。
// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
fn spawn_with_port_retry(options: &IosLaunchOptions, id: u64) -> IosSessionHandle {
    let mut attempt = options.clone();
    for _ in 0..10 {
        match spawn_supervised(&attempt, id) {
            Ok(handle) => return handle,
            Err(error) if error.code() == "tunnel_port_busy" => {
                attempt.tunnel_info_port = unique_tunnel_port();
            }
            Err(error) => panic!("起会话失败：{error}"),
        }
    }
    panic!("连续 10 次都撞上被占用的假隧道信息端口");
}

// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
struct Fixture {
    pid_dir: PathBuf,
    argv_log: PathBuf,
}

// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
impl Fixture {
    fn new(tag: &str) -> Self {
        let base =
            std::env::temp_dir().join(format!("quadcontrol-ios-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        Self {
            argv_log: base.join("argv.log"),
            pid_dir: base,
        }
    }

    fn env(&self, extra: &[(&str, String)]) -> Vec<(String, String)> {
        let mut env = vec![
            (
                "QUADCONTROL_FAKE_PID_DIR".to_owned(),
                self.pid_dir.to_string_lossy().into_owned(),
            ),
            (
                "QUADCONTROL_FAKE_ARGV_LOG".to_owned(),
                self.argv_log.to_string_lossy().into_owned(),
            ),
        ];
        for (key, value) in extra {
            env.push(((*key).to_owned(), value.clone()));
        }
        env
    }

    fn argv_lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.argv_log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// 等所有四个子进程都写下 PID，返回它们。
    fn pids(&self, expected: usize) -> Vec<u32> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let pids: Vec<u32> = std::fs::read_dir(&self.pid_dir)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "pid"))
                .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
                .filter_map(|text| text.trim().parse::<u32>().ok())
                .collect();
            if pids.len() >= expected {
                return pids;
            }
            assert!(
                Instant::now() < deadline,
                "只看到 {} 个子进程 PID，期望 {expected}",
                pids.len()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

// 只被 Launch 模式的用例用到，而那些用例是 `cfg(unix)` 的
// （windows 上 Launch 直接返回 `windows_session_unsupported`）。
// 不加这个 gate，交叉 clippy 会把它判成 dead_code。
#[cfg(unix)]
fn launch_options(
    fixture: &Fixture,
    wda_port: u16,
    mjpeg_port: u16,
    extra_env: &[(&str, String)],
) -> IosLaunchOptions {
    let mut env = fixture.env(extra_env);
    // 假 `forward` 要真的把字节管到这两个上游，否则会话永远到不了 Running。
    env.push(("QUADCONTROL_FAKE_FORWARD_8100".into(), wda_port.to_string()));
    env.push((
        "QUADCONTROL_FAKE_FORWARD_9100".into(),
        mjpeg_port.to_string(),
    ));
    IosLaunchOptions {
        udid: FAKE_UDID.to_owned(),
        mode: LaunchMode::Launch {
            ios: helper(),
            wda: WdaIds {
                bundle_id: "com.example.WebDriverAgentRunner".into(),
                testrunner_id: "com.example.WebDriverAgentRunner.xctrunner".into(),
                xctestconfig: "WebDriverAgentRunner.xctest".into(),
            },
            env,
        },
        tunnel_info_port: unique_tunnel_port(),
        status_budget: Duration::from_secs(20),
    }
}

/// 合成一个**只发有限帧就关闭**的 MJPEG 上游，用来模拟链路死亡。
fn synthetic_mjpeg_closing_after(frames: usize) -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { return };
            thread::spawn(move || {
                let head = "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=fr\r\n\r\n";
                if stream.write_all(head.as_bytes()).is_err() {
                    return;
                }
                for _ in 0..frames {
                    let frame = vec![8u8; 1024];
                    let part = format!(
                        "--fr\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame.len()
                    );
                    if stream.write_all(part.as_bytes()).is_err()
                        || stream.write_all(&frame).is_err()
                        || stream.write_all(b"\r\n").is_err()
                    {
                        return;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                // 关掉 = 上游 EOF = 链路死亡。
            });
        }
    });
    port
}

/// 合成一个 MJPEG 上游，供会话的代理连接。
fn synthetic_mjpeg() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { return };
            thread::spawn(move || {
                let head = "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=fr\r\n\r\n";
                if stream.write_all(head.as_bytes()).is_err() {
                    return;
                }
                loop {
                    let frame = vec![8u8; 1024];
                    let part = format!(
                        "--fr\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        frame.len()
                    );
                    if stream.write_all(part.as_bytes()).is_err()
                        || stream.write_all(&frame).is_err()
                        || stream.write_all(b"\r\n").is_err()
                    {
                        return;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            });
        }
    });
    port
}

fn wait_for_running(handle: &IosSessionHandle) -> u16 {
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        match handle.status() {
            IosSessionStatus::Running { proxy_port } => return proxy_port,
            IosSessionStatus::Starting => {}
            other => panic!("会话没能进入 Running：{other:?}"),
        }
        assert!(Instant::now() < deadline, "会话启动超时");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_settled(handle: &IosSessionHandle, budget: Duration) -> IosSessionStatus {
    let deadline = Instant::now() + budget;
    loop {
        let status = handle.status();
        if !matches!(
            status,
            IosSessionStatus::Starting
                | IosSessionStatus::Running { .. }
                | IosSessionStatus::Stopping
        ) {
            return status;
        }
        assert!(Instant::now() < deadline, "会话未在预算内结束：{status:?}");
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn assert_process_gone(pid: u32) {
    unsafe {
        assert_eq!(libc::kill(pid as libc::pid_t, 0), -1, "pid {pid} 仍存在");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH),
            "pid {pid} 没有被回收"
        );
    }
}

/// windows 版判据，与 `quadcontrol-android/tests/supervised.rs` 同一套：
/// 打不开句柄说明进程已走；打得开就看退出码不是 `STILL_ACTIVE`(259)。
///
/// 目前**没有调用方**：检查 PID 的用例都属于 Launch 模式，而 Launch 在 windows 上
/// 按方案直接被拒（P6 才做进程树回收）。刻意保留并 `allow(dead_code)` 而不是删掉：
/// P6 打开 windows Launch 时需要的就是这段判据，且留着能让它持续参与交叉编译，
/// 不至于到那时才发现写错了。
#[cfg(windows)]
#[allow(dead_code)]
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
        assert!(ok == 0 || code != 259, "pid {pid} 仍在运行");
    }
}

#[test]
fn handle_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<IosSessionHandle>();
}

/// 正常起停：四个子进程都起来，停止后**一个孤儿都不留**。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn launch_then_stop_leaves_no_orphans() {
    let fixture = Fixture::new("launch-stop");
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = launch_options(&fixture, wda.port, mjpeg, &[]);

    let handle = spawn_with_port_retry(&options, 1);
    let proxy_port = wait_for_running(&handle);
    assert!(proxy_port > 0);
    let pids = fixture.pids(4);
    assert_eq!(pids.len(), 4, "应当正好四个子进程：tunnel/runwda/forward×2");

    // 代理确实在转发上游的帧。
    let deadline = Instant::now() + Duration::from_secs(10);
    while handle.stats().frames == 0 {
        assert!(Instant::now() < deadline, "代理没有计到帧");
        thread::sleep(Duration::from_millis(20));
    }
    assert!(handle.client().is_some(), "Running 时应当能拿到 WDA 客户端");

    handle.request_stop().unwrap();
    assert_eq!(
        wait_for_settled(&handle, IOS_SHUTDOWN_DEADLINE + Duration::from_secs(3)),
        IosSessionStatus::Exited {
            reason: ExitReason::Requested
        }
    );
    for pid in pids {
        assert_process_gone(pid);
    }
    // 退出序列里 `DELETE /session` 是礼貌性调用，但应当发生过。
    assert!(wda.saw("DELETE /session/FAKESESSION"));
    // 代理端口已释放。
    assert!(TcpListener::bind(("127.0.0.1", proxy_port)).is_ok());
}

/// 方案要求的硬指标：**四个子进程全部忽略 SIGINT** 时，
/// `request_stop` 到 done 的墙钟必须 < 7 s（`IOS_SHUTDOWN_DEADLINE`）。
/// 这是并行梯子存在的唯一理由——串行梯子这里会花 20 s。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn stop_beats_the_deadline_even_when_every_child_ignores_sigint() {
    let fixture = Fixture::new("ignore-sigint");
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = launch_options(
        &fixture,
        wda.port,
        mjpeg,
        &[("QUADCONTROL_FAKE_IGNORE_SIGINT", "1".to_owned())],
    );

    let handle = spawn_with_port_retry(&options, 2);
    wait_for_running(&handle);
    let pids = fixture.pids(4);

    let started = Instant::now();
    handle.request_stop().unwrap();
    let status = wait_for_settled(&handle, Duration::from_secs(20));
    let elapsed = started.elapsed();

    assert!(
        matches!(status, IosSessionStatus::Exited { .. }),
        "{status:?}"
    );
    // 下界：SIGINT 被全部忽略，梯子**必须**等满 3 s 才升级到 SIGTERM。
    // 没有这条断言，「子进程其实没忽略 SIGINT」这类替身失效会让用例假通过——
    // 本机就踩过一次（macOS 的 /bin/sh trap 不生效）。
    assert!(
        elapsed >= Duration::from_secs(3),
        "只用了 {elapsed:?}：子进程没有真的忽略 SIGINT，这条用例没测到梯子"
    );
    assert!(
        elapsed < IOS_SHUTDOWN_DEADLINE,
        "四个子进程忽略 SIGINT 时停止用了 {elapsed:?}，超出 {IOS_SHUTDOWN_DEADLINE:?}"
    );
    for pid in pids {
        assert_process_gone(pid);
    }
}

/// `runwda` 立刻失败 → 按 stderr 关键字分类成对应错误码。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn runwda_immediate_failure_maps_to_a_stable_code() {
    for (stderr, expected) in [
        (
            "codesign failed: certificate has expired",
            "wda_signature_expired",
        ),
        ("WebDriverAgentRunner is not installed", "wda_not_installed"),
    ] {
        let fixture = Fixture::new(&format!("runwda-{expected}"));
        let wda = FakeWda::start(FakeWdaConfig::healthy());
        let mjpeg = synthetic_mjpeg();
        let options = launch_options(
            &fixture,
            wda.port,
            mjpeg,
            &[("QUADCONTROL_FAKE_RUNWDA_STDERR", stderr.to_owned())],
        );

        let handle = spawn_with_port_retry(&options, 3);
        let status = wait_for_settled(&handle, Duration::from_secs(30));
        assert_eq!(
            status,
            IosSessionStatus::Failed { code: expected },
            "stderr={stderr}"
        );
    }
}

/// `/status` 一直不通 → `wda_unreachable`，且预算是可配置的（测试用短预算）。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn status_budget_expiry_reports_wda_unreachable() {
    let fixture = Fixture::new("status-timeout");
    // 不给 8100 配上游：forward 端口通了，但后面没有 WDA。
    let mjpeg = synthetic_mjpeg();
    let mut env = fixture.env(&[]);
    env.push(("QUADCONTROL_FAKE_FORWARD_9100".into(), mjpeg.to_string()));
    let options = IosLaunchOptions {
        udid: FAKE_UDID.to_owned(),
        mode: LaunchMode::Launch {
            ios: helper(),
            wda: WdaIds {
                bundle_id: "com.example.wda".into(),
                testrunner_id: "com.example.wda.xctrunner".into(),
                xctestconfig: "WebDriverAgentRunner.xctest".into(),
            },
            env,
        },
        tunnel_info_port: unique_tunnel_port(),
        status_budget: Duration::from_secs(2),
    };

    let handle = spawn_with_port_retry(&options, 4);
    let status = wait_for_settled(&handle, Duration::from_secs(20));
    assert_eq!(
        status,
        IosSessionStatus::Failed {
            code: "wda_unreachable"
        }
    );
}

/// 隧道信息端口被占 → `tunnel_port_busy`，绝不静默复用别人的隧道。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn busy_tunnel_info_port_is_refused() {
    let fixture = Fixture::new("tunnel-busy");
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let mut options = launch_options(&fixture, wda.port, mjpeg, &[]);

    // 占住该端口并**保持**占用。
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    options.tunnel_info_port = occupied.local_addr().unwrap().port();

    let error = spawn_supervised(&options, 5).unwrap_err();
    assert_eq!(error.code(), "tunnel_port_busy");
}

/// Attach 模式：不起任何子进程，直接接上已有端口。
#[test]
fn attach_mode_runs_without_any_child_process() {
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = IosLaunchOptions::new(
        FAKE_UDID,
        LaunchMode::Attach {
            http_port: wda.port,
            mjpeg_port: mjpeg,
        },
    );

    let handle = spawn_supervised(&options, 6).unwrap();
    // `Running` 只带代理端口：WebView 只连代理，WDA 的 HTTP 端口不对外暴露。
    let proxy_port = wait_for_running(&handle);
    assert!(proxy_port > 0);

    let deadline = Instant::now() + Duration::from_secs(10);
    while handle.stats().frames == 0 {
        assert!(Instant::now() < deadline, "Attach 模式下代理没有计到帧");
        thread::sleep(Duration::from_millis(20));
    }
    // 客户端可用，GUI 的 tap / home 之类命令能经它下发。
    let client = handle.client().unwrap();
    client.home().unwrap();

    handle.request_stop().unwrap();
    assert_eq!(
        wait_for_settled(&handle, IOS_SHUTDOWN_DEADLINE),
        IosSessionStatus::Exited {
            reason: ExitReason::Requested
        }
    );
    // 下游连接随之关闭。
    thread::sleep(Duration::from_millis(100));
    let refused = TcpStream::connect(("127.0.0.1", proxy_port)).is_err();
    assert!(
        refused || TcpListener::bind(("127.0.0.1", proxy_port)).is_ok(),
        "停止后代理端口应当已释放"
    );
}

/// 子进程自亡 → 监督线程按同一序列收回其余，并报告是谁先死的。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn a_dying_child_takes_the_whole_session_down() {
    let fixture = Fixture::new("child-dies");
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = launch_options(&fixture, wda.port, mjpeg, &[]);

    let handle = spawn_with_port_retry(&options, 7);
    wait_for_running(&handle);
    let pids = fixture.pids(4);

    // 手动打死一个子进程，模拟「进程自亡」。
    #[cfg(unix)]
    unsafe {
        libc::kill(pids[0] as libc::pid_t, libc::SIGKILL);
    }

    let status = wait_for_settled(&handle, IOS_SHUTDOWN_DEADLINE + Duration::from_secs(3));
    assert!(
        matches!(
            status,
            IosSessionStatus::Exited {
                reason: ExitReason::ChildExited { .. }
            }
        ),
        "{status:?}"
    );
    // 其余进程一并被收回。
    for pid in pids {
        assert_process_gone(pid);
    }
}

/// 梯子的**最坏路径**：四个子进程同时忽略 SIGINT 与 SIGTERM，只有 SIGKILL 收得走。
///
/// 这是关闭预算最容易被突破的场景：SIGINT 等 3 s + SIGTERM 等 2 s 全部白等，
/// 之后才 SIGKILL。墙钟必须仍然装进 `IOS_SHUTDOWN_DEADLINE`（7 s）。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn stop_beats_the_deadline_when_children_ignore_sigint_and_sigterm() {
    let fixture = Fixture::new("ignore-both");
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = launch_options(
        &fixture,
        wda.port,
        mjpeg,
        &[
            ("QUADCONTROL_FAKE_IGNORE_SIGINT", "1".to_owned()),
            ("QUADCONTROL_FAKE_IGNORE_SIGTERM", "1".to_owned()),
        ],
    );

    let handle = spawn_with_port_retry(&options, 70);
    wait_for_running(&handle);
    let pids = fixture.pids(4);

    let started = Instant::now();
    handle.request_stop().unwrap();
    let status = wait_for_settled(&handle, Duration::from_secs(20));
    let elapsed = started.elapsed();

    assert!(
        matches!(status, IosSessionStatus::Exited { .. }),
        "{status:?}"
    );
    // 两级温和信号都被忽略：至少要等满 3 s + 2 s 才会 SIGKILL。
    assert!(
        elapsed >= Duration::from_secs(5),
        "只用了 {elapsed:?}：子进程没有真的同时忽略 SIGINT 与 SIGTERM"
    );
    assert!(
        elapsed < IOS_SHUTDOWN_DEADLINE,
        "最坏路径停止用了 {elapsed:?}，超出 {IOS_SHUTDOWN_DEADLINE:?}"
    );
    for pid in pids {
        assert_process_gone(pid);
    }
}

/// 启动阶段（`Starting`）收到停止请求 → 干净的 `Exited { Requested }`。
///
/// **不是** `Failed { wda_unreachable }`：用户自己点了断开，界面不该再弹一个
/// 「WDA 不可达」的故障。这里把 `/status` 永远不通（8100 没有上游）和一个短预算
/// 组合起来，确保会话确实停在 `Starting`，然后在预算耗尽前请求停止。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn stopping_while_starting_exits_cleanly() {
    let fixture = Fixture::new("stop-while-starting");
    let mjpeg = synthetic_mjpeg();
    // 刻意**不配** FORWARD_8100：`/status` 永远不通，会话会一直停在 Starting。
    let mut env = fixture.env(&[]);
    env.push(("QUADCONTROL_FAKE_FORWARD_9100".into(), mjpeg.to_string()));
    let options = IosLaunchOptions {
        udid: FAKE_UDID.to_owned(),
        mode: LaunchMode::Launch {
            ios: helper(),
            wda: WdaIds {
                bundle_id: "com.example.wda".into(),
                testrunner_id: "com.example.wda.xctrunner".into(),
                xctestconfig: "WebDriverAgentRunner.xctest".into(),
            },
            env,
        },
        tunnel_info_port: unique_tunnel_port(),
        status_budget: Duration::from_secs(30),
    };

    let handle = spawn_with_port_retry(&options, 71);
    // 刚返回时必然是 Starting，这个断言没有竞态。
    assert_eq!(handle.status(), IosSessionStatus::Starting);

    // 等四个子进程都起来（为后面的无孤儿断言取 PID）。这里**不再**复查
    // 「仍是 Starting」：那是个时间点判断，并发跑时曾偶发失败。会话是否真的停在
    // Starting 由下面的结局断言保证——`/status` 永远不通（8100 没有上游），
    // 30 s 预算内它不可能自己变成 Running，所以只要结局是 Exited{Requested}，
    // 就说明停止请求确实是在启动阶段被处理的。
    let pids = fixture.pids(4);

    handle.request_stop().unwrap();
    assert_eq!(
        wait_for_settled(&handle, IOS_SHUTDOWN_DEADLINE + Duration::from_secs(3)),
        IosSessionStatus::Exited {
            reason: ExitReason::Requested
        },
        "启动阶段被用户停止应当是干净退出，不是故障"
    );
    for pid in pids {
        assert_process_gone(pid);
    }
}

/// 上游 EOF（链路死亡）→ 会话必须收摊并报 `UpstreamClosed`。
///
/// 这条挡的是一个真实的漏网 bug：代理在上游 EOF 时只置停止标志、不置 `failure`，
/// 监督线程若只看 `failure()`，画面已经黑了而状态还停在 `Running`。
#[test]
fn upstream_eof_ends_the_session() {
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg_closing_after(2);
    let options = IosLaunchOptions::new(
        FAKE_UDID,
        LaunchMode::Attach {
            http_port: wda.port,
            mjpeg_port: mjpeg,
        },
    );

    let handle = spawn_supervised(&options, 60).unwrap();
    let status = wait_for_settled(&handle, Duration::from_secs(20));
    assert_eq!(
        status,
        IosSessionStatus::Exited {
            reason: ExitReason::UpstreamClosed
        }
    );
}

/// `shutdown_all`：广播停止并以共享预算等待；语义同 Android。
#[test]
fn shutdown_all_broadcasts_and_waits_for_every_session() {
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let handles: Vec<_> = (20..23)
        .map(|id| {
            let options = IosLaunchOptions::new(
                FAKE_UDID,
                LaunchMode::Attach {
                    http_port: wda.port,
                    mjpeg_port: mjpeg,
                },
            );
            let handle = spawn_supervised(&options, id).unwrap();
            wait_for_running(&handle);
            handle
        })
        .collect();

    let report = shutdown_all(&handles, IOS_SHUTDOWN_DEADLINE);
    assert!(report.timed_out.is_empty(), "{:?}", report.timed_out);
    for handle in &handles {
        assert!(matches!(handle.status(), IosSessionStatus::Exited { .. }));
    }
}

/// `shutdown_all` 的预算用尽要**如实上报**，不是假装成功。
#[test]
fn shutdown_all_reports_timeouts() {
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = IosLaunchOptions::new(
        FAKE_UDID,
        LaunchMode::Attach {
            http_port: wda.port,
            mjpeg_port: mjpeg,
        },
    );
    let handle = spawn_supervised(&options, 30).unwrap();
    wait_for_running(&handle);

    // 零预算：一定来不及，必须上报超时。
    let report = shutdown_all(&[handle], Duration::ZERO);
    assert_eq!(report.timed_out, vec![30]);
}

/// fixture argv 断言：假 `ios` 收到的每个 `--flag`，都必须出现在**真机抓的
/// usage 文本**里。这样 go-ios 改语法就会变成红测试，而不是运行时才炸。
#[test]
// Launch 模式在 windows 上按方案直接返回 `windows_session_unsupported`
// （没有 Job Object 收不回进程树，P6 再做），所以这些用例只在 unix 上跑。
#[cfg(unix)]
fn every_flag_we_pass_appears_in_the_real_usage_fixture() {
    let fixture = Fixture::new("argv");
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let options = launch_options(&fixture, wda.port, mjpeg, &[]);
    let handle = spawn_with_port_retry(&options, 40);
    wait_for_running(&handle);
    handle.request_stop().unwrap();
    wait_for_settled(&handle, IOS_SHUTDOWN_DEADLINE + Duration::from_secs(3));

    let usage = include_str!("fixtures/go-ios-1.2.1/ios-usage-block.stderr.txt");
    let global = include_str!("fixtures/go-ios-1.2.1/ios-help.txt");

    // 通用 options 块（`ios --help` 的 "Global options:"）里的 flag，任何子命令
    // 都能用。
    let global_flags: Vec<&str> = global
        .lines()
        .skip_while(|line| !line.starts_with("Global options:"))
        .take_while(|line| line.starts_with("Global options:") || line.starts_with("  "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|token| token.starts_with("--"))
        .map(|token| token.split('=').next().unwrap())
        .collect();
    assert!(
        global_flags.contains(&"--udid") && global_flags.contains(&"--tunnel-info-port"),
        "通用 options 块没解析出来：{global_flags:?}"
    );

    let lines = fixture.argv_lines();
    assert!(!lines.is_empty(), "假 ios 没有记录任何 argv");

    let mut saw_subcommands = Vec::new();
    for line in &lines {
        let args: Vec<&str> = line.split_whitespace().collect();
        let positional: Vec<&str> = args
            .iter()
            .copied()
            .filter(|a| !a.starts_with('-'))
            .collect();
        saw_subcommands.push(positional.join(" "));

        // 关键：只拿**这个子命令自己**的 usage 行做允许集，不是整个 usage 文件。
        // 否则 `--bundle-id`（webinspector 的写法）会冒充 runwda 的 `--bundleid`
        // 混过去，语法漂移就测不出来了。
        let subcommand = match positional.as_slice() {
            ["tunnel", "start", ..] => "ios tunnel start",
            ["runwda", ..] => "ios runwda",
            ["forward", ..] => "ios forward",
            other => panic!("假 ios 收到了意料之外的子命令：{other:?}"),
        };
        let usage_line = usage
            .lines()
            .map(str::trim)
            .find(|candidate| candidate.starts_with(subcommand))
            .unwrap_or_else(|| panic!("usage fixture 里找不到 `{subcommand}`"));

        for arg in args.iter().filter(|a| a.starts_with("--")) {
            let flag = arg.split('=').next().unwrap();
            let in_subcommand = usage_line.contains(&format!("{flag}="))
                || usage_line.contains(&format!("{flag}]"))
                || usage_line.contains(&format!("{flag} "));
            assert!(
                in_subcommand || global_flags.contains(&flag),
                "{flag} 既不在 `{subcommand}` 的 usage 行里、也不是通用 option\
                 （go-ios 语法漂移了）：{line}"
            );
        }
    }

    // 子命令本身也要对得上 usage 里的写法。
    assert!(usage.contains("ios tunnel start"), "usage fixture 变了");
    assert!(usage.contains("ios runwda"), "usage fixture 变了");
    assert!(usage.contains("ios forward"), "usage fixture 变了");
    let joined = saw_subcommands.join(" | ");
    assert!(joined.contains("tunnel start"), "{joined}");
    assert!(joined.contains("runwda"), "{joined}");
    assert!(joined.contains("forward"), "{joined}");
}

/// Linux：`kill -9` 掉父进程后，子进程应随之消失（`PR_SET_PDEATHSIG`）。
///
/// 做法是 **re-exec 自己**：用 `current_exe()` 再起一个本测试二进制，让它跑下面那个
/// `#[ignore]` 的 `pdeathsig_child_harness`——那个 harness 在**它自己**的进程里起一个
/// Launch 会话（于是四个假子进程的父亲是它，而不是本测试），把 PID 写到共享目录后
/// 长睡。然后本测试 `kill -9` 掉它：父亲没有机会做任何清理，子进程只可能靠
/// PDEATHSIG 自己死掉。这才是「可证伪的无孤儿」，读 `/proc` 里根本查不到 PDEATHSIG。
///
/// 标 `cfg(unix)` 而不是 `cfg(target_os = "linux")`：这样**函数体在 macOS 上也参与
/// 编译**（避免交付从未编译过的代码），只是在非 Linux 上被 `ignore` 跳过——macOS
/// 没有 PDEATHSIG 的等价机制，方案里如实标注了这一点。
#[test]
#[cfg(unix)]
#[cfg_attr(not(target_os = "linux"), ignore = "PDEATHSIG 是 Linux 专有机制")]
fn children_die_with_a_sigkilled_parent() {
    let pid_dir = std::env::temp_dir().join(format!(
        "quadcontrol-ios-pdeathsig-{}-parent",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&pid_dir);
    std::fs::create_dir_all(&pid_dir).unwrap();

    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "pdeathsig_child_harness",
            "--ignored",
            "--nocapture",
        ])
        .env(CHILD_HARNESS_MARKER, pid_dir.to_string_lossy().into_owned())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        // stderr 落盘：中间父进程起会话失败时，这是唯一的诊断线索（CI 上只跑 Linux，
        // 本机 macOS 看不到这条用例）。
        .stderr(std::fs::File::create(pid_dir.join("harness.stderr")).unwrap())
        .spawn()
        .unwrap();

    // 等它把四个孙子进程的 PID 落盘。
    let deadline = Instant::now() + Duration::from_secs(60);
    let grandchildren = loop {
        let pids: Vec<u32> = std::fs::read_dir(&pid_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "pid"))
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .filter_map(|t| t.trim().parse::<u32>().ok())
            .collect();
        if pids.len() >= 4 {
            break pids;
        }
        if Instant::now() >= deadline || child.try_wait().ok().flatten().is_some() {
            let names: Vec<String> = std::fs::read_dir(&pid_dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            let stderr =
                std::fs::read_to_string(pid_dir.join("harness.stderr")).unwrap_or_default();
            let _ = child.kill();
            panic!(
                "中间父进程没能起好会话，只看到 {} 个 PID；目录 {names:?}；harness stderr:\n{stderr}",
                pids.len()
            );
        }
        thread::sleep(Duration::from_millis(50));
    };

    // SIGKILL：父亲不可能执行任何清理代码。
    unsafe {
        assert_eq!(libc::kill(child.id() as libc::pid_t, libc::SIGKILL), 0);
    }
    let _ = child.wait();

    // 孙子进程只能靠 PDEATHSIG 自己退出。
    let deadline = Instant::now() + Duration::from_secs(30);
    for pid in grandchildren {
        loop {
            let gone = unsafe { libc::kill(pid as libc::pid_t, 0) } == -1;
            if gone {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "父进程被 kill -9 后 pid {pid} 仍存活：PDEATHSIG 没起作用"
            );
            thread::sleep(Duration::from_millis(100));
        }
    }
    let _ = std::fs::remove_dir_all(&pid_dir);
}

/// 只由 [`children_die_with_a_sigkilled_parent`] 通过 re-exec 调起的辅助进程。
///
/// 永久 `#[ignore]`：它会长睡直到被 `kill -9`，绝不能进常规测试轮次。没有那个
/// 环境变量时直接返回，避免有人手动 `--ignored` 跑到它而挂住。
#[test]
#[cfg(unix)]
#[ignore = "由 children_die_with_a_sigkilled_parent 通过 re-exec 调起"]
fn pdeathsig_child_harness() {
    let Ok(pid_dir) = std::env::var(CHILD_HARNESS_MARKER) else {
        return;
    };
    let wda = FakeWda::start(FakeWdaConfig::healthy());
    let mjpeg = synthetic_mjpeg();
    let mut env = vec![
        ("QUADCONTROL_FAKE_PID_DIR".to_owned(), pid_dir),
        (
            "QUADCONTROL_FAKE_FORWARD_8100".to_owned(),
            wda.port.to_string(),
        ),
        (
            "QUADCONTROL_FAKE_FORWARD_9100".to_owned(),
            mjpeg.to_string(),
        ),
    ];
    // 忽略 SIGINT，确保这些进程不是被别的信号顺手带走的。
    env.push(("QUADCONTROL_FAKE_IGNORE_SIGINT".to_owned(), "1".to_owned()));

    let options = IosLaunchOptions {
        udid: FAKE_UDID.to_owned(),
        mode: LaunchMode::Launch {
            ios: helper(),
            wda: WdaIds {
                bundle_id: "com.example.wda".into(),
                testrunner_id: "com.example.wda.xctrunner".into(),
                xctestconfig: "WebDriverAgentRunner.xctest".into(),
            },
            env,
        },
        tunnel_info_port: unique_tunnel_port(),
        status_budget: Duration::from_secs(30),
    };
    // 本 harness 是**另一个进程**：它的端口计数从头开始，会撞上父测试进程里其它
    // 用例正持有的假隧道端口（CI 上实测 `TunnelPortBusy(28200)`）。同样只对
    // `tunnel_port_busy` 重试，其它错误照样 panic。
    let handle = spawn_with_port_retry(&options, 51);
    wait_for_running(&handle);
    // 长睡等着被 kill -9；绝不自行退出，否则测的就不是 PDEATHSIG。
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
