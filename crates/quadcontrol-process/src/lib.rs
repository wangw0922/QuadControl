//! 与具体引擎无关的子进程通用设施：进程组配置、终止梯子、stderr 有界缓冲。
//!
//! 这里的每一项原先都住在 `quadcontrol-android`，抽出来是为了让 iOS 侧
//! （go-ios / WebDriverAgent）复用同一套「可见、可断开、不留孤儿」的进程契约。
//! 行为与抽出前一致：时序、超时、缓冲大小都没有改动。
//!
//! 唯一的接口变化是**信号升级判断由参数传入**：原实现直接读 CLI 的全局信号
//! 计数，这里改成 `escalated: &dyn Fn() -> bool`。CLI 传入原来的全局函数；
//! 从不安装信号处理的调用方（GUI）传恒 false 的闭包。

use std::collections::VecDeque;
use std::io::{self, Read};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// stderr 环形缓冲的容量上限；超出后丢弃最旧的字节，只保留尾部。
pub const STDERR_LIMIT: usize = 64 * 1024;

/// 固定容量的字节环形缓冲：写满后从头丢弃，用来保留 stderr 的尾部。
#[derive(Debug)]
pub struct RingBuffer {
    bytes: VecDeque<u8>,
    limit: usize,
}
impl RingBuffer {
    /// 建立一个最多保留 `limit` 字节的缓冲。
    pub fn new(limit: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(limit),
            limit,
        }
    }
    /// 追加数据；超过上限时逐字节丢弃最旧内容。
    pub fn push(&mut self, data: &[u8]) {
        for &b in data {
            if self.bytes.len() == self.limit {
                self.bytes.pop_front();
            }
            self.bytes.push_back(b);
        }
    }
    /// 以有损 UTF-8 解码返回当前内容。
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.iter().copied().collect::<Vec<_>>()).into_owned()
    }
}

/// 把 `reader` 读到 EOF，边读边写入 `buffer`。
///
/// 注意：这个函数**可能永远读不到 EOF**。被控引擎常会拉起子进程（如 scrcpy
/// 会拉起 adb），孙进程继承同一管道，杀掉直接子进程后管道仍不关闭。因此持有
/// 这个线程句柄的调用方必须做**有界**等待，不能无条件 join。
pub fn drain_stderr(mut reader: impl Read, buffer: Arc<Mutex<RingBuffer>>) {
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

/// 把子进程放进独立的进程组，好让终止梯子能整组回收。
///
/// unix 用 `pre_exec` 里的 `setpgid(0, 0)`；windows 用
/// `CREATE_NEW_PROCESS_GROUP` 创建标志。
#[cfg(unix)]
pub fn configure_process(command: &mut Command) -> Result<(), io::Error> {
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
/// 把子进程放进独立的进程组；windows 版本用 `CREATE_NEW_PROCESS_GROUP`
/// 创建标志，好让 `CTRL_BREAK_EVENT` 只打到这一组。
#[cfg(windows)]
pub fn configure_process(command: &mut Command) -> Result<(), io::Error> {
    use std::os::windows::process::CommandExt;
    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP);
    Ok(())
}

/// 按「温和 → 强硬」的梯子终止子进程（整个进程组）。
///
/// unix：先给进程组发 `SIGINT`，等最多 3 秒；不退则 `SIGTERM` 等最多 2 秒；
/// 仍不退就 `SIGKILL` 并阻塞收尸。
///
/// windows 的差异：没有信号，第一级用
/// `GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT)`，而这一级**只在进程有控制台时
/// 才有意义**（`GetConsoleWindow()` 非空），GUI 宿主下直接跳到强制终止；第二、
/// 三级合并为一次 `TerminateProcess`，没有 unix 那种 SIGTERM/SIGKILL 之分。
///
/// `escalated` 是用户催促的信号升级判断（例如第二次 Ctrl-C）。它在梯子的每一级
/// 前、以及等待循环的每一次轮询中**实时**求值：一旦升级就跳过剩余的温和等待，
/// 直接走强制终止。
pub fn terminate(child: &mut Child, escalated: &dyn Fn() -> bool) -> Result<(), io::Error> {
    terminate_impl(child, escalated)
}

#[cfg(unix)]
fn terminate_impl(child: &mut Child, escalated: &dyn Fn() -> bool) -> Result<(), io::Error> {
    let pid = -(child.id() as i32);
    if !escalated() {
        unsafe {
            libc::kill(pid, libc::SIGINT);
        }
    }
    if !escalated() && wait_for(child, Duration::from_secs(3), escalated) {
        return Ok(());
    }
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    if wait_for(child, Duration::from_secs(2), escalated) {
        return Ok(());
    }
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    child.wait()?;
    Ok(())
}

#[cfg(windows)]
fn terminate_impl(child: &mut Child, escalated: &dyn Fn() -> bool) -> Result<(), io::Error> {
    let has_console = unsafe { !windows_sys::Win32::System::Console::GetConsoleWindow().is_null() };
    if has_console && !escalated() {
        unsafe {
            let _ = windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent(
                windows_sys::Win32::System::Console::CTRL_BREAK_EVENT,
                child.id(),
            );
        }
    }
    if has_console && !escalated() && wait_for(child, Duration::from_secs(3), escalated) {
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

/// 在 `timeout` 内以 100 ms 轮询等待子进程退出。
///
/// 返回 `true` 表示已退出；`false` 表示超时**或**期间发生信号升级——两种情况
/// 调用方都应进入梯子的下一级。绝不无界阻塞。
pub fn wait_for(child: &mut Child, timeout: Duration, escalated: &dyn Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if child.try_wait().ok().flatten().is_some() {
            return true;
        }
        if escalated() {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}
