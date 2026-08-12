use adb_probe::{AdbCommand, CommandRunner, ExecutionError, ProbeError, SystemRunner};
use quadcontrol_android::{SessionHandle, SessionStatus, SHUTDOWN_DEADLINE};
use tauri::Manager;
use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

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

#[derive(Debug, Eq, PartialEq)]
struct IosDevice {
    id: String,
    name: String,
}

/// go-ios 的 `ios list` 默认输出 JSON（`{"deviceList":["<udid>", ...]}`）。
/// 这里手写最小提取，避免为一个字段引入 JSON 依赖；同时兼容按行输出的形态。
///
/// **未经真机验证**：本机没有 go-ios，格式依据其文档而非实测——P4 接 iPhone
/// 面板时用真设备复核。
fn parse_ios_devices(output: &str) -> Result<Vec<IosDevice>, String> {
    let udids = if let Some(list) = extract_device_list(output) {
        list
    } else {
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .filter(|line| !line.eq_ignore_ascii_case("list of attached devices:"))
            .map(|line| line.strip_prefix("UDID:").unwrap_or(line).trim().to_owned())
            .collect()
    };

    udids
        .into_iter()
        .map(|udid| {
            if udid.is_empty() || udid.contains(char::is_whitespace) {
                return Err("go-ios returned an unparsable device list".to_owned());
            }
            Ok(IosDevice {
                name: short_ios_name(&udid),
                id: udid,
            })
        })
        .collect()
}

/// 从 `{"deviceList":["a","b"]}` 里取出数组元素；不是这个形状就返回 None。
fn extract_device_list(output: &str) -> Option<Vec<String>> {
    let start = output.find("\"deviceList\"")?;
    let open = output[start..].find('[')? + start;
    let close = output[open..].find(']')? + open;
    Some(
        output[open + 1..close]
            .split(',')
            .map(|item| item.trim().trim_matches('"').to_owned())
            .filter(|item| !item.is_empty())
            .collect(),
    )
}

/// 界面上不显示完整 UDID：又长又不可读，且没必要把完整设备标识摆在屏幕上。
fn short_ios_name(udid: &str) -> String {
    let tail: String = udid
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("iPhone ····{tail}")
}

fn list_ios_devices() -> Result<Vec<IosDevice>, &'static str> {
    let binary = std::env::var_os("IOS_BIN").unwrap_or_else(|| "ios".into());
    let output = Command::new(binary).arg("list").output().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            "ios_not_found"
        } else {
            eprintln!("go-ios could not be executed: {error}");
            "ios_failed"
        }
    })?;
    if !output.status.success() {
        eprintln!(
            "go-ios `ios list` exited with {}",
            output.status.code().unwrap_or(-1)
        );
        return Err("ios_failed");
    }
    parse_ios_devices(&String::from_utf8_lossy(&output.stdout)).map_err(|reason| {
        eprintln!("go-ios output rejected: {reason}");
        "ios_failed"
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

    let ios_error = match list_ios_devices() {
        Ok(ios_devices) => {
            devices.extend(ios_devices.into_iter().map(|device| DeviceDto {
                id: device.id,
                name: device.name,
                platform: "ios".to_owned(),
                status: "pairing_required".to_owned(),
                detail: None,
            }));
            None
        }
        Err(code) => Some(code.to_owned()),
    };

    DeviceListDto {
        devices,
        android_error,
        ios_error,
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_device_list, parse_ios_devices, short_ios_name};

    #[test]
    fn parses_go_ios_json_device_list() {
        let devices =
            parse_ios_devices(r#"{"deviceList":["REDACTEDSERIAL1","REDACTEDSERIAL2"]}"#).unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "REDACTEDSERIAL1");
        assert_eq!(devices[0].name, "iPhone ····IAL1");
    }

    #[test]
    fn parses_plain_line_device_list() {
        let devices =
            parse_ios_devices("List of attached devices:\nREDACTEDSERIAL\nUDID: REDACTEDSERIAL2\n")
                .unwrap();

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[1].id, "REDACTEDSERIAL2");
    }

    #[test]
    fn empty_output_yields_no_devices() {
        assert!(parse_ios_devices("").unwrap().is_empty());
        assert!(parse_ios_devices(r#"{"deviceList":[]}"#)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn rejects_unparsable_line() {
        assert!(parse_ios_devices("REDACTEDSERIAL with-space\n").is_err());
    }

    #[test]
    fn ignores_non_matching_json_shape() {
        assert!(extract_device_list(r#"{"other":[1]}"#).is_none());
    }

    #[test]
    fn short_name_keeps_only_the_tail() {
        assert_eq!(short_ios_name("REDACTEDSERIAL"), "iPhone ····RIAL");
        assert_eq!(short_ios_name("ab"), "iPhone ····ab");
    }
}

#[tauri::command]
async fn list_devices() -> Result<DeviceListDto, String> {
    tauri::async_runtime::spawn_blocking(list_devices_blocking)
        .await
        .map_err(|error| format!("device discovery worker failed: {error}"))
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
    let mut sessions = store
        .sessions
        .lock()
        .map_err(|_| "session_start_failed".to_owned())?;
    if let Some(existing) = sessions.get(&device_id) {
        if matches!(existing.status(), SessionStatus::Running | SessionStatus::Stopping) {
            return Err("session_already_running".into());
        }
    }
    sessions.remove(&device_id);
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
    let id = store.next_id.fetch_add(1, Ordering::Relaxed);
    match quadcontrol_android::spawn_supervised(&options, id) {
        Ok(handle) => {
            sessions.insert(device_id, handle);
            Ok(())
        }
        Err(error) => {
            eprintln!("session start failed: {error}");
            Err(session_error_code(&error).into())
        }
    }
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
        handle
            .request_stop()
            .map_err(|error| {
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
            SessionStatus::Exited { code, stderr_tail } => SessionDto {
                device_id: device_id.clone(),
                state: "exited".into(),
                exit_code: Some(code),
                stderr_tail: Some(truncate_tail(stderr_tail, 2000)),
            },
            SessionStatus::Failed { .. } => session_dto(device_id.clone(), SessionStatus::Failed { message: "session_failed".into() }),
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
        .invoke_handler(tauri::generate_handler![
            ping,
            list_devices,
            start_session,
            stop_session,
            sessions
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
            let store = worker.state::<SessionStore>();
            let handles = match store.sessions.lock() {
                Ok(mut sessions) => sessions.drain().map(|(_, handle)| handle).collect::<Vec<_>>(),
                Err(_) => {
                    eprintln!("session store is poisoned during shutdown");
                    return None;
                }
            };
            Some(quadcontrol_android::shutdown_all(&handles, SHUTDOWN_DEADLINE))
        })
        .await;
        match result {
            Ok(Some(report)) if !report.timed_out.is_empty() => eprintln!(
                "session shutdown timed out for {} session(s)",
                report.timed_out.len()
            ),
            Err(error) => eprintln!("shutdown worker failed: {error}"),
            _ => {}
        }
        SHUTDOWN_DONE.store(true, Ordering::SeqCst);
        app_handle.exit(0);
    });
}
