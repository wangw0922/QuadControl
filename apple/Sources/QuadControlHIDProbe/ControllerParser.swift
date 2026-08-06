import Foundation

/// Parses `system_profiler SPBluetoothDataType -json`.
///
/// Parsing is kept separate from execution, like the ADB probe, so every shape of
/// output can be unit tested without Bluetooth hardware.
public enum ControllerParser {
    /// `controller_supportedServices` looks like
    /// `0x392039 < HFP AVRCP A2DP HID Braille LEA AACP GATT SerialPort >`.
    public static func supportedServices(from raw: String) -> [String] {
        guard let open = raw.firstIndex(of: "<"), let close = raw.lastIndex(of: ">"), open < close else {
            return []
        }
        return raw[raw.index(after: open) ..< close]
            .split(whereSeparator: { $0 == " " || $0 == "," })
            .map(String.init)
            .filter { !$0.isEmpty }
    }

    /// system_profiler reports switch states as `attrib_on` / `attrib_off`.
    public static func attribute(_ raw: String?) -> Observation {
        switch raw {
        case "attrib_on": return .yes
        case "attrib_off": return .no
        default: return .unknown
        }
    }

    public static func parse(_ data: Data) throws -> ControllerReport? {
        let root: Any
        do {
            root = try JSONSerialization.jsonObject(with: data)
        } catch {
            throw HIDProbeError.profileOutputMalformed
        }
        guard let object = root as? [String: Any] else {
            throw HIDProbeError.profileOutputMalformed
        }
        // An absent or empty array is a valid observation: no controller on this host.
        guard let entries = object["SPBluetoothDataType"] as? [[String: Any]],
              let first = entries.first else {
            return nil
        }
        guard let properties = first["controller_properties"] as? [String: Any] else {
            return nil
        }

        let services = (properties["controller_supportedServices"] as? String)
            .map(supportedServices) ?? []
        return ControllerReport(
            powered: attribute(properties["controller_state"] as? String),
            discoverable: attribute(properties["controller_discoverable"] as? String),
            chipset: properties["controller_chipset"] as? String,
            transport: properties["controller_transport"] as? String,
            supportedServices: services,
            // Only claim `no` when the service list was actually readable.
            hidProfileSupported: services.isEmpty
                ? .unknown
                : (services.contains("HID") ? .yes : .no)
        )
    }
}
