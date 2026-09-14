use quadcontrol_android::wireless;
use quadcontrol_android::{
    devices, filtered_args, install_signal_handlers, resolve_adb, resolve_scrcpy, select_device,
    LaunchOptions, Session, HELP,
};
use std::io::BufRead;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Some(command) = args.first().cloned() {
        match command.as_str() {
            "pair-qr" => return pair_qr(&args[1..]),
            "pair-code" => return pair_code(&args[1..]),
            "connect" => return connect_command(&args[1..]),
            _ => {}
        }
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }
    let adb = take_value(&mut args, "--adb")?.map(PathBuf::from);
    let serial = take_value(&mut args, "--serial")?;
    let bit_rate = take_value(&mut args, "--bit-rate")?.unwrap_or_else(|| "8M".into());
    let screen_off = take_flag(&mut args, "--screen-off");
    let audio_on_computer = !take_flag(&mut args, "--no-audio");
    // Reject bad passthrough before touching any device: an argument error should
    // not depend on what happens to be plugged in.
    filtered_args(&args)?;
    let adb = resolve_adb(adb.as_deref());
    let selected = select_device(&devices(&adb)?, serial.as_deref())?;
    let session = Session::start(&LaunchOptions {
        adb,
        scrcpy: resolve_scrcpy(None),
        serial: selected,
        bit_rate,
        screen_off,
        audio_on_computer,
        passthrough: args,
        scrcpy_env: Vec::new(),
    })?;
    install_signal_handlers();
    let (code, stderr) = session.wait_or_stop_on_signal_with_stderr()?;
    if code != 0 {
        if !stderr.trim().is_empty() {
            eprintln!("scrcpy failed (stderr tail):\n{}", stderr.trim_end());
        }
        eprintln!("Check device authorization and connection ownership; see https://github.com/Genymobile/scrcpy/releases.");
    }

    std::process::exit(code)
}

fn adb_path(args: &[String]) -> PathBuf {
    args.iter()
        .position(|a| a == "--adb")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_adb(None))
}
fn endpoint(args: &[String]) -> Result<&str, Box<dyn std::error::Error>> {
    args.first()
        .map(String::as_str)
        .ok_or_else(|| "需要 ip:port 参数".into())
}
fn warn_if_below_baseline(precheck: &wireless::AdbPrecheck) {
    if precheck.below_tested_baseline {
        let (major, minor, patch) = precheck.version;
        let (bmajor, bminor, bpatch) = wireless::TESTED_BASELINE;
        eprintln!(
            "提示:adb {major}.{minor}.{patch} 低于本项目实测基线 {bmajor}.{bminor}.{bpatch},\n\
             功能应可用但未在该版本上实测过 / adb below tested baseline."
        );
    }
}

fn pair_qr(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let adb = adb_path(args);
    // 显式恢复入口:只有用户主动加这个参数才会重启 adb server。
    if args.iter().any(|a| a == "--restart-adb-server") {
        eprintln!("正在重启 adb server(会短暂断开本机其他 adb 会话)…");
        wireless::restart_adb_server(&adb)?;
    }
    warn_if_below_baseline(&wireless::check_adb(&adb)?);
    let (serial, visible, _reason) = wireless::pair_qr_command(&adb, |qr| {
        println!("{qr}");
        println!("{}", wireless::pairing_instructions());
        println!("等待手机扫码配对… / Waiting for the phone to pair…");
    })?;
    println!("配对并连接成功 / Paired and connected: {serial}");
    println!("当前可见设备 / Visible devices: {visible}");
    Ok(())
}

fn pair_code(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = endpoint(args)?;
    if endpoint
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok())
        .is_none()
    {
        return Err("endpoint 必须是 ip:port".into());
    }
    let adb = adb_path(args);
    // pair-code 是 mDNS 不可用时的降级路径，这里容忍该错误。
    match wireless::check_adb(&adb) {
        Ok(precheck) => warn_if_below_baseline(&precheck),
        Err(wireless::WirelessError::MdnsUnavailable) => {}
        Err(error) => return Err(error.into()),
    }
    print!("请输入手机显示的 6 位配对码 / Enter the 6-digit pairing code: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut code = String::new();
    std::io::stdin().lock().read_line(&mut code)?;
    let trimmed = code.trim().to_owned();
    // 输入缓冲与副本都要擦，和 QR 路径同一条纪律。
    wireless::wipe_secret_string(&mut code);
    if trimmed.len() != 6 || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        let mut trimmed = trimmed;
        wireless::wipe_secret_string(&mut trimmed);
        return Err("配对码必须是 6 位数字".into());
    }
    let result = wireless::pair(&adb, endpoint, &trimmed);
    let mut trimmed = trimmed;
    wireless::wipe_secret_string(&mut trimmed);
    result?;
    println!("配对成功 / Pairing succeeded");
    Ok(())
}
fn connect_command(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = endpoint(args)?;
    let adb = adb_path(args);
    wireless::connect(&adb, endpoint)?;
    println!("连接成功 / Connected: {endpoint}");
    Ok(())
}
fn take_value(
    args: &mut Vec<String>,
    flag: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    if let Some(i) = args.iter().position(|a| a == flag) {
        if i + 1 >= args.len() {
            return Err(format!("{flag} requires a value").into());
        }
        let v = args.remove(i + 1);
        args.remove(i);
        return Ok(Some(v));
    }
    Ok(None)
}
fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(i) = args.iter().position(|a| a == flag) {
        args.remove(i);
        true
    } else {
        false
    }
}
