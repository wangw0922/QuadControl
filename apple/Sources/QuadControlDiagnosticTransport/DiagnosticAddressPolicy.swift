import Foundation
import Network

public enum DiagnosticAddressPolicy {
    public static func peerKey(_ endpoint: NWEndpoint) -> String? {
        guard case let .hostPort(host, _) = endpoint else {
            return nil
        }
        let value = normalized(String(describing: host))
        if let address = IPv4Address(value) {
            return "v4:\(address.rawValue.base64EncodedString())"
        }
        if let address = IPv6Address(value) {
            return "v6:\(address.rawValue.base64EncodedString())"
        }
        return nil
    }

    public static func isPrivateOrLinkLocal(_ endpoint: NWEndpoint) -> Bool {
        guard case let .hostPort(host, _) = endpoint else {
            return false
        }
        return isPrivateOrLinkLocal(host: String(describing: host))
    }

    public static func allowedClientHost(_ rawHost: String) -> NWEndpoint.Host? {
        guard rawHost == rawHost.trimmingCharacters(in: .whitespacesAndNewlines),
              !rawHost.contains("["),
              !rawHost.contains("]"),
              !rawHost.contains("%") else {
            return nil
        }

        if let address = IPv4Address(rawHost) {
            let bytes = [UInt8](address.rawValue)
            guard bytes == [127, 0, 0, 1] || isPrivateOrLinkLocal(address) else {
                return nil
            }
            return .ipv4(address)
        }
        if let address = IPv6Address(rawHost) {
            let bytes = [UInt8](address.rawValue)
            guard bytes == Array(repeating: 0, count: 15) + [1]
                    || isPrivateOrLinkLocal(address) else {
                return nil
            }
            return .ipv6(address)
        }
        return nil
    }

    public static func isAllowedClientHost(_ rawHost: String) -> Bool {
        allowedClientHost(rawHost) != nil
    }

    public static func isPrivateOrLinkLocal(host rawHost: String) -> Bool {
        let host = normalized(rawHost)

        if let address = IPv4Address(host) {
            return isPrivateOrLinkLocal(address)
        }

        if let address = IPv6Address(host) {
            return isPrivateOrLinkLocal(address)
        }

        return false
    }

    private static func normalized(_ rawHost: String) -> String {
        let withoutBrackets = rawHost.trimmingCharacters(in: CharacterSet(charactersIn: "[]"))
        return withoutBrackets.split(separator: "%", maxSplits: 1).first.map(String.init) ?? withoutBrackets
    }

    private static func isPrivateOrLinkLocal(_ address: IPv4Address) -> Bool {
        let bytes = [UInt8](address.rawValue)
        guard bytes.count == 4 else { return false }
        return bytes[0] == 10
            || (bytes[0] == 172 && (16 ... 31).contains(bytes[1]))
            || (bytes[0] == 192 && bytes[1] == 168)
            || (bytes[0] == 169 && bytes[1] == 254)
    }

    private static func isPrivateOrLinkLocal(_ address: IPv6Address) -> Bool {
        let bytes = [UInt8](address.rawValue)
        guard bytes.count == 16 else { return false }
        let uniqueLocal = bytes[0] & 0xfe == 0xfc
        let linkLocal = bytes[0] == 0xfe && bytes[1] & 0xc0 == 0x80
        return uniqueLocal || linkLocal
    }
}
