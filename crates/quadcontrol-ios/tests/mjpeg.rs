//! MJPEG 代理的行为测试：全部用**测试内合成的上游**，不碰真机、不加 HTTP 依赖。

use quadcontrol_ios::mjpeg::{Proxy, MAX_FRAME_BYTES};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const BOUNDARY: &str = "upstreamframe";

/// 起一个合成上游：发 multipart 响应头，然后按 `frames` 吐帧。
///
/// 返回端口和「上游被连接过几次」的计数器——「下游重连 3 次，上游只连一次」这条
/// 断言就靠它。
fn synthetic_upstream(
    frames: Vec<Vec<u8>>,
    repeat: bool,
    with_content_length: bool,
) -> (u16, Arc<AtomicU64>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&connections);

    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            counter.fetch_add(1, Ordering::SeqCst);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary={BOUNDARY}\r\n\r\n"
            );
            if stream.write_all(head.as_bytes()).is_err() {
                continue;
            }
            let frames = frames.clone();
            thread::spawn(move || loop {
                for frame in &frames {
                    let part = if with_content_length {
                        format!(
                            "--{BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                            frame.len()
                        )
                    } else {
                        format!("--{BOUNDARY}\r\nContent-Type: image/jpeg\r\n\r\n")
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
    (port, connections)
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
    let (upstream, _) = synthetic_upstream(vec![vec![7u8; 512]; 3], true, true);
    let proxy = Proxy::start(upstream).unwrap();
    wait_for_frames(&proxy, 5);

    let stats = proxy.stats();
    assert!(stats.frames >= 5, "{stats:?}");
    assert!(stats.last_frame_at.is_some());
}

#[test]
fn frames_also_split_without_content_length() {
    // 没有 Content-Length 时要能扫到边界切帧。
    let (upstream, _) = synthetic_upstream(vec![vec![3u8; 256]; 2], true, false);
    let proxy = Proxy::start(upstream).unwrap();
    wait_for_frames(&proxy, 3);
    assert!(proxy.stats().frames >= 3);
}

#[test]
fn downstream_receives_renderable_multipart_frames() {
    let (upstream, _) = synthetic_upstream(vec![vec![9u8; 1024]], true, true);
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
    let (upstream, _) = synthetic_upstream(vec![vec![1u8; 256 * 1024]; 4], true, true);
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
    let (upstream, connections) = synthetic_upstream(vec![vec![5u8; 2048]], true, true);
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

/// 最新下游获胜：新连接到来后旧连接被关闭。
#[test]
fn newest_downstream_wins() {
    let (upstream, _) = synthetic_upstream(vec![vec![4u8; 1024]], true, true);
    let proxy = Proxy::start(upstream).unwrap();

    let mut first = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut first), 1024);

    let mut second = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut second), 1024);

    // 旧连接应当被关掉：读到 EOF 或报错，总之不会再有完整帧源源不断。
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut scratch = [0u8; 4096];
    loop {
        match first.read(&mut scratch) {
            Ok(0) | Err(_) => break,
            Ok(_) => assert!(
                Instant::now() < deadline,
                "新下游连上后旧连接还在源源不断收帧"
            ),
        }
    }
}

#[test]
fn stop_closes_the_downstream_connection() {
    let (upstream, _) = synthetic_upstream(vec![vec![6u8; 1024]], true, true);
    let mut proxy = Proxy::start(upstream).unwrap();
    let mut reader = connect_downstream(proxy.local_port());
    assert_eq!(read_one_frame(&mut reader), 1024);

    proxy.stop();

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut scratch = [0u8; 4096];
    loop {
        match reader.read(&mut scratch) {
            Ok(0) | Err(_) => break,
            Ok(_) => assert!(Instant::now() < deadline, "stop() 之后下游连接没有关闭"),
        }
    }
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

/// 单帧超限：断流并报 `proxy_failed`。
#[test]
fn oversized_frame_tears_down_the_stream() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
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
    let (upstream, _) = synthetic_upstream(vec![vec![2u8; 512]], false, true);
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
