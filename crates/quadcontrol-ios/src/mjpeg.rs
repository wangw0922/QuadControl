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
//! - accept 循环，**广播给所有下游**：每条下游连接各有一个写线程，互不驱逐。
//!   **上游始终最多一条**，且跨下游重连保持不断。
//!
//!   这条契约原来是「最新下游连接获胜」，依据是「WebKit 对 MJPEG 会自行重连」。
//!   **真机实测（iPhone SE 3 + macOS WKWebView，2026-09-15）证明那个假设是错的**：
//!   用 curl 连一次代理端口做计数，代理按旧契约关掉了 WebView 那条连接
//!   （`lsof` 显示 CLOSED），此后 `<img>` **永远停在最后一帧、不重连也不报错**，
//!   而转发端口上的帧是新的。所以代理不再踢任何人。
//!
//!   代价如实写明：一条**读停了但不关闭**的下游，现在会一直占着一个写线程和一个
//!   `Arc` 帧引用，直到对端关闭或 [`Proxy::stop`]；旧的驱逐逻辑顺带清掉了这种连接。
//! - 按 multipart 边界切帧，单帧上限 [`MAX_FRAME_BYTES`]，超限即断流并记
//!   [`crate::Error::ProxyFailed`]。
//! - 只保留「最新一帧 + 单调递增序号」：下游慢时旧帧被覆盖并计
//!   `backpressure_drops`（定义见 [`FrameStats::backpressure_drops`]）。
//!   这是评审阻塞项 B3 的裁决——无界队列会让 `backpressure_drops` 恒零，
//!   而且把内存压力变成延迟累积。
//! - **不解码 JPEG，不做黑帧检测。** 停帧不是锁屏信号，锁定提示只由 `/wda/locked`
//!   驱动。这一条现在是**实测结论**（iPhone SE 3 + WDA 16.12.8）：锁屏后 MJPEG
//!   照常出帧（3 s 41 帧，每帧约 20 KB 全黑），`/screenshot` 返回 13 KB 的合法
//!   全黑 PNG、不报错，点按也静默成功。也就是说「没有帧」和「锁屏了」之间没有
//!   任何相关性，靠画面猜锁定状态一定会猜错。
//! - 上游**必须先收到一条 HTTP 请求**才会推流，见 [`UPSTREAM_REQUEST`]。
//!
//! # 实测帧率基线
//!
//! iPhone SE 3（750×1334）。2026-09-14：fps 15 / quality 50，单帧约 87 KB，
//! 3 秒 41 帧 ≈ 13.7 fps。2026-09-15 重测另三组（帧体积与上一组不同，可能是画面
//! 内容不同，未核实；两组都按实测保留）：
//! fps 15 / quality 50 ≈ 14 fps、帧约 40 KB；fps 30 / quality 30 ≈ 17 fps、
//! 帧约 37 KB（即现在的 [`crate::session::MJPEG_FPS`] / [`crate::session::MJPEG_QUALITY`]）；
//! `mjpegScalingFactor` 50 不提升帧率——瓶颈在设备端截屏，所以不设缩放。
//! **限帧只在编码器侧做，与设备刷新率无关**（红线）。

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
/// `stop()` 里「还要不要 join 下一个线程」的总预算；语义见 [`Proxy::stop`]。
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
    /// 背压丢帧数：**发布新帧时，上一帧一个写线程都没取走过**。
    ///
    /// 没有任何下游时不计——那是正常空转，不是背压（见 [`Slot::publish`]）。
    pub backpressure_drops: u64,
    /// 最后一帧到达的时刻；一帧都没有时是 None。
    pub last_frame_at: Option<Instant>,
}

/// 上游/下游共享的「最新帧 + 单调递增序号」广播槽。
struct Slot {
    state: Mutex<SlotState>,
    ready: Condvar,
    drops: AtomicU64,
    last_frame_at: Mutex<Option<Instant>>,
    stopping: AtomicBool,
    /// 当前活跃的下游写线程数。
    writers: AtomicU64,
}

/// 槽内状态。只保留最新一帧，靠 `seq` 让每条下游各自判断「有没有新帧」。
struct SlotState {
    /// 最新一帧。用 [`Arc`] 是为了广播给 N 条下游时只复制一个引用计数。
    frame: Option<Arc<Vec<u8>>>,
    /// 已发布的帧总数，也是帧序号；从 0 开始，第一帧是 1。
    seq: u64,
    /// 当前这一帧有没有被**任何**写线程取走过。[`Slot::publish`] 靠它算背压丢帧。
    consumed: bool,
}

/// 写线程退出时把 [`Slot::writers`] 减回去、并把自己的 socket 从注册表里摘掉，
/// panic 路径也不漏。不摘的话 fd 会随下游的每次重连无限增长。
struct WriterGuard<'a> {
    slot: &'a Arc<Slot>,
    sockets: &'a Arc<Mutex<Vec<(u64, TcpStream)>>>,
    id: u64,
}
impl Drop for WriterGuard<'_> {
    fn drop(&mut self) {
        self.slot.writers.fetch_sub(1, Ordering::AcqRel);
        self.sockets
            .lock()
            .expect("sockets mutex poisoned")
            .retain(|(id, _)| *id != self.id);
    }
}

impl Slot {
    fn publish(&self, frame: Vec<u8>) {
        let mut state = self.state.lock().expect("frame mutex poisoned");
        // 上一帧**一个写线程都没取走过** = 有人在消费但没跟上，记一次背压丢帧。
        //
        // **没有下游时不计**：`backpressure_drops` 是给「画面卡是因为下游慢」用的
        // 诊断量。GUI 还没连上来（或用户已断开）时帧当然会被覆盖，那是正常的空转，
        // 不是背压；照计只会让这个指标在最常见的场景里虚高到没法用。
        //
        // 广播语义下「被消费」= 至少一条下游读到了；多条下游里只要有一条跟得上就
        // 不算丢——丢帧要归咎到「所有人都没跟上」，否则一条慢下游会让指标失真。
        if state.seq > 0 && !state.consumed && self.writers.load(Ordering::Acquire) > 0 {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
        state.frame = Some(Arc::new(frame));
        state.seq += 1;
        state.consumed = false;
        *self.last_frame_at.lock().expect("instant mutex poisoned") = Some(Instant::now());
        self.ready.notify_all();
    }

    /// 已发布的帧总数（= 当前帧序号）。
    fn frames(&self) -> u64 {
        self.state.lock().expect("frame mutex poisoned").seq
    }

    /// 等到比 `last_seq` 新的一帧并返回它的引用；停止时返回 None。
    ///
    /// 有界等待（100 ms 轮一次停止标志），绝不无限挂起。写线程的 `last_seq` 从 0
    /// 起，而第一帧的序号是 1，所以**新连上的下游立刻拿到当前最新帧**，不用等下
    /// 一帧才有画面可画。
    fn next_frame(&self, last_seq: &mut u64) -> Option<Arc<Vec<u8>>> {
        let mut state = self.state.lock().expect("frame mutex poisoned");
        loop {
            if self.stopping.load(Ordering::Acquire) {
                return None;
            }
            if state.seq > *last_seq {
                *last_seq = state.seq;
                state.consumed = true;
                return state.frame.clone();
            }
            let (next, _) = self
                .ready
                .wait_timeout(state, Duration::from_millis(100))
                .expect("frame mutex poisoned");
            state = next;
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
    /// 上游 socket 的一份句柄；`stop()` 用它 shutdown 打断阻塞的读。
    /// 与下游分开存：下游会来来去去，上游整条代理只有一条。
    upstream: TcpStream,
    /// **活跃**下游 socket 的句柄，`(id, socket)`。`stop()` 用它们打断阻塞的写；
    /// 写线程退出时按 id 摘掉自己（见 [`WriterGuard`]），所以这里不会无限增长。
    sockets: Arc<Mutex<Vec<(u64, TcpStream)>>>,
    threads: Vec<JoinHandle<()>>,
}

impl Proxy {
    /// 连上游 `127.0.0.1:<upstream_port>`，在 `127.0.0.1:0` 起下游监听。
    ///
    /// 上游**在这里连一次**，之后跨下游连接的来去一直保持——前端重设 `<img>`
    /// 的 `src` 时不该导致重新拉一条设备侧的流。
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
            state: Mutex::new(SlotState {
                frame: None,
                seq: 0,
                consumed: false,
            }),
            ready: Condvar::new(),
            drops: AtomicU64::new(0),
            last_frame_at: Mutex::new(None),
            stopping: AtomicBool::new(false),
            writers: AtomicU64::new(0),
        });
        let failure = Arc::new(Mutex::new(None));
        let upstream_handle = upstream
            .try_clone()
            .map_err(|e| Error::ProxyFailed(format!("MJPEG upstream clone failed: {e}")))?;
        let sockets = Arc::new(Mutex::new(Vec::new()));

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
            upstream: upstream_handle,
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
            frames: self.slot.frames(),
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

    /// 关监听、断上下游、join 全部线程。
    ///
    /// accept 与 read 都可能正阻塞着，所以先置停止标志并 `shutdown` 所有 socket
    /// 把它们踢醒，再 join。
    ///
    /// 关于 [`JOIN_BUDGET`] 的**如实说明**：`std` 的 `JoinHandle::join` 没有超时，
    /// 所以单次 join 一旦开始就是无界的——不卡住靠的是上面的 `shutdown`，它保证
    /// 每个线程都已经在退出路上。这里的预算只做一件事：在**开始下一次** join 之前
    /// 检查是否已超时，超了就不再 join 剩下的线程（它们已被 shutdown，会自行退出）。
    /// 换句话说预算限制的是「还要不要等下一个」，不是「等当前这个多久」。
    pub fn stop(&mut self) {
        self.slot.stopping.store(true, Ordering::Release);
        self.slot.ready.notify_all();
        let _ = self.upstream.shutdown(Shutdown::Both);
        // 作用域刻意收紧：写线程退出时也要锁 `sockets` 把自己摘掉，join 时还握着
        // 这把锁就是死锁。
        {
            let held = self.sockets.lock().expect("sockets mutex poisoned");
            for (_, socket) in held.iter() {
                let _ = socket.shutdown(Shutdown::Both);
            }
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

/// 连上上游后必须发的 HTTP 请求。
///
/// **真机实测（iPhone SE 3 + WDA 16.12.8）**：对 WDA 的 MJPEG 端口只连接、不发
/// 请求，4 秒内收到 **0 字节**；发出下面这一行之后立刻开始推流。早先 `pump_upstream`
/// 直接开读，于是代理向下游回了响应头却永远拿不到帧，GUI 上显示成
/// 「已连接 · 0 帧/秒」——典型的「连上了 ≠ 真的产生效果」。
const UPSTREAM_REQUEST: &str = "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";

/// 读上游、切帧、发布到广播槽。
fn pump_upstream(stream: TcpStream, slot: &Arc<Slot>, failure: &Arc<Mutex<Option<String>>>) {
    // 先发请求再读：不发的话上游一个字节都不会给（见 [`UPSTREAM_REQUEST`]）。
    if let Err(error) = request_stream(&stream) {
        *failure.lock().expect("failure mutex poisoned") = Some(error);
        slot.stopping.store(true, Ordering::Release);
        slot.ready.notify_all();
        return;
    }
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

/// 向上游发出 [`UPSTREAM_REQUEST`]；写失败按 `proxy_failed` 记录。
fn request_stream(stream: &TcpStream) -> Result<(), String> {
    let mut stream = stream
        .try_clone()
        .map_err(|error| format!("MJPEG upstream socket could not be cloned: {error}"))?;
    stream
        .write_all(UPSTREAM_REQUEST.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|error| format!("MJPEG upstream did not accept the stream request: {error}"))
}

/// 读上游的 HTTP 响应头，取出 multipart 边界。
///
/// 真机上游响应的形状（WDA 16.12.8，已实测）：
///
/// ```text
/// HTTP/1.0 200 OK
/// Content-Type: multipart/x-mixed-replace; boundary=--BoundaryString
/// ```
///
/// 注意 **boundary 值本身带 `--`**，正文分隔行也就是 `--BoundaryString`，不是四个
/// 横线。下面的 `trim_start_matches("--")` 正是为它准备的：统一成不带 `--` 的形式，
/// 切帧时由 [`read_frame`] 自己补一个 `--`。每段的头是
/// `Content-type: image/jpeg` + `Content-Length: <n>`（真机首字母大小写混用，所以
/// 头名比较一律 `eq_ignore_ascii_case`），JPEG 正文之后紧跟 `\r\n`。
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

/// accept 循环：**每条下游各起一个写线程，谁也不驱逐谁**。
///
/// 旧实现是「最新连接获胜」，真机上被证伪：WKWebView 的 `<img>` 被踢掉之后不会
/// 重连，画面就此冻住（见模块文档）。
fn serve_downstream(
    listener: TcpListener,
    slot: &Arc<Slot>,
    sockets: &Arc<Mutex<Vec<(u64, TcpStream)>>>,
) {
    let mut next_id: u64 = 0;
    while !slot.stopping.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => {
                // **必须显式转回阻塞**：listener 是非阻塞的（accept 循环靠它感知停止
                // 标志），而 macOS/BSD 的 `accept` 让新 socket **继承** O_NONBLOCK，
                // Linux 不继承。不改回来的话，写线程读请求头时会拿到 `WouldBlock`
                // 并被当成失败，下游在 WebKit 稍慢发 GET 时就永远收不到响应头。
                if stream.set_nonblocking(false).is_err() {
                    continue;
                }
                // 两份句柄：一份进 `sockets`（`stop()` 用它 shutdown 打断阻塞的写），
                // 一份交给写线程。克隆不了就放弃这条连接。
                let Ok(for_sockets) = stream.try_clone() else {
                    continue;
                };
                let id = next_id;
                next_id += 1;
                sockets
                    .lock()
                    .expect("sockets mutex poisoned")
                    .push((id, for_sockets));
                let slot = Arc::clone(slot);
                let sockets = Arc::clone(sockets);
                thread::spawn(move || write_downstream(stream, &slot, &sockets, id));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(ACCEPT_POLL),
            Err(_) => return,
        }
    }
}

/// 给一条下游连接持续写帧。
///
/// **写线程死亡 = 连接关闭**：每一条提前 return 的路径都先 `shutdown(Both)`。
/// 否则这条 socket 会留在「已建立但永远没人写」的状态，下游读方既拿不到帧也等不到
/// EOF，只能一直挂着——对 WebKit 来说就是一张永不刷新、也不报错的空白图。
fn write_downstream(
    mut stream: TcpStream,
    slot: &Arc<Slot>,
    sockets: &Arc<Mutex<Vec<(u64, TcpStream)>>>,
    id: u64,
) {
    // 活跃写线程计数：`publish` 靠它判断「有没有人在消费」，没有下游时不该把
    // 被覆盖的帧记成背压丢帧（见 [`Slot::publish`]）。守卫同时负责把这条 socket
    // 从 `sockets` 里摘掉。
    slot.writers.fetch_add(1, Ordering::AcqRel);
    let _guard = WriterGuard { slot, sockets, id };

    // WebKit 会发一个真正的 GET，先把请求行和头读掉再回响应，否则它会挂在那。
    {
        let mut reader = BufReader::new(match stream.try_clone() {
            Ok(clone) => clone,
            Err(_) => {
                let _ = stream.shutdown(Shutdown::Both);
                return;
            }
        });
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
                Ok(_) if line.trim().is_empty() => break,
                Ok(_) => {}
            }
        }
    }

    let head = format!(
        "HTTP/1.1 200 OK\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Content-Type: multipart/x-mixed-replace; boundary={DOWNSTREAM_BOUNDARY}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        let _ = stream.shutdown(Shutdown::Both);
        return;
    }

    // 从 0 起：第一帧序号是 1，所以一连上就能拿到当前最新帧。
    let mut last_seq = 0u64;
    while let Some(frame) = slot.next_frame(&mut last_seq) {
        let part = format!(
            "--{DOWNSTREAM_BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            frame.len()
        );
        if stream.write_all(part.as_bytes()).is_err()
            || stream.write_all(&frame).is_err()
            || stream.write_all(b"\r\n").is_err()
        {
            let _ = stream.shutdown(Shutdown::Both);
            return;
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}
