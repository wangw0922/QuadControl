import Darwin
import Foundation
import Network
import QuadControlDiagnosticTransport

/// Resolves the numeric private address the phone should connect to.
///
/// Before this existed the listener could only print "private address of the selected
/// interface" and the user had to find it with `ipconfig` themselves. The QR payload
/// needs a concrete host, so the address is resolved here and still checked against the
/// shared private-address policy — the listener never advertises a public address.
enum InterfaceAddress {
    static func privateIPv4(for interfaceType: NWInterface.InterfaceType) -> String? {
        guard let names = interfaceNames(for: interfaceType) else { return nil }
        return names.lazy.compactMap(numericIPv4).first { DiagnosticAddressPolicy.isPrivateOrLinkLocal(host: $0) }
    }

    /// `NWPathMonitor` is the only supported way to learn which BSD interfaces back a
    /// given Network.framework interface type, and it reports asynchronously.
    private static func interfaceNames(for interfaceType: NWInterface.InterfaceType) -> [String]? {
        let monitor = NWPathMonitor(requiredInterfaceType: interfaceType)
        let semaphore = DispatchSemaphore(value: 0)
        let box = NameBox()
        monitor.pathUpdateHandler = { path in
            box.set(path.availableInterfaces.map(\.name))
            semaphore.signal()
        }
        monitor.start(queue: DispatchQueue(label: "QuadControl.InterfaceAddress"))
        defer { monitor.cancel() }
        guard semaphore.wait(timeout: .now() + 3) == .success else { return nil }
        return box.get()
    }

    private static func numericIPv4(for interfaceName: String) -> String? {
        var storage: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&storage) == 0, let first = storage else { return nil }
        defer { freeifaddrs(storage) }

        for entry in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let flags = Int32(entry.pointee.ifa_flags)
            guard flags & IFF_UP != 0, flags & IFF_LOOPBACK == 0 else { continue }
            guard String(cString: entry.pointee.ifa_name) == interfaceName else { continue }
            guard let address = entry.pointee.ifa_addr,
                  address.pointee.sa_family == UInt8(AF_INET) else { continue }

            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            guard getnameinfo(
                address,
                socklen_t(address.pointee.sa_len),
                &host,
                socklen_t(host.count),
                nil,
                0,
                NI_NUMERICHOST
            ) == 0 else { continue }
            let bytes = host.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }
            return String(decoding: bytes, as: UTF8.self)
        }
        return nil
    }
}

private final class NameBox: @unchecked Sendable {
    private let lock = NSLock()
    private var names: [String] = []

    func set(_ value: [String]) {
        lock.lock()
        defer { lock.unlock() }
        names = value
    }

    func get() -> [String] {
        lock.lock()
        defer { lock.unlock() }
        return names
    }
}
