use adb_probe::{AdbCommand, CommandRunner, ExecutionError, ProbeError, SystemRunner};
use quadcontrol_android::wireless::{self, PairCancel, ROUND_TIMEOUT};
use quadcontrol_android::{SessionHandle, SessionStatus, SHUTDOWN_DEADLINE};
use quadcontrol_ios::session::{
    ExitReason, IosLaunchOptions, IosSessionHandle, IosSessionStatus, LaunchMode, WdaIds,
    IOS_SHUTDOWN_DEADLINE,
};
use quadcontrol_ios::wda::WindowSize;
use serde::Serialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Manager;

#[derive(Debug, Serialize)]
struct DeviceDto {
    id: String,
    name: String,
    platform: String,
    status: String,
    detail: Option<String>,
}

/// 错误以**稳定错误码**回传，不回传原始 stderr：一来 adb/go-ios 的英文原文会直接
/// 出现在中文界面里（违反双语约束），二来它可能带着设备序列号。原文只写进本进程
/// 的 stderr 供开发者排查（机器可读输出保持英文）。
#[derive(Debug, Serialize)]
struct DeviceListDto {
    devices: Vec<DeviceDto>,
    android_error: Option<String>,
    ios_error: Option<String>,
}

struct SessionStore {
    sessions: Mutex<HashMap<String, SessionHandle>>,
    /// Windows 上 `start_session` 直接拒绝（见其文档注释），这个计数器与
    /// `session_error_code` 都用不上——但 CI 是 `-D warnings`，死代码会让
    /// Windows 构建失败，所以显式放行。
    #[cfg_attr(windows, allow(dead_code))]
    next_id: AtomicU64,
}

enum PairingState {
    Idle,
    Waiting {
        generation: u64,
        started: Instant,
    },
    Succeeded {
        generation: u64,
        device_name: String,
    },
    Failed {
        generation: u64,
        code: &'static str,
    },
    Cancelling {
        generation: u64,
    },
}

struct PairingStore {
    state: Mutex<PairingState>,
    cancel: Mutex<Option<PairCancel>>,
    next_generation: AtomicU64,
}

/// 不派生 `Debug`：`modules` 是配对密码的等价编码，一次普通调试日志就能把整张
/// 矩阵打出去——与 `PairingSecret` 去掉 derive(Debug) 是同一个理由。
#[derive(Serialize)]
struct PairingStartDto {
    generation: u64,
    side: usize,
    modules: Vec<bool>,
    total_secs: u64,
}

#[derive(Debug, Serialize)]
struct PairingStatusDto {
    generation: u64,
    state: String,
    remaining_secs: Option<u64>,
    device_name: Option<String>,
    error_code: Option<String>,
}

fn pairing_generation(state: &PairingState) -> u64 {
    match state {
        PairingState::Idle => 0,
        PairingState::Waiting { generation, .. }
        | PairingState::Succeeded { generation, .. }
        | PairingState::Failed { generation, .. }
        | PairingState::Cancelling { generation } => *generation,
    }
}

fn pairing_is_active(state: &PairingState) -> bool {
    matches!(
        state,
        PairingState::Waiting { .. } | PairingState::Cancelling { .. }
    )
}

fn pairing_status_from(state: &PairingState) -> PairingStatusDto {
    match state {
        PairingState::Idle => PairingStatusDto {
            generation: 0,
            state: "idle".into(),
            remaining_secs: None,
            device_name: None,
            error_code: None,
        },
        PairingState::Waiting {
            generation,
            started,
        } => PairingStatusDto {
            generation: *generation,
            state: "waiting".into(),
            remaining_secs: Some(
                ROUND_TIMEOUT
                    .as_secs()
                    .saturating_sub(started.elapsed().as_secs()),
            ),
            device_name: None,
            error_code: None,
        },
        PairingState::Succeeded {
            generation,
            device_name,
        } => PairingStatusDto {
            generation: *generation,
            state: "succeeded".into(),
            remaining_secs: None,
            device_name: Some(device_name.clone()),
            error_code: None,
        },
        PairingState::Failed { generation, code } => PairingStatusDto {
            generation: *generation,
            state: "failed".into(),
            remaining_secs: None,
            device_name: None,
            error_code: Some((*code).into()),
        },
        PairingState::Cancelling { generation } => PairingStatusDto {
            generation: *generation,
            state: "cancelling".into(),
            remaining_secs: None,
            device_name: None,
            error_code: None,
        },
    }
}

fn set_pairing_result(store: &PairingStore, generation: u64, result: Result<String, &'static str>) {
    let Ok(mut state) = store.state.lock() else {
        return;
    };
    if pairing_generation(&state) != generation {
        return;
    }
    if matches!(*state, PairingState::Cancelling { .. }) {
        *state = PairingState::Idle;
        if let Ok(mut cancel) = store.cancel.lock() {
            *cancel = None;
        }
        return;
    }
    *state = match result {
        Ok(name) => PairingState::Succeeded {
            generation,
            device_name: name,
        },
        Err(code) => PairingState::Failed { generation, code },
    };
    if let Ok(mut cancel) = store.cancel.lock() {
        *cancel = None;
    }
}

fn pairing_error_code(error: &wireless::WirelessError) -> &'static str {
    match error {
        wireless::WirelessError::RoundTimedOut | wireless::WirelessError::TimedOut => {
            "pair_timed_out"
        }
        wireless::WirelessError::Cancelled => "pair_cancelled",
        _ => "pair_failed",
    }
}

fn paired_device_name(serial: &str) -> String {
    match quadcontrol_android::devices(&quadcontrol_android::resolve_adb(None)) {
        Ok(devices) => devices
            .into_iter()
            .find(|device| device.serial == serial)
            .and_then(|device| device.model)
            .unwrap_or_else(|| "Android".into()),
        Err(_) => "Android".into(),
    }
}

#[derive(Debug, Serialize)]
struct SessionDto {
    device_id: String,
    state: String,
    exit_code: Option<i32>,
    stderr_tail: Option<String>,
}

#[cfg_attr(windows, allow(dead_code))]
fn session_error_code(error: &quadcontrol_android::Error) -> &'static str {
    match error {
        quadcontrol_android::Error::ScrcpyNotFound(_) => "scrcpy_not_found",
        quadcontrol_android::Error::ScrcpyVersion(_) => "scrcpy_version",
        _ => "session_start_failed",
    }
}

fn truncate_tail(text: String, limit: usize) -> String {
    let mut chars = text.chars().rev().take(limit).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect()
}

fn session_dto(device_id: String, status: SessionStatus) -> SessionDto {
    match status {
        SessionStatus::Running => SessionDto {
            device_id,
            state: "running".into(),
            exit_code: None,
            stderr_tail: None,
        },
        SessionStatus::Stopping => SessionDto {
            device_id,
            state: "stopping".into(),
            exit_code: None,
            stderr_tail: None,
        },
        SessionStatus::Exited { code, .. } => SessionDto {
            device_id,
            state: "exited".into(),
            exit_code: Some(code),
            stderr_tail: None,
        },
        SessionStatus::Failed { .. } => SessionDto {
            device_id,
            state: "failed".into(),
            exit_code: None,
            stderr_tail: Some("session_failed".into()),
        },
    }
}

/// iOS 设备发现走 [`quadcontrol_ios::list_devices`]（P4.1 起是唯一实现）。
///
/// 只保留 `ios_not_found` / `ios_failed` 两个界面码里的第一个作为特例：其余一律
/// 用库的稳定码，界面才能区分「usbmuxd 没跑」这种可自救的场景。
fn list_ios_devices() -> Result<Vec<quadcontrol_ios::IosDevice>, String> {
    let ios = quadcontrol_ios::resolve_ios(None);
    quadcontrol_ios::list_devices(&ios).map_err(|error| {
        eprintln!("go-ios device discovery failed: {error}");
        match error {
            quadcontrol_ios::Error::IosNotFound(_) => "ios_not_found".to_owned(),
            other => other.code().to_owned(),
        }
    })
}

fn android_status(state: &adb_probe::DeviceState) -> &'static str {
    match state {
        adb_probe::DeviceState::Device => "available",
        adb_probe::DeviceState::Unauthorized => "unauthorized",
        adb_probe::DeviceState::Offline => "offline",
        adb_probe::DeviceState::Other(_) => "other",
    }
}

/// 用 adb-probe 的类型化接口而不是 `quadcontrol_android::devices`：后者把
/// `AdbNotFound` / `TimedOut` 拍平成字符串，界面就无法区分「adb 没装」这个
/// 最常见的新用户场景。只读三命令约束不变。
fn list_android_devices() -> Result<Vec<adb_probe::Device>, &'static str> {
    let adb = quadcontrol_android::resolve_adb(None);
    let output = SystemRunner::new(adb.as_os_str())
        .run(AdbCommand::DevicesLong)
        .map_err(|error| match error {
            ExecutionError::AdbNotFound => "adb_not_found",
            ExecutionError::TimedOut(_) => "adb_timed_out",
            other => {
                eprintln!("adb devices -l failed: {other}");
                "adb_failed"
            }
        })?;
    if output.status != 0 {
        eprintln!("adb devices -l exited with {}", output.status);
        return Err("adb_failed");
    }
    adb_probe::parse_devices(&output.stdout).map_err(|error| {
        eprintln!(
            "adb devices -l output rejected: {}",
            match &error {
                ProbeError::InvalidOutput { reason, .. } => reason.as_str(),
                _ => "unexpected probe error",
            }
        );
        "adb_failed"
    })
}

fn list_devices_blocking() -> DeviceListDto {
    let (android_devices, android_error) = match list_android_devices() {
        Ok(devices) => (devices, None),
        Err(code) => (Vec::new(), Some(code.to_owned())),
    };
    let mut devices = android_devices
        .into_iter()
        .map(|device| DeviceDto {
            id: device.serial.clone(),
            name: device.model.unwrap_or(device.serial),
            platform: "android".to_owned(),
            status: android_status(&device.state).to_owned(),
            detail: None,
        })
        .collect::<Vec<_>>();

    // go-ios 列得出来就是可连接：iOS 侧的「配对」是系统级信任，不是我们的流程。
    let ios_error = match list_ios_devices() {
        Ok(ios_devices) => {
            devices.extend(ios_devices.into_iter().map(|device| DeviceDto {
                id: device.id,
                name: device.name,
                platform: "ios".to_owned(),
                status: "available".to_owned(),
                detail: None,
            }));
            None
        }
        Err(code) => Some(code),
    };

    DeviceListDto {
        devices,
        android_error,
        ios_error,
    }
}

// ---------------------------------------------------------------------------
// iPhone 控制面板（界面 C）
// ---------------------------------------------------------------------------

/// WDA 的三个标识默认值。
///
/// **未真机校准**：这些是 go-ios 与 WebDriverAgent 的上游默认值，而个人开发者
/// 团队签不了 `com.facebook.*`，所以真实使用几乎一定要用下面三个环境变量覆盖。
/// 设置界面归 P7。
const DEFAULT_WDA_BUNDLE_ID: &str = "com.facebook.WebDriverAgentRunner.xctrunner";
const DEFAULT_WDA_TESTRUNNER_ID: &str = "com.facebook.WebDriverAgentRunner.xctrunner";
const DEFAULT_WDA_XCTESTCONFIG: &str = "WebDriverAgentRunner.xctest";

/// macOS 的系统「iPhone 镜像」。存在与否 = 系统版本够不够（macOS 15+）。
const IPHONE_MIRRORING_APP: &str = "/System/Applications/iPhone Mirroring.app";

/// 同一时刻最多一个 iOS 会话（跨设备）。
///
/// 不是偷懒：go-ios 的隧道信息服务是每主机一份，多隧道行为我们没验证过
/// （`IOS_PANEL_PLAN.md` §一）。多设备是 P4 之后的事。
struct IosSessionStore {
    session: Mutex<Option<IosSlot>>,
    next_id: AtomicU64,
    meta: Mutex<IosMeta>,
}

/// 会话槽位。
///
/// 有 `Starting` 占位而不是「锁外 None、成功后再放句柄」，是因为
/// `spawn_supervised` 的预检（`ios version` + 端口探测）要跑在 `spawn_blocking`
/// 里，那段时间 store 一旦是 None 就有两个后果：1 秒轮询读到 idle，把前端的
/// 会话归属清掉，会话变成无主的；两次并发 `start_ios_session` 都能穿过检查，
/// **真的起两条 tunnel**。占位让第二次调用在窗口内就命中
/// `ios_session_already_running`。
enum IosSlot {
    /// 预检与 spawn 还没回来。
    Starting,
    /// 起会话失败；停在这里直到下一次 start 覆盖它，界面才有错误码可显示。
    Failed {
        code: &'static str,
    },
    Handle(IosSessionHandle),
}

/// 槽位对外的状态。占位与失败都能用库的状态枚举表达，DTO 映射因此只有一套。
fn slot_status(slot: &IosSlot) -> IosSessionStatus {
    match slot {
        IosSlot::Starting => IosSessionStatus::Starting,
        IosSlot::Failed { code } => IosSessionStatus::Failed { code },
        IosSlot::Handle(handle) => handle.status(),
    }
}

fn ios_slot_is_live(slot: &IosSlot) -> bool {
    match slot {
        IosSlot::Starting => true,
        IosSlot::Failed { .. } => false,
        IosSlot::Handle(handle) => ios_session_is_live(&handle.status()),
    }
}

/// 会话的派生量：帧率差分的上一帧快照与最近一次窗口尺寸。
///
/// 窗口尺寸要缓存，是因为前端按同一个值给画面容器定尺；`ios_tap` 用缓存值算
/// 内容矩形，前后端才对得上同一个 letterbox。
#[derive(Default)]
struct IosMeta {
    frames: Option<(u64, Instant)>,
    fps: u32,
    window: Option<WindowSize>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct WindowDto {
    width: u32,
    height: u32,
}

/// 会话状态 DTO。
///
/// **绝不含 `stderr_tail` 与 UDID**：iOS 子进程的 stderr 尾部带着 UDID 与
/// bundle id（`IOS_PANEL_PLAN.md` §二的红线），只写本进程 stderr。
#[derive(Debug, Serialize, PartialEq, Eq)]
struct IosStatusDto {
    state: String,
    code: Option<String>,
    exit_reason: Option<String>,
    proxy_port: Option<u16>,
    fps: u32,
    locked: Option<bool>,
    window: Option<WindowDto>,
}

fn ios_idle_dto() -> IosStatusDto {
    IosStatusDto {
        state: "idle".into(),
        code: None,
        exit_reason: None,
        proxy_port: None,
        fps: 0,
        locked: None,
        window: None,
    }
}

fn ios_status_dto(status: &IosSessionStatus) -> IosStatusDto {
    let mut dto = ios_idle_dto();
    match status {
        IosSessionStatus::Starting => dto.state = "starting".into(),
        IosSessionStatus::Running { proxy_port } => {
            dto.state = "running".into();
            dto.proxy_port = Some(*proxy_port);
        }
        IosSessionStatus::Stopping => dto.state = "stopping".into(),
        IosSessionStatus::Exited { reason } => {
            dto.state = "exited".into();
            dto.exit_reason = Some(
                match reason {
                    ExitReason::Requested => "requested",
                    ExitReason::ChildExited { .. } => "child_exited",
                    ExitReason::UpstreamClosed => "upstream_closed",
                }
                .into(),
            );
        }
        IosSessionStatus::Failed { code } => {
            dto.state = "failed".into();
            dto.code = Some((*code).into());
        }
    }
    dto
}

/// 两次计帧快照的差分。
///
/// 间隔过短就沿用上一次的值：1 秒轮询偶尔会被前一次的 HTTP 超时挤在一起，
/// 除以一个接近零的间隔会得到荒谬的帧率。
fn diff_fps(previous: Option<(u64, Instant)>, frames: u64, now: Instant, last: u32) -> u32 {
    let Some((previous_frames, previous_at)) = previous else {
        return 0;
    };
    let elapsed = now.saturating_duration_since(previous_at);
    if elapsed < Duration::from_millis(200) {
        return last;
    }
    let delta = frames.saturating_sub(previous_frames) as f64;
    (delta / elapsed.as_secs_f64()).round() as u32
}

/// 容器内像素坐标 → 设备点坐标。
///
/// 画面是 `object-fit: contain`，容器与设备长宽比不同时必然留黑边；按容器尺寸
/// 直接线性换算会让点按整体偏移。这里先算内容矩形，再归一化，最后乘设备点尺寸。
/// 落在黑边上的点返回 None（调用方不转发）。
fn content_point(
    x_px: f64,
    y_px: f64,
    container_w: f64,
    container_h: f64,
    window: WindowSize,
) -> Option<(f64, f64)> {
    if container_w <= 0.0 || container_h <= 0.0 || window.width == 0 || window.height == 0 {
        return None;
    }
    let (window_w, window_h) = (f64::from(window.width), f64::from(window.height));
    let scale = (container_w / window_w).min(container_h / window_h);
    let (content_w, content_h) = (window_w * scale, window_h * scale);
    let normalized_x = (x_px - (container_w - content_w) / 2.0) / content_w;
    let normalized_y = (y_px - (container_h - content_h) / 2.0) / content_h;
    if !(0.0..=1.0).contains(&normalized_x) || !(0.0..=1.0).contains(&normalized_y) {
        return None;
    }
    Some((normalized_x * window_w, normalized_y * window_h))
}

/// 取一个可用的 WDA 客户端；会话没在跑就是 `ios_session_not_running`。
///
/// 只在锁内做 `client()`（克隆很便宜），**绝不持锁发 HTTP**：一次 5 秒超时的
/// 请求会把整个状态轮询一起堵住。
fn ios_client(store: &IosSessionStore) -> Result<quadcontrol_ios::wda::Client, String> {
    let session = store
        .session
        .lock()
        .map_err(|_| "ios_session_not_running".to_owned())?;
    session
        .as_ref()
        .and_then(|slot| match slot {
            IosSlot::Handle(handle) => handle.client(),
            _ => None,
        })
        .ok_or_else(|| "ios_session_not_running".to_owned())
}

/// 在阻塞线程池里跑一次 WDA 调用。
async fn ios_call<T, F>(store: &IosSessionStore, call: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&quadcontrol_ios::wda::Client) -> Result<T, quadcontrol_ios::Error> + Send + 'static,
{
    let client = ios_client(store)?;
    tauri::async_runtime::spawn_blocking(move || call(&client))
        .await
        .map_err(|error| {
            eprintln!("iOS command worker failed: {error}");
            "ios_process_error".to_owned()
        })?
        .map_err(|error| {
            eprintln!("iOS command failed: {error}");
            error.code().to_owned()
        })
}

/// 截图落盘目录：系统「图片」目录 → home → 报错。
fn screenshot_path(now: chrono::DateTime<chrono::Local>) -> Result<std::path::PathBuf, String> {
    let directory = dirs::picture_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "screenshot_failed".to_owned())?;
    Ok(directory.join(format!("QuadControl-{}.png", now.format("%Y%m%d-%H%M%S"))))
}

/// 起 iOS 会话的平台闸门。
///
/// 用运行时 `cfg!` 而不是 `#[cfg]` 分支：后者会让另外两个平台上的整段代码变成
/// 死代码，而 CI 是 `-D warnings`。
fn ios_launch_allowed() -> Result<(), String> {
    if cfg!(windows) {
        // Job Object 之前收不回进程树，与 Android 同理直接拒绝（P6）。
        return Err("windows_session_unsupported".into());
    }
    if cfg!(target_os = "macos") {
        // macOS 的产品路径是系统「iPhone 镜像」引导卡，不起 WDA 链路。仅 debug
        // 构建 + 显式环境变量时放行，用于本机真机验收；界面上也不显示这个按钮，
        // 这里是第二道闸。
        let dev_path = cfg!(debug_assertions)
            && std::env::var("QUADCONTROL_IOS_DEV_WDA").as_deref() == Ok("1");
        if !dev_path {
            return Err("macos_uses_iphone_mirroring".into());
        }
    }
    Ok(())
}

fn wda_ids_from_env() -> WdaIds {
    let read = |name: &str, fallback: &str| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| fallback.to_owned())
    };
    WdaIds {
        bundle_id: read("QUADCONTROL_WDA_BUNDLE_ID", DEFAULT_WDA_BUNDLE_ID),
        testrunner_id: read("QUADCONTROL_WDA_TESTRUNNER_ID", DEFAULT_WDA_TESTRUNNER_ID),
        xctestconfig: read("QUADCONTROL_WDA_XCTESTCONFIG", DEFAULT_WDA_XCTESTCONFIG),
    }
}

/// 换槽位；锁被毒化时只记日志——启动失败的收尾不该再 panic 一次。
fn set_ios_slot(store: &IosSessionStore, slot: IosSlot) {
    match store.session.lock() {
        Ok(mut session) => *session = Some(slot),
        Err(_) => eprintln!("iOS session store is poisoned; slot was not updated"),
    }
}

fn ios_session_is_live(status: &IosSessionStatus) -> bool {
    matches!(
        status,
        IosSessionStatus::Starting | IosSessionStatus::Running { .. } | IosSessionStatus::Stopping
    )
}

#[tauri::command]
async fn host_platform() -> Result<String, String> {
    Ok(if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(windows) {
        "windows"
    } else {
        "linux"
    }
    .to_owned())
}

/// macOS 开发路径是否放行（debug 构建 + `QUADCONTROL_IOS_DEV_WDA=1`）。
///
/// 前端据此在 macOS 上切到 Win/Linux 的完整面板版式，这是方案 §四真机验收
/// 步骤 2–3 的唯一入口；release 构建里恒为 false，产品路径仍只有引导卡。
#[tauri::command]
async fn ios_dev_wda_enabled() -> Result<bool, String> {
    Ok(cfg!(target_os = "macos") && ios_launch_allowed().is_ok())
}

#[tauri::command]
async fn start_ios_session(
    store: tauri::State<'_, IosSessionStore>,
    device_id: String,
) -> Result<(), String> {
    ios_launch_allowed()?;
    let store = store.inner();
    let id = {
        let mut session = store
            .session
            .lock()
            .map_err(|_| "ios_process_error".to_owned())?;
        if let Some(existing) = session.as_ref() {
            if ios_slot_is_live(existing) {
                return Err("ios_session_already_running".into());
            }
        }
        // 上一个已经是 Exited / Failed：换成本次的占位。
        *session = Some(IosSlot::Starting);
        store.next_id.fetch_add(1, Ordering::Relaxed)
    };
    // GUI 只起完整链路；`LaunchMode::Attach` 只存在于库与其测试里（方案 §三）。
    let options = IosLaunchOptions::new(
        device_id,
        LaunchMode::Launch {
            ios: quadcontrol_ios::resolve_ios(None),
            wda: wda_ids_from_env(),
            env: Vec::new(),
        },
    );
    // `spawn_supervised` 会跑 `ios version` 与端口探测（阻塞），不能占着 async
    // 运行时，也不能在持锁期间做。
    let spawned = tauri::async_runtime::spawn_blocking(move || {
        quadcontrol_ios::session::spawn_supervised(&options, id)
    })
    .await;
    // 不管走哪条路，占位都必须被换掉，否则 store 永远停在 `Starting`，
    // 界面也就永远停在「正在启动会话…」。
    let handle = match spawned {
        Err(error) => {
            eprintln!("iOS session start worker failed: {error}");
            set_ios_slot(
                store,
                IosSlot::Failed {
                    code: "ios_process_error",
                },
            );
            return Err("ios_process_error".into());
        }
        Ok(Err(error)) => {
            eprintln!("iOS session start failed: {error}");
            let code = error.code();
            set_ios_slot(store, IosSlot::Failed { code });
            return Err(code.to_owned());
        }
        Ok(Ok(handle)) => handle,
    };
    let mut session = store
        .session
        .lock()
        .map_err(|_| "ios_process_error".to_owned())?;
    if let Some(IosSlot::Handle(existing)) = session.as_ref() {
        // 占位本该挡住并发的第二次调用；真到了这里说明有别的路径塞了句柄进来，
        // 宁可把刚起的这份收回，也不能让两条 WDA 链路同时活着。
        if ios_session_is_live(&existing.status()) {
            let _ = handle.request_stop();
            return Err("ios_session_already_running".into());
        }
    }
    *session = Some(IosSlot::Handle(handle));
    if let Ok(mut meta) = store.meta.lock() {
        *meta = IosMeta::default();
    }
    Ok(())
}

#[tauri::command]
async fn stop_ios_session(store: tauri::State<'_, IosSessionStore>) -> Result<(), String> {
    let store = store.inner();
    let session = store
        .session
        .lock()
        .map_err(|_| "ios_process_error".to_owned())?;
    match session.as_ref() {
        Some(IosSlot::Handle(handle)) => handle.request_stop().map_err(|error| {
            eprintln!("iOS session stop failed: {error}");
            "ios_process_error".to_owned()
        })?,
        // 占位期间还没有监督线程可停：预检只有几百毫秒，等它换成句柄后再停。
        // 这里不报错，免得界面把「还没起完」显示成失败。
        Some(IosSlot::Starting) => eprintln!("iOS session is still starting; stop was ignored"),
        Some(IosSlot::Failed { .. }) | None => {}
    }
    Ok(())
}

#[tauri::command]
async fn ios_session_status(
    store: tauri::State<'_, IosSessionStore>,
) -> Result<IosStatusDto, String> {
    let store = store.inner();
    let snapshot = {
        let session = store
            .session
            .lock()
            .map_err(|_| "ios_process_error".to_owned())?;
        session.as_ref().map(|slot| match slot {
            IosSlot::Handle(handle) => (handle.status(), handle.stats().frames, handle.client()),
            placeholder => (slot_status(placeholder), 0, None),
        })
    };
    let Some((status, frames, client)) = snapshot else {
        return Ok(ios_idle_dto());
    };
    let mut dto = ios_status_dto(&status);
    if dto.state != "running" {
        if let Ok(mut meta) = store.meta.lock() {
            *meta = IosMeta::default();
        }
        return Ok(dto);
    }
    let now = Instant::now();
    if let Ok(mut meta) = store.meta.lock() {
        meta.fps = diff_fps(meta.frames, frames, now, meta.fps);
        meta.frames = Some((frames, now));
        dto.fps = meta.fps;
        dto.window = meta.window.map(|window| WindowDto {
            width: window.width,
            height: window.height,
        });
    }
    if let Some(client) = client {
        // 两次查询各自带 5 秒超时（`wda::REQUEST_TIMEOUT`）；查不到就保持原值，
        // 不把一次瞬时超时升级成「会话失败」。
        let (locked, window) = tauri::async_runtime::spawn_blocking(move || {
            (client.locked().ok(), client.window_size().ok())
        })
        .await
        .unwrap_or((None, None));
        dto.locked = locked;
        if let Some(window) = window {
            dto.window = Some(WindowDto {
                width: window.width,
                height: window.height,
            });
            if let Ok(mut meta) = store.meta.lock() {
                meta.window = Some(window);
            }
        }
    }
    Ok(dto)
}

#[tauri::command]
async fn ios_tap(
    store: tauri::State<'_, IosSessionStore>,
    x_px: f64,
    y_px: f64,
    width_px: f64,
    height_px: f64,
) -> Result<(), String> {
    let store = store.inner();
    // 用状态轮询缓存的窗口尺寸：前端的画面容器按同一个值定尺，两边才算出同一个
    // 内容矩形。缓存为空（刚起、还没轮询到）才现查一次。
    let cached = store.meta.lock().ok().and_then(|meta| meta.window);
    let client = ios_client(store)?;
    tauri::async_runtime::spawn_blocking(move || {
        let window = match cached {
            Some(window) => window,
            None => client.window_size()?,
        };
        match content_point(x_px, y_px, width_px, height_px, window) {
            // 点在黑边上：不转发，也不是错误。
            None => Ok(()),
            Some((x_pt, y_pt)) => client.tap(x_pt, y_pt),
        }
    })
    .await
    .map_err(|error| {
        eprintln!("iOS tap worker failed: {error}");
        "ios_process_error".to_owned()
    })?
    .map_err(|error| {
        eprintln!("iOS tap failed: {error}");
        error.code().to_owned()
    })
}

#[tauri::command]
async fn ios_send_text(
    store: tauri::State<'_, IosSessionStore>,
    text: String,
) -> Result<(), String> {
    ios_call(store.inner(), move |client| client.send_text(&text)).await
}

#[tauri::command]
async fn ios_home(store: tauri::State<'_, IosSessionStore>) -> Result<(), String> {
    ios_call(store.inner(), |client| client.home()).await
}

#[tauri::command]
async fn ios_wake(store: tauri::State<'_, IosSessionStore>) -> Result<(), String> {
    ios_call(store.inner(), |client| client.wake()).await
}

#[tauri::command]
async fn ios_screenshot(store: tauri::State<'_, IosSessionStore>) -> Result<String, String> {
    let png = ios_call(store.inner(), |client| client.screenshot_png()).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = screenshot_path(chrono::Local::now())?;
        std::fs::write(&path, png).map_err(|error| {
            eprintln!("screenshot could not be written: {error}");
            "screenshot_failed".to_owned()
        })?;
        Ok(path.display().to_string())
    })
    .await
    .map_err(|error| {
        eprintln!("screenshot worker failed: {error}");
        "screenshot_failed".to_owned()
    })?
}

#[tauri::command]
async fn iphone_mirroring_available() -> Result<bool, String> {
    Ok(cfg!(target_os = "macos") && std::path::Path::new(IPHONE_MIRRORING_APP).exists())
}

/// 只是把系统应用调到前台。**不驱动、不注入、不自动化**它（方案 §一）。
#[tauri::command]
async fn open_iphone_mirroring() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("unsupported".into());
    }
    let status = tauri::async_runtime::spawn_blocking(|| {
        std::process::Command::new("open")
            .args(["-b", "com.apple.ScreenContinuity"])
            .status()
    })
    .await
    .map_err(|error| {
        eprintln!("iPhone Mirroring worker failed: {error}");
        "iphone_mirroring_failed".to_owned()
    })?
    .map_err(|error| {
        eprintln!("iPhone Mirroring could not be opened: {error}");
        "iphone_mirroring_failed".to_owned()
    })?;
    if status.success() {
        Ok(())
    } else {
        eprintln!("`open -b com.apple.ScreenContinuity` exited with {status}");
        Err("iphone_mirroring_failed".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        content_point, diff_fps, ios_slot_is_live, ios_status_dto, pairing_status_from,
        screenshot_path, set_pairing_result, slot_status, wda_ids_from_env, ExitReason,
        IosSessionStatus, IosSlot, PairingState, PairingStore, WindowDto, WindowSize,
    };
    use chrono::TimeZone;
    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    #[test]
    fn manual_pair_inputs_require_ip_literal_port_and_six_digits() {
        assert!(super::validate_manual_inputs("192.0.2.1", 37145, "123456").is_ok());
        assert!(super::validate_manual_inputs("example.invalid", 37145, "123456").is_err());
        assert!(super::validate_manual_inputs("192.0.2.1", 0, "123456").is_err());
        assert!(super::validate_manual_inputs("192.0.2.1", 37145, "12345x").is_err());
        assert!(super::validate_manual_inputs("192.0.2.1", 37145, "12345").is_err());
    }

    #[test]
    fn late_worker_result_cannot_overwrite_new_generation() {
        let store = PairingStore {
            state: Mutex::new(PairingState::Waiting {
                generation: 2,
                started: Instant::now(),
            }),
            cancel: Mutex::new(None),
            next_generation: AtomicU64::new(2),
        };
        set_pairing_result(&store, 1, Ok("old".into()));
        assert_eq!(
            pairing_status_from(&store.state.lock().unwrap()).generation,
            2
        );
        assert_eq!(
            pairing_status_from(&store.state.lock().unwrap()).state,
            "waiting"
        );
    }

    /// iPhone SE 3 的点尺寸；letterbox 的三种形态都用它当被控设备。
    const SE3: WindowSize = WindowSize {
        width: 375,
        height: 667,
    };

    fn approx(actual: (f64, f64), expected: (f64, f64)) {
        assert!(
            (actual.0 - expected.0).abs() < 0.01 && (actual.1 - expected.1).abs() < 0.01,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn tap_maps_through_the_content_rect_without_letterbox() {
        // 容器与设备同比例：中心点还是中心点，右下角还是右下角。
        approx(
            content_point(187.5, 333.5, 375.0, 667.0, SE3).unwrap(),
            (187.5, 333.5),
        );
        approx(
            content_point(375.0, 667.0, 375.0, 667.0, SE3).unwrap(),
            (375.0, 667.0),
        );
    }

    #[test]
    fn tap_accounts_for_pillarbox_bars_on_the_sides() {
        // 容器 500×667 比设备宽：内容 375×667 居中，左右各 62.5px 黑边。
        approx(
            content_point(250.0, 333.5, 500.0, 667.0, SE3).unwrap(),
            (187.5, 333.5),
        );
        approx(
            content_point(62.5, 0.0, 500.0, 667.0, SE3).unwrap(),
            (0.0, 0.0),
        );
        // 左右黑边上的点不转发。
        assert!(content_point(10.0, 333.5, 500.0, 667.0, SE3).is_none());
        assert!(content_point(490.0, 333.5, 500.0, 667.0, SE3).is_none());
    }

    #[test]
    fn tap_accounts_for_letterbox_bars_above_and_below() {
        // 容器 375×867 比设备高：内容 375×667 居中，上下各 100px 黑边。
        approx(
            content_point(187.5, 433.5, 375.0, 867.0, SE3).unwrap(),
            (187.5, 333.5),
        );
        approx(
            content_point(0.0, 100.0, 375.0, 867.0, SE3).unwrap(),
            (0.0, 0.0),
        );
        assert!(content_point(187.5, 50.0, 375.0, 867.0, SE3).is_none());
        assert!(content_point(187.5, 820.0, 375.0, 867.0, SE3).is_none());
    }

    #[test]
    fn tap_rejects_degenerate_containers_and_windows() {
        assert!(content_point(1.0, 1.0, 0.0, 667.0, SE3).is_none());
        assert!(content_point(1.0, 1.0, 375.0, 0.0, SE3).is_none());
        assert!(content_point(
            1.0,
            1.0,
            375.0,
            667.0,
            WindowSize {
                width: 0,
                height: 0
            }
        )
        .is_none());
    }

    #[test]
    fn fps_is_the_frame_delta_over_the_elapsed_time() {
        let start = Instant::now();
        // 第一次没有上一帧快照：0，而不是把总帧数当帧率。
        assert_eq!(diff_fps(None, 90, start, 0), 0);
        assert_eq!(
            diff_fps(Some((90, start)), 105, start + Duration::from_secs(1), 0),
            15
        );
        // 半秒 8 帧 = 16 fps。
        assert_eq!(
            diff_fps(
                Some((90, start)),
                98,
                start + Duration::from_millis(500),
                15
            ),
            16
        );
        // 间隔过短：沿用上一次的值，不除以接近零的时间。
        assert_eq!(
            diff_fps(Some((90, start)), 91, start + Duration::from_millis(10), 15),
            15
        );
        // 代理重启后帧数回退不能变成负数。
        assert_eq!(
            diff_fps(Some((90, start)), 3, start + Duration::from_secs(1), 15),
            0
        );
    }

    #[test]
    fn status_dto_never_carries_diagnostics() {
        let starting = ios_status_dto(&IosSessionStatus::Starting);
        assert_eq!(starting.state, "starting");
        assert_eq!(starting.proxy_port, None);

        let running = ios_status_dto(&IosSessionStatus::Running { proxy_port: 51234 });
        assert_eq!(running.state, "running");
        assert_eq!(running.proxy_port, Some(51234));
        assert_eq!(running.locked, None);
        assert_eq!(running.window, None);

        assert_eq!(
            ios_status_dto(&IosSessionStatus::Stopping).state,
            "stopping"
        );

        let exited = ios_status_dto(&IosSessionStatus::Exited {
            reason: ExitReason::UpstreamClosed,
        });
        assert_eq!(exited.state, "exited");
        assert_eq!(exited.exit_reason.as_deref(), Some("upstream_closed"));
        assert_eq!(exited.code, None);

        let failed = ios_status_dto(&IosSessionStatus::Failed {
            code: "wda_unreachable",
        });
        assert_eq!(failed.state, "failed");
        assert_eq!(failed.code.as_deref(), Some("wda_unreachable"));
    }

    /// DTO 序列化里**不能**出现 stderr 尾部或 UDID：iOS 子进程的 stderr 带着
    /// 设备标识与 bundle id（方案 §二红线）。这条测试守的是字段表本身。
    #[test]
    fn status_dto_serializes_only_the_allowed_fields() {
        let json = serde_json::to_string(&super::IosStatusDto {
            state: "running".into(),
            code: None,
            exit_reason: None,
            proxy_port: Some(51234),
            fps: 15,
            locked: Some(false),
            window: Some(WindowDto {
                width: 375,
                height: 667,
            }),
        })
        .unwrap();
        assert!(!json.contains("stderr"), "{json}");
        assert!(!json.contains("udid"), "{json}");
        assert!(json.contains("\"proxy_port\":51234"), "{json}");
    }

    #[test]
    fn screenshot_file_name_carries_a_sortable_timestamp() {
        let at = chrono::Local
            .with_ymd_and_hms(2026, 9, 14, 8, 5, 3)
            .unwrap();
        let path = screenshot_path(at).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "QuadControl-20260914-080503.png"
        );
    }

    /// 占位与失败槽位要映射成界面认得的状态。
    ///
    /// `IosSlot::Handle` 这一支没法在单测里构造（要真的起子进程），它只是把
    /// `IosSessionHandle::status()` 原样转出去，覆盖在库的 session 测试里。
    #[test]
    fn slot_placeholders_map_to_starting_and_failed() {
        assert_eq!(slot_status(&IosSlot::Starting), IosSessionStatus::Starting);
        assert_eq!(
            ios_status_dto(&slot_status(&IosSlot::Starting)).state,
            "starting"
        );

        let failed = slot_status(&IosSlot::Failed {
            code: "wda_signature_expired",
        });
        let dto = ios_status_dto(&failed);
        assert_eq!(dto.state, "failed");
        assert_eq!(dto.code.as_deref(), Some("wda_signature_expired"));
    }

    /// 占位算「在跑」——第二次并发 start 必须在预检窗口内就被拒。
    #[test]
    fn starting_placeholder_blocks_a_second_start() {
        assert!(ios_slot_is_live(&IosSlot::Starting));
        assert!(!ios_slot_is_live(&IosSlot::Failed {
            code: "tunnel_failed"
        }));
    }

    /// 环境变量为空串时回退默认值，而不是把空 bundle id 交给 go-ios。
    #[test]
    fn wda_ids_fall_back_to_defaults_for_blank_environment_values() {
        let names = [
            "QUADCONTROL_WDA_BUNDLE_ID",
            "QUADCONTROL_WDA_TESTRUNNER_ID",
            "QUADCONTROL_WDA_XCTESTCONFIG",
        ];
        let previous = names.map(std::env::var_os);
        for name in names {
            std::env::set_var(name, "   ");
        }
        let ids = wda_ids_from_env();
        assert_eq!(ids.bundle_id, super::DEFAULT_WDA_BUNDLE_ID);
        assert_eq!(ids.testrunner_id, super::DEFAULT_WDA_TESTRUNNER_ID);
        assert_eq!(ids.xctestconfig, super::DEFAULT_WDA_XCTESTCONFIG);

        std::env::set_var("QUADCONTROL_WDA_BUNDLE_ID", "com.example.wda.xctrunner");
        assert_eq!(
            wda_ids_from_env().bundle_id,
            "com.example.wda.xctrunner".to_owned()
        );
        for (name, value) in names.iter().zip(previous) {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    /// macOS 的产品路径没有 WDA 链路：没有开发开关就必须拒绝。
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_refuses_to_launch_without_the_development_switch() {
        let previous = std::env::var_os("QUADCONTROL_IOS_DEV_WDA");
        std::env::remove_var("QUADCONTROL_IOS_DEV_WDA");
        assert_eq!(
            super::ios_launch_allowed(),
            Err("macos_uses_iphone_mirroring".to_owned())
        );
        if let Some(value) = previous {
            std::env::set_var("QUADCONTROL_IOS_DEV_WDA", value);
        }
    }

    /// Windows 上收不回进程树，起会话是**有意**拒绝（P6）。
    #[cfg(windows)]
    #[test]
    fn windows_refuses_to_launch() {
        assert_eq!(
            super::ios_launch_allowed(),
            Err("windows_session_unsupported".to_owned())
        );
    }

    #[test]
    fn pairing_status_countdown_uses_backend_start_time() {
        let status = pairing_status_from(&PairingState::Waiting {
            generation: 7,
            started: Instant::now() - Duration::from_secs(2),
        });
        assert!(status.remaining_secs.unwrap() <= 298);
        assert!(status.remaining_secs.unwrap() > 0);
    }
}

#[tauri::command]
async fn list_devices() -> Result<DeviceListDto, String> {
    tauri::async_runtime::spawn_blocking(list_devices_blocking)
        .await
        .map_err(|error| format!("device discovery worker failed: {error}"))
}

#[tauri::command]
async fn start_pairing(
    app: tauri::AppHandle,
    store: tauri::State<'_, PairingStore>,
) -> Result<PairingStartDto, String> {
    let store = store.inner();
    let secret = wireless::generate_pairing_secret();
    let matrix = wireless::qr_matrix(&secret.payload).map_err(|_| "pair_failed".to_owned())?;
    let cancel = PairCancel::new();
    let generation = store.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut state = store.state.lock().map_err(|_| "pair_failed".to_owned())?;
        if pairing_is_active(&state) {
            return Err("pairing_already_running".into());
        }
        *state = PairingState::Waiting {
            generation,
            started: Instant::now(),
        };
        *store.cancel.lock().map_err(|_| "pair_failed".to_owned())? = Some(cancel.clone());
    }
    // Tauri's blocking pool owns the ADB work; the managed app state remains
    // alive for the lifetime of this worker.
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            let adb = quadcontrol_android::resolve_adb(None);
            let outcome = wireless::pair_qr_command_with_cancel(&adb, secret, |_| {}, &cancel);
            match outcome {
                Ok((serial, _, _)) => Ok(paired_device_name(&serial)),
                Err(error) => Err(pairing_error_code(&error)),
            }
        })
        .await;
        let result = match result {
            Ok(result) => result,
            Err(_) => Err("pair_failed"),
        };
        set_pairing_result(&app.state::<PairingStore>(), generation, result);
    });
    Ok(PairingStartDto {
        generation,
        side: matrix.side,
        modules: matrix.modules,
        total_secs: ROUND_TIMEOUT.as_secs(),
    })
}

#[tauri::command]
async fn pairing_status(store: tauri::State<'_, PairingStore>) -> Result<PairingStatusDto, String> {
    let store = store.inner();
    let state = store.state.lock().map_err(|_| "pair_failed".to_owned())?;
    Ok(pairing_status_from(&state))
}

#[tauri::command]
async fn cancel_pairing(store: tauri::State<'_, PairingStore>) -> Result<(), String> {
    let store = store.inner();
    let mut state = store.state.lock().map_err(|_| "pair_failed".to_owned())?;
    if let PairingState::Waiting { generation, .. } = *state {
        if let Some(cancel) = store
            .cancel
            .lock()
            .map_err(|_| "pair_failed".to_owned())?
            .as_ref()
        {
            cancel.cancel();
        }
        *state = PairingState::Cancelling { generation };
    }
    Ok(())
}

fn valid_pair_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

fn validate_manual_inputs(host: &str, port: u16, code: &str) -> Result<(), &'static str> {
    if !valid_pair_host(host) {
        return Err("pair_failed");
    }
    if port == 0 || !code.as_bytes().iter().all(u8::is_ascii_digit) || code.len() != 6 {
        return Err("pair_failed");
    }
    Ok(())
}

/// 手动配对与二维码配对**共用同一个状态机与取消令牌**。
///
/// 早期实现让它另起一个不在 `PairingStore` 里的 worker，于是：两路 `adb pair`
/// 可以并发、退出清理够不着它、它还能带着 300 秒的上线等待一直跑下去。配对是
/// 有状态的设备写操作，必须只有一条在飞、且始终可取消、可回收。
#[tauri::command]
async fn pair_manual(
    app: tauri::AppHandle,
    store: tauri::State<'_, PairingStore>,
    host: String,
    port: u16,
    mut code: String,
) -> Result<u64, String> {
    if validate_manual_inputs(&host, port, &code).is_err() {
        wireless::wipe_secret_string(&mut code);
        return Err("pair_failed".into());
    }
    let store = store.inner();
    let cancel = PairCancel::new();
    let generation = store.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut state = store.state.lock().map_err(|_| "pair_failed".to_owned())?;
        if pairing_is_active(&state) {
            wireless::wipe_secret_string(&mut code);
            return Err("pairing_already_running".into());
        }
        *state = PairingState::Waiting {
            generation,
            started: Instant::now(),
        };
        *store.cancel.lock().map_err(|_| "pair_failed".to_owned())? = Some(cancel.clone());
    }
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            let adb = quadcontrol_android::resolve_adb(None);
            let endpoint = if host.contains(':') {
                format!("[{host}]:{port}")
            } else {
                format!("{host}:{port}")
            };
            let paired = wireless::pair(&adb, &endpoint, &code);
            wireless::wipe_secret_string(&mut code);
            let guid = paired.map_err(|_| "pair_failed")?;
            // 配对端口 ≠ 连接端口：上线失败不代表配对失败，交给界面引导用户
            // 另行输入连接端口，绝不拿配对端口去 connect。
            match wireless::wait_for_guid_with_cancel(&adb, &guid, ROUND_TIMEOUT, &cancel) {
                Ok(serial) => Ok(paired_device_name(&serial)),
                Err(_) if cancel.is_cancelled() => Err("pair_cancelled"),
                Err(_) => Err("manual_pair_needs_connect_port"),
            }
        })
        .await;
        let result = match result {
            Ok(result) => result,
            Err(_) => Err("pair_failed"),
        };
        set_pairing_result(&app.state::<PairingStore>(), generation, result);
    });
    // 只表示 worker 已起飞；真正的结果由界面轮询 `pairing_status` 按 generation 取。
    Ok(generation)
}

/// Windows 上**有意不提供**会话控制，而不是尚未实现。
///
/// 「所有会话可见、可断开」是我们对用户的安全边界承诺。Windows 上要真正收回
/// scrcpy 的整棵进程树需要 Job Object（kill-on-close），那是 P6 的内容且本项目
/// 目前没有 Windows 验证环境。没有它，「断开连接」可能留下仍在控制手机的孤儿
/// 进程——与其假装支持，不如明确拒绝。
#[cfg(windows)]
#[tauri::command]
#[allow(unused_variables)]
async fn start_session(
    store: tauri::State<'_, SessionStore>,
    device_id: String,
    screen_off: bool,
    audio_on_computer: bool,
) -> Result<(), String> {
    Err("windows_session_unsupported".into())
}

#[cfg(not(windows))]
#[tauri::command]
async fn start_session(
    store: tauri::State<'_, SessionStore>,
    device_id: String,
    screen_off: bool,
    audio_on_computer: bool,
) -> Result<(), String> {
    let store = store.inner();
    let id = {
        let mut sessions = store
            .sessions
            .lock()
            .map_err(|_| "session_start_failed".to_owned())?;
        if let Some(existing) = sessions.get(&device_id) {
            if matches!(
                existing.status(),
                SessionStatus::Running | SessionStatus::Stopping
            ) {
                return Err("session_already_running".into());
            }
        }
        sessions.remove(&device_id);
        store.next_id.fetch_add(1, Ordering::Relaxed)
    };
    let options = quadcontrol_android::LaunchOptions {
        adb: quadcontrol_android::resolve_adb(None),
        scrcpy: quadcontrol_android::resolve_scrcpy(None),
        serial: device_id.clone(),
        bit_rate: "8M".into(),
        screen_off,
        audio_on_computer,
        passthrough: Vec::new(),
        scrcpy_env: Vec::new(),
    };
    // `Session::start` 会跑 `scrcpy --version`（带超时）再 spawn——都是阻塞
    // 调用，不能占着 async 运行时，也不能在持锁期间做。
    let spawned = tauri::async_runtime::spawn_blocking(move || {
        quadcontrol_android::spawn_supervised(&options, id)
    })
    .await
    .map_err(|error| {
        eprintln!("session start worker failed: {error}");
        "session_start_failed".to_owned()
    })?;
    let handle = spawned.map_err(|error| {
        eprintln!("session start failed: {error}");
        session_error_code(&error).to_owned()
    })?;
    let mut sessions = store
        .sessions
        .lock()
        .map_err(|_| "session_start_failed".to_owned())?;
    if let Some(existing) = sessions.get(&device_id) {
        // 阻塞期间被另一次调用抢先：不能让两份 scrcpy 同时控制一台手机，
        // 把刚起的这份收回。
        if matches!(
            existing.status(),
            SessionStatus::Running | SessionStatus::Stopping
        ) {
            let _ = handle.request_stop();
            return Err("session_already_running".into());
        }
    }
    sessions.insert(device_id, handle);
    Ok(())
}

#[tauri::command]
async fn stop_session(
    store: tauri::State<'_, SessionStore>,
    device_id: String,
) -> Result<(), String> {
    let store = store.inner();
    let sessions = store
        .sessions
        .lock()
        .map_err(|_| "session_start_failed".to_owned())?;
    if let Some(handle) = sessions.get(&device_id) {
        handle.request_stop().map_err(|error| {
            eprintln!("session stop failed: {error}");
            "session_start_failed".to_owned()
        })?;
    }
    Ok(())
}

#[tauri::command]
async fn sessions(store: tauri::State<'_, SessionStore>) -> Result<Vec<SessionDto>, String> {
    let store = store.inner();
    let sessions = store
        .sessions
        .lock()
        .map_err(|_| "session_failed".to_owned())?;
    Ok(sessions
        .iter()
        .map(|(device_id, handle)| match handle.status() {
            // 状态卡按 GUI_PLAN 要显示「退出码 + stderr 尾」，但只在非零退出时
            // 才有诊断价值；正常退出不外传原始输出。
            SessionStatus::Exited { code, stderr_tail } => SessionDto {
                device_id: device_id.clone(),
                state: "exited".into(),
                exit_code: Some(code),
                stderr_tail: (code != 0).then(|| truncate_tail(stderr_tail, 2000)),
            },
            SessionStatus::Failed { .. } => session_dto(
                device_id.clone(),
                SessionStatus::Failed {
                    message: "session_failed".into(),
                },
            ),
            status => session_dto(device_id.clone(), status),
        })
        .collect())
}

#[tauri::command]
async fn ping() -> Result<String, String> {
    // Tauri synchronous commands run on the main thread. Keep this async plus
    // spawn_blocking shape as the template for future Session/device work.
    tauri::async_runtime::spawn_blocking(|| {
        format!("QuadControl GUI {}", env!("CARGO_PKG_VERSION"))
    })
    .await
    .map_err(|error| format!("ping worker failed: {error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .manage(SessionStore {
            sessions: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        })
        .manage(IosSessionStore {
            session: Mutex::new(None),
            next_id: AtomicU64::new(1),
            meta: Mutex::new(IosMeta::default()),
        })
        .manage(PairingStore {
            state: Mutex::new(PairingState::Idle),
            cancel: Mutex::new(None),
            next_generation: AtomicU64::new(0),
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            list_devices,
            start_pairing,
            pairing_status,
            cancel_pairing,
            pair_manual,
            start_session,
            stop_session,
            sessions,
            host_platform,
            ios_dev_wda_enabled,
            start_ios_session,
            stop_ios_session,
            ios_session_status,
            ios_tap,
            ios_send_text,
            ios_home,
            ios_wake,
            ios_screenshot,
            iphone_mirroring_available,
            open_iphone_mirroring
        ]);
    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("error while building QuadControl GUI: {error}");
            return;
        }
    };
    app.run(|app_handle, event| match event {
        // 退出前必须先收回会话，但**不能在事件循环里同步等**——最长 7 秒的清理会
        // 让窗口停止响应、被系统判定卡死。因此拦下退出，把清理丢到后台，完成后
        // 再真正退出。macOS 的 Cmd+Q 只发 ExitRequested、不发 CloseRequested，
        // 所以两个入口都要拦。
        tauri::RunEvent::ExitRequested { api, .. } if !SHUTDOWN_DONE.load(Ordering::SeqCst) => {
            api.prevent_exit();
            begin_shutdown(app_handle.clone());
        }
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } if !SHUTDOWN_DONE.load(Ordering::SeqCst) => {
            api.prevent_close();
            begin_shutdown(app_handle.clone());
        }
        _ => {}
    });
}

/// 清理只跑一次；完成后置位，让随后的退出事件直接放行。
static SHUTDOWN_STARTED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_DONE: AtomicBool = AtomicBool::new(false);

fn begin_shutdown(app_handle: tauri::AppHandle) {
    if SHUTDOWN_STARTED.swap(true, Ordering::SeqCst) {
        return; // 用户连点关闭：清理已在进行，忽略
    }
    tauri::async_runtime::spawn(async move {
        let worker = app_handle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            // 先发配对取消，再去停会话——会话那几秒里配对 worker 正好在回收，
            // 两条链路的等待因此重叠，而不是串行相加。
            let cancelled_at = Instant::now();
            let pairing = worker.state::<PairingStore>();
            if let Ok(cancel) = pairing.cancel.lock() {
                if let Some(cancel) = cancel.as_ref() {
                    cancel.cancel();
                }
            }

            let store = worker.state::<SessionStore>();
            let handles = match store.sessions.lock() {
                Ok(mut sessions) => sessions
                    .drain()
                    .map(|(_, handle)| handle)
                    .collect::<Vec<_>>(),
                Err(_) => {
                    eprintln!("session store is poisoned during shutdown");
                    Vec::new()
                }
            };
            let ios_store = worker.state::<IosSessionStore>();
            let ios_handles = match ios_store.session.lock() {
                // 占位与失败槽位没有子进程可收；真正在跑的只有 `Handle`。
                Ok(mut session) => match session.take() {
                    Some(IosSlot::Handle(handle)) => vec![handle],
                    _ => Vec::new(),
                },
                Err(_) => {
                    eprintln!("iOS session store is poisoned during shutdown");
                    Vec::new()
                }
            };
            // 两条链路**先全部下停止请求**，再共用同一个截止点依次等 done：
            // 串行相加会是 7 + 7 秒，而两边的终止梯子本来就该并行跑完。
            for handle in &handles {
                let _ = handle.request_stop();
            }
            for handle in &ios_handles {
                let _ = handle.request_stop();
            }
            // 两个预算目前都是 7 秒；取大的那个，常量哪天分叉了也不会有人被截断。
            let deadline_at = Instant::now() + SHUTDOWN_DEADLINE.max(IOS_SHUTDOWN_DEADLINE);
            let android_report = quadcontrol_android::shutdown_all(
                &handles,
                deadline_at.saturating_duration_since(Instant::now()),
            );
            let ios_report = quadcontrol_ios::session::shutdown_all(
                &ios_handles,
                deadline_at.saturating_duration_since(Instant::now()),
            );
            let timed_out = android_report.timed_out.len() + ios_report.timed_out.len();

            // 配对用自己的预算（> 单次 adb 命令超时），不与会话预算共用。
            while cancelled_at.elapsed() < wireless::PAIRING_SHUTDOWN_DEADLINE {
                let active = pairing
                    .state
                    .lock()
                    .map(|state| pairing_is_active(&state))
                    .unwrap_or(false);
                if !active {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            if pairing
                .state
                .lock()
                .map(|state| pairing_is_active(&state))
                .unwrap_or(false)
            {
                eprintln!("pairing worker did not finish before exit");
            }
            Some(timed_out)
        })
        .await;
        match result {
            Ok(Some(timed_out)) if timed_out > 0 => {
                eprintln!("session shutdown timed out for {timed_out} session(s)")
            }
            Err(error) => eprintln!("shutdown worker failed: {error}"),
            _ => {}
        }
        SHUTDOWN_DONE.store(true, Ordering::SeqCst);
        app_handle.exit(0);
    });
}
