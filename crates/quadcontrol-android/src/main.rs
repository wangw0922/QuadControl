use quadcontrol_android::{
    devices, filtered_args, install_signal_handlers, resolve_adb, select_device, LaunchOptions,
    Session, HELP,
};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return Ok(());
    }
    let adb = take_value(&mut args, "--adb")?.map(PathBuf::from);
    let serial = take_value(&mut args, "--serial")?;
    let bit_rate = take_value(&mut args, "--bit-rate")?.unwrap_or_else(|| "8M".into());
    let screen_off = take_flag(&mut args, "--screen-off");
    // Reject bad passthrough before touching any device: an argument error should
    // not depend on what happens to be plugged in.
    filtered_args(&args)?;
    let adb = resolve_adb(adb.as_deref());
    let selected = select_device(&devices(&adb)?, serial.as_deref())?;
    let session = Session::start(&LaunchOptions {
        adb,
        scrcpy: PathBuf::from("scrcpy"),
        serial: selected,
        bit_rate,
        screen_off,
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
