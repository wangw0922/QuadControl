use std::io::Write;
use std::thread;
use std::time::Duration;

fn main() {
    let mode = std::env::var("QUADCONTROL_FAKE_MODE").unwrap_or_default();
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
