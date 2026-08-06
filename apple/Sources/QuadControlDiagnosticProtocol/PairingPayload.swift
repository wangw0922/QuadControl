import Foundation

/// Contents of the QR code the Mac listener displays so a phone does not have to type
/// the 43-character token. Mirrors `DiagnosticPairingPayload` in
/// `proto/quadcontrol/v1/session.proto`.
///
/// This is deliberately not a URL. It is decoded straight into the client's existing
/// validation path and is never handed to the system to open, written to disk, or put
/// on a pasteboard. `validate()` checks shape only — callers must still run `host`
/// through `DiagnosticAddressPolicy` so a scanned code cannot point the client at a
/// public address.
public struct DiagnosticPairingPayload: Codable, Equatable, Sendable {
    /// A QR payload is ~110 bytes. Anything larger is not one of ours and is rejected
    /// before it reaches the JSON decoder.
    public static let maxEncodedLength = 512

    public let version: UInt32
    public let host: String
    public let port: UInt16
    public let token: String

    public init(
        version: UInt32 = DiagnosticConstants.version,
        host: String,
        port: UInt16,
        token: String
    ) {
        self.version = version
        self.host = host
        self.port = port
        self.token = token
    }

    public func validate() throws {
        guard version == DiagnosticConstants.version else {
            throw DiagnosticFailure.version
        }
        // Longest possible numeric literal is an IPv6 address; anything longer is not
        // an address this client will accept anyway.
        guard !host.isEmpty, host.count <= 45 else {
            throw DiagnosticFailure.malformed
        }
        guard port > 0 else {
            throw DiagnosticFailure.malformed
        }
        // Enforces the exact 43-character base64url form of a 32-byte token.
        _ = try PairingToken.decode(token)
    }

    public func encoded() throws -> Data {
        try validate()
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(self)
        guard data.count <= Self.maxEncodedLength else {
            throw DiagnosticFailure.length
        }
        return data
    }

    public static func decode(_ data: Data) throws -> DiagnosticPairingPayload {
        guard !data.isEmpty, data.count <= maxEncodedLength else {
            throw DiagnosticFailure.length
        }
        let payload: DiagnosticPairingPayload
        do {
            payload = try JSONDecoder().decode(DiagnosticPairingPayload.self, from: data)
        } catch {
            throw DiagnosticFailure.malformed
        }
        try payload.validate()
        return payload
    }

    private enum CodingKeys: String, CodingKey {
        case version = "protocol_version"
        case host
        case port
        case token
    }
}
