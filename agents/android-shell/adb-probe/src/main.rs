use adb_probe::{cli_args_are_valid, exit_code_for, report_json, run_probe, SystemRunner};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !cli_args_are_valid(&args) {
        eprintln!("usage: quadcontrol-adb-probe [--format json]");
        std::process::exit(64);
    }

    let result = run_probe(&SystemRunner::new("adb"));
    match &result {
        Ok(report) => println!("{}", report_json(report)),
        Err(error) => eprintln!("adb probe failed: {error}"),
    }
    std::process::exit(exit_code_for(&result));
}
