import CryptoKit
import Foundation

public enum PairingToken {
    public static func encode(_ token: Data) throws -> String {
        try requireLength(token, DiagnosticConstants.tokenLength)
        return token
            .base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    public static func decode(_ text: String) throws -> Data {
        guard !text.isEmpty, text.range(of: #"^[A-Za-z0-9_-]{43}$"#, options: .regularExpression) != nil else {
            throw DiagnosticFailure.length
        }
        var value = text.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        value.append(String(repeating: "=", count: (4 - value.count % 4) % 4))
        guard let token = Data(base64Encoded: value) else {
            throw DiagnosticFailure.malformed
        }
        try requireLength(token, DiagnosticConstants.tokenLength)
        return token
    }
}

public enum DiagnosticTranscript {
    private static let domain = Data("QuadControlDiagnostic".utf8)

    private static func append(_ field: Data, to result: inout Data) {
        let length = UInt32(field.count)
        result.append(UInt8((length >> 24) & 0xff))
        result.append(UInt8((length >> 16) & 0xff))
        result.append(UInt8((length >> 8) & 0xff))
        result.append(UInt8(length & 0xff))
        result.append(field)
    }

    private static func canonical(_ label: String, fields: [Data]) -> Data {
        var result = domain
        var version = DiagnosticConstants.version.bigEndian
        result.append(withUnsafeBytes(of: &version) { Data($0) })
        append(Data(label.utf8), to: &result)
        for field in fields {
            append(field, to: &result)
        }
        return result
    }

    public static func clientProof(
        hello: DiagnosticHello,
        challenge: DiagnosticChallenge
    ) throws -> Data {
        try hello.validate()
        try challenge.validate()
        return canonical(
            "client-proof",
            fields: [
                hello.clientNonce,
                challenge.serverNonce,
                challenge.challengeID,
                challenge.serverID,
            ]
        )
    }

    private static func sessionInfo(
        hello: DiagnosticHello,
        challenge: DiagnosticChallenge
    ) throws -> Data {
        try hello.validate()
        try challenge.validate()
        return canonical(
            "session-key",
            fields: [
                hello.clientNonce,
                challenge.serverNonce,
                challenge.challengeID,
                challenge.serverID,
            ]
        )
    }

    public static func serverFinished(
        hello: DiagnosticHello,
        challenge: DiagnosticChallenge,
        sessionID: Data
    ) throws -> Data {
        try hello.validate()
        try challenge.validate()
        try requireLength(sessionID, DiagnosticConstants.identifierLength)
        return canonical(
            "server-finished",
            fields: [
                hello.clientNonce,
                challenge.serverNonce,
                challenge.challengeID,
                challenge.serverID,
                sessionID,
            ]
        )
    }

    public static func clientHeartbeat(
        sessionID: Data,
        sequence: UInt64,
        status: String
    ) throws -> Data {
        try requireLength(sessionID, DiagnosticConstants.identifierLength)
        guard status == "alive" else { throw DiagnosticFailure.malformed }
        return canonicalHeartbeat(
            label: "client-heartbeat",
            sessionID: sessionID,
            sequence: sequence,
            semanticValue: Data(status.utf8)
        )
    }

    public static func serverAck(
        sessionID: Data,
        sequence: UInt64,
        status: String
    ) throws -> Data {
        try requireLength(sessionID, DiagnosticConstants.identifierLength)
        guard status == "ok" else { throw DiagnosticFailure.malformed }
        return canonicalHeartbeat(
            label: "server-ack",
            sessionID: sessionID,
            sequence: sequence,
            semanticValue: Data(status.utf8)
        )
    }

    private static func canonicalHeartbeat(
        label: String,
        sessionID: Data,
        sequence: UInt64,
        semanticValue: Data
    ) -> Data {
        var bigEndianSequence = sequence.bigEndian
        let sequenceData = withUnsafeBytes(of: &bigEndianSequence) { Data($0) }
        return canonical(label, fields: [sessionID, sequenceData, semanticValue])
    }

    public static func mac(key: Data, transcript: Data) throws -> Data {
        try requireLength(key, DiagnosticConstants.macLength)
        return Data(
            HMAC<SHA256>.authenticationCode(
                for: transcript,
                using: SymmetricKey(data: key)
            )
        )
    }

    public static func isValid(mac: Data, key: Data, transcript: Data) -> Bool {
        guard mac.count == DiagnosticConstants.macLength,
              key.count == DiagnosticConstants.macLength else {
            return false
        }
        return HMAC<SHA256>.isValidAuthenticationCode(
            mac,
            authenticating: transcript,
            using: SymmetricKey(data: key)
        )
    }

    public static func sessionKey(
        token: Data,
        hello: DiagnosticHello,
        challenge: DiagnosticChallenge
    ) throws -> Data {
        try requireLength(token, DiagnosticConstants.tokenLength)
        let info = try sessionInfo(hello: hello, challenge: challenge)
        let salt = hello.clientNonce + challenge.serverNonce
        let key = HKDF<SHA256>.deriveKey(
            inputKeyMaterial: SymmetricKey(data: token),
            salt: salt,
            info: info,
            outputByteCount: DiagnosticConstants.macLength
        )
        return key.withUnsafeBytes { Data($0) }
    }
}

public final class OneTimeToken: @unchecked Sendable {
    public let value: Data
    private let createdAtNanos: UInt64
    private let lock = NSLock()
    private var consumed = false

    public init(createdAtNanos: UInt64 = monotonicNowNanos()) {
        value = secureRandomData(count: DiagnosticConstants.tokenLength)
        self.createdAtNanos = createdAtNanos
    }

    public init(
        value: Data,
        createdAtNanos: UInt64 = monotonicNowNanos()
    ) throws {
        try requireLength(value, DiagnosticConstants.tokenLength)
        self.value = value
        self.createdAtNanos = createdAtNanos
    }

    public func consumeIfValid(nowNanos: UInt64 = monotonicNowNanos()) throws {
        lock.lock()
        defer { lock.unlock() }
        guard nowNanos >= createdAtNanos,
              nowNanos - createdAtNanos <= DiagnosticConstants.tokenTTLNanos else {
            throw DiagnosticFailure.expired
        }
        guard !consumed else {
            throw DiagnosticFailure.consumed
        }
        consumed = true
    }
}

public final class ServerHandshake {
    private enum State: Equatable {
        case awaitingHello
        case awaitingProof
        case complete
    }

    private let token: OneTimeToken
    private let serverID: Data
    private let createdAtNanos: UInt64
    private var state = State.awaitingHello
    private var hello: DiagnosticHello?
    private var challenge: DiagnosticChallenge?

    public init(
        token: OneTimeToken,
        serverID: Data = secureRandomData(count: DiagnosticConstants.identifierLength),
        nowNanos: UInt64 = monotonicNowNanos()
    ) throws {
        try requireLength(serverID, DiagnosticConstants.identifierLength)
        self.token = token
        self.serverID = serverID
        self.createdAtNanos = nowNanos
    }

    public func receiveHello(
        _ input: DiagnosticHello,
        nowNanos: UInt64 = monotonicNowNanos()
    ) throws -> DiagnosticChallenge {
        try checkDeadline(nowNanos)
        guard state == .awaitingHello else {
            throw DiagnosticFailure.state
        }
        try input.validate()
        let next = DiagnosticChallenge(
            serverNonce: secureRandomData(count: DiagnosticConstants.nonceLength),
            challengeID: secureRandomData(count: DiagnosticConstants.identifierLength),
            serverID: serverID
        )
        hello = input
        challenge = next
        state = .awaitingProof
        return next
    }

    public func receiveProof(
        _ input: DiagnosticClientProof,
        nowNanos: UInt64 = monotonicNowNanos()
    ) throws -> (finished: DiagnosticServerFinished, sessionKey: Data) {
        try checkDeadline(nowNanos)
        guard state == .awaitingProof,
              let hello,
              let challenge else {
            throw DiagnosticFailure.state
        }
        try input.validate()
        let transcript = try DiagnosticTranscript.clientProof(hello: hello, challenge: challenge)
        guard DiagnosticTranscript.isValid(mac: input.proof, key: token.value, transcript: transcript) else {
            throw DiagnosticFailure.authentication
        }
        try token.consumeIfValid(nowNanos: nowNanos)
        state = .complete
        let sessionID = secureRandomData(count: DiagnosticConstants.identifierLength)
        let sessionKey = try DiagnosticTranscript.sessionKey(
            token: token.value,
            hello: hello,
            challenge: challenge
        )
        let proof = try DiagnosticTranscript.mac(
            key: sessionKey,
            transcript: DiagnosticTranscript.serverFinished(
                hello: hello,
                challenge: challenge,
                sessionID: sessionID
            )
        )
        return (
            DiagnosticServerFinished(sessionID: sessionID, proof: proof),
            sessionKey
        )
    }

    private func checkDeadline(_ nowNanos: UInt64) throws {
        guard nowNanos >= createdAtNanos,
              nowNanos - createdAtNanos <= DiagnosticConstants.handshakeTimeoutNanos else {
            throw DiagnosticFailure.timeout
        }
    }
}

public final class ClientHandshake {
    private enum State: Equatable {
        case awaitingChallenge
        case awaitingFinished
        case complete
    }

    public let hello: DiagnosticHello
    private let token: Data
    private var state = State.awaitingChallenge
    private var challenge: DiagnosticChallenge?

    public init(
        token: Data,
        clientNonce: Data = secureRandomData(count: DiagnosticConstants.nonceLength)
    ) throws {
        try requireLength(token, DiagnosticConstants.tokenLength)
        try requireLength(clientNonce, DiagnosticConstants.nonceLength)
        self.token = token
        hello = DiagnosticHello(clientNonce: clientNonce)
    }

    public func receiveChallenge(_ challenge: DiagnosticChallenge) throws -> DiagnosticClientProof {
        guard state == .awaitingChallenge else {
            throw DiagnosticFailure.state
        }
        try challenge.validate()
        self.challenge = challenge
        state = .awaitingFinished
        let transcript = try DiagnosticTranscript.clientProof(hello: hello, challenge: challenge)
        return DiagnosticClientProof(
            proof: try DiagnosticTranscript.mac(key: token, transcript: transcript)
        )
    }

    public func receiveFinished(
        _ finished: DiagnosticServerFinished
    ) throws -> (sessionID: Data, sessionKey: Data) {
        guard state == .awaitingFinished,
              let challenge else {
            throw DiagnosticFailure.state
        }
        try finished.validate()
        let sessionKey = try DiagnosticTranscript.sessionKey(
            token: token,
            hello: hello,
            challenge: challenge
        )
        let transcript = try DiagnosticTranscript.serverFinished(
            hello: hello,
            challenge: challenge,
            sessionID: finished.sessionID
        )
        guard DiagnosticTranscript.isValid(
            mac: finished.proof,
            key: sessionKey,
            transcript: transcript
        ) else {
            throw DiagnosticFailure.authentication
        }
        state = .complete
        return (finished.sessionID, sessionKey)
    }
}

public final class HeartbeatSequence: @unchecked Sendable {
    private let lock = NSLock()
    private var sent: UInt64 = 0
    private var received: UInt64 = 0

    public init() {}

    public func next() throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard sent < UInt64.max else {
            throw DiagnosticFailure.resourceLimit
        }
        sent += 1
        return sent
    }

    public func accept(_ sequence: UInt64) throws {
        lock.lock()
        defer { lock.unlock() }
        guard sequence > received else {
            throw DiagnosticFailure.replay
        }
        received = sequence
    }
}
