//! MJPEG 代理的行为测试：全部用**测试内合成的上游**，不碰真机、不加 HTTP 依赖。

use quadcontrol_ios::mjpeg::{Proxy, MAX_FRAME_BYTES};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const BOUNDARY: &str = "upstreamframe";

/// 真机 WDA 上游响应头的形状（iPhone SE 3 + WDA 16.12.8，已实测）。
///
/// 三处和我们自己的合成上游不同，每一处都能把切帧写错：`HTTP/1.0` 而不是 1.1；
/// **boundary 值本身带 `--`**（于是正文分隔行是 `--BoundaryString`，不是四个横线）；
/// 段头首字母大小写混用（`Content-type` 而不是 `Content-Type`）。
const REAL_DEVICE_BOUNDARY_VALUE: &str = "--BoundaryString";

/// 读掉上游连接上的 HTTP 请求（请求行 + 头，直到空行），返回请求行。
///
/// 真机实测：对 WDA 的 MJPEG 端口**只连接不发请求，4 秒内 0 字节**；发出
/// `GET / HTTP/1.1` 之后才立刻推流。所以合成上游也照这个规矩来——不然代理里
/// 「忘了发请求」这个真机上必死的 bug，在测试里根本暴露不出来。
fn read_upstream_request(stream: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).ok()? == 0 {
        return None;
    }
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.trim().is_empty() => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Some(request_line.trim_end().to_owned())
}

/// 起一个合成上游：**先等下游发来请求**，再发 multipart 响应头并按 `frames` 吐帧。
///
/// 返回端口、「上游被连接过几次」的计数器（「下游重连 3 次，上游只连一次」这条
/// 断言靠它）、以及收到的请求行（证明代理确实发了请求，而不是连上就读）。
fn synthetic_upstream(
    frames: Vec<Vec<u8>>,
    repeat: bool,
    with_content_length: bool,
) -> (u16, Arc<AtomicU64>, Arc<Mutex<Vec<String>>>) {
    synthetic_upstream_shaped(frames, repeat, with_content_length, BOUNDARY, false)
}

/// [`synthetic_upstream`] 的可调形状版：boundary 值与段头大小写都能指定。
fn synthetic_upstream_shaped(
    frames: Vec<Vec<u8>>,
    repeat: bool,
    with_content_length: bool,
    boundary_value: &'static str,
    lowercase_part_header: bool,
) -> (u16, Arc<AtomicU64>, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&connections);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    // 正文分隔行永远是「`--` + 去掉前导 `--` 的 boundary 值」。
    let separator = format!("--{}", boundary_value.trim_start_matches("--"));

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            counter.fetch_add(1, Ordering::SeqCst);
            // 关键：不发请求就一个字节都不给，照 WDA 的真机行为。
            let Some(request_line) = read_upstream_request(&stream) else {
                continue;
            };
            seen.lock().unwrap().push(request_line);
            let head = format!(
                "HTTP/1.0 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary={boundary_value}\r\n\r\n"
            );
            if stream.write_all(head.as_bytes()).is_err() {
                continue;
            }
            let separator = separator.clone();
            let content_type = if lowercase_part_header {
                "Content-type"
            } else {
                "Content-Type"
            };
            let frames = frames.clone();
            thread::spawn(move || loop {
                for frame in &frames {
                    let part = if with_content_length {
                        format!(
                            "{separator}\r\n{content_type}: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                            frame.len()
                        )
                    } else {
                        format!("{separator}\r\n{content_type}: image/jpeg\r\n\r\n")
                    };
                    if stream.write_all(part.as_bytes()).is_err()
                        || stream.write_all(frame).is_err()
                        || stream.write_all(b"\r\n").is_err()
                    {
                        return;
                    }
                }
                if !repeat {
                    // 不循环时就关掉上游，模拟链路死亡。
                    return;
                }
                thread::sleep(Duration::from_millis(5));
            });
        }
    });
    (port, connections, requests)
}

/// 连上代理并读掉响应头，返回可继续读帧的 reader。
///
/// 每个 socket 都设读超时：没有它，代理若不回响应，测试会**永久挂住**——
/// 实测就这样把整个 `cargo test` 卡死过 20 分钟，而挂住比失败难查得多。
/// 有了超时，同样的故障会变成一条带行号的断言失败。
fn connect_downstream(port: u16) -> BufReader<TcpStream> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut saw_multipart = false;
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("代理没有在超时内回出响应头");
        if line
            .to_ascii_lowercase()
            .contains("multipart/x-mixed-replace")
        {
            saw_multipart = true;
            assert!(
                line.contains("boundary="),
                "下游响应必须带 boundary：{line}"
            );
        }
        if line.trim().is_empty() {
            break;
        }
    }
    assert!(
        saw_multipart,
        "下游响应必须是 multipart/x-mixed-replace，否则 WebKit 的 <img> 渲染不了"
    );
    reader
}

/// 从下游读出一帧的字节数（按 Content-Length）。
fn read_one_frame(reader: &mut BufReader<TcpStream>) -> usize {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            panic!("下游在读到一帧之前就关了");
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
        }
        if line.trim().is_empty() && length.is_some() {
            break;
        }
    }
    let length = length.unwrap();
    let mut frame = vec![0u8; length];
    reader.read_exact(&mut frame).unwrap();
    let mut trailer = [0u8; 2];
    reader.read_exact(&mut trailer).unwrap();
    length
}

fn wait_for_frames(proxy: &Proxy, at_least: u64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while proxy.stats().frames < at_least {
        assert!(
            Instant::now() < deadline,
            "上游帧没在预算内到达：{:?}",
            proxy.stats()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn counts_frames_from_the_upstream() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![7u8; 512]; 3], true, true);
    let proxy = Proxy::start(upstream).unwrap();
    wait_for_frames(&proxy, 5);

    let stats = proxy.stats();
    assert!(stats.frames >= 5, "{stats:?}");
    assert!(stats.last_frame_at.is_some());
}

#[test]
fn frames_also_split_without_content_length() {
    // 没有 Content-Length 时要能扫到边界切帧。
    let (upstream, _, _) = synthetic_upstream(vec![vec![3u8; 256]; 2], true, false);
    let proxy = Proxy::start(upstream).unwrap();
    wait_for_frames(&proxy, 3);
    assert!(proxy.stats().frames >= 3);
}

#[test]
fn downstream_receives_renderable_multipart_frames() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![9u8; 1024]], true, true);
    let proxy = Proxy::start(upstream).unwrap();
    let mut reader = connect_downstream(proxy.local_port());
    for _ in 0..3 {
        assert_eq!(read_one_frame(&mut reader), 1024);
    }
}

/// 单槽缓冲：下游读得慢时旧帧被覆盖并计 `backpressure_drops`。
///
/// 用**大帧**（256 KiB）才能真的把 loopback 的发送缓冲填满——macOS 上小帧会被
/// 内核缓冲吸收，写方根本不阻塞，丢帧计数就恒零，用例会假通过。
#[test]
fn slow_downstream_drops_stale_frames() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![1u8; 256 * 1024]; 4], true, true);
    let proxy = Proxy::start(upstream).unwrap();
    // 连上但**几乎不读**，让下游写阻塞。
    let _slow = connect_downstream(proxy.local_port());

    let deadline = Instant::now() + Duration::from_secs(10);
    while proxy.stats().backpressure_drops == 0 {
        assert!(
            Instant::now() < deadline,
            "慢下游没有产生丢帧计数：{:?}",
            proxy.stats()
        );
        thread::sleep(Duration::from_millis(20));
    }
    let stats = proxy.stats();
    assert!(stats.backpressure_drops > 0, "{stats:?}");
    // 丢帧不该把计帧也带停：帧还在从上游进来。
    assert!(stats.frames > stats.backpressure_drops.saturating_sub(1));
}

/// 下游重连 3 次，帧仍持续，且**上游只被连过一次**。
#[test]
fn downstream_reconnects_keep_one_upstream_connection() {
    let (upstream, connections, _) = synthetic_upstream(vec![vec![5u8; 2048]], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    for round in 0..3 {
        let mut reader = connect_downstream(proxy.local_port());
        assert_eq!(read_one_frame(&mut reader), 2048, "第 {round} 轮没读到帧");
        drop(reader);
        thread::sleep(Duration::from_millis(50));
    }

    assert_eq!(
        connections.load(Ordering::SeqCst),
        1,
        "下游重连不应导致重新拉取上游流"
    );
    // 重连之后帧仍在继续。
    let before = proxy.stats().frames;
    wait_for_frames(&proxy, before + 1);
}

/// 新下游连上**不会**关掉已有的下游（旧契约「最新连接获胜」已被真机证伪）。
///
/// 断言是正面的：第二条读到帧之后，第一条还能再读到帧。socket 带 10 s 读超时，
/// 所以「其实已经被关掉/挂死」会变成一次带行号的失败，而不是无限等待。
#[test]
fn a_new_downstream_does_not_evict_the_previous() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![4u8; 1024]], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    let mut first = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut first), 1024);

    let mut second = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut second), 1024);

    assert_eq!(
        read_one_frame(&mut first),
        1024,
        "第二条下游连上之后，第一条不该被踢掉"
    );
}

/// 两条下游**同时**各自持续收帧：代理是广播，不是独占。
#[test]
fn two_downstreams_both_receive_frames() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![4u8; 1024]], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    let mut first = connect_downstream(proxy.local_port());
    let mut second = connect_downstream(proxy.local_port());

    for round in 0..5 {
        assert_eq!(read_one_frame(&mut first), 1024, "第一条第 {round} 帧");
        assert_eq!(read_one_frame(&mut second), 1024, "第二条第 {round} 帧");
    }
}

/// 一次「探针」连接（连上、读一帧、断开）不得把已有下游一起带走。
///
/// 这正是真机上踩的那一脚：GUI 画面好好的，用 curl 连一次代理端口做计数，旧契约
/// 就把 WebView 那条连接关了，`<img>` 从此冻在最后一帧、不重连也不报错。
#[test]
fn a_probe_connection_does_not_evict_the_first() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![6u8; 1024]], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    let mut first = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut first), 1024);

    {
        let mut probe = connect_downstream(proxy.local_port());
        assert_eq!(read_one_frame(&mut probe), 1024);
    } // 探针在这里断开。

    let frames_at_probe_exit = proxy.stats().frames;
    // 读固定条数而不是「睡 2 秒再看」：读超时把失败变成确定的 panic。
    for round in 0..5 {
        assert_eq!(
            read_one_frame(&mut first),
            1024,
            "探针断开后第一条下游停在第 {round} 帧"
        );
    }
    // 而且必须是**新**帧，不是缓冲里攒下的旧帧。有界等待而不是立刻断言：探针期间
    // 第一条没读，帧攒在内核缓冲里，上面 5 次读可能在微秒级读完，比合成上游的
    // 5 ms 一帧还快——直接断言就是在和上游时钟赛跑（实测 1/40 偶发）。
    wait_for_frames(&proxy, frames_at_probe_exit + 1);
}

/// 在 `budget` 内把流读到干净 EOF（`Ok(0)`）；读超时或出错都算失败。
fn drain_until_eof(reader: &mut BufReader<TcpStream>, budget: Duration, message: &str) {
    reader
        .get_ref()
        .set_read_timeout(Some(budget))
        .expect("设置读超时失败");
    let deadline = Instant::now() + budget;
    let mut scratch = [0u8; 16 * 1024];
    loop {
        match reader.read(&mut scratch) {
            Ok(0) => return,
            Ok(_) => assert!(Instant::now() < deadline, "{message}（仍在源源不断收数据）"),
            Err(error) => panic!("{message}：读失败而不是 EOF（{error}）"),
        }
    }
}

/// 下游**先连上、隔 100 ms 才发 GET**，仍须及时拿到响应头。
///
/// 这条挡的是 accept 继承 O_NONBLOCK 的坑：listener 是非阻塞的，而 macOS/BSD 的
/// accept 让新 socket 继承这个标志（Linux 不继承）。若不显式 `set_nonblocking(false)`，
/// 写线程读请求头时会拿到 `WouldBlock` 并当成失败关掉连接——WebKit 只要稍慢发 GET
/// 就永远得不到画面。原来的用例都是连上立刻发 GET，恰好躲开了这个窗口。
#[test]
fn delayed_request_still_gets_a_response_head() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![7u8; 1024]], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    let mut stream = TcpStream::connect(("127.0.0.1", proxy.local_port())).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    // 关键：先沉默 100 ms，再发请求。
    thread::sleep(Duration::from_millis(100));
    stream
        .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("延迟发 GET 之后 1 s 内没收到响应头（accept 继承了 O_NONBLOCK？）");
    assert!(line.starts_with("HTTP/1.1 200"), "{line}");
}

/// 慢下游（连上但不读）必须在 2 s 内**要么被断开、要么仍有帧在产出**，不能挂死。
///
/// 「挂住」是最难查的失败模式：既没有 EOF 也没有帧，看起来像还连着。这条用例把它
/// 变成一次明确的失败。
#[test]
fn slow_downstream_does_not_wedge_the_proxy() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![1u8; 256 * 1024]; 4], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    let mut stream = TcpStream::connect(("127.0.0.1", proxy.local_port())).unwrap();
    stream
        .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .unwrap();
    // 不读，让下游写阻塞。
    let frames_before = proxy.stats().frames;

    thread::sleep(Duration::from_secs(2));

    // 要么下游已被断开（读到 EOF / 出错），要么上游仍在计帧——两者都说明代理没卡死。
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut scratch = [0u8; 1024];
    let downstream_closed = matches!(stream.read(&mut scratch), Ok(0));
    let still_producing = proxy.stats().frames > frames_before;
    assert!(
        downstream_closed || still_producing,
        "慢下游把代理卡死了：既没断开也没有新帧（{:?}）",
        proxy.stats()
    );
}

/// 没有任何下游时，帧被覆盖**不算**背压丢帧。
///
/// `backpressure_drops` 是给「画面卡是因为下游慢」用的诊断量。GUI 还没连上来时帧
/// 当然会在单槽里被覆盖，那是正常空转；照计会让这个指标在最常见的场景里虚高。
#[test]
fn no_downstream_means_no_backpressure_drops() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![2u8; 1024]; 4], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    // 一个下游都不连，等上游吐足够多的帧（必然发生覆盖）。
    wait_for_frames(&proxy, 8);

    let stats = proxy.stats();
    assert!(stats.frames >= 8, "{stats:?}");
    assert_eq!(
        stats.backpressure_drops, 0,
        "没有下游时不应记背压丢帧：{stats:?}"
    );
}

#[test]
fn stop_closes_the_downstream_connection() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![6u8; 1024]], true, true);
    let mut proxy = Proxy::start(upstream).unwrap();
    let mut reader = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut reader), 1024);

    proxy.stop();

    // 同样要求干净 EOF：`stop()` 必须真的把下游连接关掉，而不是停止供帧就算完。
    drain_until_eof(
        &mut reader,
        Duration::from_secs(2),
        "stop() 之后下游连接没有关闭",
    );
    // 停掉之后监听端口也不该再接受连接。
    thread::sleep(Duration::from_millis(100));
    assert!(
        TcpStream::connect(("127.0.0.1", proxy.local_port()))
            .and_then(|mut s| {
                s.write_all(b"GET / HTTP/1.1\r\n\r\n")?;
                let mut buffer = Vec::new();
                s.set_read_timeout(Some(Duration::from_millis(500)))?;
                let _ = s.read_to_end(&mut buffer);
                Ok(buffer)
            })
            .map(|body| body.is_empty())
            .unwrap_or(true),
        "stop() 之后监听端口仍在提供数据"
    );
}

/// 代理必须**主动向上游发 HTTP 请求**，否则真机上一帧都拿不到。
///
/// 真机实测（iPhone SE 3 + WDA 16.12.8）：只连接不发请求，4 秒内 0 字节；发出
/// `GET / HTTP/1.1` 后立刻推流。早先代理连上就开读，于是它向下游回了响应头却
/// 永远没有帧，GUI 显示「已连接 · 0 帧/秒」。
#[test]
fn the_proxy_sends_a_request_before_reading_the_upstream() {
    let (upstream, _, requests) = synthetic_upstream(vec![vec![7u8; 512]; 3], true, true);
    let proxy = Proxy::start(upstream).unwrap();
    // 合成上游在收到请求之前一个字节都不发，所以「有帧」本身就证明请求发出去了。
    wait_for_frames(&proxy, 3);

    let seen = requests.lock().unwrap().clone();
    assert!(!seen.is_empty(), "上游没有收到任何请求");
    assert!(
        seen.iter().all(|line| line.starts_with("GET ")),
        "上游收到的不是 GET 请求：{seen:?}"
    );
}

/// 真机上游的形状也要能正确切帧。
///
/// 三个真机细节一次测全：`HTTP/1.0`、boundary 值**自带 `--`**（正文分隔行是
/// `--BoundaryString`，不是四个横线）、段头 `Content-type` 小写 t。
#[test]
fn real_device_upstream_shape_splits_frames() {
    let (upstream, _, requests) = synthetic_upstream_shaped(
        vec![vec![8u8; 1024]; 3],
        true,
        true,
        REAL_DEVICE_BOUNDARY_VALUE,
        true,
    );
    let proxy = Proxy::start(upstream).unwrap();
    wait_for_frames(&proxy, 5);

    assert!(proxy.is_alive(), "真机形状的上游把代理弄死了");
    assert!(proxy.failure().is_none(), "{:?}", proxy.failure());
    assert!(!requests.lock().unwrap().is_empty());

    // 下游拿到的必须是一帧完整的、长度正确的 JPEG 段。
    let mut reader = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut reader), 1024);
}

/// 单帧超限：断流并报 `proxy_failed`。
#[test]
fn oversized_frame_tears_down_the_stream() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_upstream_request(&stream).unwrap();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary={BOUNDARY}\r\n\r\n"
        );
        stream.write_all(head.as_bytes()).unwrap();
        // 只**声明**一个超限的长度，不真的发 4 MiB：代理必须在读之前就拒绝。
        let part = format!(
            "--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            MAX_FRAME_BYTES + 1
        );
        stream.write_all(part.as_bytes()).unwrap();
        thread::sleep(Duration::from_secs(3));
    });

    let proxy = Proxy::start(port).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(error) = proxy.failure() {
            assert_eq!(error.code(), "proxy_failed");
            assert!(error.to_string().contains("exceeds"), "{error}");
            break;
        }
        assert!(Instant::now() < deadline, "超限帧没有触发断流");
        thread::sleep(Duration::from_millis(20));
    }
}

/// 上游 EOF：链路死亡，代理自行收摊，下游连接随之关闭。
#[test]
fn upstream_eof_tears_down_the_proxy() {
    let (upstream, _, _) = synthetic_upstream(vec![vec![2u8; 512]], false, true);
    let proxy = Proxy::start(upstream).unwrap();
    wait_for_frames(&proxy, 1);

    // 上游一 EOF 代理就收摊，所以这里**连都可能连不上**——那同样是通过：
    // 要证明的是「链路死亡后下游拿不到持续画面」，不是「一定能建连再断开」。
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut scratch = [0u8; 4096];
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", proxy.local_port())) else {
        return;
    };
    if stream
        .write_all(b"GET /stream HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return;
    }
    loop {
        match stream.read(&mut scratch) {
            Ok(0) | Err(_) => break,
            Ok(_) => assert!(Instant::now() < deadline, "上游 EOF 后下游还在收帧"),
        }
    }
}

#[test]
fn start_reports_a_refused_upstream() {
    // 绑一个端口再立刻释放，拿到一个大概率没人听的端口号。
    let port = {
        let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        probe.local_addr().unwrap().port()
    };
    // 用短预算：默认的 10 s 重试是为了等 `ios forward` 绑端口，这里只想看拒连。
    let error = Proxy::start_with_connect_budget(port, Duration::from_millis(200)).unwrap_err();
    assert_eq!(error.code(), "proxy_failed");
}
