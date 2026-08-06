import Foundation

/// M0 capability probe for the "macOS Bluetooth HID" milestone item.
///
/// It answers what can be answered by reading: does this host have a Bluetooth
/// controller, does that controller carry the HID profile, and do the APIs a HID
/// peripheral would need exist. It deliberately does **not** answer whether macOS will
/// actually let a process present itself as a keyboard — that requires publishing an
/// SDP record and having a real device pair, which changes system Bluetooth state and
/// needs a person to accept a pairing prompt. That stays an explicit `unknown`.
public enum HIDProbe {
    public static let peripheralRoleNote = """
    Read-only probing cannot decide this. Confirming it requires publishing a HID SDP \
    record, accepting L2CAP on PSM 0x11/0x13, and having a real device pair — an active \
    experiment that changes system Bluetooth state and needs a person to accept the \
    pairing prompt on the other device.
    """

    public static func run(
        runner: ProfileRunner = SystemProfileRunner(),
        osVersion: String = ProcessInfo.processInfo.operatingSystemVersionString
    ) -> Result<HIDProbeReport, HIDProbeError> {
        let controller: ControllerReport?
        do {
            controller = try ControllerParser.parse(try runner.run())
        } catch let error as HIDProbeError {
            return .failure(error)
        } catch {
            return .failure(.profileOutputMalformed)
        }

        let frameworkLoaded = PeripheralRoleAPIs.loadFramework()
        var notes = [
            "macOS normally acts as a HID host (it accepts keyboards and mice). Presenting the Mac to a phone as a keyboard is the opposite role.",
            "A controller that lists HID supports the profile; it does not mean this process may take the peripheral role.",
            "No Bluetooth state was changed: no SDP record was published, no channel opened, no device paired. Only the Objective-C runtime was inspected.",
            "The controller's Bluetooth address is intentionally omitted; it is a stable hardware identifier this diagnostic does not need.",
        ]
        if controller == nil {
            notes.append("No Bluetooth controller was reported by system_profiler on this host.")
        }

        return .success(
            HIDProbeReport(
                macosVersion: osVersion,
                bluetoothFrameworkLoaded: frameworkLoaded ? .yes : .no,
                controller: controller,
                peripheralRoleAPIs: PeripheralRoleAPIs.report(frameworkLoaded: frameworkLoaded),
                canActAsHIDPeripheral: .unknown,
                canActAsHIDPeripheralNote: peripheralRoleNote,
                notes: notes
            )
        )
    }

    /// `0` report produced, `1` probe failed, `64` bad usage — same convention as the
    /// ADB probe CLI.
    public static func exitCode(for result: Result<HIDProbeReport, HIDProbeError>) -> Int32 {
        switch result {
        case .success: return 0
        case .failure: return 1
        }
    }

    public static func argumentsAreValid(_ arguments: [String]) -> Bool {
        arguments.isEmpty || arguments == ["--format", "json"]
    }
}
