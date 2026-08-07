//! Passive ADB capability discovery.
//!
//! The runner accepts only three fixed commands. It never pairs, connects, opens a
//! device shell, restarts the ADB server, or changes device state.

use std::ffi::OsString;
use std::fmt::{self, Write as _};
use std::io::Read;
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const OUTPUT_LIMIT: usize = 64 * 1024;
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AdbCommand {
    Version,
    DevicesLong,
    MdnsServices,
}

impl AdbCommand {
    pub const fn args(self) -> &'static [&'static str] {
        match self {
            Self::Version => &["version"],
            Self::DevicesLong => &["devices", "-l"],
            Self::MdnsServices => &["mdns", "services"],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    AdbNotFound,
    TimedOut(AdbCommand),
    Io(String),
    OutputTooLarge(AdbCommand),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdbNotFound => write!(f, "adb executable was not found"),
            Self::TimedOut(command) => write!(f, "adb {command:?} timed out"),
            Self::Io(error) => write!(f, "adb process error: {error}"),
            Self::OutputTooLarge(command) => {
                write!(
                    f,
                    "adb {command:?} exceeded the {OUTPUT_LIMIT}-byte output limit"
                )
            }
        }
    }
}

pub trait CommandRunner {
    fn run(&self, command: AdbCommand) -> Result<CommandOutput, ExecutionError>;
}

pub struct SystemRunner {
    adb_path: OsString,
    timeout: Duration,
    output_limit: usize,
}

impl SystemRunner {
    pub fn new(adb_path: impl Into<OsString>) -> Self {
        Self {
            adb_path: adb_path.into(),
            timeout: COMMAND_TIMEOUT,
            output_limit: OUTPUT_LIMIT,
        }
    }

    pub fn with_limits(
        adb_path: impl Into<OsString>,
        timeout: Duration,
        output_limit: usize,
    ) -> Self {
        Self {
            adb_path: adb_path.into(),
            timeout,
            output_limit,
        }
    }
}

struct CapturedOutput {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_limited<R: Read + Send + 'static>(
    mut reader: R,
    limit: usize,
) -> thread::JoinHandle<Result<CapturedOutput, std::io::Error>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut truncated = false;
        let mut chunk = [0; 4096];

        loop {
            let count = reader.read(&mut chunk)?;
            if count == 0 {
                return Ok(CapturedOutput { bytes, truncated });
            }

            let remaining = limit.saturating_sub(bytes.len());
            let stored = remaining.min(count);
            bytes.extend_from_slice(&chunk[..stored]);
            truncated |= stored < count;
        }
    })
}

fn join_capture(
    handle: thread::JoinHandle<Result<CapturedOutput, std::io::Error>>,
    stream: &str,
) -> Result<CapturedOutput, ExecutionError> {
    handle
        .join()
        .map_err(|_| ExecutionError::Io(format!("{stream} reader panicked")))?
        .map_err(|error| ExecutionError::Io(format!("failed to read {stream}: {error}")))
}

impl CommandRunner for SystemRunner {
    fn run(&self, command: AdbCommand) -> Result<CommandOutput, ExecutionError> {
        let mut child = ProcessCommand::new(&self.adb_path)
            .args(command.args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    ExecutionError::AdbNotFound
                } else {
                    ExecutionError::Io(error.to_string())
                }
            })?;

        let stdout_reader = read_limited(
            child.stdout.take().expect("stdout was configured as piped"),
            self.output_limit,
        );
        let stderr_reader = read_limited(
            child.stderr.take().expect("stderr was configured as piped"),
            self.output_limit,
        );
        let started = Instant::now();

        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(ExecutionError::TimedOut(command));
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(ExecutionError::Io(error.to_string()));
                }
            }
        };

        let stdout = join_capture(stdout_reader, "stdout")?;
        let stderr = join_capture(stderr_reader, "stderr")?;
        if stdout.truncated || stderr.truncated {
            return Err(ExecutionError::OutputTooLarge(command));
        }

        Ok(CommandOutput {
            status: status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&stdout.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&stderr.bytes).into_owned(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
    Other(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    Usb,
    Tcp,
    Emulator,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Device {
    pub serial: String,
    pub state: DeviceState,
    pub transport: Transport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingObservation {
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MdnsAvailability {
    Available,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MdnsReport {
    pub availability: MdnsAvailability,
    pub pairing_service_advertised: bool,
    pub connect_service_advertised: bool,
    pub malformed_lines: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeReport {
    pub adb_version: String,
    pub devices: Vec<Device>,
    pub mdns: MdnsReport,
    pub wireless_connected: bool,
    pub paired_with_this_host: PairingObservation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Execution(ExecutionError),
    CommandFailed {
        command: AdbCommand,
        status: i32,
        stderr: String,
    },
    InvalidOutput {
        command: AdbCommand,
        reason: String,
    },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Execution(error) => write!(f, "{error}"),
            Self::CommandFailed {
                command,
                status,
                stderr,
            } => write!(
                f,
                "adb {command:?} failed with exit {status}: {}",
                stderr.trim()
            ),
            Self::InvalidOutput { command, reason } => {
                write!(f, "adb {command:?} returned invalid output: {reason}")
            }
        }
    }
}

pub fn parse_version(stdout: &str) -> Result<String, ProbeError> {
    stdout
        .lines()
        .find_map(|line| {
            line.strip_prefix("Android Debug Bridge version ")
                .map(str::trim)
                .filter(|version| !version.is_empty())
                .map(str::to_owned)
        })
        .ok_or_else(|| ProbeError::InvalidOutput {
            command: AdbCommand::Version,
            reason: "version line is missing".to_owned(),
        })
}

fn is_network_serial(serial: &str) -> bool {
    serial.contains("_adb-tls-connect._tcp")
        || serial
            .rsplit_once(':')
            .is_some_and(|(host, port)| !host.is_empty() && port.parse::<u16>().is_ok())
}

pub fn parse_devices(stdout: &str) -> Result<Vec<Device>, ProbeError> {
    let mut lines = stdout.lines();
    let header_found = lines.any(|line| line.trim() == "List of devices attached");
    if !header_found {
        return Err(ProbeError::InvalidOutput {
            command: AdbCommand::DevicesLong,
            reason: "device-list header is missing".to_owned(),
        });
    }

    Ok(lines
        .filter_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() < 2 {
                return None;
            }

            let serial = fields[0].to_owned();
            let state = match fields[1] {
                "device" => DeviceState::Device,
                "offline" => DeviceState::Offline,
                "unauthorized" => DeviceState::Unauthorized,
                other => DeviceState::Other(other.to_owned()),
            };
            let transport = if serial.starts_with("emulator-") {
                Transport::Emulator
            } else if is_network_serial(&serial) {
                Transport::Tcp
            } else if fields[2..].iter().any(|field| field.starts_with("usb:")) {
                Transport::Usb
            } else {
                Transport::Unknown
            };

            Some(Device {
                serial,
                state,
                transport,
            })
        })
        .collect())
}

pub fn parse_mdns(stdout: &str) -> MdnsReport {
    let mut pairing_service_advertised = false;
    let mut connect_service_advertised = false;
    let mut malformed_lines = 0;

    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line == "List of discovered mdns services" {
            continue;
        }

        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 3 {
            malformed_lines += 1;
            continue;
        }

        match fields[1].trim_end_matches('.') {
            "_adb-tls-pairing._tcp" => pairing_service_advertised = true,
            "_adb-tls-connect._tcp" => connect_service_advertised = true,
            _ => malformed_lines += 1,
        }
    }

    MdnsReport {
        availability: MdnsAvailability::Available,
        pairing_service_advertised,
        connect_service_advertised,
        malformed_lines,
    }
}

fn command_failure(command: AdbCommand, output: CommandOutput) -> ProbeError {
    ProbeError::CommandFailed {
        command,
        status: output.status,
        stderr: output.stderr,
    }
}

fn mdns_is_unsupported(output: &CommandOutput) -> bool {
    let message = format!("{}\n{}", output.stdout, output.stderr).to_ascii_lowercase();
    message.contains("unknown command") || message.contains("not supported")
}

pub fn run_probe(runner: &dyn CommandRunner) -> Result<ProbeReport, ProbeError> {
    let version_output = runner
        .run(AdbCommand::Version)
        .map_err(ProbeError::Execution)?;
    if version_output.status != 0 {
        return Err(command_failure(AdbCommand::Version, version_output));
    }
    let adb_version = parse_version(&version_output.stdout)?;

    let devices_output = runner
        .run(AdbCommand::DevicesLong)
        .map_err(ProbeError::Execution)?;
    if devices_output.status != 0 {
        return Err(command_failure(AdbCommand::DevicesLong, devices_output));
    }
    let devices = parse_devices(&devices_output.stdout)?;

    let mdns_output = runner
        .run(AdbCommand::MdnsServices)
        .map_err(ProbeError::Execution)?;
    let mdns = if mdns_output.status == 0 {
        parse_mdns(&mdns_output.stdout)
    } else if mdns_is_unsupported(&mdns_output) {
        MdnsReport {
            availability: MdnsAvailability::Unsupported,
            pairing_service_advertised: false,
            connect_service_advertised: false,
            malformed_lines: 0,
        }
    } else {
        return Err(command_failure(AdbCommand::MdnsServices, mdns_output));
    };

    let wireless_connected = devices
        .iter()
        .any(|device| device.transport == Transport::Tcp && device.state == DeviceState::Device);

    Ok(ProbeReport {
        adb_version,
        devices,
        mdns,
        wireless_connected,
        paired_with_this_host: PairingObservation::Unknown,
    })
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            control if control <= '\u{1f}' => {
                write!(&mut escaped, "\\u{:04x}", control as u32)
                    .expect("writing to a String cannot fail");
            }
            other => escaped.push(other),
        }
    }
    escaped
}

fn device_state_text(state: &DeviceState) -> &str {
    match state {
        DeviceState::Device => "device",
        DeviceState::Offline => "offline",
        DeviceState::Unauthorized => "unauthorized",
        DeviceState::Other(value) => value,
    }
}

fn transport_text(transport: Transport) -> &'static str {
    match transport {
        Transport::Usb => "usb",
        Transport::Tcp => "tcp",
        Transport::Emulator => "emulator",
        Transport::Unknown => "unknown",
    }
}

pub fn report_json(report: &ProbeReport) -> String {
    let devices = report
        .devices
        .iter()
        .map(|device| {
            format!(
                "{{\"serial\":\"{}\",\"state\":\"{}\",\"transport\":\"{}\"}}",
                json_escape(&device.serial),
                json_escape(device_state_text(&device.state)),
                transport_text(device.transport)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mdns_availability = match report.mdns.availability {
        MdnsAvailability::Available => "available",
        MdnsAvailability::Unsupported => "unsupported",
    };

    format!(
        concat!(
            "{{\"adb_version\":\"{}\",\"devices\":[{}],",
            "\"mdns\":{{\"availability\":\"{}\",",
            "\"pairing_service_advertised\":{},",
            "\"connect_service_advertised\":{},",
            "\"malformed_lines\":{}}},",
            "\"wireless_connected\":{},",
            "\"paired_with_this_host\":\"unknown\"}}"
        ),
        json_escape(&report.adb_version),
        devices,
        mdns_availability,
        report.mdns.pairing_service_advertised,
        report.mdns.connect_service_advertised,
        report.mdns.malformed_lines,
        report.wireless_connected,
    )
}

pub fn exit_code_for(result: &Result<ProbeReport, ProbeError>) -> i32 {
    if result.is_ok() {
        0
    } else {
        1
    }
}

pub fn cli_args_are_valid(args: &[String]) -> bool {
    match args {
        [] => true,
        [flag, value] => flag == "--format" && value == "json",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io::Cursor;

    struct FakeRunner {
        responses: RefCell<VecDeque<(AdbCommand, Result<CommandOutput, ExecutionError>)>>,
        calls: RefCell<Vec<AdbCommand>>,
    }

    impl FakeRunner {
        fn new(responses: Vec<(AdbCommand, Result<CommandOutput, ExecutionError>)>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, command: AdbCommand) -> Result<CommandOutput, ExecutionError> {
            self.calls.borrow_mut().push(command);
            let (expected, result) = self
                .responses
                .borrow_mut()
                .pop_front()
                .expect("unexpected command");
            assert_eq!(command, expected);
            result
        }
    }

    fn ok(stdout: &str) -> Result<CommandOutput, ExecutionError> {
        Ok(CommandOutput {
            status: 0,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    fn success_runner(devices: &str, mdns: &str) -> FakeRunner {
        FakeRunner::new(vec![
            (
                AdbCommand::Version,
                ok("Android Debug Bridge version 1.0.41\nVersion 35.0.2\n"),
            ),
            (AdbCommand::DevicesLong, ok(devices)),
            (AdbCommand::MdnsServices, ok(mdns)),
        ])
    }

    #[test]
    fn commands_are_an_exact_read_only_whitelist() {
        assert_eq!(AdbCommand::Version.args(), ["version"]);
        assert_eq!(AdbCommand::DevicesLong.args(), ["devices", "-l"]);
        assert_eq!(AdbCommand::MdnsServices.args(), ["mdns", "services"]);
    }

    #[test]
    fn bounded_reader_drains_but_flags_oversized_output() {
        let capture = read_limited(Cursor::new(vec![b'x'; 5]), 4)
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(capture.bytes, b"xxxx");
        assert!(capture.truncated);
    }

    #[test]
    fn parses_version_and_rejects_empty_output() {
        assert_eq!(
            parse_version("Android Debug Bridge version 1.0.41\n").unwrap(),
            "1.0.41"
        );
        assert!(matches!(
            parse_version(""),
            Err(ProbeError::InvalidOutput {
                command: AdbCommand::Version,
                ..
            })
        ));
    }

    #[test]
    fn parses_states_and_explicit_transport_evidence() {
        let devices = parse_devices(concat!(
            "List of devices attached\n",
            "USB123 device usb:1-2 product:pixel transport_id:1\n",
            "10.0.0.2:5555 offline product:pixel transport_id:2\n",
            "emulator-5554 unauthorized transport_id:3\n",
            "adb-ABC._adb-tls-connect._tcp device product:pixel\n",
            "SERIAL recovery product:pixel\n",
        ))
        .unwrap();

        assert_eq!(devices[0].transport, Transport::Usb);
        assert_eq!(devices[0].state, DeviceState::Device);
        assert_eq!(devices[1].transport, Transport::Tcp);
        assert_eq!(devices[1].state, DeviceState::Offline);
        assert_eq!(devices[2].transport, Transport::Emulator);
        assert_eq!(devices[2].state, DeviceState::Unauthorized);
        assert_eq!(devices[3].transport, Transport::Tcp);
        assert_eq!(devices[4].transport, Transport::Unknown);
        assert_eq!(devices[4].state, DeviceState::Other("recovery".to_owned()));
    }

    /// Captured verbatim from a Samsung Galaxy (SM_S9180) over USB with
    /// platform-tools 37.0.1, except that the serial is redacted — this project does
    /// not persist device identifiers. Real output carries `model:` and `device:`
    /// fields the hand-written fixtures never had, and `device:dm3q` collides with the
    /// `device` state keyword. Parsing is positional, so it survives; this test keeps
    /// it that way.
    #[test]
    fn parses_real_hardware_device_line() {
        let devices = parse_devices(concat!(
            "List of devices attached\n",
            "REDACTEDSERIAL         device usb:1048576X product:dm3qzcx ",
            "model:SM_S9180 device:dm3q transport_id:1\n",
        ))
        .unwrap();

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].state, DeviceState::Device);
        assert_eq!(devices[0].transport, Transport::Usb);
        assert_eq!(devices[0].serial, "REDACTEDSERIAL");
    }

    /// The same handset before the user approved the USB-debugging prompt. Real
    /// unauthorized lines still carry `usb:` evidence but none of the product fields.
    #[test]
    fn parses_real_hardware_unauthorized_line() {
        let devices = parse_devices(concat!(
            "List of devices attached\n",
            "REDACTEDSERIAL         unauthorized usb:1048576X transport_id:1\n",
        ))
        .unwrap();

        assert_eq!(devices[0].state, DeviceState::Unauthorized);
        assert_eq!(devices[0].transport, Transport::Usb);
    }

    /// Real `adb version` output is four lines. Only the first carries the bridge
    /// version; the platform-tools build number on line two is deliberately not
    /// reported.
    #[test]
    fn parses_real_hardware_version_output() {
        let version = parse_version(concat!(
            "Android Debug Bridge version 1.0.41\n",
            "Version 37.0.1-15733141\n",
            "Installed as /opt/homebrew/bin/adb\n",
            "Running on Darwin 25.5.0 (arm64)\n",
        ))
        .unwrap();

        assert_eq!(version, "1.0.41");
    }

    #[test]
    fn rejects_empty_or_unframed_device_output() {
        for output in ["", "USB123 device usb:1-2\n"] {
            assert!(matches!(
                parse_devices(output),
                Err(ProbeError::InvalidOutput {
                    command: AdbCommand::DevicesLong,
                    ..
                })
            ));
        }
    }

    #[test]
    fn mdns_distinguishes_services_duplicates_headers_and_malformed_lines() {
        let report = parse_mdns(concat!(
            "List of discovered mdns services\n",
            "phone _adb-tls-pairing._tcp. 192.0.2.1:37001\n",
            "phone _adb-tls-pairing._tcp. 192.0.2.1:37001\n",
            "phone _adb-tls-connect._tcp. 192.0.2.1:37002\n",
            "bad _adb-tls-connect._tcp.\n",
            "garbage\n",
        ));

        assert!(report.pairing_service_advertised);
        assert!(report.connect_service_advertised);
        assert_eq!(report.malformed_lines, 2);
    }

    #[test]
    fn empty_mdns_output_is_available_without_advertisements() {
        let report = parse_mdns("");
        assert_eq!(report.availability, MdnsAvailability::Available);
        assert!(!report.pairing_service_advertised);
        assert!(!report.connect_service_advertised);
    }

    #[test]
    fn wireless_connection_is_derived_from_devices_not_mdns() {
        let disconnected = success_runner(
            "List of devices attached\n",
            "phone _adb-tls-connect._tcp. 192.0.2.1:37002\n",
        );
        let report = run_probe(&disconnected).unwrap();
        assert!(report.mdns.connect_service_advertised);
        assert!(!report.wireless_connected);

        let connected = success_runner(
            "List of devices attached\n192.0.2.1:5555 device product:pixel\n",
            "",
        );
        let report = run_probe(&connected).unwrap();
        assert!(!report.mdns.connect_service_advertised);
        assert!(report.wireless_connected);
    }

    #[test]
    fn successful_probe_runs_exact_sequence_and_keeps_pairing_unknown() {
        let runner = success_runner("List of devices attached\n", "");
        let report = run_probe(&runner).unwrap();
        assert_eq!(report.paired_with_this_host, PairingObservation::Unknown);
        assert_eq!(
            *runner.calls.borrow(),
            [
                AdbCommand::Version,
                AdbCommand::DevicesLong,
                AdbCommand::MdnsServices,
            ]
        );
    }

    #[test]
    fn unsupported_mdns_is_partial_success() {
        let runner = FakeRunner::new(vec![
            (
                AdbCommand::Version,
                ok("Android Debug Bridge version 1.0.41\n"),
            ),
            (AdbCommand::DevicesLong, ok("List of devices attached\n")),
            (
                AdbCommand::MdnsServices,
                Ok(CommandOutput {
                    status: 1,
                    stdout: String::new(),
                    stderr: "adb: unknown command mdns".to_owned(),
                }),
            ),
        ]);

        assert_eq!(
            run_probe(&runner).unwrap().mdns.availability,
            MdnsAvailability::Unsupported
        );
    }

    #[test]
    fn missing_adb_and_timeout_remain_clear_execution_errors() {
        for error in [
            ExecutionError::AdbNotFound,
            ExecutionError::TimedOut(AdbCommand::Version),
        ] {
            let runner = FakeRunner::new(vec![(AdbCommand::Version, Err(error.clone()))]);
            assert_eq!(run_probe(&runner), Err(ProbeError::Execution(error)));
        }
    }

    #[test]
    fn failed_core_command_preserves_status_and_stderr() {
        let runner = FakeRunner::new(vec![(
            AdbCommand::Version,
            Ok(CommandOutput {
                status: 2,
                stdout: String::new(),
                stderr: "bad adb".to_owned(),
            }),
        )]);

        assert_eq!(
            run_probe(&runner),
            Err(ProbeError::CommandFailed {
                command: AdbCommand::Version,
                status: 2,
                stderr: "bad adb".to_owned(),
            })
        );
    }

    #[test]
    fn report_json_uses_stable_fields_and_valid_control_escapes() {
        let report = ProbeReport {
            adb_version: "1.0.41\npreview".to_owned(),
            devices: vec![Device {
                serial: "quoted\"\\\u{0001}".to_owned(),
                state: DeviceState::Other("recovery\tmode".to_owned()),
                transport: Transport::Unknown,
            }],
            mdns: MdnsReport {
                availability: MdnsAvailability::Available,
                pairing_service_advertised: true,
                connect_service_advertised: false,
                malformed_lines: 0,
            },
            wireless_connected: false,
            paired_with_this_host: PairingObservation::Unknown,
        };
        let json = report_json(&report);

        assert!(json.starts_with('{') && json.ends_with('}'));
        assert!(json.contains("\"adb_version\":\"1.0.41\\npreview\""));
        assert!(json.contains("quoted\\\"\\\\\\u0001"));
        assert!(json.contains("recovery\\tmode"));
        assert!(json.contains("\"paired_with_this_host\":\"unknown\""));
    }

    #[test]
    fn result_maps_to_cli_exit_code() {
        let ok = run_probe(&success_runner("List of devices attached\n", ""));
        assert_eq!(exit_code_for(&ok), 0);
        let error = Err(ProbeError::Execution(ExecutionError::AdbNotFound));
        assert_eq!(exit_code_for(&error), 1);
    }

    #[test]
    fn validates_the_complete_cli_argument_list() {
        assert!(cli_args_are_valid(&[]));
        assert!(cli_args_are_valid(&[
            "--format".to_owned(),
            "json".to_owned(),
        ]));
        assert!(!cli_args_are_valid(&["--format".to_owned()]));
        assert!(!cli_args_are_valid(&[
            "--format".to_owned(),
            "json".to_owned(),
            "extra".to_owned(),
        ]));
    }
}
