import Foundation
import ObjectiveC

/// Checks whether the APIs a Bluetooth HID *peripheral* would need exist on this host.
///
/// macOS normally acts as a HID **host** — it accepts keyboards and mice. Presenting
/// the Mac to a phone *as* a keyboard is the opposite role, and needs three things:
/// publish an SDP record carrying the HID service class, accept incoming L2CAP on the
/// HID control and interrupt PSMs, and tear the record down afterwards.
///
/// This only asks the Objective-C runtime whether the classes and selectors exist.
/// Nothing is invoked, so no SDP record is published, no Bluetooth state changes, and
/// no permission prompt is triggered. Symbol presence is necessary but nowhere near
/// sufficient — see `canActAsHIDPeripheral`, which stays `unknown`.
public enum PeripheralRoleAPIs {
    public static let frameworkPath =
        "/System/Library/Frameworks/IOBluetooth.framework/IOBluetooth"

    public struct Requirement: Sendable {
        public let name: String
        public let className: String
        public let selector: String
        public let isClassMethod: Bool
        public let purpose: String
    }

    public static let requirements: [Requirement] = [
        Requirement(
            name: "IOBluetoothSDPServiceRecord.publishedServiceRecordWithDictionary:",
            className: "IOBluetoothSDPServiceRecord",
            selector: "publishedServiceRecordWithDictionary:",
            isClassMethod: true,
            purpose: "Publish the HID service record that makes this Mac discoverable as a keyboard"
        ),
        Requirement(
            name: "IOBluetoothSDPServiceRecord.removeServiceRecord",
            className: "IOBluetoothSDPServiceRecord",
            selector: "removeServiceRecord",
            isClassMethod: false,
            purpose: "Withdraw the published record so the Mac stops advertising the role"
        ),
        Requirement(
            name: "IOBluetoothL2CAPChannel.registerForChannelOpenNotifications:selector:withPSM:direction:",
            className: "IOBluetoothL2CAPChannel",
            selector: "registerForChannelOpenNotifications:selector:withPSM:direction:",
            isClassMethod: true,
            purpose: "Accept incoming HID control (PSM 0x11) and interrupt (PSM 0x13) channels"
        ),
    ]

    /// Loading a framework is read-only; it does not start a session or change state.
    @discardableResult
    public static func loadFramework(at path: String = frameworkPath) -> Bool {
        if NSClassFromString("IOBluetoothSDPServiceRecord") != nil { return true }
        return dlopen(path, RTLD_LAZY) != nil
    }

    public static func availability(of requirement: Requirement) -> Observation {
        guard let cls = NSClassFromString(requirement.className) else { return .no }
        guard let selector = NSSelectorFromString(requirement.selector) as Selector? else {
            return .unknown
        }
        let found = requirement.isClassMethod
            ? class_getClassMethod(cls, selector) != nil
            : class_getInstanceMethod(cls, selector) != nil
        return found ? .yes : .no
    }

    public static func report(frameworkLoaded: Bool) -> [APIAvailability] {
        requirements.map { requirement in
            APIAvailability(
                name: requirement.name,
                // Without the framework the runtime cannot answer; that is not the same
                // as the API being absent.
                present: frameworkLoaded ? availability(of: requirement) : .unknown,
                purpose: requirement.purpose
            )
        }
    }
}
