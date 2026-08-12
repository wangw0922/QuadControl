//! Safe, visible scrcpy process ownership for the desktop clients.
//!
//! Device-list parsing intentionally comes from `adb-probe`. If another shared
//! consumer appears, that parser should move to its own shared crate.

pub use adb_probe::DeviceState;
use adb_probe::{parse_devices, CommandRunner, Device, SystemRunner};
use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub mod wireless;

pub const MIN_SCRCPY: (u64, u64) = (4, 1);
pub const STDERR_LIMIT: usize = 64 * 1024;
/// 监督会话的统一关闭预算；它覆盖终止梯子的最多 5 秒，并留出收尾余量。
pub const SHUTDOWN_DEADLINE: Duration = Duration::from_secs(7);
/// 进程退出后再给 stderr 抽取线程的收尾时间，远小于关闭预算，不影响上面的期限。
const STDERR_SETTLE_BUDGET: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub enum Error {
    Usage(String),
    Adb(String),
    ScrcpyNotFound(PathBuf),
    ScrcpyVersion(String),
    InvalidPassthrough(String),
    Process(io::Error),
    NotFound(String),
    WrongState(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(s) | Self::Adb(s) | Self::ScrcpyVersion(s)
            | Self::InvalidPassthrough(s) | Self::NotFound(s) | Self::WrongState(s) => f.write_str(s),
            Self::ScrcpyNotFound(path) => write!(f, "scrcpy was not found at {}. Install scrcpy from https://github.com/Genymobile/scrcpy/releases, then try again.", path.display()),
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

pub fn resolve_adb(explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("ADB").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("adb"))
}

pub fn devices(adb: &Path) -> Result<Vec<Device>, Error> {
    let output = SystemRunner::new(adb.as_os_str())
        .run(adb_probe::AdbCommand::DevicesLong)
        .map_err(|e| Error::Adb(e.to_string()))?;
    if output.status != 0 {
        return Err(Error::Adb(format!(
            "adb devices -l failed with exit {}: {}",
            output.status,
            output.stderr.trim()
        )));
    }
    parse_devices(&output.stdout).map_err(|e| Error::Adb(e.to_string()))
}

pub fn select_device(list: &[Device], serial: Option<&str>) -> Result<String, Error> {
    if let Some(serial) = serial {
        return match list.iter().find(|d| d.serial == serial) {
            Some(d) if d.state == DeviceState::Device => Ok(serial.to_owned()),
            Some(d) => Err(Error::WrongState(format!(
                "device {serial} is {}, not device",
                state_name(&d.state)
            ))),
            None => Err(Error::NotFound(format!(
                "device {serial} is not present in adb devices -l"
            ))),
        };
    }
    let ready: Vec<_> = list
        .iter()
        .filter(|d| d.state == DeviceState::Device)
        .collect();
    match ready.as_slice() {
        [one] => Ok(one.serial.clone()),
        [] => {
            if let Some(d) = list.iter().find(|d| d.state == DeviceState::Unauthorized) {
                Err(Error::WrongState(format!(
                    "device {} is unauthorized; approve this computer on the device",
                    d.serial
                )))
            } else if let Some(d) = list.iter().find(|d| d.state == DeviceState::Offline) {
                Err(Error::WrongState(format!(
                    "device {} is offline; reconnect the device",
                    d.serial
                )))
            } else {
                Err(Error::WrongState("no adb device is ready".into()))
            }
        }
        _ => Err(Error::Usage(
            "multiple adb devices are ready; pass --serial".into(),
        )),
    }
}

fn state_name(s: &DeviceState) -> &str {
    match s {
        DeviceState::Device => "device",
        DeviceState::Offline => "offline",
        DeviceState::Unauthorized => "unauthorized",
        DeviceState::Other(s) => s,
    }
}

pub fn validate_scrcpy(scrcpy: &Path) -> Result<(), Error> {
    validate_scrcpy_with_env(scrcpy, &[])
}

pub fn validate_scrcpy_with_env(scrcpy: &Path, env: &[(String, String)]) -> Result<(), Error> {
    let mut command = Command::new(scrcpy);
    command.envs(env.iter().map(|(key, value)| (key, value)));
    let output = command
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Error::ScrcpyNotFound(scrcpy.to_path_buf())
            } else {
                Error::Process(e)
            }
        })?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        validate_version_text(&text)
    } else {
        Err(Error::ScrcpyVersion(format!("scrcpy --version failed: {}. Install scrcpy from https://github.com/Genymobile/scrcpy/releases.", text.trim())))
    }
}

pub fn validate_version_text(text: &str) -> Result<(), Error> {
    let token = text.lines().find_map(|line| {
        line.split_whitespace()
            .find(|word| word.chars().next().is_some_and(|c| c.is_ascii_digit()))
    });
    let token = token.ok_or_else(|| Error::ScrcpyVersion("could not parse scrcpy version; install stable scrcpy 4.1 or newer from https://github.com/Genymobile/scrcpy/releases".into()))?;
    if token.contains('-') {
        return Err(Error::ScrcpyVersion(format!(
            "scrcpy prerelease {token} is not supported; install stable scrcpy 4.1 or newer"
        )));
    }
    let mut parts = token.split('.').map(|p| p.parse::<u64>());
    let major = parts
        .next()
        .transpose()
        .map_err(|_| Error::ScrcpyVersion("invalid scrcpy version".into()))?
        .ok_or_else(|| Error::ScrcpyVersion("invalid scrcpy version".into()))?;
    let minor = parts
        .next()
        .transpose()
        .map_err(|_| Error::ScrcpyVersion("invalid scrcpy version".into()))?
        .unwrap_or(0);
    if (major, minor) < MIN_SCRCPY {
        return Err(Error::ScrcpyVersion(format!(
            "scrcpy {token} is too old; install stable scrcpy 4.1 or newer"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub adb: PathBuf,
    pub scrcpy: PathBuf,
    pub serial: String,
    pub bit_rate: String,
    pub screen_off: bool,
    pub passthrough: Vec<String>,
    pub scrcpy_env: Vec<(String, String)>,
}

pub fn filtered_args(args: &[String]) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut after_separator = false;
    while i < args.len() {
        let arg = &args[i];
        if after_separator {
            out.push(arg.clone());
            i += 1;
            continue;
        }
        if arg == "--" {
            after_separator = true;
            out.push(arg.clone());
            i += 1;
            continue;
        }
        if let Some(name) = arg.strip_prefix("--") {
            let name = name.split_once('=').map_or(name, |(n, _)| n);
            let kind = long_kind(name);
            if kind == Some(OptionKind::Deny) {
                return Err(Error::InvalidPassthrough(format!(
                    "scrcpy option --{name} is denied: {}",
                    deny_reason(name)
                )));
            }
            if kind == Some(OptionKind::Conflict) {
                return Err(Error::InvalidPassthrough(format!(
                    "scrcpy option --{name} conflicts with wrapper-owned configuration"
                )));
            }
            out.push(arg.clone());
            i += 1;
            continue;
        }
        if let Some(shorts) = arg.strip_prefix('-').filter(|s| !s.is_empty()) {
            let bytes = shorts.as_bytes();
            let mut pos = 0;
            while pos < bytes.len() {
                let c = bytes[pos] as char;
                let (kind, takes) = short_kind(c);
                if takes
                    && pos + 1 == bytes.len()
                    && (args.get(i + 1).is_none() || args[i + 1] == "--")
                {
                    return Err(Error::InvalidPassthrough(format!(
                        "scrcpy option -{c} requires a value"
                    )));
                }
                if kind == OptionKind::Deny {
                    return Err(Error::InvalidPassthrough(format!(
                        "scrcpy option -{c} is denied: {}",
                        deny_reason(short_name(c))
                    )));
                }
                if kind == OptionKind::Conflict {
                    return Err(Error::InvalidPassthrough(format!(
                        "scrcpy option -{c} conflicts with wrapper-owned configuration"
                    )));
                }
                if takes {
                    if pos + 1 == bytes.len() {
                        i += 1;
                    }
                    break;
                }
                pos += 1;
            }
            out.push(arg.clone());
            i += 1;
            continue;
        }
        out.push(arg.clone());
        i += 1;
    }
    Ok(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptionKind {
    Deny,
    Conflict,
    Other,
}
fn long_kind(s: &str) -> Option<OptionKind> {
    Some(match s {
        "serial" | "select-usb" | "select-tcpip" | "window-title" | "video-bit-rate" => {
            OptionKind::Conflict
        }
        "screen-off-timeout" | "tcpip" => OptionKind::Deny,
        "record" => OptionKind::Deny,
        "no-cleanup" | "stay-awake" | "show-touches" | "power-off-on-close"
        | "kill-adb-on-close" | "no-window" | "no-playback" | "no-video-playback" | "no-video"
        | "otg" => OptionKind::Deny,
        _ => OptionKind::Other,
    })
}
fn short_kind(c: char) -> (OptionKind, bool) {
    match c {
        's' | 'b' => (OptionKind::Conflict, true),
        'd' | 'e' => (OptionKind::Conflict, false),
        'w' | 't' | 'N' | 'r' => (OptionKind::Deny, c == 'r'),
        _ => (OptionKind::Other, false),
    }
}
fn short_name(c: char) -> &'static str {
    match c {
        'w' => "stay-awake",
        't' => "show-touches",
        'N' => "no-playback",
        'r' => "record",
        _ => "unknown",
    }
}
fn deny_reason(s: &str) -> &str {
    match s {
        "no-cleanup" => "it breaks the wrapper cleanup contract",
        "tcpip" => "it changes connection state",
        "stay-awake" | "show-touches" | "screen-off-timeout" | "power-off-on-close" => {
            "it changes device settings"
        }
        "kill-adb-on-close" => "it affects other adb users",
        "no-window" | "no-playback" | "no-video-playback" | "no-video" | "otg" => {
            "it violates the visible mirroring boundary"
        }
        "record" => "recording is owned by the wrapper policy",
        _ => "it is outside the wrapper policy",
    }
}

pub fn build_args(options: &LaunchOptions) -> Result<Vec<String>, Error> {
    let mut args = filtered_args(&options.passthrough)?;
    args.splice(
        0..0,
        [
            "--serial".into(),
            options.serial.clone(),
            "--window-title=QuadControl".into(),
            "--video-bit-rate".into(),
            options.bit_rate.clone(),
        ],
    );
    if options.screen_off {
        args.push("--turn-screen-off".into());
    }
    Ok(args)
}

#[derive(Debug)]
pub struct RingBuffer {
    bytes: VecDeque<u8>,
    limit: usize,
}
impl RingBuffer {
    fn new(limit: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(limit),
            limit,
        }
    }
    fn push(&mut self, data: &[u8]) {
        for &b in data {
            if self.bytes.len() == self.limit {
                self.bytes.pop_front();
            }
            self.bytes.push_back(b);
        }
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.iter().copied().collect::<Vec<_>>()).into_owned()
    }
}

pub struct Session {
    child: Child,
    stderr: Arc<Mutex<RingBuffer>>,
    stderr_thread: Option<JoinHandle<()>>,
    /// 抽取线程读到 EOF 时置位。监督线程据此做**有界**等待，既能拿到完整的
    /// stderr 尾部，又不必 join——scrcpy 会拉起 adb，孙进程继承同一管道，
    /// 杀掉 scrcpy 后管道仍不关闭，join 会永久阻塞。
    stderr_eof: Arc<AtomicBool>,
}
impl Session {
    pub fn start(options: &LaunchOptions) -> Result<Self, Error> {
        validate_scrcpy_with_env(&options.scrcpy, &options.scrcpy_env)?;
        let args = build_args(options)?;
        let mut command = Command::new(&options.scrcpy);
        command
            .args(args)
            .envs(options.scrcpy_env.iter().map(|(key, value)| (key, value)))
            .env("ADB", &options.adb)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped());
        configure_process(&mut command)?;
        let mut child = command.spawn()?;
        let stderr = Arc::new(Mutex::new(RingBuffer::new(STDERR_LIMIT)));
        let shared = Arc::clone(&stderr);
        let stderr_eof = Arc::new(AtomicBool::new(false));
        let eof_flag = Arc::clone(&stderr_eof);
        let reader = child.stderr.take().expect("stderr is piped");
        let thread = thread::spawn(move || {
            drain_stderr(reader, shared);
            eof_flag.store(true, Ordering::Release);
        });
        Ok(Self {
            child,
            stderr,
            stderr_thread: Some(thread),
            stderr_eof,
        })
    }
    pub fn stop(&mut self) -> Result<(), Error> {
        terminate(&mut self.child)
    }
    /// 非消费地检查进程状态，不会 join stderr 抽取线程，也不产生其它收尾副作用。
    pub fn try_wait(&mut self) -> Result<Option<i32>, Error> {
        Ok(self
            .child
            .try_wait()?
            .map(|status| status.code().unwrap_or(1)))
    }
    pub fn wait(self) -> Result<i32, Error> {
        Ok(self.wait_with_stderr()?.0)
    }
    pub fn wait_with_stderr(mut self) -> Result<(i32, String), Error> {
        let status = self.child.wait()?;
        if let Some(t) = self.stderr_thread.take() {
            let _ = t.join();
        }
        let tail = self.stderr_tail();
        Ok((status.code().unwrap_or(1), tail))
    }
    pub fn wait_or_stop_on_signal(self) -> Result<i32, Error> {
        Ok(self.wait_or_stop_on_signal_with_stderr()?.0)
    }
    pub fn wait_or_stop_on_signal_with_stderr(mut self) -> Result<(i32, String), Error> {
        loop {
            if self.child.try_wait()?.is_some() {
                break;
            }
            if signal_requested() {
                self.stop()?;
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let status = self.child.wait()?;
        if let Some(t) = self.stderr_thread.take() {
            let _ = t.join();
        }
        let tail = self.stderr_tail();
        Ok((status.code().unwrap_or(1), tail))
    }
    pub fn stderr_tail(&self) -> String {
        self.stderr.lock().expect("stderr mutex poisoned").text()
    }

    /// 在 `budget` 内等待 stderr 抽取到 EOF 再取尾部；超时就返回当前内容。
    ///
    /// 进程刚退出时抽取线程可能还没读完最后一段，而会话失败时**恰恰是最后那段**
    /// 才是诊断信息。这里用有界等待换取完整性，绝不无界阻塞（理由见
    /// [`Session::stderr_eof`] 上的注释）。
    pub fn stderr_tail_settled(&self, budget: Duration) -> String {
        let deadline = Instant::now() + budget;
        while !self.stderr_eof.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        self.stderr_tail()
    }
}

/// 监督线程对 scrcpy 会话生命周期的可观察快照。
#[derive(Clone, Debug, PartialEq)]
pub enum SessionStatus {
    Running,
    Stopping,
    Exited { code: i32, stderr_tail: String },
    Failed { message: String },
}

enum StopCommand {
    Stop,
}

/// 不暴露 `Session` 的监督句柄；所有进程操作都由内部监督线程执行。
pub struct SessionHandle {
    id: u64,
    status: Arc<Mutex<SessionStatus>>,
    stop_tx: mpsc::Sender<StopCommand>,
    done_rx: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl SessionHandle {
    /// 返回创建监督会话时指定的稳定标识。
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 返回监督线程当前状态的快照；状态读取不会暴露 Session 的可变访问。
    pub fn status(&self) -> SessionStatus {
        self.status.lock().expect("status mutex poisoned").clone()
    }

    /// 请求停止会话。重复请求及监督线程已结束时都视为成功。
    pub fn request_stop(&self) -> Result<(), Error> {
        let _ = self.stop_tx.send(StopCommand::Stop);
        Ok(())
    }
}

/// 启动一个只由监督线程持有并操作的会话。
pub fn spawn_supervised(options: &LaunchOptions, id: u64) -> Result<SessionHandle, Error> {
    let session = Session::start(options)?;
    let status = Arc::new(Mutex::new(SessionStatus::Running));
    let (stop_tx, stop_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let thread_status = Arc::clone(&status);
    thread::spawn(move || {
        supervise(session, stop_rx, done_tx, thread_status);
    });
    Ok(SessionHandle {
        id,
        status,
        stop_tx,
        done_rx: Arc::new(Mutex::new(done_rx)),
    })
}

fn supervise(
    mut session: Session,
    stop_rx: mpsc::Receiver<StopCommand>,
    done_tx: mpsc::Sender<()>,
    status: Arc<Mutex<SessionStatus>>,
) {
    let result = loop {
        if stop_rx.try_recv().is_ok() {
            *status.lock().expect("status mutex poisoned") = SessionStatus::Stopping;
            break session.stop().and_then(|()| {
                session
                    .try_wait()?
                    .ok_or_else(|| Error::WrongState("stopped session was not reaped".into()))
            });
        }
        match session.try_wait() {
            Ok(Some(code)) => break Ok(code),
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => break Err(error),
        }
    };

    match result {
        Ok(code) => {
            let tail = session.stderr_tail_settled(STDERR_SETTLE_BUDGET);
            *status.lock().expect("status mutex poisoned") = SessionStatus::Exited {
                code,
                stderr_tail: tail,
            };
        }
        Err(error) => {
            *status.lock().expect("status mutex poisoned") = SessionStatus::Failed {
                message: error.to_string(),
            };
        }
    }
    let _ = done_tx.send(());
}

/// 批量关闭的结果；超时项由调用方负责上报或后续处置。
#[derive(Debug, PartialEq)]
pub struct ShutdownReport {
    pub timed_out: Vec<u64>,
}

/// 请求所有会话停止，并以一个共享总预算等待监督线程完成。
///
/// 超时只记录会话 ID，不根据保存的 PID 盲目强杀：PID 复用会误伤其它进程。
/// 设备侧的强制回收仍由持有 `Child` 的监督线程执行终止梯子；这里的安全目标是
/// 不留下仍在控制手机的 scrcpy 孤儿，而不是消灭 zombie 表项。
pub fn shutdown_all(handles: &[SessionHandle], deadline: Duration) -> ShutdownReport {
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
fn drain_stderr(mut reader: impl Read, buffer: Arc<Mutex<RingBuffer>>) {
    let mut chunk = [0u8; 4096];
    while let Ok(n) = reader.read(&mut chunk) {
        if n == 0 {
            break;
        }
        buffer
            .lock()
            .expect("stderr mutex poisoned")
            .push(&chunk[..n]);
    }
}

#[cfg(unix)]
fn configure_process(command: &mut Command) -> Result<(), Error> {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        });
    }
    Ok(())
}
#[cfg(windows)]
fn configure_process(command: &mut Command) -> Result<(), Error> {
    use std::os::windows::process::CommandExt;
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP);
    Ok(())
}
#[cfg(unix)]
fn terminate(child: &mut Child) -> Result<(), Error> {
    let pid = -(child.id() as i32);
    if !signal_escalated() {
        unsafe {
            libc::kill(pid, libc::SIGINT);
        }
    }
    if !signal_escalated() && wait_for(child, Duration::from_secs(3)) {
        return Ok(());
    }
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    if wait_for(child, Duration::from_secs(2)) {
        return Ok(());
    }
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    child.wait()?;
    Ok(())
}
#[cfg(windows)]
fn terminate(child: &mut Child) -> Result<(), Error> {
    let has_console = unsafe { !windows_sys::Win32::System::Console::GetConsoleWindow().is_null() };
    if has_console && !signal_escalated() {
        unsafe {
            let _ = windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent(
                windows_sys::Win32::System::Console::CTRL_BREAK_EVENT,
                child.id(),
            );
        }
    }
    if has_console && !signal_escalated() && wait_for(child, Duration::from_secs(3)) {
        return Ok(());
    }
    unsafe {
        let handle = windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_TERMINATE,
            0,
            child.id(),
        );
        if !handle.is_null() {
            let _ = windows_sys::Win32::System::Threading::TerminateProcess(handle, 1);
            let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
        }
    }
    child.wait()?;
    Ok(())
}
fn wait_for(child: &mut Child, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if child.try_wait().ok().flatten().is_some() {
            return true;
        }
        if signal_escalated() {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

static SIGNAL_COUNT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
#[cfg(unix)]
extern "C" fn signal_handler(_: libc::c_int) {
    SIGNAL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
#[cfg(unix)]
pub fn install_signal_handlers() {
    unsafe {
        let handler = signal_handler as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}
#[cfg(windows)]
unsafe extern "system" fn console_handler(_: u32) -> i32 {
    SIGNAL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    1
}
#[cfg(windows)]
pub fn install_signal_handlers() {
    unsafe {
        let _ =
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console_handler), 1);
    }
}
#[cfg(not(any(unix, windows)))]
pub fn install_signal_handlers() {}
pub fn signal_requested() -> bool {
    SIGNAL_COUNT.load(std::sync::atomic::Ordering::Relaxed) != 0
}
fn signal_escalated() -> bool {
    SIGNAL_COUNT.load(std::sync::atomic::Ordering::Relaxed) > 1
}

pub const HELP: &str = "Usage: quadcontrol-scrcpy [--adb PATH] [--serial SERIAL] [--bit-rate RATE] [--screen-off] [scrcpy options]\n\nDenied options:\n  --no-cleanup       breaks the wrapper cleanup contract\n  --tcpip            changes connection state\n  -w/-t/--screen-off-timeout/--power-off-on-close changes device settings\n  --kill-adb-on-close affects other adb users\n  --no-window/-r/-N/--no-video* /--otg violates visible mirroring\n\nThe wrapper owns serial, window title, video bitrate, and screen-off flags.\n";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_versions_and_prereleases() {
        assert!(validate_version_text("scrcpy 4.1\n").is_ok());
        assert!(validate_version_text("scrcpy 4.2-rc1\n").is_err());
        assert!(validate_version_text("scrcpy 3.3\n").is_err());
        assert!(validate_version_text("not a version\n").is_err());
        assert!(validate_version_text("x\r\nscrcpy 4.1\r\n").is_ok());
    }
    #[test]
    fn semantic_filter_is_systematic() {
        let conflicts = [
            ("serial", "wrapper-owned configuration"),
            ("select-usb", "wrapper-owned configuration"),
            ("select-tcpip", "wrapper-owned configuration"),
            ("window-title", "wrapper-owned configuration"),
            ("video-bit-rate", "wrapper-owned configuration"),
        ];
        for (name, reason) in conflicts {
            for args in [
                vec![format!("--{name}=value")],
                vec![format!("--{name}"), "value".into()],
            ] {
                let error = filtered_args(&args).unwrap_err().to_string();
                assert!(error.contains(reason), "{args:?}: {error}");
            }
        }

        let denied = [
            ("no-cleanup", "cleanup contract"),
            ("tcpip", "connection state"),
            ("screen-off-timeout", "device settings"),
            ("record", "recording"),
            ("stay-awake", "device settings"),
            ("show-touches", "device settings"),
            ("power-off-on-close", "device settings"),
            ("kill-adb-on-close", "other adb users"),
            ("no-window", "visible mirroring"),
            ("no-playback", "visible mirroring"),
            ("no-video-playback", "visible mirroring"),
            ("no-video", "visible mirroring"),
            ("otg", "visible mirroring"),
        ];
        for (name, reason) in denied {
            for args in [
                vec![format!("--{name}=value")],
                vec![format!("--{name}"), "value".into()],
            ] {
                let error = filtered_args(&args).unwrap_err().to_string();
                assert!(error.contains(reason), "{args:?}: {error}");
            }
        }

        for (alias, reason) in [
            ('s', "wrapper-owned configuration"),
            ('b', "wrapper-owned configuration"),
            ('d', "wrapper-owned configuration"),
            ('e', "wrapper-owned configuration"),
            ('w', "device settings"),
            ('t', "device settings"),
            ('N', "visible mirroring"),
            ('r', "recording"),
        ] {
            let args = if matches!(alias, 's' | 'b' | 'r') {
                vec![format!("-{alias}"), "value".into()]
            } else {
                vec![format!("-{alias}")]
            };
            let error = filtered_args(&args).unwrap_err().to_string();
            assert!(error.contains(reason), "{args:?}: {error}");
        }
        for combination in ["-wt", "-sN", "-ber", "-dT"] {
            assert!(
                filtered_args(&[combination.into()]).is_err(),
                "{combination}"
            );
        }
        assert!(filtered_args(&["-b".into()])
            .unwrap_err()
            .to_string()
            .contains("requires a value"));
        assert!(filtered_args(&["-s".into(), "--".into()]).is_err());
        assert!(filtered_args(&["--".into(), "--no-cleanup".into(), "-wt".into()]).is_ok());
    }
    #[test]
    fn selection_branches() {
        let d = |serial: &str, state| Device {
            serial: serial.into(),
            model: None,
            state,
            transport: adb_probe::Transport::Unknown,
        };
        assert_eq!(
            select_device(&[d("REDACTEDSERIAL", DeviceState::Device)], None).unwrap(),
            "REDACTEDSERIAL"
        );
        assert!(select_device(
            &[d("a", DeviceState::Device), d("b", DeviceState::Device)],
            None
        )
        .is_err());
        assert!(select_device(&[d("a", DeviceState::Unauthorized)], None).is_err());
    }
}
