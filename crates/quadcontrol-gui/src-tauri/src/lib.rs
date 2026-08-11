use adb_probe::{AdbCommand, CommandRunner, ExecutionError, ProbeError, SystemRunner};
use serde::Serialize;
use std::process::Command;

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
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping, list_devices])
        .run(tauri::generate_context!())
        .expect("error while running QuadControl GUI");
}
