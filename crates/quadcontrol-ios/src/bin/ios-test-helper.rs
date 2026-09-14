//! 假 `ios` 二进制，照 `scrcpy-test-helper` 的模式用环境变量控制行为。
//!
//! 它存在的理由和 Android 侧一样：**只有驱动真正的子进程，才能暴露「监督线程
//! 接线错了」这类错误**——纯单测做不到。
//!
//! 关键的一点：`forward` 必须是**真的 TCP 转发**。会话会去连自己 bind 0 拿到的
//! 本地端口，若假 `forward` 只是长驻发呆，那个端口上没人监听，Launch 模式永远
//! 到不了 `Running`，「正常起停无孤儿」和「4 个子进程全忽略 SIGINT」这两条用例
//! 就根本写不出来。所以它把 `<hostPort>` 绑起来，把字节管到测试指定的上游。
//!
//! 环境变量：
//! - `QUADCONTROL_FAKE_ARGV_LOG`：把本次收到的 argv 追加写到该文件（供语法断言）。
//! - `QUADCONTROL_FAKE_PID_DIR`：把各子命令的 PID 写成 `<dir>/<子命令>.pid`。
//! - `QUADCONTROL_FAKE_IGNORE_SIGINT=1`：长驻子命令忽略 SIGINT（测并行梯子）。
//! - `QUADCONTROL_FAKE_IGNORE_SIGTERM=1`：再忽略 SIGTERM，只有 SIGKILL 收得走
//!   （梯子的最坏路径）。
//! - `QUADCONTROL_FAKE_RUNWDA_STDERR`：`runwda` 立刻把它打到 stderr 并退出。
//! - `QUADCONTROL_FAKE_FORWARD_<targetPort>`：该目标端口要转发到的本机上游端口。
//! - `QUADCONTROL_FAKE_TUNNEL_DELAY_MS`：隧道注册进信息服务前的延迟（默认 300）。
//! - `QUADCONTROL_FAKE_TUNNEL_NEVER=1`：隧道永不注册，用来测 `tunnel_failed`。
//!
//! `tunnel start` 还会**真的绑上** `--tunnel-info-port` 并在上面跑一个最小的隧道
//! 信息服务：会话的就绪判据是 `GET /tunnels` 里出现本机 UDID，替身只绑不应答的话
//! Launch 模式就再也起不来了（见 [`run_tunnel`]）。

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    log_argv(&args);

    // 子命令识别要跳过通用 options（`--udid=…` 等可以出现在子命令之前）。
    let positional: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with('-'))
        .collect();

    match positional.as_slice() {
        ["version", ..] => println!("{}", fixture("ios-version.stdout.json")),
        ["list", ..] => {
            // 真 go-ios 会往 stderr 打一条 JSON 日志行；替身照打，用来证明
            // 解析器只读 stdout。
            eprint!("{}", fixture("ios-list-empty.stderr.jsonl"));
            println!("{}", fixture("ios-list-empty.stdout.json"));
        }
        ["tunnel", "start", ..] => run_tunnel(&args),
        ["runwda", ..] => run_wda(),
        ["forward", host_port, target_port, ..] => run_forward(host_port, target_port),
        _ => {
            eprintln!("ios-test-helper: unsupported argv {args:?}");
            std::process::exit(64);
        }
    }
}

/// 把 fixture 内容编进二进制：测试跑在任意 cwd 下都能拿到。
fn fixture(name: &str) -> &'static str {
    match name {
        "ios-version.stdout.json" => {
            include_str!("../../tests/fixtures/go-ios-1.2.1/ios-version.stdout.json")
        }
        "ios-list-empty.stdout.json" => {
            include_str!("../../tests/fixtures/go-ios-1.2.1/ios-list-empty.stdout.json")
        }
        "ios-list-empty.stderr.jsonl" => {
            include_str!("../../tests/fixtures/go-ios-1.2.1/ios-list-empty.stderr.jsonl")
        }
        other => panic!("unknown fixture {other}"),
    }
}

/// 把 argv 逐行追加进日志文件，供 fixture 语法断言读取。
fn log_argv(args: &[String]) {
    let Ok(path) = std::env::var("QUADCONTROL_FAKE_ARGV_LOG") else {
        return;
    };
    let mut line = args.join(" ");
    line.push('\n');
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(line.as_bytes()));
}

fn write_pid(name: &str) {
    if let Ok(dir) = std::env::var("QUADCONTROL_FAKE_PID_DIR") {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.pid")),
            std::process::id().to_string(),
        );
    }
}

/// 按环境变量忽略温和信号，用来构造「必须升级到下一级」的场景。
///
/// 同时忽略 SIGINT 与 SIGTERM 时，只有 SIGKILL 能收走这些进程——那是终止梯子的
/// 最坏路径，也是关闭预算最容易被突破的地方。
#[cfg(unix)]
fn maybe_ignore_signals() {
    if std::env::var("QUADCONTROL_FAKE_IGNORE_SIGINT").as_deref() == Ok("1") {
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_IGN);
        }
    }
    if std::env::var("QUADCONTROL_FAKE_IGNORE_SIGTERM").as_deref() == Ok("1") {
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_IGN);
        }
    }
}
#[cfg(not(unix))]
fn maybe_ignore_signals() {}

/// 长驻直到被信号打死。
fn run_forever(name: &str) -> ! {
    maybe_ignore_signals();
    write_pid(name);
    loop {
        thread::sleep(Duration::from_millis(50));
    }
}

/// `tunnel start`：绑上 `--tunnel-info-port` 并起一个最小隧道信息服务。
///
/// 真 go-ios 1.2.1 在这个端口上**立刻**开 HTTP 服务，但设备隧道要再过约 1.1 s
/// 才注册进去。会话的就绪判据是 `GET /tunnels` 里出现本机 UDID（见
/// `session::await_tunnel`），所以替身必须把这个时间差如实模拟出来：
/// `QUADCONTROL_FAKE_TUNNEL_DELAY_MS`（默认 300）之前返回 `[]`，之后返回含
/// `--udid=` 参数值的数组；`QUADCONTROL_FAKE_TUNNEL_NEVER=1` 则永远返回 `[]`。
/// 其它路径 404，和真 go-ios 的 `/` 一致。
fn run_tunnel(args: &[String]) -> ! {
    maybe_ignore_signals();
    let port = args
        .iter()
        .find_map(|a| a.strip_prefix("--tunnel-info-port="))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(0);
    let udid = args
        .iter()
        .find_map(|a| a.strip_prefix("--udid="))
        .unwrap_or("")
        .to_owned();
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("ios-test-helper: tunnel could not bind {port}: {error}");
            std::process::exit(1);
        }
    };
    // PID 在**绑定之后、延迟开始之前**落盘：测试拿它的 mtime 当隧道起动时刻 t₀，
    // 再和 runwda.pid 的 mtime 比，验证 runwda 确实等到了注册之后才起。
    write_pid("tunnel");

    let started = std::time::Instant::now();
    let never = std::env::var("QUADCONTROL_FAKE_TUNNEL_NEVER").as_deref() == Ok("1");
    let delay = Duration::from_millis(
        std::env::var("QUADCONTROL_FAKE_TUNNEL_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300),
    );

    for incoming in listener.incoming() {
        let Ok(stream) = incoming else { continue };
        let udid = udid.clone();
        thread::spawn(move || {
            let registered = !never && started.elapsed() >= delay;
            serve_tunnel_request(stream, &udid, registered);
        });
    }
    std::process::exit(0);
}

/// 处理一条隧道信息服务的连接：只认 `GET /tunnels`，其它一律 404。
///
/// 每条连接都带 `Connection: close` 并在写完后关掉：ureq 会复用连接池里的连接，
/// 若替身把一条已经写完的连接挂着不关，客户端下一轮轮询可能卡在一条死连接上。
fn serve_tunnel_request(mut stream: TcpStream, udid: &str, registered: bool) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(clone) => clone,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }
    // 读掉剩下的头，避免客户端还在写而我们已经关闭（RST）。
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line.trim().is_empty() => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let is_tunnels = request_line.starts_with("GET /tunnels");
    let body = if !is_tunnels {
        "404 page not found\n".to_owned()
    } else if registered {
        // 形状照 `tests/fixtures/go-ios-1.2.1/tunnels.stdout.json`。
        format!(
            r#"[{{"udid":"{udid}","rsdPort":50028,"address":"fdxx::1","userspaceTun":true,"userspaceTunPort":60106}}]"#
        )
    } else {
        "[]".to_owned()
    };
    let status = if is_tunnels {
        "200 OK"
    } else {
        "404 Not Found"
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

/// `runwda`：可按环境变量立刻以指定 stderr 退出，用来测关键字分类。
fn run_wda() -> ! {
    if let Ok(message) = std::env::var("QUADCONTROL_FAKE_RUNWDA_STDERR") {
        eprintln!("{message}");
        std::process::exit(1);
    }
    run_forever("runwda")
}

/// `forward <hostPort> <targetPort>`：真 TCP 转发。
///
/// 绑 `127.0.0.1:<hostPort>`，把每条连接双向管到
/// `QUADCONTROL_FAKE_FORWARD_<targetPort>` 指定的本机端口（测试里分别是假 WDA
/// HTTP 服务和合成 MJPEG 上游）。没配上游就只监听不转发——用来模拟「端口通了但
/// 后面没有服务」。
fn run_forward(host_port: &str, target_port: &str) -> ! {
    maybe_ignore_signals();
    write_pid(&format!("forward-{target_port}"));

    let listener = match TcpListener::bind(("127.0.0.1", host_port.parse::<u16>().unwrap_or(0))) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("ios-test-helper: could not bind {host_port}: {error}");
            std::process::exit(1);
        }
    };
    let upstream = std::env::var(format!("QUADCONTROL_FAKE_FORWARD_{target_port}"))
        .ok()
        .and_then(|port| port.parse::<u16>().ok());

    for incoming in listener.incoming() {
        let Ok(downstream) = incoming else { continue };
        let Some(upstream_port) = upstream else {
            let _ = downstream.shutdown(Shutdown::Both);
            continue;
        };
        thread::spawn(move || {
            let Ok(up) = TcpStream::connect(("127.0.0.1", upstream_port)) else {
                let _ = downstream.shutdown(Shutdown::Both);
                return;
            };
            // 两个方向各一个线程；任一方向结束就关掉整条连接。
            let pairs = [
                (downstream.try_clone(), up.try_clone()),
                (up.try_clone(), downstream.try_clone()),
            ];
            let mut joins = Vec::new();
            for (from, to) in pairs {
                if let (Ok(mut from), Ok(mut to)) = (from, to) {
                    joins.push(thread::spawn(move || {
                        let _ = std::io::copy(&mut from, &mut to);
                        let _ = to.shutdown(Shutdown::Both);
                    }));
                }
            }
            for join in joins {
                let _ = join.join();
            }
        });
    }
    std::process::exit(0);
}
