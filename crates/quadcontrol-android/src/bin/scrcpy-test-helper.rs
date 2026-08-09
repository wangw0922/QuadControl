use std::io::Write;
use std::thread;
use std::time::Duration;

fn main() {
    let mode = std::env::var("QUADCONTROL_FAKE_MODE").unwrap_or_default();
    if mode.starts_with("pair-") && fake_adb_pairing(&mode) {
        return;
    }
    if std::env::args().any(|a| a == "--version") {
        match mode.as_str() {
            "prerelease" => println!("scrcpy 4.1-rc1"),
            "invalid" => println!("not a version"),
            "old" => println!("scrcpy 3.3"),
            "version-fail" => {
                eprintln!("version command failed");
                std::process::exit(9);
            }
            _ => println!("scrcpy 4.1\nrevision: test"),
        };
        return;
    }
    if mode == "adb-ready" {
        println!("List of devices attached\nREDACTEDSERIAL device product:test");
        return;
    }
    if mode == "adb-unauthorized" {
        println!("List of devices attached\nREDACTEDSERIAL unauthorized product:test");
        return;
    }
    if mode == "stderr-flood" {
        for _ in 0..200_000 {
            eprintln!("stderr-flood");
        }
    }
    if let Ok(path) = std::env::var("QUADCONTROL_FAKE_PID") {
        std::fs::write(path, std::process::id().to_string()).unwrap();
    }
    if mode == "wait" {
        loop {
            thread::sleep(Duration::from_millis(100));
        }
    }
    if let Some(code) = mode
        .strip_prefix("exit-")
        .and_then(|code| code.parse::<i32>().ok())
    {
        std::process::exit(code);
    }
    std::io::stderr().flush().unwrap();
}

/// 假 adb：驱动 `pair-qr` 的主流程测试。
/// 只有它能暴露"状态机定义了却没接进主流程"这类接线错误——纯单测做不到。
/// 返回 true 表示本次调用已被当作 adb 处理完毕。
fn fake_adb_pairing(mode: &str) -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sub = args.first().map(String::as_str).unwrap_or("");
    match sub {
        "version" => {
            println!("Android Debug Bridge version 1.0.41\nVersion 37.0.1-13206524");
            true
        }
        "mdns" if args.get(1).map(String::as_str) == Some("check") => {
            println!("mdns daemon version [openscreen discovery]");
            true
        }
        "mdns" if mode == "pair-service-lost" => {
            // 有状态：调用次数决定返回什么，用来构造"服务消失后 mDNS 再失败"。
            let counter = std::env::var("QUADCONTROL_FAKE_COUNTER").unwrap_or_default();
            let n = std::fs::read_to_string(&counter)
                .ok()
                .and_then(|v| v.trim().parse::<u32>().ok())
                .unwrap_or(0);
            let _ = std::fs::write(&counter, (n + 1).to_string());
            let name = std::env::var("QUADCONTROL_FAKE_SERVICE").unwrap_or_default();
            match n {
                0 => {
                    println!("List of discovered mdns services");
                    println!("{name}._adb-tls-pairing._tcp. 127.0.0.1:41002");
                    true
                }
                1 => {
                    // 服务消失：pair 仍在飞 → service-lost 待决
                    println!("List of discovered mdns services");
                    true
                }
                _ => {
                    eprintln!("mdns query failed");
                    std::process::exit(1);
                }
            }
        }
        "mdns" => {
            // 同名竞争：两台手机以同一 instance name 广播不同端点。
            let name = std::env::var("QUADCONTROL_FAKE_SERVICE").unwrap_or_default();
            println!("List of discovered mdns services");
            if mode == "pair-competition" || mode == "pair-success" {
                println!("{name}._adb-tls-pairing._tcp. 127.0.0.1:37145");
            }
            if mode == "pair-competition" {
                println!("{name}._adb-tls-pairing._tcp. 127.0.0.1:41002");
            }
            println!("adb-FAKEGUID-0001._adb-tls-connect._tcp. 127.0.0.1:5555");
            true
        }
        "pair" => {
            // 记录本次 pair 的端点，供测试断言"每个端点都各起了一个进程"。
            if let Ok(path) = std::env::var("QUADCONTROL_FAKE_PAIR_LOG") {
                let endpoint = args.get(1).cloned().unwrap_or_default();
                // 连 PID 一起记：测试要用它证明败者确实被 round-close 回收，
                // 而不是仅仅从 Vec 里被移除。
                let mut line = format!("{endpoint} {}", std::process::id());
                line.push('\n');
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut file| file.write_all(line.as_bytes()));
            }
            if args.get(1).map(String::as_str) == Some("127.0.0.1:41002") {
                if mode == "pair-service-lost" {
                    // 活过随后的 mDNS 失败再自行退出，并留下"我不是被杀的"证据。
                    // 若实现提前杀掉在飞 pair（规范禁止），SIGKILL 下这个标记写不出来。
                    thread::sleep(Duration::from_millis(1500));
                    if let Ok(path) = std::env::var("QUADCONTROL_FAKE_SURVIVED") {
                        let _ = std::fs::write(path, "survived");
                    }
                    eprintln!("pair failed after service disappeared");
                    std::process::exit(1);
                }
                // 竞争场景：永不返回，用来验证胜出后它会被 kill+reap。
                loop {
                    thread::sleep(Duration::from_millis(50));
                }
            }
            println!("Successfully paired to 127.0.0.1:37145 [guid=adb-FAKEGUID-0001]");
            true
        }
        "devices" => {
            println!("List of devices attached\nadb-FAKEGUID-0001 device product:test");
            true
        }
        "connect" => {
            println!("connected to 127.0.0.1:5555");
            true
        }
        "-s" => {
            // getprop ro.serialno
            println!("REDACTEDSERIAL");
            true
        }
        _ => false,
    }
}
