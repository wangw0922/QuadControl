import Foundation

public enum DiagnosticConstants {
    public static let version: UInt32 = 1
    public static let maxFrameLength = 4_096
    public static let tokenLength = 32
    public static let nonceLength = 32
    public static let identifierLength = 16
    public static let macLength = 32
    public static let tokenTTLNanos: UInt64 = 180_000_000_000
    public static let handshakeTimeout: TimeInterval = 10
    public static let handshakeTimeoutNanos: UInt64 = 10_000_000_000
    public static let heartbeatInterval: TimeInterval = 5
    public static let heartbeatTimeout: TimeInterval = 15
    public static let maxPending = 8
    public static let maxActive = 1
    public static let maxFailuresPerPeer = 5
    public static let maxFailuresGlobal = 20
    public static let failureWindowNanos: UInt64 = 60_000_000_000
}

public enum DiagnosticFailure: String, Error, Codable, Equatable, Sendable {
    case malformed
    case version
    case length
    case state
    case authentication
    case expired
    case consumed
    case replay
    case resourceLimit
    case rateLimited
    case timeout
    case network
}

public func monotonicNowNanos() -> UInt64 {
    DispatchTime.now().uptimeNanoseconds
}

public func requireLength(_ data: Data, _ length: Int) throws {
    guard data.count == length else {
        throw DiagnosticFailure.length
    }
}

public func secureRandomData(count: Int) -> Data {
    var generator = SystemRandomNumberGenerator()
    return Data((0 ..< count).map { _ in UInt8.random(in: .min ... .max, using: &generator) })
}
