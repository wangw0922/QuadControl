//! iPhone（go-ios + WebDriverAgent）会话链路。
//!
//! 结构与 `quadcontrol-android` 对称：解析与执行分离、监督线程独占子进程、
//! 终止梯子保证不留孤儿。差别只在被控引擎——这里是 go-ios 的四个长驻子进程
//! （tunnel / runwda / forward×2）加一个进程内的 MJPEG 代理。
//!
//! 事实来源：[`docs/IOS_PANEL_PLAN.md`] 是规格，
//! [`agents/ios-wda/README.md`] 是真机测量。其中最重要的一条设计约束是
//! **「符号存在 ≠ 可调用 ≠ 真的产生效果」**：WDA 有三种静默失败（锁屏截图返回
//! 合法全黑 PNG、`/wda/keys` 返回成功但字符不送达、锁屏启动应用才会响亮报错），
//! 所以本库的结果码一律来自**效果核验**，不是 HTTP 200。
//!
//! 红线：不使用私有 API，不绕过锁屏（[`wda::Client::wake`] 只是公开的 unlock
//! 手势，**绝不发送密码**），不触碰设备刷新率，不持久化 UDID。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub mod devices;
pub mod mjpeg;
pub mod session;
pub mod wda;

pub use devices::{parse_ios_devices, short_ios_name, IosDevice};

/// 唯一实测过的 go-ios 版本；低于它拒绝启动。
pub const MIN_GO_IOS: (u64, u64, u64) = (1, 2, 1);

/// go-ios 隧道信息服务的默认端口（`ios --help` 的 `--tunnel-info-port`）。
///
/// 它是**每主机一份**的固定端口，不像会话端口那样 bind 0 由 OS 分配。启动前
/// 探测它是否被占用：占用即 [`Error::TunnelPortBusy`]，绝不静默复用别人的隧道。
pub const DEFAULT_TUNNEL_INFO_PORT: u16 = 28100;

/// 稳定错误枚举。每个变体的 [`Error::code`] 与 `IOS_PANEL_PLAN.md` §三 P4.2
/// 的错误码字符串一一对应，GUI 只认这些字符串，不解析 `Display` 文案。
#[derive(Debug)]
pub enum Error {
    /// go-ios 二进制不存在。
    IosNotFound(PathBuf),
    /// go-ios 版本低于 [`MIN_GO_IOS`]，或版本输出无法解析。
    IosVersion(String),
    /// Linux 上 usbmuxd 守护进程没跑（或 macOS 上等价的 usbmuxd 不可达）。
    UsbmuxdUnavailable(String),
    /// 隧道信息端口已被占用；不复用别人的隧道。
    TunnelPortBusy(u16),
    /// `ios tunnel start` 起不来或早退。
    TunnelFailed(String),
    /// WDA 没装在设备上（`runwda` stderr 关键字判定）。
    WdaNotInstalled(String),
    /// WDA 签名/证书过期（个人团队 7 天就过期，是最常见的失败）。
    WdaSignatureExpired(String),
    /// WDA 起来了但 HTTP 不可达（`/status` 轮询超预算）。
    WdaUnreachable(String),
    /// `POST /session` 失败。
    WdaSessionFailed(String),
    /// `ios forward` 起不来或早退。
    ForwardFailed(String),
    /// MJPEG 代理失败（上游 EOF、单帧超限断流等）。
    ProxyFailed(String),
    /// 设备处于锁定状态，动作不转发；`wake` 之后仍锁定也归这里。
    DeviceLocked,
    /// 文字已发出但**回读不含发送内容**，无法确认送达。
    TextUnconfirmed,
    /// 设备上没有聚焦的输入框，文字无处可送。
    ///
    /// 真机上 `GET /session/{s}/element/active` 此时返回 `nosuchelement`。
    /// 这不是故障，是一个用户自己就能修的状态，所以单独给一个码。
    NoActiveElement,
    /// 截屏失败（或返回的 base64 无法解码）。
    ScreenshotFailed(String),
    /// 已有 iOS 会话在跑；同一时刻最多一个。
    AlreadyRunning,
    /// Windows 暂不支持起会话（无 Job Object 收不回进程树，见 P6）。
    WindowsSessionUnsupported,
    /// 底层 IO / 进程错误。
    Process(io::Error),
}

impl Error {
    /// 给 GUI 用的稳定错误码。**改这里等于改 GUI 契约**，两边要一起动。
    pub fn code(&self) -> &'static str {
        match self {
            Self::IosNotFound(_) => "ios_not_found",
            Self::IosVersion(_) => "ios_version",
            Self::UsbmuxdUnavailable(_) => "usbmuxd_unavailable",
            Self::TunnelPortBusy(_) => "tunnel_port_busy",
            Self::TunnelFailed(_) => "tunnel_failed",
            Self::WdaNotInstalled(_) => "wda_not_installed",
            Self::WdaSignatureExpired(_) => "wda_signature_expired",
            Self::WdaUnreachable(_) => "wda_unreachable",
            Self::WdaSessionFailed(_) => "wda_session_failed",
            Self::ForwardFailed(_) => "forward_failed",
            Self::ProxyFailed(_) => "proxy_failed",
            Self::DeviceLocked => "device_locked",
            Self::TextUnconfirmed => "text_unconfirmed",
            Self::NoActiveElement => "no_active_element",
            Self::ScreenshotFailed(_) => "screenshot_failed",
            Self::AlreadyRunning => "ios_session_already_running",
            Self::WindowsSessionUnsupported => "windows_session_unsupported",
            // 方案的错误码表里没有「通用进程错误」这一格。这里自造一个码而不是
            // 复用别的，免得 GUI 把 IO 故障误显示成设备问题；报告里作为偏差列出。
            Self::Process(_) => "ios_process_error",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IosNotFound(path) => write!(
                f,
                "go-ios was not found at {}. Install go-ios 1.2.1 or newer, then try again.",
                path.display()
            ),
            Self::IosVersion(s) => f.write_str(s),
            Self::UsbmuxdUnavailable(s) => f.write_str(s),
            Self::TunnelPortBusy(port) => write!(
                f,
                "the go-ios tunnel info port {port} is already in use; stop the other go-ios tunnel first"
            ),
            Self::TunnelFailed(s)
            | Self::WdaNotInstalled(s)
            | Self::WdaSignatureExpired(s)
            | Self::WdaUnreachable(s)
            | Self::WdaSessionFailed(s)
            | Self::ForwardFailed(s)
            | Self::ProxyFailed(s)
            | Self::ScreenshotFailed(s) => f.write_str(s),
            Self::DeviceLocked => f.write_str("the iPhone is locked; unlock it on the device"),
            Self::TextUnconfirmed => {
                f.write_str("the text could not be confirmed as delivered; check the iPhone")
            }
            Self::NoActiveElement => {
                f.write_str("no focused text field on the device; tap into a text field first")
            }
            Self::AlreadyRunning => f.write_str("an iOS session is already running"),
            Self::WindowsSessionUnsupported => {
                f.write_str("iOS sessions are not supported on Windows yet")
            }
            Self::Process(e) => write!(f, "process error: {e}"),
        }
    }
}

impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Process(e)
    }
}

/// 解析 go-ios 二进制路径：显式参数 → `IOS_BIN` 环境变量 → `ios`（走 PATH）。
pub fn resolve_ios(explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("IOS_BIN").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("ios"))
}

/// 钳制过的 go-ios 版本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// 高于唯一实测版本 [`MIN_GO_IOS`]。放行，但调用方应写一条警告日志：
    /// 语法漂移只会在运行时暴露，我们没在这个版本上测过。
    pub newer_than_verified: bool,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 跑 `ios version` 并钳制版本。
///
/// go-ios 把 JSON 结果打到 **stdout**，结构化日志行打到 stderr，所以只解析
/// stdout。低于 [`MIN_GO_IOS`] 拒绝；更高放行并置 `newer_than_verified`。
pub fn check_version(ios: &Path) -> Result<Version, Error> {
    let output = Command::new(ios)
        .arg("version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Error::IosNotFound(ios.to_path_buf())
            } else {
                Error::Process(e)
            }
        })?;
    if !output.status.success() {
        return Err(Error::IosVersion(format!(
            "`ios version` exited with {}",
            output.status.code().unwrap_or(-1)
        )));
    }
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

/// 从 `{"version":"1.2.1"}` 解析并钳制。
///
/// 真机 1.2.1 的输出没有 `v` 前缀，但历史版本有，两种都接受。
pub fn parse_version(stdout: &str) -> Result<Version, Error> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| Error::IosVersion(format!("could not parse `ios version` output: {e}")))?;
    let raw = value
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::IosVersion("`ios version` output has no version field".into()))?;
    let token = raw.trim().trim_start_matches('v');
    // 预发布后缀（1.3.0-rc1）不接受：钳制的前提是这个版本被实测过。
    if token.contains('-') {
        return Err(Error::IosVersion(format!(
            "go-ios prerelease {raw} is not supported; install stable go-ios 1.2.1 or newer"
        )));
    }
    let mut parts = token.split('.');
    let mut next = |name: &str| -> Result<u64, Error> {
        parts
            .next()
            .and_then(|p| p.parse::<u64>().ok())
            .ok_or_else(|| Error::IosVersion(format!("go-ios version {raw} has no {name}")))
    };
    let major = next("major")?;
    let minor = next("minor")?;
    // patch 缺省按 0 算：`1.3` 也是合法的版本表达。
    let patch = parts
        .next()
        .and_then(|p| p.parse::<u64>().ok())
        .unwrap_or(0);
    let found = (major, minor, patch);
    if found < MIN_GO_IOS {
        return Err(Error::IosVersion(format!(
            "go-ios {raw} is too old; install go-ios 1.2.1 or newer"
        )));
    }
    Ok(Version {
        major,
        minor,
        patch,
        newer_than_verified: found > MIN_GO_IOS,
    })
}

/// 跑 `ios list` 并解析设备列表。
///
/// 只读 stdout：stderr 是结构化 JSON 日志行（真机 1.2.1 在没有隧道时会打一条
/// WARN），混进来会让解析失败。
pub fn list_devices(ios: &Path) -> Result<Vec<IosDevice>, Error> {
    let output = Command::new(ios)
        .arg("list")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Error::IosNotFound(ios.to_path_buf())
            } else {
                Error::Process(e)
            }
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Linux 上最常见的失败是 usbmuxd 没跑；单独成码，界面才能给对提示。
        if looks_like_usbmuxd_failure(&stderr) {
            return Err(Error::UsbmuxdUnavailable(
                "usbmuxd is not reachable; make sure the usbmuxd service is running".into(),
            ));
        }
        return Err(Error::UsbmuxdUnavailable(format!(
            "`ios list` exited with {}",
            output.status.code().unwrap_or(-1)
        )));
    }
    // 解析失败是 go-ios 输出形状变了，不是 usbmuxd 的问题；归到进程错误，
    // 免得界面把它提示成「请启动 usbmuxd」。
    parse_ios_devices(&String::from_utf8_lossy(&output.stdout)).map_err(|reason| {
        Error::Process(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("go-ios output rejected: {reason}"),
        ))
    })
}

/// usbmuxd 不可达的关键字判定。
///
/// **未在 Linux 真机上校准**：本机只有 macOS，这些串来自 go-ios 源码里的错误
/// 文案。真机验收时要复核并回写。
fn looks_like_usbmuxd_failure(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    ["usbmuxd", "connection refused", "no such file or directory"]
        .iter()
        .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_clamp_rejects_old_and_flags_newer() {
        let exact = parse_version(r#"{"version":"1.2.1"}"#).unwrap();
        assert_eq!((exact.major, exact.minor, exact.patch), (1, 2, 1));
        assert!(!exact.newer_than_verified);

        assert!(
            parse_version(r#"{"version":"1.3.0"}"#)
                .unwrap()
                .newer_than_verified
        );
        assert!(parse_version(r#"{"version":"v1.2.1"}"#).is_ok());
        // patch 省略按 0 算，1.2 < 1.2.1 所以要拒。
        assert!(parse_version(r#"{"version":"1.2"}"#).is_err());
        assert!(parse_version(r#"{"version":"1.2.0"}"#).is_err());
        assert!(parse_version(r#"{"version":"0.9.9"}"#).is_err());
        assert!(parse_version(r#"{"version":"1.3.0-rc1"}"#).is_err());
        assert!(parse_version(r#"{"other":"1.2.1"}"#).is_err());
        assert!(parse_version("not json").is_err());
    }

    /// 真机抓的 `ios version` stdout 原样进解析器。
    #[test]
    fn fixture_version_output_parses() {
        let fixture = include_str!("../tests/fixtures/go-ios-1.2.1/ios-version.stdout.json");
        assert_eq!(parse_version(fixture).unwrap().to_string(), "1.2.1");
    }

    #[test]
    fn error_codes_match_the_plan() {
        let cases: Vec<(Error, &str)> = vec![
            (Error::IosNotFound(PathBuf::from("ios")), "ios_not_found"),
            (Error::IosVersion(String::new()), "ios_version"),
            (
                Error::UsbmuxdUnavailable(String::new()),
                "usbmuxd_unavailable",
            ),
            (Error::TunnelPortBusy(28100), "tunnel_port_busy"),
            (Error::TunnelFailed(String::new()), "tunnel_failed"),
            (Error::NoActiveElement, "no_active_element"),
            (Error::WdaNotInstalled(String::new()), "wda_not_installed"),
            (
                Error::WdaSignatureExpired(String::new()),
                "wda_signature_expired",
            ),
            (Error::WdaUnreachable(String::new()), "wda_unreachable"),
            (Error::WdaSessionFailed(String::new()), "wda_session_failed"),
            (Error::ForwardFailed(String::new()), "forward_failed"),
            (Error::ProxyFailed(String::new()), "proxy_failed"),
            (Error::DeviceLocked, "device_locked"),
            (Error::TextUnconfirmed, "text_unconfirmed"),
            (Error::ScreenshotFailed(String::new()), "screenshot_failed"),
            (Error::AlreadyRunning, "ios_session_already_running"),
            (
                Error::WindowsSessionUnsupported,
                "windows_session_unsupported",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.code(), expected);
        }
    }

    #[test]
    fn resolve_ios_prefers_explicit_then_environment_then_default() {
        // 环境变量是进程全局的；本 crate 只有这一处测它，不需要额外的锁。
        let previous = std::env::var_os("IOS_BIN");
        std::env::set_var("IOS_BIN", "/env/ios");
        assert_eq!(
            resolve_ios(Some(Path::new("/explicit/ios"))),
            PathBuf::from("/explicit/ios")
        );
        assert_eq!(resolve_ios(None), PathBuf::from("/env/ios"));
        std::env::remove_var("IOS_BIN");
        assert_eq!(resolve_ios(None), PathBuf::from("ios"));
        if let Some(value) = previous {
            std::env::set_var("IOS_BIN", value);
        }
    }
}
