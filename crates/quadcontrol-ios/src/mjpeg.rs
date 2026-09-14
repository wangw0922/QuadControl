//! MJPEG 代理：把 WDA 的 MJPEG 流转给 WebView 的 `<img>`，并顺路计帧。
//!
//! # 为什么需要它
//!
//! 1. 帧率标签要计帧，而 `<img>` 对 MJPEG 不暴露逐帧事件；
//! 2. 链路死亡（tunnel / runwda / forward 任一退出、上游 EOF）时代理一关，
//!    前端画面立即断——不会留一张「看起来还在连着」的静止画面。
//!
//! 替代方案已核对并否掉：前端 `fetch()` 流式解析要放宽 CSP 的 `connect-src`；
//! Tauri 自定义协议不适合无限流。
//!
//! # 行为契约
//!
//! - accept 循环，**最新下游连接获胜**（新连接到来就关掉旧的；WebKit 对 MJPEG
//!   会自行重连）；**上游始终最多一条**，且跨下游重连保持不断。
//! - 按 multipart 边界切帧，单帧上限 [`MAX_FRAME_BYTES`]，超限即断流并记
//!   [`crate::Error::ProxyFailed`]。
//! - 只保留「最新一帧」单槽缓冲：下游慢时覆盖旧帧并计 `backpressure_drops`。
//!   这是评审阻塞项 B3 的裁决——无界队列会让 `backpressure_drops` 恒零，
//!   而且把内存压力变成延迟累积。
//! - **不解码 JPEG，不做黑帧检测。** 停帧不是锁屏信号（真机：锁屏后截图是合法
//!   全黑 PNG、无错误），锁定提示只由 `/wda/locked` 驱动。

use crate::Error;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// 单帧上限；超过即认为上游在乱吐，断流报 `proxy_failed`。
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
/// 下游响应用的 multipart 边界。固定值即可：每条下游连接各自独立。
const DOWNSTREAM_BOUNDARY: &str = "quadcontrolframe";
/// `stop()` 时等待各线程收尾的上限；绝不无界 join。
const JOIN_BUDGET: Duration = Duration::from_secs(2);
/// accept 循环的轮询间隔（listener 设为非阻塞，靠它感知停止标志）。
const ACCEPT_POLL: Duration = Duration::from_millis(20);
/// 连上游的有界重试预算。
///
/// 必须重试：`ios forward` 是刚被 spawn 的**独立进程**，它绑上本地端口需要时间，
/// 而我们拿到的端口号只是「bind 0 取号后释放」的结果，此刻还没人在听。一次性
/// connect 会稳定偶发 `Connection refused`（实测就是这样红的）。HTTP 侧因为
/// `/status` 本来就在轮询而没暴露这个问题。
const UPSTREAM_CONNECT_BUDGET: Duration = Duration::from_secs(10);

/// 代理的可观察计数。`last_frame_at` 用 [`Instant`]，不进任何序列化状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    /// 从上游成功切出的帧数。GUI 的「N 帧/秒」标签由它的差分算出。
    pub frames: u64,
    /// 下游写得比上游慢、被新帧覆盖掉的旧帧数。
    pub backpressure_drops: u64,
    /// 最后一帧到达的时刻；一帧都没有时是 None。
    pub last_frame_at: Option<Instant>,
}

/// 上游/下游共享的单槽最新帧缓冲。
struct Slot {
    frame: Mutex<Option<Vec<u8>>>,
    ready: Condvar,
    frames: AtomicU64,
    drops: AtomicU64,
    last_frame_at: Mutex<Option<Instant>>,
    stopping: AtomicBool,
}

impl Slot {
    fn publish(&self, frame: Vec<u8>) {
        let mut held = self.frame.lock().expect("frame mutex poisoned");
        // 槽里还压着上一帧 = 下游没跟上，覆盖并记一次丢帧。
        if held.is_some() {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
        *held = Some(frame);
        self.frames.fetch_add(1, Ordering::Relaxed);
        *self.last_frame_at.lock().expect("instant mutex poisoned") = Some(Instant::now());
        self.ready.notify_all();
    }

    /// 取走最新帧；停止时返回 None。有界等待，绝不无限挂起。
    fn take(&self) -> Option<Vec<u8>> {
        let mut held = self.frame.lock().expect("frame mutex poisoned");
        loop {
            if self.stopping.load(Ordering::Acquire) {
                return None;
            }
            if let Some(frame) = held.take() {
                return Some(frame);
            }
            let (next, _) = self
                .ready
                .wait_timeout(held, Duration::from_millis(100))
                .expect("frame mutex poisoned");
            held = next;
        }
    }
}

/// 运行中的 MJPEG 代理。
///
/// `Debug` 只打端口与计数：帧内容是采集到的屏幕数据，绝不进任何诊断输出。
pub struct Proxy {
    local_port: u16,
    slot: Arc<Slot>,
    failure: Arc<Mutex<Option<String>>>,
    /// 停止时要 shutdown 的 socket，用来打断阻塞在 read/write 上的线程。
    sockets: Arc<Mutex<Vec<TcpStream>>>,
    threads: Vec<JoinHandle<()>>,
}

impl Proxy {
    /// 连上游 `127.0.0.1:<upstream_port>`，在 `127.0.0.1:0` 起下游监听。
    ///
    /// 上游**在这里连一次**，之后跨下游重连一直保持——WebKit 重连 `<img>` 时
    /// 不该导致重新拉一条设备侧的流。
    pub fn start(upstream_port: u16) -> Result<Self, Error> {
        Self::start_with_connect_budget(upstream_port, UPSTREAM_CONNECT_BUDGET)
    }

    /// 同 [`Proxy::start`]，但可指定连上游的重试预算（测试用短预算）。
    pub fn start_with_connect_budget(
        upstream_port: u16,
        connect_budget: Duration,
    ) -> Result<Self, Error> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| Error::ProxyFailed(format!("MJPEG proxy could not bind: {e}")))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| Error::ProxyFailed(format!("MJPEG proxy has no local address: {e}")))?
            .port();
        listener
            .set_nonblocking(true)
            .map_err(|e| Error::ProxyFailed(format!("MJPEG proxy listener: {e}")))?;

        let upstream = connect_upstream(upstream_port, connect_budget)?;

        let slot = Arc::new(Slot {
            frame: Mutex::new(None),
            ready: Condvar::new(),
            frames: AtomicU64::new(0),
            drops: AtomicU64::new(0),
            last_frame_at: Mutex::new(None),
            stopping: AtomicBool::new(false),
        });
        let failure = Arc::new(Mutex::new(None));
        let sockets = Arc::new(Mutex::new(vec![upstream.try_clone().map_err(|e| {
            Error::ProxyFailed(format!("MJPEG upstream clone failed: {e}"))
        })?]));

        let mut threads = Vec::new();
        threads.push({
            let slot = Arc::clone(&slot);
            let failure = Arc::clone(&failure);
            thread::spawn(move || pump_upstream(upstream, &slot, &failure))
        });
        threads.push({
            let slot = Arc::clone(&slot);
            let sockets = Arc::clone(&sockets);
            thread::spawn(move || serve_downstream(listener, &slot, &sockets))
        });

        Ok(Self {
            local_port,
            slot,
            failure,
            sockets,
            threads,
        })
    }

    /// 下游监听端口；GUI 把它拼成 `<img src>`。
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// 当前计数快照。
    pub fn stats(&self) -> FrameStats {
        FrameStats {
            frames: self.slot.frames.load(Ordering::Relaxed),
            backpressure_drops: self.slot.drops.load(Ordering::Relaxed),
            last_frame_at: *self
                .slot
                .last_frame_at
                .lock()
                .expect("instant mutex poisoned"),
        }
    }

    /// 代理是否还活着。
    ///
    /// 上游 EOF（链路死亡）不是「错误」，[`Proxy::failure`] 仍是 None，但代理已经
    /// 收摊。监督线程必须靠这个判据发现链路死了并收回子进程——只看 `failure()`
    /// 会让会话永远停在 `Running`，画面已黑而状态还说「已连接」。
    pub fn is_alive(&self) -> bool {
        !self.slot.stopping.load(Ordering::Acquire)
    }

    /// 上游出错时的原因；没出错就是 None。
    pub fn failure(&self) -> Option<Error> {
        self.failure
            .lock()
            .expect("failure mutex poisoned")
            .clone()
            .map(Error::ProxyFailed)
    }

    /// 关监听、断上下游、有界 join 全部线程。
    ///
    /// accept 与 read 都可能正阻塞着，所以先置停止标志并 `shutdown` 所有 socket
    /// 把它们踢醒，再 join；join 也有预算（[`JOIN_BUDGET`]），绝不无界等待。
    pub fn stop(&mut self) {
        self.slot.stopping.store(true, Ordering::Release);
        self.slot.ready.notify_all();
        for socket in self.sockets.lock().expect("sockets mutex poisoned").iter() {
            let _ = socket.shutdown(Shutdown::Both);
        }
        let deadline = Instant::now() + JOIN_BUDGET;
        for thread in self.threads.drain(..) {
            // join 本身没有超时；靠上面的 shutdown 保证线程都已经在退出路上。
            // 预算耗尽还没结束就放手——宁可漏一个已被 shutdown 的线程，也不卡住
            // 整个关闭序列。
            if Instant::now() >= deadline {
                break;
            }
            let _ = thread.join();
        }
    }
}

impl std::fmt::Debug for Proxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proxy")
            .field("local_port", &self.local_port)
            .field("stats", &self.stats())
            .finish_non_exhaustive()
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 在 `budget` 内反复尝试连上游，直到成功或预算耗尽。绝不无界重试。
fn connect_upstream(port: u16, budget: Duration) -> Result<TcpStream, Error> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let deadline = Instant::now() + budget;
    loop {
        match TcpStream::connect(address) {
            Ok(stream) => return Ok(stream),
            // 预算耗尽时把**最后一次**的失败原因原样带出去，不留下空诊断。
            Err(error) if Instant::now() >= deadline => {
                return Err(Error::ProxyFailed(format!(
                    "MJPEG upstream port {port} did not accept a connection within {} s: {error}",
                    budget.as_secs()
                )));
            }
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// 读上游、切帧、投进单槽。
fn pump_upstream(stream: TcpStream, slot: &Arc<Slot>, failure: &Arc<Mutex<Option<String>>>) {
    let mut reader = BufReader::new(stream);
    let boundary = match read_upstream_boundary(&mut reader) {
        Ok(boundary) => boundary,
        Err(reason) => {
            *failure.lock().expect("failure mutex poisoned") = Some(reason);
            slot.stopping.store(true, Ordering::Release);
            slot.ready.notify_all();
            return;
        }
    };

    loop {
        if slot.stopping.load(Ordering::Acquire) {
            return;
        }
        match read_frame(&mut reader, &boundary) {
            Ok(Some(frame)) => slot.publish(frame),
            // 上游 EOF：链路死了，代理跟着关，前端画面立即断。
            Ok(None) => {
                slot.stopping.store(true, Ordering::Release);
                slot.ready.notify_all();
                return;
            }
            Err(reason) => {
                *failure.lock().expect("failure mutex poisoned") = Some(reason);
                slot.stopping.store(true, Ordering::Release);
                slot.ready.notify_all();
                return;
            }
        }
    }
}

/// 读上游的 HTTP 响应头，取出 multipart 边界。
fn read_upstream_boundary(reader: &mut BufReader<TcpStream>) -> Result<String, String> {
    let mut content_type = None;
    loop {
        let line = read_line(reader)?;
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-type") {
                content_type = Some(value.trim().to_owned());
            }
        }
    }
    let content_type =
        content_type.ok_or_else(|| "MJPEG upstream sent no Content-Type".to_owned())?;
    let boundary = content_type
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            part.strip_prefix("boundary=")
                .map(|b| b.trim_matches('"').to_owned())
        })
        .ok_or_else(|| format!("MJPEG upstream Content-Type has no boundary: {content_type}"))?;
    // 上游的边界写法有带不带 `--` 两种，统一成不带，切帧时自己补。
    Ok(boundary.trim_start_matches("--").to_owned())
}

/// 切出一帧。返回 `Ok(None)` 表示上游正常 EOF。
fn read_frame(
    reader: &mut BufReader<TcpStream>,
    boundary: &str,
) -> Result<Option<Vec<u8>>, String> {
    // 找到边界行。
    loop {
        let line = match read_line_opt(reader)? {
            Some(line) => line,
            None => return Ok(None),
        };
        if line.trim_end().trim_end_matches("--").ends_with(boundary) && line.contains("--") {
            break;
        }
    }

    // 读本段的头，取 Content-Length。
    let mut length = None;
    loop {
        let line = match read_line_opt(reader)? {
            Some(line) => line,
            None => return Ok(None),
        };
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
    }

    match length {
        Some(length) => {
            if length > MAX_FRAME_BYTES {
                return Err(format!(
                    "MJPEG frame of {length} bytes exceeds the {MAX_FRAME_BYTES} byte limit"
                ));
            }
            let mut frame = vec![0u8; length];
            match reader.read_exact(&mut frame) {
                Ok(()) => Ok(Some(frame)),
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
                Err(e) => Err(format!("MJPEG upstream read failed: {e}")),
            }
        }
        // 没有 Content-Length 就扫到下一个边界为止，同样受上限约束。
        None => scan_to_boundary(reader, boundary),
    }
}

/// 没有 `Content-Length` 时，累积字节直到遇见下一个边界。
fn scan_to_boundary(
    reader: &mut BufReader<TcpStream>,
    boundary: &str,
) -> Result<Option<Vec<u8>>, String> {
    let needle = format!("--{boundary}");
    let mut frame = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        match reader.read(&mut byte) {
            Ok(0) => {
                return if frame.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(frame))
                }
            }
            Ok(_) => frame.push(byte[0]),
            Err(e) => return Err(format!("MJPEG upstream read failed: {e}")),
        }
        if frame.len() > MAX_FRAME_BYTES {
            return Err(format!(
                "MJPEG frame exceeded the {MAX_FRAME_BYTES} byte limit with no boundary in sight"
            ));
        }
        if frame.ends_with(needle.as_bytes()) {
            frame.truncate(frame.len() - needle.len());
            while frame.last().is_some_and(|&b| b == b'\r' || b == b'\n') {
                frame.pop();
            }
            return Ok(Some(frame));
        }
    }
}

fn read_line(reader: &mut BufReader<TcpStream>) -> Result<String, String> {
    read_line_opt(reader)?.ok_or_else(|| "MJPEG upstream closed while reading headers".to_owned())
}

fn read_line_opt(reader: &mut BufReader<TcpStream>) -> Result<Option<String>, String> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(line)),
        Err(e) => Err(format!("MJPEG upstream read failed: {e}")),
    }
}

/// accept 循环：最新下游连接获胜。
fn serve_downstream(listener: TcpListener, slot: &Arc<Slot>, sockets: &Arc<Mutex<Vec<TcpStream>>>) {
    let mut current: Option<TcpStream> = None;
    while !slot.stopping.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                // 新连接获胜：先把旧的踢掉，保证同一时刻只有一条下游在写。
                if let Some(previous) = current.take() {
                    let _ = previous.shutdown(Shutdown::Both);
                }
                // 需要三份句柄：一份给 `sockets`（stop() 用它 shutdown 打断阻塞的
                // 写），一份留在 `current`（下一次 accept 时踢掉它），一份交给写
                // 线程。克隆不了就放弃这条连接——WebKit 会自己重连。
                let Ok(for_sockets) = stream.try_clone() else {
                    continue;
                };
                let Ok(for_writer) = stream.try_clone() else {
                    continue;
                };
                // `sockets` 只保留 [上游, 当前下游]：旧下游已经 shutdown，
                // 再留着句柄只会让 fd 随 WebKit 的每次重连无限增长。
                {
                    let mut held = sockets.lock().expect("sockets mutex poisoned");
                    held.truncate(1);
                    held.push(for_sockets);
                }
                current = Some(stream);
                let slot = Arc::clone(slot);
                thread::spawn(move || write_downstream(for_writer, &slot));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
            Err(_) => return,
        }
    }
    if let Some(previous) = current.take() {
        let _ = previous.shutdown(Shutdown::Both);
    }
}

/// 给一条下游连接持续写帧。
fn write_downstream(mut stream: TcpStream, slot: &Arc<Slot>) {
    // WebKit 会发一个真正的 GET，先把请求行和头读掉再回响应，否则它会挂在那。
    {
        let mut reader = BufReader::new(match stream.try_clone() {
            Ok(clone) => clone,
            Err(_) => return,
        });
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) if line.trim().is_empty() => break,
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    let head = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: multipart/x-mixed-replace; boundary={DOWNSTREAM_BOUNDARY}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }

    while let Some(frame) = slot.take() {
        let part = format!(
            "--{DOWNSTREAM_BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            frame.len()
        );
        if stream.write_all(part.as_bytes()).is_err()
            || stream.write_all(&frame).is_err()
            || stream.write_all(b"\r\n").is_err()
        {
            return;
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}
