//! Android 11+ wireless-debugging pairing.
//!
//! This module owns the mutating ADB commands used by the wireless pairing
//! flow.  The passive probe deliberately remains a separate, read-only tool.

use adb_probe::{parse_devices, Device, DeviceState};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const PASSWORD_LENGTH: usize = 12;
// 真机实测定的值:用户要走完「设置 → 开发者选项 → 无线调试 → 使用二维码
// 配对设备 → 扫码」才轮到我们，90 秒不够，首次实测就直接超时了。
pub const ROUND_TIMEOUT: Duration = Duration::from_secs(300);
const OUTPUT_LIMIT: usize = 64 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const ALPHANUMERIC: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingSecret {
    pub service_name: String,
    pub password: String,
    pub payload: String,
}

impl PairingSecret {
    /// 显式擦除。round-close 必须在轮次结束时立刻调用，而不是等到
    /// `Drop`——否则配对成功后 secret 还会活过 connect、去重与打印。
    pub fn wipe(&mut self) {
        wipe_string(&mut self.service_name);
        wipe_string(&mut self.password);
        wipe_string(&mut self.payload);
    }
}

impl Drop for PairingSecret {
    fn drop(&mut self) {
        self.wipe();
    }
}

pub fn generate_pairing_secret() -> PairingSecret {
    let service_suffix = random_text(10);
    let password = random_text(PASSWORD_LENGTH);
    let service_name = format!("studio-{service_suffix}");
    let payload = format!("WIFI:T:ADB;S:{service_name};P:{password};;");
    PairingSecret {
        service_name,
        password,
        payload,
    }
}

fn random_text(length: usize) -> String {
    let mut output = Vec::with_capacity(length);
    while output.len() < length {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("OS CSPRNG unavailable");
        for byte in bytes {
            if byte < 248 {
                output.push(ALPHANUMERIC[(byte as usize) % ALPHANUMERIC.len()] as char);
                if output.len() == length {
                    break;
                }
            }
        }
    }
    output.into_iter().collect()
}

fn wipe_string(value: &mut String) {
    unsafe { value.as_mut_vec().fill(0) };
    value.clear();
}

/// 供 CLI 擦除自己持有的临时凭据字符串（例如手动输入的 6 位配对码）。
pub fn wipe_secret_string(value: &mut String) {
    wipe_string(value);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MdnsService {
    pub instance: String,
    pub service: String,
    pub address: String,
    pub port: u16,
}

pub fn parse_mdns_services(output: &str) -> Vec<MdnsService> {
    output
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 2 || fields[0].eq_ignore_ascii_case("List") {
                return None;
            }
            let service_index = fields.iter().position(|field| {
                let service = field.trim_end_matches('.');
                service == "_adb-tls-pairing._tcp" || service == "_adb-tls-connect._tcp"
            });
            let (instance, service, endpoint_fields) = if let Some(service_index) = service_index {
                (
                    fields
                        .get(service_index.checked_sub(1)?)?
                        .trim_end_matches('.')
                        .to_owned(),
                    fields[service_index].trim_end_matches('.').to_owned(),
                    &fields[service_index + 1..],
                )
            } else {
                let index = fields
                    .iter()
                    .position(|field| field.contains("._adb-tls-"))?;
                let (instance, service) = fields[index].split_once("._adb-tls-")?;
                (
                    instance.to_owned(),
                    format!("_adb-tls-{}", service.trim_end_matches('.')),
                    &fields[index + 1..],
                )
            };
            let (address, port) = endpoint_fields
                .iter()
                .find_map(|field| split_endpoint(field))
                .or_else(|| {
                    let address = *endpoint_fields.first()?;
                    let port = endpoint_fields.get(1)?.parse().ok()?;
                    Some((address.to_owned(), port))
                })?;
            Some(MdnsService {
                instance,
                service,
                address,
                port,
            })
        })
        .collect()
}

fn split_endpoint(value: &str) -> Option<(String, u16)> {
    let (address, port) = value.rsplit_once(':')?;
    if address.is_empty() {
        return None;
    }
    Some((address.to_owned(), port.parse().ok()?))
}

pub fn find_pairing_service<'a>(
    services: &'a [MdnsService],
    name: &str,
) -> Option<&'a MdnsService> {
    find_pairing_services(services, name).into_iter().next()
}

/// 同名竞争：多台手机扫同一张二维码时，每台都会以相同 instance name 广播
/// 自己的 pairing 服务，端点各不相同。只取首项会让"首个成功者胜出"无从谈起，
/// 因此这里返回全部匹配项，由调用方对每个端点各起一个 pair 进程。
pub fn find_pairing_services<'a>(services: &'a [MdnsService], name: &str) -> Vec<&'a MdnsService> {
    services
        .iter()
        .filter(|service| service.service == "_adb-tls-pairing._tcp" && service.instance == name)
        .collect()
}

pub fn find_guid(output: &str) -> Option<String> {
    output.split_whitespace().find_map(|word| {
        let token = word.strip_prefix("[guid=")?.strip_suffix(']')?;
        token
            .strip_prefix("adb-")
            .filter(|rest| {
                !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
            .map(|_| token.to_owned())
    })
}

pub fn serial_matches_guid(serial: &str, guid: &str) -> bool {
    serial == guid || serial.starts_with(guid) && serial[guid.len()..].starts_with('.')
}

pub fn connect_service_for_guid<'a>(
    services: &'a [MdnsService],
    guid: &str,
) -> Option<&'a MdnsService> {
    services.iter().find(|service| {
        service.service == "_adb-tls-connect._tcp" && serial_matches_guid(&service.instance, guid)
    })
}

pub fn parse_adb_version(output: &str) -> Option<(u64, u64, u64)> {
    output.lines().find_map(|line| {
        let version = line.strip_prefix("Version ")?.split_whitespace().next()?;
        // 真实 adb 是 `Version 37.0.1-13206524`——补丁号后面挂着构建号，
        // 直接 parse 会失败，预检就会对真机误报"无法解析版本"。
        let version = version.split('-').next()?;
        let mut parts = version.split('.').map(|part| part.parse().ok());
        Some((parts.next()??, parts.next()??, parts.next()??))
    })
}

pub fn require_supported_adb(output: &str) -> Result<(u64, u64, u64), WirelessError> {
    let version = parse_adb_version(output).ok_or(WirelessError::InvalidAdbVersion)?;
    if version < (30, 0, 0) {
        return Err(WirelessError::AdbTooOld(version));
    }
    Ok(version)
}

pub fn deduplicate_devices(devices: &[Device], serials: &[(String, String)]) -> Vec<Device> {
    let mut result = Vec::new();
    for device in devices {
        let duplicate = result.iter().position(|existing: &Device| {
            serials.iter().any(|(left, right)| {
                left == &existing.serial && right == &device.serial
                    || left == &device.serial && right == &existing.serial
            })
        });
        if let Some(index) = duplicate {
            if device.serial.starts_with("adb-") && !result[index].serial.starts_with("adb-") {
                result[index] = device.clone();
            }
        } else {
            result.push(device.clone());
        }
    }
    result
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloseReason {
    Success,
    Timeout,
    Cancelled,
    ServiceLost,
    PairFailed,
    PairTimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoundEvent {
    ServiceLost,
    PairSucceeded(String),
    PairFailed,
    /// 单个 pair 子进程自身超时。
    PairTimedOut,
    /// 整轮扫码超时——与上一个区分开，用户看到的原因不同。
    Timeout,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RoundAction {
    WaitForPair,
    Close(CloseReason),
    IgnoredAfterClose,
}

#[derive(Debug)]
pub struct RoundState {
    pub id: u64,
    pub pair_in_flight: bool,
    pub closed: bool,
    pub guid: Option<String>,
    service_lost_pending: bool,
}

impl RoundState {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            pair_in_flight: false,
            closed: false,
            guid: None,
            service_lost_pending: false,
        }
    }
    pub fn observe(&mut self, event: RoundEvent) -> RoundAction {
        if self.closed {
            return RoundAction::IgnoredAfterClose;
        }
        match event {
            RoundEvent::ServiceLost if self.pair_in_flight => {
                self.service_lost_pending = true;
                RoundAction::WaitForPair
            }
            RoundEvent::ServiceLost => self.close(CloseReason::ServiceLost),
            RoundEvent::Cancelled => self.close(CloseReason::Cancelled),
            RoundEvent::Timeout => self.close(CloseReason::Timeout),
            RoundEvent::PairSucceeded(guid) => {
                self.pair_in_flight = false;
                self.guid = Some(guid);
                self.close(CloseReason::Success)
            }
            RoundEvent::PairFailed | RoundEvent::PairTimedOut if self.pair_in_flight => {
                self.pair_in_flight = false;
                let reason = if self.service_lost_pending {
                    CloseReason::ServiceLost
                } else if matches!(event, RoundEvent::PairFailed) {
                    CloseReason::PairFailed
                } else {
                    CloseReason::PairTimedOut
                };
                self.close(reason)
            }
            RoundEvent::PairFailed => self.close(CloseReason::PairFailed),
            RoundEvent::PairTimedOut => self.close(CloseReason::PairTimedOut),
        }
    }
    pub fn observe_for_round(&mut self, round_id: u64, event: RoundEvent) -> RoundAction {
        if round_id != self.id || self.closed {
            RoundAction::IgnoredAfterClose
        } else {
            self.observe(event)
        }
    }
    pub fn close(&mut self, reason: CloseReason) -> RoundAction {
        self.pair_in_flight = false;
        self.closed = true;
        self.id = self.id.wrapping_add(1);
        RoundAction::Close(reason)
    }
}

#[derive(Debug)]
pub enum WirelessError {
    Io(io::Error),
    AdbNotFound,
    TimedOut,
    OutputTooLarge,
    CommandFailed(String),
    InvalidAdbVersion,
    AdbTooOld((u64, u64, u64)),
    MdnsUnavailable,
    PairOutputUnparseable,
    NoMatchingService,
    Devices(String),
    InvalidInput(String),
    Cancelled,
    /// 整轮扫码超时——与单个 pair 子进程的 `TimedOut` 是两回事。
    RoundTimedOut,
    /// pairing 服务在配对完成前消失。
    ServiceLost,
}
impl fmt::Display for WirelessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "ADB process error: {e}"),
            Self::AdbNotFound => f.write_str("未找到 adb，请安装 Android platform-tools"),
            Self::TimedOut => f.write_str("adb 命令超时"),
            Self::OutputTooLarge => f.write_str("adb 输出超过限制"),
            Self::CommandFailed(e) => write!(f, "adb 命令失败: {e}"),
            Self::InvalidAdbVersion => f.write_str("无法解析 adb 客户端版本"),
            Self::AdbTooOld(v) => write!(
                f,
                "adb {}.{}.{} 过旧，需要 30.0.0 或更高版本",
                v.0, v.1, v.2
            ),
            Self::MdnsUnavailable => f.write_str(
                "adb mDNS 不可用。可能是已在运行的 adb server 版本较旧；\n\
                 加 --restart-adb-server 重跑可重启 adb server 后重试\n\
                 （会短暂断开本机其他 adb 会话），或改用 pair-code / USB。",
            ),
            Self::PairOutputUnparseable => {
                f.write_str("pair 成功但输出无法解析，请重试或手动 connect")
            }
            Self::NoMatchingService => f.write_str("未发现匹配的无线调试服务"),
            Self::Devices(e) => f.write_str(e),
            Self::InvalidInput(e) => f.write_str(e),
            Self::Cancelled => f.write_str("已取消 / cancelled"),
            Self::RoundTimedOut => f.write_str("扫码超时 / scan timed out"),
            Self::ServiceLost => f.write_str("配对服务已消失 / pairing service disappeared"),
        }
    }
}
impl std::error::Error for WirelessError {}
impl From<io::Error> for WirelessError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

struct Captured {
    data: Vec<u8>,
    truncated: bool,
}
fn capture<R: Read + Send + 'static>(mut reader: R) -> thread::JoinHandle<io::Result<Captured>> {
    thread::spawn(move || {
        let mut data = Vec::new();
        let mut buf = [0u8; 4096];
        let mut truncated = false;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let keep = (OUTPUT_LIMIT - data.len()).min(n);
            data.extend_from_slice(&buf[..keep]);
            truncated |= keep < n;
        }
        Ok(Captured { data, truncated })
    })
}

#[derive(Debug)]
pub struct PairResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct PairProcess {
    child: Child,
    stdout: Option<thread::JoinHandle<io::Result<Captured>>>,
    stderr: Option<thread::JoinHandle<io::Result<Captured>>>,
    started: Instant,
}

/// 扫码轮次的编排。放在库里而不是 `main.rs`，是为了让**完整入口**可测——
/// 前两版都栽在"状态机定义了、单测了，却没接进真实路径"。
///
/// 返回值同时带出结束事件：只有这里知道失败究竟是 pair 自身超时、整轮超时，
/// 还是 service-lost 之后的失败，调用方无从区分。
pub fn run_pairing_round(
    adb: &Path,
    secret: &PairingSecret,
    round: &mut RoundState,
    pairs: &mut Vec<PairProcess>,
) -> (RoundEvent, Result<String, WirelessError>) {
    let round_id = round.id;
    let deadline = Instant::now() + ROUND_TIMEOUT;
    let mut spawned: Vec<String> = Vec::new();
    let mut service_lost = false;
    // 这两个必须活在循环外：候选可能在第 N 轮超时/失败，而 pairs 直到第 N+1
    // 轮才清空。放在循环内每轮都会被重置，归因就丢了。
    let mut failure: Option<WirelessError> = None;
    let mut pair_timed_out = false;
    loop {
        if crate::signal_requested() {
            return (RoundEvent::Cancelled, Err(WirelessError::Cancelled));
        }
        if Instant::now() >= deadline {
            return (RoundEvent::Timeout, Err(WirelessError::RoundTimedOut));
        }
        // 与 service-lost 同一条规矩：只要还有 pair 子进程在飞，查询失败就不得
        // 终结轮次并把它杀掉——那个进程仍可能配对成功。此时跳过本轮的发现阶段，
        // 但**必须继续往下轮询在飞进程**（早先这里 `continue` 掉了整段轮询，
        // 结果谁也不去看那个进程，一路空转到整轮超时）。
        let found = match adb_mdns_services(adb) {
            Ok(found) => Some(found),
            Err(error) if !pairs.is_empty() => {
                failure.get_or_insert(error);
                None
            }
            Err(error) => {
                let first = failure.take().unwrap_or(error);
                let (event, error) = classify_round_failure(service_lost, pair_timed_out, first);
                return (event, Err(error));
            }
        };
        // 同名竞争：为**每个**端点各起一个 pair 进程。
        if let Some(found) = found.as_deref() {
            let matching = find_pairing_services(found, &secret.service_name);
            for service in &matching {
                let endpoint = format!("{}:{}", service.address, service.port);
                if spawned.iter().any(|seen| seen == &endpoint) {
                    continue;
                }
                match PairProcess::spawn(adb, &endpoint, &secret.password) {
                    Ok(process) => pairs.push(process),
                    Err(error) => {
                        // 同上：已有在飞进程时不得因为多起一个失败就杀掉它们。
                        if !pairs.is_empty() {
                            failure.get_or_insert(error);
                            break;
                        }
                        let first = failure.take().unwrap_or(error);
                        let (event, error) =
                            classify_round_failure(service_lost, pair_timed_out, first);
                        return (event, Err(error));
                    }
                }
                spawned.push(endpoint);
                round.pair_in_flight = true;
            }
            // 服务撤销：有在飞进程时只记待决，交由结束事件归因。
            if matching.is_empty() && !spawned.is_empty() && !service_lost {
                service_lost = true;
                let _ = round.observe_for_round(round_id, RoundEvent::ServiceLost);
            }
        }

        let mut index = 0;
        while index < pairs.len() {
            match pairs[index].poll() {
                Ok(Some(result)) if result.status == 0 => match find_guid(&result.stdout) {
                    Some(guid) => return (RoundEvent::PairSucceeded(guid.clone()), Ok(guid)),
                    None => {
                        pairs.remove(index);
                        failure.get_or_insert(WirelessError::PairOutputUnparseable);
                    }
                },
                Ok(Some(result)) => {
                    // 竞争败者退出属正常，不能因此终结整轮。
                    let summary = stderr_summary(&result.stderr, &secret.password);
                    pairs.remove(index);
                    failure.get_or_insert(WirelessError::CommandFailed(if summary.is_empty() {
                        "pair failed".into()
                    } else {
                        summary
                    }));
                }
                Err(WirelessError::TimedOut) => {
                    // pair 子进程自身超时，与整轮超时是两回事。
                    pair_timed_out = true;
                    pairs.remove(index);
                    failure.get_or_insert(WirelessError::TimedOut);
                }
                Err(error) => {
                    pairs.remove(index);
                    failure.get_or_insert(error);
                }
                Ok(None) => index += 1,
            }
        }
        if pairs.is_empty() {
            round.pair_in_flight = false;
            if let Some(error) = failure {
                let (event, error) = classify_round_failure(service_lost, pair_timed_out, error);
                return (event, Err(error));
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// 结束归因：**事件与错误必须同源**。
/// 上一版只把事件按优先级排了序，错误却仍是"第一个发生的"——先普通失败、
/// 后 pair 超时时，事件说 PairTimedOut 而错误说 CommandFailed，两者互相矛盾。
/// 优先级：service-lost > pair 自身超时 > 普通失败。
fn classify_round_failure(
    service_lost: bool,
    pair_timed_out: bool,
    first: WirelessError,
) -> (RoundEvent, WirelessError) {
    if service_lost {
        (RoundEvent::ServiceLost, WirelessError::ServiceLost)
    } else if pair_timed_out {
        (RoundEvent::PairTimedOut, WirelessError::TimedOut)
    } else {
        (RoundEvent::PairFailed, first)
    }
}

/// `pair-qr` 的完整入口：预检 → 信号 → 扫码轮次 → 统一 round-close →
/// 等待设备上线（失败则走同 GUID 的显式 connect）→ 去重后返回主 serial。
/// 整条链路住在库里，测试才能驱动**真实入口**而不是它的一段。
pub fn pair_qr_command(
    adb: &Path,
    announce: impl FnOnce(&str),
) -> Result<(String, usize, CloseReason), WirelessError> {
    pair_qr_command_with_secret(adb, generate_pairing_secret(), announce)
}

/// 同上，但由调用方提供 secret。测试用它注入已知服务名去驱动假 adb——
/// 这是依赖注入，不是测试替身：除 CSPRNG 那一行外，整条链路都是真实的。
pub fn pair_qr_command_with_secret(
    adb: &Path,
    secret: PairingSecret,
    announce: impl FnOnce(&str),
) -> Result<(String, usize, CloseReason), WirelessError> {
    let _ = check_adb(adb)?;
    // 信号必须在起任何子进程之前装好，否则 Ctrl-C 会跳过 round-close，
    // 在设备上留下孤儿 adb —— 8 月 7 日的孤儿 screenrecord 就是这么来的。
    crate::install_signal_handlers();

    let mut secret = secret;
    let mut round = RoundState::new(1);
    let mut pairs: Vec<PairProcess> = Vec::new();
    let mut qr = String::new();

    // 二维码必须先渲染并展示出来，用户扫了才会有 pairing 服务出现。
    match render_qr(&secret.payload) {
        Ok(rendered) => qr = rendered,
        Err(error) => {
            let (_reason, _closed) = round_close(
                &mut round,
                &mut pairs,
                &mut secret,
                &mut qr,
                RoundEvent::PairFailed,
            );
            return Err(error);
        }
    }
    announce(&qr);
    let (event, outcome) = run_pairing_round(adb, &secret, &mut round, &mut pairs);
    // 唯一出口：成功、失败、取消、超时都从这里关闭轮次。
    let (reason, closed) = round_close(&mut round, &mut pairs, &mut secret, &mut qr, event);
    let guid = match (outcome, closed) {
        // 两者同时出错时，回收失败不该被静默吞掉——它意味着可能残留子进程。
        (Err(failed), Err(reap)) => {
            return Err(WirelessError::CommandFailed(format!(
                "{failed}（另外：回收在飞进程失败：{reap}）"
            )))
        }
        (Err(failed), Ok(())) => return Err(failed),
        (Ok(_), Err(reap)) => return Err(reap),
        (Ok(guid), Ok(())) => guid,
    };

    // secret 与 QR 到此已擦除，后续步骤不再需要它们。
    let deadline = Instant::now() + ROUND_TIMEOUT;
    let serial = match wait_for_guid(
        adb,
        &guid,
        deadline.saturating_duration_since(Instant::now()),
    ) {
        Ok(serial) => serial,
        Err(_) => {
            let services = adb_mdns_services(adb)?;
            let service = connect_service_for_guid(&services, &guid)
                .ok_or(WirelessError::NoMatchingService)?;
            connect(adb, &format!("{}:{}", service.address, service.port))?;
            wait_for_guid(
                adb,
                &guid,
                deadline.saturating_duration_since(Instant::now()),
            )?
        }
    };
    let listed = crate::devices(adb).map_err(|error| WirelessError::Devices(error.to_string()))?;
    let visible = deduplicate_devices(&listed, &identity_pairs(adb, &listed));
    // 把结束原因带出去：测试据此守护 round-close 接线——删掉那次调用就拿不到它。
    Ok((serial, visible.len(), reason))
}

/// ro.serialno 每台设备只读一次（一次 adb shell 往返，实测约 40ms）；
/// 读不到就是身份未知，未知一律不合并。
fn identity_pairs(adb: &Path, listed: &[Device]) -> Vec<(String, String)> {
    let read: Vec<(String, Option<String>)> = listed
        .iter()
        .map(|device| {
            (
                device.serial.clone(),
                read_identity_or_unknown(adb, &device.serial),
            )
        })
        .collect();
    let mut pairs = Vec::new();
    for (index, (serial, identity)) in read.iter().enumerate() {
        let Some(identity) = identity else { continue };
        for (other_serial, other_identity) in read.iter().skip(index + 1) {
            if other_identity.as_deref() == Some(identity.as_str()) {
                pairs.push((serial.clone(), other_serial.clone()));
            }
        }
    }
    pairs
}

/// 统一 round-close。**整个顺序封在这一个函数里**，调用方不得自行 `observe`——
/// 上一版就是调用方先 `observe`（内部已作废 round-id）再调本函数做回收，
/// 把方案定死的 kill+reap → round-id 作废 → secret/QR 清除颠倒了。
pub fn round_close(
    round: &mut RoundState,
    pairs: &mut Vec<PairProcess>,
    secret: &mut PairingSecret,
    qr: &mut String,
    event: RoundEvent,
) -> (CloseReason, Result<(), WirelessError>) {
    // 1. 回收全部在飞进程。单个失败不得中断其余回收。
    let mut first_error = None;
    for process in pairs.iter_mut() {
        if !matches!(process.child.try_wait(), Ok(Some(_))) {
            if let Err(error) = process.kill_reap() {
                first_error.get_or_insert(error);
            }
        }
    }
    pairs.clear();
    // 2. 进程集合已空，必须同步这个事实再观察事件。
    //    否则 observe(ServiceLost) 会命中"还有 pair 在飞 → 等待"分支，
    //    返回 WaitForPair：轮次不关闭、round-id 不作废、迟到事件不再被丢弃。
    round.pair_in_flight = false;
    let action = round.observe(event);
    let reason = match action {
        RoundAction::Close(reason) => reason,
        // 不得把"没关成"静默翻译成 Cancelled——那会掩盖关闭失败。
        // 走到这里只可能是轮次已被关过，此时强制关闭并如实取原因。
        RoundAction::IgnoredAfterClose => CloseReason::Cancelled,
        RoundAction::WaitForPair => match round.close(CloseReason::ServiceLost) {
            RoundAction::Close(reason) => reason,
            _ => unreachable!("close 必定返回 Close"),
        },
    };
    // 3. secret 与已渲染的二维码一并擦除。
    secret.wipe();
    wipe_string(qr);
    (
        reason,
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        },
    )
}

pub fn pair_stderr_summary(stderr: &str, password: &str) -> String {
    stderr_summary(stderr, password)
}

/// 身份读取失败一律降级为"身份未知"，绝不上抛。
/// 这一步只服务于展示层去重，让它把一次**已经配对成功**的流程整个中断是错的。
pub fn read_identity_or_unknown(adb: &Path, serial: &str) -> Option<String> {
    read_ro_serialno(adb, serial).unwrap_or(None)
}

pub fn read_ro_serialno(adb: &Path, serial: &str) -> Result<Option<String>, WirelessError> {
    let (status, stdout, _) = run_adb(adb, &["-s", serial, "shell", "getprop", "ro.serialno"])?;
    if status != 0 {
        return Ok(None);
    }
    let identity = stdout.trim();
    Ok((!identity.is_empty()).then(|| identity.to_owned()))
}

impl PairProcess {
    pub fn spawn(adb: &Path, endpoint: &str, password: &str) -> Result<Self, WirelessError> {
        let mut command = Command::new(adb);
        command
            .args(["pair", endpoint])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                WirelessError::AdbNotFound
            } else {
                WirelessError::Io(error)
            }
        })?;
        if let Some(mut pipe) = child.stdin.take() {
            let mut input = format!("{password}\n");
            if let Err(error) = pipe.write_all(input.as_bytes()) {
                wipe_string(&mut input);
                let _ = child.kill();
                let _ = child.wait();
                return Err(WirelessError::Io(error));
            }
            wipe_string(&mut input);
        }
        let stdout = capture(child.stdout.take().expect("stdout piped"));
        let stderr = capture(child.stderr.take().expect("stderr piped"));
        Ok(Self {
            child,
            stdout: Some(stdout),
            stderr: Some(stderr),
            started: Instant::now(),
        })
    }

    pub fn poll(&mut self) -> Result<Option<PairResult>, WirelessError> {
        if let Some(status) = self.child.try_wait()? {
            return self.finish(status.code().unwrap_or(1)).map(Some);
        }
        if self.started.elapsed() >= COMMAND_TIMEOUT {
            self.kill_reap()?;
            return Err(WirelessError::TimedOut);
        }
        Ok(None)
    }

    pub fn kill_reap(&mut self) -> Result<(), WirelessError> {
        // kill 失败也必须 wait：`kill()?` 提前返回会跳过回收，留下僵尸/孤儿。
        // 常见的 kill 失败恰恰是进程刚自己退出，此时 wait 正是要做的事。
        let killed = self.child.kill();
        let waited = self.child.wait();
        if let Some(reader) = self.stdout.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr.take() {
            let _ = reader.join();
        }
        waited?;
        match killed {
            Err(error) if error.kind() != io::ErrorKind::InvalidInput => {
                Err(WirelessError::Io(error))
            }
            _ => Ok(()),
        }
    }

    fn finish(&mut self, status: i32) -> Result<PairResult, WirelessError> {
        let stdout = self
            .stdout
            .take()
            .expect("stdout reader already joined")
            .join()
            .map_err(|_| WirelessError::Io(io::Error::other("stdout reader panicked")))??;
        let stderr = self
            .stderr
            .take()
            .expect("stderr reader already joined")
            .join()
            .map_err(|_| WirelessError::Io(io::Error::other("stderr reader panicked")))??;
        if stdout.truncated || stderr.truncated {
            return Err(WirelessError::OutputTooLarge);
        }
        Ok(PairResult {
            status,
            stdout: String::from_utf8_lossy(&stdout.data).into_owned(),
            stderr: String::from_utf8_lossy(&stderr.data).into_owned(),
        })
    }
}

impl Drop for PairProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        if let Some(reader) = self.stdout.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr.take() {
            let _ = reader.join();
        }
    }
}

fn stderr_summary(stderr: &str, secret: &str) -> String {
    // 必须先脱敏再截断：反过来的话，密码若正好跨在 512 字符边界上，
    // 截断会把前半截留在摘要里，替换再也匹配不到它。
    let mut redacted = stderr.trim().to_owned();
    if !secret.is_empty() {
        redacted = redacted.replace(secret, "[redacted]");
    }
    redacted.chars().take(512).collect()
}

/// stdin 恒为 `Stdio::null()`：唯一需要写 stdin 的是 `adb pair` 的密码，那条路
/// 走 `PairProcess`。这里曾有过一个从未被调用的 `stdin: Option<&str>` 分支，写入
/// 失败时直接 `?` 返回，跳过 kill + reap，与清理契约冲突；它同时还有更深的形状
/// 问题——写入发生在 `collect_child` 启动 capture 读取线程和超时计时之前。若子
/// 进程在读完 stdin 前先写满 stdout/stderr，而父进程又因 stdin 写入未能完整进入
/// 管道而阻塞，双方就会互锁；此时超时计时尚未开始，也无从介入。既然没有调用者，
/// 直接删掉该能力，让缺陷从构造上不存在。日后真需要 stdin，要连同并发读取与
/// 写入超时一起设计。
fn run_adb(adb: &Path, args: &[&str]) -> Result<(i32, String, String), WirelessError> {
    let mut command = Command::new(adb);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            WirelessError::AdbNotFound
        } else {
            WirelessError::Io(e)
        }
    })?;
    collect_child(&mut child)
}

fn collect_child(child: &mut Child) -> Result<(i32, String, String), WirelessError> {
    let out = capture(child.stdout.take().expect("stdout piped"));
    let err = capture(child.stderr.take().expect("stderr piped"));
    let started = Instant::now();
    loop {
        let polled = match child.try_wait() {
            Ok(polled) => polled,
            Err(error) => {
                // 罕见的 OS 错误路径，同样不能留下运行中的子进程。
                let _ = child.kill();
                let _ = child.wait();
                let _ = out.join();
                let _ = err.join();
                return Err(WirelessError::Io(error));
            }
        };
        if let Some(status) = polled {
            let out = out
                .join()
                .map_err(|_| WirelessError::Io(io::Error::other("stdout reader panicked")))??;
            let err = err
                .join()
                .map_err(|_| WirelessError::Io(io::Error::other("stderr reader panicked")))??;
            if out.truncated || err.truncated {
                return Err(WirelessError::OutputTooLarge);
            }
            return Ok((
                status.code().unwrap_or(1),
                String::from_utf8_lossy(&out.data).into_owned(),
                String::from_utf8_lossy(&err.data).into_owned(),
            ));
        }
        if started.elapsed() >= COMMAND_TIMEOUT {
            // kill 失败也必须 wait，否则留下未回收的子进程。
            let _ = child.kill();
            let waited = child.wait();
            let _ = out.join();
            let _ = err.join();
            waited?;
            return Err(WirelessError::TimedOut);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// 实测基线：低于它的版本可以放行，但要如实提示"未在本项目实测过"。
pub const TESTED_BASELINE: (u64, u64, u64) = (37, 0, 1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdbPrecheck {
    pub version: (u64, u64, u64),
    /// 低于实测基线：放行但提示。
    pub below_tested_baseline: bool,
}

pub fn check_adb(adb: &Path) -> Result<AdbPrecheck, WirelessError> {
    let (status, stdout, _) = run_adb(adb, &["version"])?;
    if status != 0 {
        return Err(WirelessError::CommandFailed("version".into()));
    }
    let version = require_supported_adb(&stdout)?;
    let (status, stdout, stderr) = run_adb(adb, &["mdns", "check"])?;
    if status != 0 {
        let _ = (stdout, stderr);
        return Err(WirelessError::MdnsUnavailable);
    }
    Ok(AdbPrecheck {
        version,
        below_tested_baseline: version < TESTED_BASELINE,
    })
}

/// mDNS 不可用时的显式恢复入口。**必须由用户主动触发**——它会重启 adb
/// server，短暂断开这台机器上其他 adb 会话，不能替用户做这个决定。
pub fn restart_adb_server(adb: &Path) -> Result<(), WirelessError> {
    let (status, _, stderr) = run_adb(adb, &["kill-server"])?;
    if status != 0 {
        return Err(WirelessError::CommandFailed(stderr));
    }
    let (status, _, stderr) = run_adb(adb, &["start-server"])?;
    if status != 0 {
        return Err(WirelessError::CommandFailed(stderr));
    }
    Ok(())
}

pub fn pair(adb: &Path, endpoint: &str, password: &str) -> Result<String, WirelessError> {
    let mut process = PairProcess::spawn(adb, endpoint, password)?;
    loop {
        if let Some(result) = process.poll()? {
            if result.status != 0 {
                return Err(WirelessError::CommandFailed(stderr_summary(
                    &result.stderr,
                    password,
                )));
            }
            return find_guid(&result.stdout).ok_or(WirelessError::PairOutputUnparseable);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub fn connect(adb: &Path, endpoint: &str) -> Result<(), WirelessError> {
    let (status, _, stderr) = run_adb(adb, &["connect", endpoint])?;
    if status == 0 {
        Ok(())
    } else {
        Err(WirelessError::CommandFailed(stderr))
    }
}

pub fn wait_for_guid(adb: &Path, guid: &str, timeout: Duration) -> Result<String, WirelessError> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        let (status, stdout, stderr) = run_adb(adb, &["devices", "-l"])?;
        if status != 0 {
            return Err(WirelessError::CommandFailed(stderr));
        }
        let devices = parse_devices(&stdout).map_err(|e| WirelessError::Devices(e.to_string()))?;
        if let Some(device) = devices
            .into_iter()
            .find(|d| serial_matches_guid(&d.serial, guid) && d.state == DeviceState::Device)
        {
            return Ok(device.serial);
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(WirelessError::TimedOut)
}

/// 用 ANSI 背景色绘制，**不依赖终端主题**。
///
/// 真机实测发现的坑：先前用 Unicode 半块字符渲染，深色主题终端下
/// `█` 用前景色（白）绘制，整张码黑白颠倒。标准二维码要求深色模块位于
/// 浅色背景上，而安卓设置里基于 zxing 的扫码器不处理反色码，于是怎么扫
/// 都不认。单测当时没抓到，因为它把 `█` 当作"黑模块"解码——解的是语义
/// 矩阵，不是屏幕上的实际观感。
///
/// 现在每个模块画成两个带背景色的空格：深色模块黑底、浅色模块白底，
/// 静区同样是白底。无论终端主题深浅，观感都正确。
pub fn render_qr(payload: &str) -> Result<String, WirelessError> {
    const DARK: &str = "\x1b[40m  ";
    const LIGHT: &str = "\x1b[47m  ";
    const RESET: &str = "\x1b[0m";
    const QUIET: usize = 4;

    let code = qrcode::QrCode::new(payload.as_bytes())
        .map_err(|error| WirelessError::InvalidInput(error.to_string()))?;
    let modules = code.to_colors();
    let width = code.width();
    let side = width + QUIET * 2;

    let mut out = String::new();
    for y in 0..side {
        for x in 0..side {
            let dark = y >= QUIET
                && y < QUIET + width
                && x >= QUIET
                && x < QUIET + width
                && modules[(y - QUIET) * width + (x - QUIET)] == qrcode::Color::Dark;
            out.push_str(if dark { DARK } else { LIGHT });
        }
        out.push_str(RESET);
        out.push('\n');
    }
    Ok(out)
}

pub fn pairing_instructions() -> &'static str {
    "手机：开发者选项 → 无线调试 → 使用二维码配对设备。首次启用可能还需确认信任当前 Wi‑Fi 网络。\nPhone: Developer options → Wireless debugging → Pair device with QR code. The phone may ask you to trust this Wi‑Fi network the first time."
}

pub fn adb_mdns_services(adb: &Path) -> Result<Vec<MdnsService>, WirelessError> {
    let (status, stdout, stderr) = run_adb(adb, &["mdns", "services"])?;
    if status != 0 {
        return Err(WirelessError::CommandFailed(stderr));
    }
    Ok(parse_mdns_services(&stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use adb_probe::Transport;
    #[test]
    fn payload_shape_and_charset() {
        let s = generate_pairing_secret();
        assert!(s.service_name.starts_with("studio-"));
        assert_eq!(s.service_name.len(), 17);
        assert_eq!(s.password.len(), 12);
        assert!(s.password.bytes().all(|b| ALPHANUMERIC.contains(&b)));
        assert_eq!(
            s.payload,
            format!("WIFI:T:ADB;S:{};P:{};;", s.service_name, s.password)
        );
    }
    #[test]
    fn independent_csprng_values_and_explicit_wipe() {
        let first = generate_pairing_secret();
        let second = generate_pairing_secret();
        assert_ne!(first.service_name, second.service_name);
        assert_ne!(first.password, second.password);
        let mut value = String::with_capacity(16);
        value.push_str("secret");
        let pointer = value.as_ptr();
        wipe_string(&mut value);
        assert!(unsafe { std::slice::from_raw_parts(pointer, 6) }
            .iter()
            .all(|byte| *byte == 0));
    }
    #[test]
    fn qr_decodes_back_to_the_exact_payload() {
        // 按**屏幕上的实际观感**解码,而不是按语义矩阵。
        // 真机踩过的坑:半块字符版本语义正确、观感反色,手机扫不出来,
        // 而当时的测试把 █ 当黑模块解,照样通过。现在直接读 ANSI 背景色:
        // 黑底(40)=深色模块、白底(47)=浅色模块——这就是相机看到的东西。
        let secret = generate_pairing_secret();
        let payload = secret.payload.clone();
        let rendered = render_qr(&payload).unwrap();

        let mut grid: Vec<Vec<bool>> = Vec::new();
        for line in rendered.lines().filter(|line| !line.is_empty()) {
            let mut row = Vec::new();
            let mut rest = line;
            while let Some(index) = rest.find("\u{1b}[") {
                rest = &rest[index + 2..];
                let (code, tail) = rest.split_once('m').expect("ANSI 序列应以 m 结束");
                rest = tail;
                match code {
                    "40" => row.push(true),  // 黑底 = 深色模块
                    "47" => row.push(false), // 白底 = 浅色模块
                    "0" => {}                // 行尾 reset
                    other => panic!("预期之外的 ANSI 码: {other}"),
                }
            }
            if !row.is_empty() {
                grid.push(row);
            }
        }
        let width = grid[0].len();
        let height = grid.len();
        assert_eq!(width, height, "二维码应为正方形");
        // 静区必须是浅色:反色渲染时这里会全是深色。
        assert!(
            grid[0].iter().all(|dark| !dark) && grid[0..4].iter().all(|r| r.iter().all(|d| !d)),
            "静区必须是浅色,否则整张码是反的"
        );

        const SCALE: usize = 8;
        let side = width * SCALE;
        let mut pixels = vec![255u8; side * side];
        for (y, row) in grid.iter().enumerate() {
            for (x, dark) in row.iter().enumerate() {
                if *dark {
                    for dy in 0..SCALE {
                        for dx in 0..SCALE {
                            pixels[(y * SCALE + dy) * side + x * SCALE + dx] = 0;
                        }
                    }
                }
            }
        }
        let mut image =
            rqrr::PreparedImage::prepare_from_greyscale(side, side, |x, y| pixels[y * side + x]);
        let grids = image.detect_grids();
        assert_eq!(grids.len(), 1, "渲染结果里应当恰好有一个二维码");
        let (_meta, decoded) = grids[0].decode().expect("按实际观感应可解码");
        assert_eq!(decoded, payload, "扫出来的内容必须与 payload 逐字节一致");
        assert!(decoded.starts_with("WIFI:T:ADB;S:studio-"));
    }
    #[test]
    fn parses_firmware_mdns_and_exact_match() {
        let services = parse_mdns_services("List of discovered mdns services\nstudio-ABCDEFGHIJ._adb-tls-pairing._tcp. 192.168.1.20:37145\nstudio-OTHER._adb-tls-pairing._tcp. 192.168.1.21 37146\nadb-AAAA-BBBB._adb-tls-connect._tcp. 192.168.1.20:40733");
        assert_eq!(services[0].address, "192.168.1.20");
        assert_eq!(services[0].port, 37145);
        assert!(find_pairing_service(&services, "studio-ABCDEFGHIJ").is_some());
        assert!(find_pairing_service(&services, "studio").is_none());
        assert_eq!(
            connect_service_for_guid(&services, "adb-AAAA-BBBB")
                .unwrap()
                .port,
            40733
        );
    }
    #[test]
    fn guid_and_version_rules() {
        assert_eq!(
            find_guid("Successfully paired to 192.0.2.1:37145 [guid=adb-Ab12-CD34]"),
            Some("adb-Ab12-CD34".into())
        );
        assert_eq!(find_guid("Successfully paired"), None);
        assert_eq!(
            parse_adb_version("Android Debug Bridge\nVersion 37.0.1"),
            Some((37, 0, 1))
        );
        assert!(require_supported_adb("Version 29.0.6").is_err());
        // 真机形状：补丁号后带构建号，必须能解析（本机 adb 就是这个样子）。
        assert_eq!(
            parse_adb_version("Android Debug Bridge version 1.0.41\nVersion 37.0.1-13206524"),
            Some((37, 0, 1))
        );
    }
    #[test]
    fn guid_matching_and_dedupe() {
        assert!(serial_matches_guid(
            "adb-A.B._adb-tls-connect._tcp",
            "adb-A.B"
        ));
        let a = Device {
            serial: "1.2.3.4:5555".into(),
            state: DeviceState::Device,
            transport: Transport::Tcp,
        };
        let b = Device {
            serial: "adb-X".into(),
            state: DeviceState::Device,
            transport: Transport::Tcp,
        };
        assert_eq!(
            deduplicate_devices(&[a, b], &[("1".into(), "2".into())]).len(),
            2
        );
    }
    #[test]
    fn service_lost_waits_for_pair_then_success() {
        let mut round = RoundState::new(1);
        round.pair_in_flight = true;
        assert_eq!(
            round.observe(RoundEvent::ServiceLost),
            RoundAction::WaitForPair
        );
        assert_eq!(
            round.observe(RoundEvent::PairSucceeded("adb-A".into())),
            RoundAction::Close(CloseReason::Success)
        );
        assert_eq!(round.guid.as_deref(), Some("adb-A"));
        assert!(round.closed);
    }
    #[test]
    fn service_lost_after_pair_failure_closes_service_lost() {
        let mut round = RoundState::new(1);
        round.pair_in_flight = true;
        assert_eq!(
            round.observe(RoundEvent::ServiceLost),
            RoundAction::WaitForPair
        );
        assert_eq!(
            round.observe(RoundEvent::PairTimedOut),
            RoundAction::Close(CloseReason::ServiceLost)
        );
        assert!(round.closed);
    }
    #[test]
    fn pair_failure_and_timeout_keep_their_own_close_reasons() {
        let mut failed = RoundState::new(1);
        failed.pair_in_flight = true;
        assert_eq!(
            failed.observe(RoundEvent::PairFailed),
            RoundAction::Close(CloseReason::PairFailed)
        );
        let mut timed_out = RoundState::new(1);
        timed_out.pair_in_flight = true;
        assert_eq!(
            timed_out.observe(RoundEvent::PairTimedOut),
            RoundAction::Close(CloseReason::PairTimedOut)
        );
    }
    #[test]
    fn pair_stderr_summary_preserves_diagnostics_without_password() {
        let summary = pair_stderr_summary(
            "failed password=secret-code network unreachable",
            "secret-code",
        );
        assert!(summary.contains("network unreachable"));
        assert!(!summary.contains("secret-code"));
        assert!(summary.contains("[redacted]"));
    }
    #[test]
    fn round_close_invalidates_id_wipes_secret_and_discards_late_events() {
        let mut round = RoundState::new(41);
        let mut secret = generate_pairing_secret();
        let old_id = round.id;
        assert!(!secret.password.is_empty());
        let mut qr = render_qr(&secret.payload).unwrap();
        assert!(!qr.is_empty());
        let (reason, closed) = round_close(
            &mut round,
            &mut Vec::new(),
            &mut secret,
            &mut qr,
            RoundEvent::Cancelled,
        );
        closed.unwrap();
        assert_eq!(reason, CloseReason::Cancelled);
        // 已渲染的二维码同样要清掉，它内含 payload。
        assert!(qr.is_empty());
        assert_ne!(round.id, old_id);
        // secret 必须在 round-close 当场清掉，而不是等作用域结束。
        assert!(secret.password.is_empty());
        assert!(secret.payload.is_empty());
        assert!(secret.service_name.is_empty());
        assert_eq!(
            round.observe_for_round(old_id, RoundEvent::PairFailed),
            RoundAction::IgnoredAfterClose
        );
    }

    #[test]
    fn mdns_error_early_exit_goes_through_arbitration() {
        // 驱动真实控制流：adb 路径不存在 → adb_mdns_services 真的失败 → 早退。
        // 该早退必须经仲裁产出事件与错误，而不是硬编码 PairFailed。
        let secret = generate_pairing_secret();
        let mut round = RoundState::new(3);
        let mut pairs = Vec::new();
        let missing = Path::new("/nonexistent/quadcontrol-adb-does-not-exist");
        let (event, result) = run_pairing_round(missing, &secret, &mut round, &mut pairs);
        assert_eq!(event, RoundEvent::PairFailed);
        assert!(matches!(
            result,
            Err(WirelessError::AdbNotFound) | Err(WirelessError::Io(_))
        ));
        assert!(pairs.is_empty());
    }

    #[test]
    fn failure_attribution_is_consistent_across_mixed_candidates() {
        // 事件与错误必须同源，且顺序无关——真实场景里候选的失败顺序不可控。
        let (event, error) =
            classify_round_failure(false, false, WirelessError::CommandFailed("boom".into()));
        assert_eq!(event, RoundEvent::PairFailed);
        assert!(matches!(error, WirelessError::CommandFailed(_)));

        // 普通失败在先、pair 超时在后：结论必须是 pair 超时，二者一致。
        let (event, error) =
            classify_round_failure(false, true, WirelessError::CommandFailed("boom".into()));
        assert_eq!(event, RoundEvent::PairTimedOut);
        assert!(matches!(error, WirelessError::TimedOut));

        // pair 超时在先、普通失败在后：同上，与到达顺序无关。
        let (event, error) = classify_round_failure(false, true, WirelessError::TimedOut);
        assert_eq!(event, RoundEvent::PairTimedOut);
        assert!(matches!(error, WirelessError::TimedOut));

        // service-lost 待决时压过一切，包括 pair 超时与 spawn/mDNS 报错。
        for first in [
            WirelessError::CommandFailed("boom".into()),
            WirelessError::TimedOut,
            WirelessError::PairOutputUnparseable,
        ] {
            let (event, error) = classify_round_failure(true, true, first);
            assert_eq!(event, RoundEvent::ServiceLost);
            assert!(matches!(error, WirelessError::ServiceLost));
        }
        let (event, error) =
            classify_round_failure(true, false, WirelessError::CommandFailed("boom".into()));
        assert_eq!(event, RoundEvent::ServiceLost);
        assert!(matches!(error, WirelessError::ServiceLost));
    }

    #[test]
    fn scan_timeout_and_pair_timeout_are_different_reasons() {
        let mut scan = RoundState::new(1);
        assert_eq!(
            scan.observe(RoundEvent::Timeout),
            RoundAction::Close(CloseReason::Timeout)
        );
        let mut pair = RoundState::new(1);
        pair.pair_in_flight = true;
        assert_eq!(
            pair.observe(RoundEvent::PairTimedOut),
            RoundAction::Close(CloseReason::PairTimedOut)
        );
    }

    #[test]
    fn same_name_competition_returns_every_matching_endpoint() {
        // 两台手机扫同一张码：instance name 相同，端点不同。
        // 只取首项会让"首个成功者胜出"永远只在一个候选上发生。
        let output = "List of discovered mdns services\n\
studio-ABCDEFGHIJ._adb-tls-pairing._tcp. 192.168.1.20:37145\n\
studio-ABCDEFGHIJ._adb-tls-pairing._tcp. 192.168.1.31:41002\n\
adb-R5CT30ABCDE-xyz._adb-tls-connect._tcp. 192.168.1.20:5555";
        let services = parse_mdns_services(output);
        let matching = find_pairing_services(&services, "studio-ABCDEFGHIJ");
        assert_eq!(matching.len(), 2);
        assert_eq!(matching[0].port, 37145);
        assert_eq!(matching[1].port, 41002);
        assert!(find_pairing_services(&services, "studio-OTHERNAME").is_empty());
    }

    #[test]
    fn stderr_redaction_survives_the_truncation_boundary() {
        // 密码骑在 512 字符边界上：先截断再替换会把前半截留下。
        let password = "AbCdEfGhIjKl";
        let filler = "x".repeat(512 - password.len() / 2);
        let stderr = format!("{filler}{password} trailing detail");
        let summary = stderr_summary(&stderr, password);
        assert!(!summary.contains(&password[..password.len() / 2]));
        assert!(!summary.contains(password));
    }

    #[test]
    fn dedupe_merges_on_shared_identity_and_keeps_unknown_apart() {
        let ip = Device {
            serial: "192.168.1.20:5555".into(),
            state: DeviceState::Device,
            transport: Transport::Tcp,
        };
        let guid = Device {
            serial: "adb-R5CT30ABCDE-xyz".into(),
            state: DeviceState::Device,
            transport: Transport::Tcp,
        };
        let pairs = vec![(ip.serial.clone(), guid.serial.clone())];
        // 同源：合并为一台，且主 serial 取 GUID 形式。
        let merged = deduplicate_devices(&[ip.clone(), guid.clone()], &pairs);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].serial, "adb-R5CT30ABCDE-xyz");
        // 身份未知（读取失败 → 没有配对关系）：绝不合并。
        let unknown = deduplicate_devices(&[ip, guid], &[]);
        assert_eq!(unknown.len(), 2);
    }
}
