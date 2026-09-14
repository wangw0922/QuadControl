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
//! - `QUADCONTROL_FAKE_RUNWDA_STDERR`：`runwda` 立刻把它打到 stderr 并退出。
//! - `QUADCONTROL_FAKE_FORWARD_<targetPort>`：该目标端口要转发到的本机上游端口。

use std::io::Write;
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
        ["tunnel", "start", ..] => run_forever("tunnel"),
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

/// 按环境变量忽略 SIGINT，用来构造「温和信号无效、必须升级」的场景。
#[cfg(unix)]
fn maybe_ignore_sigint() {
    if std::env::var("QUADCONTROL_FAKE_IGNORE_SIGINT").as_deref() == Ok("1") {
        unsafe {
            libc::signal(libc::SIGINT, libc::SIG_IGN);
        }
    }
}
#[cfg(not(unix))]
fn maybe_ignore_sigint() {}

/// 长驻直到被信号打死。
fn run_forever(name: &str) -> ! {
    maybe_ignore_sigint();
    write_pid(name);
    loop {
        thread::sleep(Duration::from_millis(50));
    }
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
    maybe_ignore_sigint();
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
