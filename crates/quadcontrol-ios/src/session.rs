//! iOS 会话监督：监督线程**独占**所有子进程，外部只拿句柄。
//!
//! # 进程与数据流（Win/Linux）
//!
//! ```text
//! IosSession 监督线程（独占所有子进程）
//!  ├─ ios tunnel start --userspace --udid U   （iOS 17+ 必需，长驻）
//!  ├─ ios runwda --udid U --bundleid B …      （长驻）
//!  ├─ ios forward --udid U <L8100> 8100       （WDA HTTP）
//!  ├─ ios forward --udid U <L9100> 9100       （WDA MJPEG）
//!  └─ MJPEG 代理（进程内，127.0.0.1:<Lproxy>）
//! ```
//!
//! # 为什么启动要在监督线程里做
//!
//! 两个理由，都是硬的：
//! 1. Linux 的 `PR_SET_PDEATHSIG` 是在**发起 fork 的那个线程**退出时触发的。若由
//!    调用方线程（GUI 的 `spawn_blocking` worker）fork 完就返回，四个子进程会立刻
//!    收到 SIGTERM——会话还没起来就死了。
//! 2. `/status` 轮询有 60 s 预算，不能把调用方堵在那里。
//!
//! 所以 [`spawn_supervised`] 只做**预检**（版本钳制、隧道端口探测），随即返回处于
//! [`IosSessionStatus::Starting`] 的句柄；真正的启动序列在监督线程里跑。
//!
//! # 退出序列
//!
//! 停代理 → `DELETE /session`（≤ 1 s，礼貌性）→ 对四个子进程走**并行**终止梯子
//! （[`quadcontrol_process::terminate_all`]）→ 全部 reap。并行是评审阻塞项 B1 的
//! 裁决：串行梯子四个进程最坏 20 s，装不进 [`IOS_SHUTDOWN_DEADLINE`]。
//! 任一子进程自亡时，监督线程按**同一序列**收回其余。

use crate::mjpeg::{FrameStats, Proxy};
use crate::wda;
use crate::{Error, DEFAULT_TUNNEL_INFO_PORT};
use quadcontrol_process::{
    configure_process, drain_stderr, terminate_all, RingBuffer, STDERR_LIMIT,
};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// 会话关闭的统一预算：覆盖并行梯子的最多 5 秒，并留出收尾余量。
pub const IOS_SHUTDOWN_DEADLINE: Duration = Duration::from_secs(7);
/// `/status` 就绪轮询的默认总预算。
pub const DEFAULT_STATUS_BUDGET: Duration = Duration::from_secs(60);
/// 进程退出后再给 stderr 抽取线程的收尾时间。
const STDERR_SETTLE_BUDGET: Duration = Duration::from_millis(200);
/// MJPEG 默认参数；**只作用在编码器侧**，与设备刷新率无关（红线）。
pub const MJPEG_FPS: u32 = 15;
pub const MJPEG_QUALITY: u32 = 50;

/// runwda 早退时的 stderr 关键字分类表。
///
/// **未真机校准**：这些串来自 WDA / Xcode 的常见错误文案，本机没有条件把每种
/// 失败都复现一遍。真机验收时要逐条核对并回写，分类错了只会影响错误码的精细度，
/// 不影响「起不来就报错」这个主行为。
const SIGNATURE_KEYWORDS: &[&str] = &[
    "signature",
    "certificate",
    "provisioning",
    "expired",
    "codesign",
    "ineligible",
];
const NOT_INSTALLED_KEYWORDS: &[&str] = &[
    "not installed",
    "notinstalled",
    "could not find",
    "no such bundle",
    "application not found",
];

/// WDA 的三个 bundle 标识。个人团队签不了 WDA 的默认 bundle id，所以必须自定义。
#[derive(Debug, Clone)]
pub struct WdaIds {
    pub bundle_id: String,
    pub testrunner_id: String,
    pub xctestconfig: String,
}

/// 会话模式。
#[derive(Debug, Clone)]
pub enum LaunchMode {
    /// 起完整链路：tunnel + runwda + forward×2 + 代理。
    Launch {
        ios: PathBuf,
        wda: WdaIds,
        env: Vec<(String, String)>,
    },
    /// 接上已经在跑的端口，不起任何子进程。
    ///
    /// **只存在于本库与其测试**：GUI 的 command 层不接受它（方案 §三 P4.1）。
    Attach { http_port: u16, mjpeg_port: u16 },
}

/// 会话启动参数。
///
/// 不派生 `Debug`：派生版会原样打印 UDID，任何一处 `{:?}` 都会把设备标识带进
/// 日志或 panic 消息。手写实现只打尾部四位。
#[derive(Clone)]
pub struct IosLaunchOptions {
    /// 目标设备 UDID。它会出现在 argv（`ps` 可见），但**不持久化、不进任何 DTO**。
    pub udid: String,
    pub mode: LaunchMode,
    /// go-ios 隧道信息服务端口。
    ///
    /// 做成字段而不是写死常量有实测理由：集成测试并发跑，一旦某条用例占住真实的
    /// 28100，其它用例的正常启动就全被判成 `TunnelPortBusy`。默认值仍是
    /// [`DEFAULT_TUNNEL_INFO_PORT`]。
    pub tunnel_info_port: u16,
    /// `/status` 就绪轮询的总预算；测试用短预算。
    pub status_budget: Duration,
}

impl std::fmt::Debug for IosLaunchOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IosLaunchOptions")
            .field("udid", &self.udid_redacted())
            .field("mode", &self.mode)
            .field("tunnel_info_port", &self.tunnel_info_port)
            .field("status_budget", &self.status_budget)
            .finish()
    }
}

impl IosLaunchOptions {
    /// 按默认端口与预算构造。
    pub fn new(udid: impl Into<String>, mode: LaunchMode) -> Self {
        Self {
            udid: udid.into(),
            mode,
            tunnel_info_port: DEFAULT_TUNNEL_INFO_PORT,
            status_budget: DEFAULT_STATUS_BUDGET,
        }
    }
}

/// 子进程种类，用于把「谁先死了」讲清楚。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildKind {
    Tunnel,
    RunWda,
    ForwardHttp,
    ForwardMjpeg,
}

impl ChildKind {
    fn label(self) -> &'static str {
        match self {
            Self::Tunnel => "tunnel",
            Self::RunWda => "runwda",
            Self::ForwardHttp => "forward(8100)",
            Self::ForwardMjpeg => "forward(9100)",
        }
    }
}

/// 会话结束的原因。
///
/// 刻意是枚举而不是字符串：stderr 尾部含 UDID 与 bundle id，**iOS 的 stderr_tail
/// 永不进任何会被序列化的状态**（红线）。诊断文本只写本进程 stderr。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitReason {
    /// 调用方请求停止。
    Requested,
    /// 某个子进程先死了，会话跟着收摊。
    ChildExited { which: ChildKind, code: i32 },
    /// MJPEG 上游关闭（链路死亡）。
    UpstreamClosed,
}

/// 监督线程对会话生命周期的可观察快照。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IosSessionStatus {
    /// 子进程正在起、`/status` 还没通。
    Starting,
    Running {
        proxy_port: u16,
        http_port: u16,
    },
    Stopping,
    Exited {
        reason: ExitReason,
    },
    /// 失败。只带稳定错误码，不带任何可能含设备标识的文本。
    Failed {
        code: &'static str,
    },
}

enum StopCommand {
    Stop,
}

struct Shared {
    status: Mutex<IosSessionStatus>,
    stats: Mutex<FrameStats>,
    client: Mutex<Option<wda::Client>>,
}

/// 不暴露子进程的监督句柄；所有进程操作都由内部监督线程执行。
pub struct IosSessionHandle {
    id: u64,
    shared: Arc<Shared>,
    stop_tx: mpsc::Sender<StopCommand>,
    done_rx: Arc<Mutex<mpsc::Receiver<()>>>,
}

/// `Debug` 只打 id 与状态。**绝不打 UDID 或 stderr 尾部**：句柄会出现在
/// GUI 的日志与 panic 消息里，设备标识不能顺着这条路漏出去。
impl std::fmt::Debug for IosSessionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IosSessionHandle")
            .field("id", &self.id)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl IosSessionHandle {
    /// 创建时指定的稳定标识。
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 当前状态快照。
    pub fn status(&self) -> IosSessionStatus {
        self.shared
            .status
            .lock()
            .expect("status mutex poisoned")
            .clone()
    }

    /// 代理的计帧快照；会话没起来时是全零。
    pub fn stats(&self) -> FrameStats {
        *self.shared.stats.lock().expect("stats mutex poisoned")
    }

    /// `Running` 时返回一个可用的 [`wda::Client`] 克隆，供 GUI 的 tap / text /
    /// home / wake / screenshot 命令使用；其它状态返回 None。
    pub fn client(&self) -> Option<wda::Client> {
        self.shared
            .client
            .lock()
            .expect("client mutex poisoned")
            .clone()
    }

    /// 请求停止。重复请求及监督线程已结束时都视为成功。
    pub fn request_stop(&self) -> Result<(), Error> {
        let _ = self.stop_tx.send(StopCommand::Stop);
        Ok(())
    }
}

/// 一个被监督的子进程。
struct SupervisedChild {
    kind: ChildKind,
    child: Child,
    stderr: Arc<Mutex<RingBuffer>>,
    stderr_eof: Arc<AtomicBool>,
    stderr_thread: Option<JoinHandle<()>>,
}

impl SupervisedChild {
    /// stderr 尾部。**只用于本进程诊断与关键字分类**，绝不进状态或 DTO。
    fn stderr_tail(&self) -> String {
        self.stderr.lock().expect("stderr mutex poisoned").text()
    }

    /// 有界等待 stderr 抽取到 EOF 再取尾部；失败诊断恰恰在最后那段里。
    fn stderr_tail_settled(&self, budget: Duration) -> String {
        let deadline = Instant::now() + budget;
        while !self.stderr_eof.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        self.stderr_tail()
    }
}

/// 启动一个由监督线程独占的 iOS 会话。
///
/// 这里只做**预检**；真正的启动序列在监督线程里跑（理由见模块文档）。
/// 返回时状态是 [`IosSessionStatus::Starting`]。
pub fn spawn_supervised(options: &IosLaunchOptions, id: u64) -> Result<IosSessionHandle, Error> {
    // Windows 没有 Job Object 就收不回进程树，直接拒绝（P6 再做）。
    // 用 cfg 而不是运行时判断，保证这条分支在 windows 上编译得过。
    #[cfg(windows)]
    if matches!(options.mode, LaunchMode::Launch { .. }) {
        return Err(Error::WindowsSessionUnsupported);
    }

    if let LaunchMode::Launch { ios, .. } = &options.mode {
        // 版本钳制在**建任何子进程之前**：语法漂移要在起进程前就挡住。
        let version = crate::check_version(ios)?;
        if version.newer_than_verified {
            eprintln!(
                "warning: go-ios {version} is newer than the only verified version 1.2.1; \
                 command syntax has not been checked against it"
            );
        }
        // 隧道信息端口是每主机一份的固定端口：占用就报错，绝不静默复用别人的隧道。
        if !port_is_free(options.tunnel_info_port) {
            return Err(Error::TunnelPortBusy(options.tunnel_info_port));
        }
    }

    let shared = Arc::new(Shared {
        status: Mutex::new(IosSessionStatus::Starting),
        stats: Mutex::new(FrameStats {
            frames: 0,
            backpressure_drops: 0,
            last_frame_at: None,
        }),
        client: Mutex::new(None),
    });
    let (stop_tx, stop_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let options = options.clone();
    let thread_shared = Arc::clone(&shared);
    thread::spawn(move || {
        supervise(options, stop_rx, done_tx, thread_shared);
    });

    Ok(IosSessionHandle {
        id,
        shared,
        stop_tx,
        done_rx: Arc::new(Mutex::new(done_rx)),
    })
}

/// 探测端口是否空闲。bind 成功即释放。
fn port_is_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// 取一个空闲的本地端口给 `ios forward` 用。
///
/// bind 0 拿到端口号后立刻释放，再交给 forward 去绑——中间有竞态窗口，但这是
/// go-ios 的 CLI 契约决定的（它只接受端口号，不接受已打开的 socket）。
fn free_port() -> Result<u16, Error> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| Error::ForwardFailed(format!("could not reserve a local port: {e}")))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| Error::ForwardFailed(format!("reserved port has no address: {e}")))
}

/// 监督线程主体。
fn supervise(
    options: IosLaunchOptions,
    stop_rx: mpsc::Receiver<StopCommand>,
    done_tx: mpsc::Sender<()>,
    shared: Arc<Shared>,
) {
    let mut children: Vec<SupervisedChild> = Vec::new();
    let mut proxy: Option<Proxy> = None;

    let outcome = start_session(&options, &mut children, &mut proxy, &shared, &stop_rx);

    let reason = match outcome {
        Err(error) => {
            // 失败诊断打到本进程 stderr（含 UDID 的尾部只到这里为止），
            // 状态里只留稳定错误码。
            eprintln!("iOS session {} failed: {error}", options.udid_redacted());
            let code = error.code();
            shutdown(&mut children, &mut proxy, &shared);
            *shared.status.lock().expect("status mutex poisoned") =
                IosSessionStatus::Failed { code };
            let _ = done_tx.send(());
            return;
        }
        Ok(reason) => reason,
    };

    shutdown(&mut children, &mut proxy, &shared);
    *shared.status.lock().expect("status mutex poisoned") = IosSessionStatus::Exited { reason };
    let _ = done_tx.send(());
}

impl IosLaunchOptions {
    /// 给日志用的去标识化 UDID：只留尾部四位，完整 UDID 不进日志。
    fn udid_redacted(&self) -> String {
        crate::short_ios_name(&self.udid)
    }
}

/// 起会话并运行到结束；返回退出原因。
fn start_session(
    options: &IosLaunchOptions,
    children: &mut Vec<SupervisedChild>,
    proxy: &mut Option<Proxy>,
    shared: &Arc<Shared>,
    stop_rx: &mpsc::Receiver<StopCommand>,
) -> Result<ExitReason, Error> {
    let (http_port, mjpeg_port) = match &options.mode {
        LaunchMode::Attach {
            http_port,
            mjpeg_port,
        } => (*http_port, *mjpeg_port),
        LaunchMode::Launch { ios, wda, env } => {
            // 顺序按方案：tunnel → runwda → forward×2。
            children.push(spawn_child(
                ChildKind::Tunnel,
                ios,
                &[
                    "tunnel".into(),
                    "start".into(),
                    "--userspace".into(),
                    format!("--udid={}", options.udid),
                    format!("--tunnel-info-port={}", options.tunnel_info_port),
                ],
                env,
                Error::TunnelFailed,
            )?);
            children.push(spawn_child(
                ChildKind::RunWda,
                ios,
                &[
                    "runwda".into(),
                    format!("--udid={}", options.udid),
                    format!("--bundleid={}", wda.bundle_id),
                    format!("--testrunnerbundleid={}", wda.testrunner_id),
                    format!("--xctestconfig={}", wda.xctestconfig),
                ],
                env,
                // spawn 本身失败（不是早退）也要报成 WDA 侧的码，不能借用
                // `tunnel_failed`——那会让界面把 runwda 的问题指向隧道。
                Error::WdaUnreachable,
            )?);
            let http_port =
                spawn_forward(ChildKind::ForwardHttp, 8100, options, ios, env, children)?;
            let mjpeg_port =
                spawn_forward(ChildKind::ForwardMjpeg, 9100, options, ios, env, children)?;
            (http_port, mjpeg_port)
        }
    };

    // 轮询 `/status`。期间任一子进程早退要立刻分类报错，而不是干等 60 s。
    let mut client = wda::Client::new(http_port);
    await_status(&client, children, options.status_budget, stop_rx)?;

    client.create_session()?;
    client.configure_mjpeg(MJPEG_FPS, MJPEG_QUALITY)?;

    let started = Proxy::start(mjpeg_port)?;
    let proxy_port = started.local_port();
    *proxy = Some(started);
    *shared.client.lock().expect("client mutex poisoned") = Some(client);
    *shared.status.lock().expect("status mutex poisoned") = IosSessionStatus::Running {
        proxy_port,
        http_port,
    };

    run_until_stop(children, proxy, shared, stop_rx)
}

/// 起一条 `ios forward`，端口被抢就换一个再试；返回最终用上的本地端口。
///
/// 为什么必须重试：[`free_port`] 是「bind 0 拿号 → 立刻释放 → 交给 go-ios 去绑」，
/// 中间那段空窗期里别的进程可能把号抢走，`ios forward` 就以
/// `Address already in use` 早退。这不是测试构造出来的——并发跑测试时实测复现过，
/// 真机上多个工具同时抢临时端口也会撞。go-ios 的 CLI 只接受端口号、不接受已打开
/// 的 socket，所以这个窗口关不掉，只能换个号重试。
const FORWARD_PORT_ATTEMPTS: usize = 5;
/// 判断 forward 是否真的绑上了：起来后给它这么久做早退检查。
const FORWARD_SETTLE: Duration = Duration::from_millis(300);

fn spawn_forward(
    kind: ChildKind,
    remote: u16,
    options: &IosLaunchOptions,
    ios: &PathBuf,
    env: &[(String, String)],
    children: &mut Vec<SupervisedChild>,
) -> Result<u16, Error> {
    let mut last = String::new();
    for _ in 0..FORWARD_PORT_ATTEMPTS {
        let local = free_port()?;
        let mut child = spawn_child(
            kind,
            ios,
            &[
                "forward".into(),
                format!("--udid={}", options.udid),
                local.to_string(),
                remote.to_string(),
            ],
            env,
            Error::ForwardFailed,
        )?;

        // 端口冲突是**立刻**暴露的：给一小段时间看它有没有早退。
        let deadline = Instant::now() + FORWARD_SETTLE;
        let mut exited = false;
        while Instant::now() < deadline {
            if child.child.try_wait().ok().flatten().is_some() {
                exited = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if !exited {
            children.push(child);
            return Ok(local);
        }

        last = child.stderr_tail_settled(STDERR_SETTLE_BUDGET);
        let _ = child.child.wait();
        if !last.to_ascii_lowercase().contains("address already in use") {
            // 不是端口冲突：换端口也没用，直接如实报错。
            eprintln!("iOS {} exited; stderr tail: {last}", kind.label());
            return Err(classify_child_failure(kind, &last));
        }
        eprintln!(
            "iOS {} lost the race for port {local}; retrying",
            kind.label()
        );
    }
    Err(Error::ForwardFailed(format!(
        "`ios {}` could not bind a local port after {FORWARD_PORT_ATTEMPTS} attempts: {last}",
        kind.label()
    )))
}

/// 起一个子进程：独立进程组 + Linux 的 PDEATHSIG + stderr 有界抽取。
fn spawn_child(
    kind: ChildKind,
    ios: &PathBuf,
    args: &[String],
    env: &[(String, String)],
    wrap: impl Fn(String) -> Error,
) -> Result<SupervisedChild, Error> {
    let mut command = Command::new(ios);
    command
        .args(args)
        .envs(env.iter().map(|(key, value)| (key, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    configure_process(&mut command).map_err(Error::Process)?;
    set_parent_death_signal(&mut command);

    let mut child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::IosNotFound(ios.clone())
        } else {
            wrap(format!("could not start `ios {}`: {e}", kind.label()))
        }
    })?;

    let stderr = Arc::new(Mutex::new(RingBuffer::new(STDERR_LIMIT)));
    let shared = Arc::clone(&stderr);
    let stderr_eof = Arc::new(AtomicBool::new(false));
    let eof_flag = Arc::clone(&stderr_eof);
    let reader = child.stderr.take().expect("stderr is piped");
    let stderr_thread = thread::spawn(move || {
        drain_stderr(reader, shared);
        eof_flag.store(true, Ordering::Release);
    });

    Ok(SupervisedChild {
        kind,
        child,
        stderr,
        stderr_eof,
        stderr_thread: Some(stderr_thread),
    })
}

/// Linux：GUI 被 SIGKILL 时子进程随之退出。
///
/// `PR_SET_PDEATHSIG` 绑的是**发起 fork 的线程**，所以这一句只有在监督线程里
/// fork 才有意义（见模块文档）。macOS 没有等价机制，如实标注：macOS 开发路径
/// 不覆盖这一项。
#[cfg(target_os = "linux")]
fn set_parent_death_signal(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
}
#[cfg(not(target_os = "linux"))]
fn set_parent_death_signal(_command: &mut Command) {}

/// 轮询 `/status` 直到通、超预算、或某个子进程早退。
fn await_status(
    client: &wda::Client,
    children: &mut [SupervisedChild],
    budget: Duration,
    stop_rx: &mpsc::Receiver<StopCommand>,
) -> Result<(), Error> {
    let deadline = Instant::now() + budget;
    loop {
        if stop_rx.try_recv().is_ok() {
            return Err(Error::WdaUnreachable(
                "the session was stopped before WDA became ready".into(),
            ));
        }
        if let Some(error) = first_child_failure(children) {
            return Err(error);
        }
        if client.status().is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::WdaUnreachable(format!(
                "WDA did not answer /status within {} s",
                budget.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// 若有子进程已退出，按其 stderr 关键字分类成错误。
fn first_child_failure(children: &mut [SupervisedChild]) -> Option<Error> {
    for child in children.iter_mut() {
        let exited = child.child.try_wait().ok().flatten();
        if exited.is_none() {
            continue;
        }
        let tail = child.stderr_tail_settled(STDERR_SETTLE_BUDGET);
        // 尾部只进本进程 stderr，不进状态。
        eprintln!(
            "iOS child {} exited; stderr tail: {tail}",
            child.kind.label()
        );
        return Some(classify_child_failure(child.kind, &tail));
    }
    None
}

/// 把子进程的 stderr 尾部分类成稳定错误码。
///
/// 关键字表见 [`SIGNATURE_KEYWORDS`] / [`NOT_INSTALLED_KEYWORDS`]，**未真机校准**。
pub fn classify_child_failure(kind: ChildKind, stderr_tail: &str) -> Error {
    let lower = stderr_tail.to_ascii_lowercase();
    if kind == ChildKind::RunWda {
        if SIGNATURE_KEYWORDS.iter().any(|k| lower.contains(k)) {
            return Error::WdaSignatureExpired(
                "the WebDriverAgent signature is not valid; re-sign and reinstall it".into(),
            );
        }
        if NOT_INSTALLED_KEYWORDS.iter().any(|k| lower.contains(k)) {
            return Error::WdaNotInstalled("WebDriverAgent is not installed on the device".into());
        }
        return Error::WdaUnreachable("`ios runwda` exited before WDA became ready".into());
    }
    match kind {
        ChildKind::Tunnel => Error::TunnelFailed("`ios tunnel start` exited".into()),
        ChildKind::ForwardHttp | ChildKind::ForwardMjpeg => {
            Error::ForwardFailed(format!("`ios {}` exited", kind.label()))
        }
        ChildKind::RunWda => unreachable!("handled above"),
    }
}

/// 运行中：等停止请求、子进程自亡、或代理断流。
fn run_until_stop(
    children: &mut [SupervisedChild],
    proxy: &mut Option<Proxy>,
    shared: &Arc<Shared>,
    stop_rx: &mpsc::Receiver<StopCommand>,
) -> Result<ExitReason, Error> {
    loop {
        if stop_rx.try_recv().is_ok() {
            return Ok(ExitReason::Requested);
        }
        // 计数持续同步出去，供 GUI 的帧率标签使用。
        if let Some(proxy) = proxy.as_ref() {
            *shared.stats.lock().expect("stats mutex poisoned") = proxy.stats();
            if let Some(error) = proxy.failure() {
                return Err(error);
            }
            // 上游 EOF 没有 `failure`，但链路已经死了：必须在这里收摊，否则会话
            // 会永远停在 `Running`——画面已经黑了，状态还说「已连接」。
            if !proxy.is_alive() {
                return Ok(ExitReason::UpstreamClosed);
            }
        }
        for child in children.iter_mut() {
            if let Ok(Some(status)) = child.child.try_wait() {
                let tail = child.stderr_tail_settled(STDERR_SETTLE_BUDGET);
                eprintln!(
                    "iOS child {} exited during the session; stderr tail: {tail}",
                    child.kind.label()
                );
                return Ok(ExitReason::ChildExited {
                    which: child.kind,
                    code: status.code().unwrap_or(1),
                });
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// 退出序列：停代理 → `DELETE /session` → 并行终止梯子 → 全部 reap。
fn shutdown(children: &mut Vec<SupervisedChild>, proxy: &mut Option<Proxy>, shared: &Arc<Shared>) {
    *shared.status.lock().expect("status mutex poisoned") = IosSessionStatus::Stopping;

    // 1. 先停代理：前端画面立即断，不留一张「像是还连着」的静止画面。
    if let Some(mut proxy) = proxy.take() {
        *shared.stats.lock().expect("stats mutex poisoned") = proxy.stats();
        proxy.stop();
    }

    // 2. 礼貌性 DELETE /session（超时 ≤ 1 s）。失败无所谓，梯子才是真回收手段。
    if let Some(client) = shared.client.lock().expect("client mutex poisoned").take() {
        let _ = client.delete_session();
    }

    // 3. 并行梯子。绝不用单进程串行的 `terminate`：四个子进程会超预算。
    //    这里不安装信号处理，所以升级判断恒 false。
    if !children.is_empty() {
        let mut borrowed: Vec<&mut Child> = children.iter_mut().map(|c| &mut c.child).collect();
        if let Err(error) = terminate_all(&mut borrowed, &|| false) {
            eprintln!("iOS session termination ladder reported: {error}");
        }
    }

    // 4. 全部 reap，并**有界**回收 stderr 线程。
    //    不无条件 join：go-ios 会拉起孙进程，管道可能一直不关（同 Android 的教训）。
    for child in children.iter_mut() {
        let _ = child.child.wait();
        let _ = child.stderr_tail_settled(STDERR_SETTLE_BUDGET);
        if let Some(thread) = child.stderr_thread.take() {
            if child.stderr_eof.load(Ordering::Acquire) {
                let _ = thread.join();
            }
        }
    }
    children.clear();
}

/// 批量关闭的结果；超时项由调用方上报或后续处置。
#[derive(Debug, PartialEq, Eq)]
pub struct ShutdownReport {
    pub timed_out: Vec<u64>,
}

/// 请求所有会话停止，并以一个共享总预算等待监督线程完成。语义同 Android。
///
/// 超时只记录会话 ID，不按保存的 PID 盲目强杀：PID 复用会误伤其它进程。
pub fn shutdown_all(handles: &[IosSessionHandle], deadline: Duration) -> ShutdownReport {
    for handle in handles {
        let _ = handle.request_stop();
    }
    let deadline_at = Instant::now() + deadline;
    let mut timed_out = Vec::new();
    for handle in handles {
        let remaining = deadline_at.saturating_duration_since(Instant::now());
        let done = handle
            .done_rx
            .lock()
            .expect("done mutex poisoned")
            .recv_timeout(remaining)
            .is_ok();
        if !done {
            timed_out.push(handle.id);
        }
    }
    ShutdownReport { timed_out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runwda_stderr_is_classified_into_stable_codes() {
        let cases = [
            (
                "Failed to codesign: certificate has expired",
                "wda_signature_expired",
            ),
            ("provisioning profile is invalid", "wda_signature_expired"),
            ("WebDriverAgentRunner is not installed", "wda_not_installed"),
            ("could not find the test runner bundle", "wda_not_installed"),
            ("some unrelated failure", "wda_unreachable"),
        ];
        for (tail, expected) in cases {
            assert_eq!(
                classify_child_failure(ChildKind::RunWda, tail).code(),
                expected,
                "{tail}"
            );
        }
    }

    #[test]
    fn other_children_map_to_their_own_codes() {
        assert_eq!(
            classify_child_failure(ChildKind::Tunnel, "boom").code(),
            "tunnel_failed"
        );
        assert_eq!(
            classify_child_failure(ChildKind::ForwardHttp, "boom").code(),
            "forward_failed"
        );
        assert_eq!(
            classify_child_failure(ChildKind::ForwardMjpeg, "boom").code(),
            "forward_failed"
        );
    }

    /// 状态与原因都不能携带可能含设备标识的自由文本。
    #[test]
    fn status_carries_no_device_identifying_text() {
        let failed = IosSessionStatus::Failed {
            code: "tunnel_failed",
        };
        let exited = IosSessionStatus::Exited {
            reason: ExitReason::ChildExited {
                which: ChildKind::RunWda,
                code: 1,
            },
        };
        for status in [failed, exited] {
            let rendered = format!("{status:?}");
            assert!(!rendered.contains("REDACTEDSERIAL"), "{rendered}");
            // 没有任何自由字符串字段能塞进 stderr 尾部。
            assert!(!rendered.to_lowercase().contains("stderr"), "{rendered}");
        }
    }

    /// 任何人把 `#[derive(Debug)]` 加回 `IosLaunchOptions`，这条就红。
    #[test]
    fn launch_options_debug_never_prints_the_full_udid() {
        let options = IosLaunchOptions::new(
            "REDACTEDSERIAL-FULL-UDID-0001",
            LaunchMode::Attach {
                http_port: 1,
                mjpeg_port: 2,
            },
        );
        let rendered = format!("{options:?}");
        assert!(!rendered.contains("REDACTEDSERIAL-FULL"), "{rendered}");
        assert!(rendered.contains("0001"));
    }

    /// P4.2 要把句柄放进 Tauri 的共享 state，那里要求 `Send + Sync`。
    #[test]
    fn handle_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<IosSessionHandle>();
    }
}
