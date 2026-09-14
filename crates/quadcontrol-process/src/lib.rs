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

/// 对一组子进程**并行**走同一条终止梯子，整组共享一份等待预算。
///
/// 与 [`terminate`] 的区别只在于「串行 vs 并行」：`terminate` 每个进程各等
/// 3 s + 2 s，四个进程最坏 20 s；这里先给**全部**进程发 `SIGINT`，然后统一等
/// 3 s；仍存活的全部 `SIGTERM`，统一等 2 s；剩下的 `SIGKILL`。最坏墙钟 5 s，
/// 与进程数无关。iOS 会话同时持有 tunnel / runwda / forward×2 四个子进程，
/// 串行梯子会超出 `IOS_SHUTDOWN_DEADLINE`，所以必须并行。
///
/// 每一级下手前都跳过已经收尸的子进程：PID 会被复用，对已 reap 的 PID 再发信号
/// 有误伤别人的风险。
///
/// `escalated` 的语义与 [`terminate`] 完全一致：实时求值，一旦升级就跳过剩余的
/// 温和等待直接强制终止。windows 分支同样只有「控制台事件 → TerminateProcess」
/// 两级，没有 SIGTERM/SIGKILL 之分。
pub fn terminate_all(children: &mut [&mut Child], escalated: &dyn Fn() -> bool) -> io::Result<()> {
    terminate_all_impl(children, escalated)
}

/// 在 `timeout` 内以 100 ms 轮询等待**全部**子进程退出，共享一份预算。
///
/// 返回 `true` 表示全部已退出；`false` 表示超时或期间发生信号升级。绝不无界阻塞。
/// 不复用 [`wait_for`]：那个函数是「每个进程各等一份 timeout」，逐个调用就退化成
/// 串行梯子，正是这里要避免的。
fn wait_all_for(
    children: &mut [&mut Child],
    timeout: Duration,
    escalated: &dyn Fn() -> bool,
) -> bool {
    let start = Instant::now();
    loop {
        if children
            .iter_mut()
            .all(|child| child.try_wait().ok().flatten().is_some())
        {
            return true;
        }
        if escalated() || start.elapsed() >= timeout {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// 对还活着的子进程逐个执行 `action`；已退出的跳过（PID 复用防误伤）。
fn for_each_live(children: &mut [&mut Child], action: impl Fn(&Child)) {
    for child in children.iter_mut() {
        if child.try_wait().ok().flatten().is_none() {
            action(child);
        }
    }
}

#[cfg(unix)]
fn terminate_all_impl(children: &mut [&mut Child], escalated: &dyn Fn() -> bool) -> io::Result<()> {
    fn signal_group(signal: libc::c_int) -> impl Fn(&Child) {
        move |child: &Child| unsafe {
            libc::kill(-(child.id() as i32), signal);
        }
    }
    if !escalated() {
        for_each_live(children, signal_group(libc::SIGINT));
        if wait_all_for(children, Duration::from_secs(3), escalated) {
            return Ok(());
        }
    }
    for_each_live(children, signal_group(libc::SIGTERM));
    if wait_all_for(children, Duration::from_secs(2), escalated) {
        return Ok(());
    }
    for_each_live(children, signal_group(libc::SIGKILL));
    for child in children.iter_mut() {
        child.wait()?;
    }
    Ok(())
}

#[cfg(windows)]
fn terminate_all_impl(children: &mut [&mut Child], escalated: &dyn Fn() -> bool) -> io::Result<()> {
    let has_console = unsafe { !windows_sys::Win32::System::Console::GetConsoleWindow().is_null() };
    if has_console && !escalated() {
        for_each_live(children, |child| unsafe {
            let _ = windows_sys::Win32::System::Console::GenerateConsoleCtrlEvent(
                windows_sys::Win32::System::Console::CTRL_BREAK_EVENT,
                child.id(),
            );
        });
        if wait_all_for(children, Duration::from_secs(3), escalated) {
            return Ok(());
        }
    }
    for_each_live(children, |child| unsafe {
        let handle = windows_sys::Win32::System::Threading::OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_TERMINATE,
            0,
            child.id(),
        );
        if !handle.is_null() {
            let _ = windows_sys::Win32::System::Threading::TerminateProcess(handle, 1);
            let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
        }
    });
    for child in children.iter_mut() {
        child.wait()?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::process::Stdio;

    /// 起一个**确实忽略指定信号**的长驻子进程。
    ///
    /// 不用 `sh -c 'trap "" INT; …'`：本机实测 macOS 的 `/bin/sh` 下 `sleep`
    /// 仍会被 SIGINT 打死（105 ms 就退了），梯子的温和等待根本没被触发，用例
    /// 会「通过」却什么都没验证。这里直接在 `pre_exec` 里 `signal(sig, SIG_IGN)`——
    /// SIG_IGN 会跨 `exec` 继承，语义由 libc 保证，不受 shell 实现差异影响。
    fn spawn_ignoring(signals: &'static [libc::c_int]) -> Child {
        use std::os::unix::process::CommandExt;
        let mut command = Command::new("sleep");
        command
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_process(&mut command).unwrap();
        unsafe {
            command.pre_exec(move || {
                for &sig in signals {
                    libc::signal(sig, libc::SIG_IGN);
                }
                Ok(())
            });
        }
        command.spawn().unwrap()
    }

    /// 起三个**忽略 SIGINT** 的子进程，断言并行梯子的墙钟停在「一份 3 s 等待」的
    /// 量级上。这条断言是本函数存在的唯一理由：串行梯子在这里会花 ≈9 s，
    /// 上限取 6 s 既能挡住串行退化，又给慢机器留出余量。
    #[test]
    fn terminate_all_shares_one_wait_budget_across_children() {
        let mut spawned: Vec<Child> = (0..3).map(|_| spawn_ignoring(&[libc::SIGINT])).collect();
        let pids: Vec<u32> = spawned.iter().map(Child::id).collect();

        let started = Instant::now();
        let mut borrowed: Vec<&mut Child> = spawned.iter_mut().collect();
        terminate_all(&mut borrowed, &|| false).unwrap();
        let elapsed = started.elapsed();

        // 下界证明这些子进程**真的**忽略了 SIGINT（否则测的就不是梯子）；
        // 上界证明三个进程共享同一份等待预算，而不是各等各的。
        assert!(
            elapsed >= Duration::from_secs(3),
            "子进程没有忽略 SIGINT，这条用例没测到梯子：{elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(6),
            "并行梯子退化成串行了：{elapsed:?}"
        );
        for pid in pids {
            unsafe {
                assert_eq!(libc::kill(pid as libc::pid_t, 0), -1, "pid {pid} 仍存在");
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
            }
        }
    }

    /// 升级（用户催促）时不做任何温和等待，直接强制终止。
    #[test]
    fn escalation_skips_the_gentle_rungs() {
        let mut child = spawn_ignoring(&[libc::SIGINT, libc::SIGTERM]);

        let started = Instant::now();
        terminate_all(&mut [&mut child], &|| true).unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    /// 空切片与已退出的子进程都不应 panic，也不应对回收过的 PID 再发信号。
    #[test]
    fn already_exited_children_are_skipped() {
        let mut child = Command::new("true").spawn().unwrap();
        child.wait().unwrap();
        terminate_all(&mut [&mut child], &|| false).unwrap();
        terminate_all(&mut [], &|| false).unwrap();
    }
}
