import Foundation

/// Whether a fact could be observed. Mirrors the ADB probe's refusal to turn "not
/// observable" into a boolean.
public enum Observation: String, Codable, Equatable, Sendable {
    case yes
    case no
    case unknown
}

/// Read-only facts about the local Bluetooth controller.
///
/// The controller's Bluetooth address is deliberately not reported. It is a stable
/// hardware identifier and this probe follows the same rule as the ADB probe: a
/// diagnostic does not persist or emit device identifiers it does not need.
public struct ControllerReport: Codable, Equatable, Sendable {
    public let powered: Observation
    public let discoverable: Observation
    public let chipset: String?
    public let transport: String?
    public let supportedServices: [String]
    /// The controller advertises the HID profile. This says the radio supports HID —
    /// it does not say macOS will let this process act as a HID peripheral.
    public let hidProfileSupported: Observation

    public init(
        powered: Observation,
        discoverable: Observation,
        chipset: String?,
        transport: String?,
        supportedServices: [String],
        hidProfileSupported: Observation
    ) {
        self.powered = powered
        self.discoverable = discoverable
        self.chipset = chipset
        self.transport = transport
        self.supportedServices = supportedServices
        self.hidProfileSupported = hidProfileSupported
    }

    private enum CodingKeys: String, CodingKey {
        case powered
        case discoverable
        case chipset
        case transport
        case supportedServices = "supported_services"
        case hidProfileSupported = "hid_profile_supported"
    }
}

/// One API the HID-peripheral role needs, and whether it exists on this host.
public struct APIAvailability: Codable, Equatable, Sendable {
    public let name: String
    public let present: Observation
    public let purpose: String

    public init(name: String, present: Observation, purpose: String) {
        self.name = name
        self.present = present
        self.purpose = purpose
    }
}

public struct HIDProbeReport: Codable, Equatable, Sendable {
    public let macosVersion: String
    public let bluetoothFrameworkLoaded: Observation
    public let controller: ControllerReport?
    public let peripheralRoleAPIs: [APIAvailability]
    /// Always `unknown`. Deciding it requires publishing an SDP record and having a
    /// device pair, which mutates system Bluetooth state — outside a read-only probe.
    public let canActAsHIDPeripheral: Observation
    public let canActAsHIDPeripheralNote: String
    public let notes: [String]

    public init(
        macosVersion: String,
        bluetoothFrameworkLoaded: Observation,
        controller: ControllerReport?,
        peripheralRoleAPIs: [APIAvailability],
        canActAsHIDPeripheral: Observation,
        canActAsHIDPeripheralNote: String,
        notes: [String]
    ) {
        self.macosVersion = macosVersion
        self.bluetoothFrameworkLoaded = bluetoothFrameworkLoaded
        self.controller = controller
        self.peripheralRoleAPIs = peripheralRoleAPIs
        self.canActAsHIDPeripheral = canActAsHIDPeripheral
        self.canActAsHIDPeripheralNote = canActAsHIDPeripheralNote
        self.notes = notes
    }

    private enum CodingKeys: String, CodingKey {
        case macosVersion = "macos_version"
        case bluetoothFrameworkLoaded = "bluetooth_framework_loaded"
        case controller
        case peripheralRoleAPIs = "peripheral_role_apis"
        case canActAsHIDPeripheral = "can_act_as_hid_peripheral"
        case canActAsHIDPeripheralNote = "can_act_as_hid_peripheral_note"
        case notes
    }

    public func json() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .prettyPrinted]
        return String(decoding: try encoder.encode(self), as: UTF8.self)
    }
}

public enum HIDProbeError: String, Error, Equatable, Sendable {
    case profileCommandMissing = "system_profiler is not available"
    case profileCommandFailed = "system_profiler exited non-zero"
    case profileCommandTimedOut = "system_profiler timed out"
    case profileOutputTooLarge = "system_profiler output exceeded the limit"
    case profileOutputMalformed = "system_profiler output could not be parsed"
}
