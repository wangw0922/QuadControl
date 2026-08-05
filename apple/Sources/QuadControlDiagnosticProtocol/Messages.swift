import Foundation

public enum DiagnosticType: String, Codable, Sendable {
    case hello
    case challenge
    case clientProof = "client_proof"
    case serverFinished = "server_finished"
    case heartbeat
    case ack
    case error
}

public protocol DiagnosticPayload: Codable, Sendable {
    func validate() throws
}

public struct DiagnosticEnvelope: Codable, Equatable, Sendable {
    public let version: UInt32
    public let type: DiagnosticType
    public let connectionID: Data
    public let sequence: UInt64
    public let payload: Data
    public let mac: Data?

    public init(
        version: UInt32 = DiagnosticConstants.version,
        type: DiagnosticType,
        connectionID: Data,
        sequence: UInt64 = 0,
        payload: Data,
        mac: Data? = nil
    ) {
        self.version = version
        self.type = type
        self.connectionID = connectionID
        self.sequence = sequence
        self.payload = payload
        self.mac = mac
    }

    private enum CodingKeys: String, CodingKey {
        case version = "protocol_version"
        case type
        case connectionID = "connection_id"
        case sequence
        case payload
        case mac
    }
}

public struct DiagnosticHello: DiagnosticPayload, Equatable {
    public let clientNonce: Data

    public init(clientNonce: Data) {
        self.clientNonce = clientNonce
    }

    public func validate() throws {
        try requireLength(clientNonce, DiagnosticConstants.nonceLength)
    }

    private enum CodingKeys: String, CodingKey {
        case clientNonce = "client_nonce"
    }
}

public struct DiagnosticChallenge: DiagnosticPayload, Equatable {
    public let serverNonce: Data
    public let challengeID: Data
    public let serverID: Data

    public init(serverNonce: Data, challengeID: Data, serverID: Data) {
        self.serverNonce = serverNonce
        self.challengeID = challengeID
        self.serverID = serverID
    }

    public func validate() throws {
        try requireLength(serverNonce, DiagnosticConstants.nonceLength)
        try requireLength(challengeID, DiagnosticConstants.identifierLength)
        try requireLength(serverID, DiagnosticConstants.identifierLength)
    }

    private enum CodingKeys: String, CodingKey {
        case serverNonce = "server_nonce"
        case challengeID = "challenge_id"
        case serverID = "server_id"
    }
}

public struct DiagnosticClientProof: DiagnosticPayload, Equatable {
    public let proof: Data

    public init(proof: Data) {
        self.proof = proof
    }

    public func validate() throws {
        try requireLength(proof, DiagnosticConstants.macLength)
    }
}

public struct DiagnosticServerFinished: DiagnosticPayload, Equatable {
    public let sessionID: Data
    public let proof: Data

    public init(sessionID: Data, proof: Data) {
        self.sessionID = sessionID
        self.proof = proof
    }

    public func validate() throws {
        try requireLength(sessionID, DiagnosticConstants.identifierLength)
        try requireLength(proof, DiagnosticConstants.macLength)
    }

    private enum CodingKeys: String, CodingKey {
        case sessionID = "session_id"
        case proof
    }
}

public struct DiagnosticHeartbeat: DiagnosticPayload, Equatable {
    public let status: String

    public init(status: String = "alive") {
        self.status = status
    }

    public func validate() throws {
        guard status == "alive" else {
            throw DiagnosticFailure.malformed
        }
    }
}

public struct DiagnosticAck: DiagnosticPayload, Equatable {
    public let status: String

    public init(status: String = "ok") {
        self.status = status
    }

    public func validate() throws {
        guard status == "ok" else {
            throw DiagnosticFailure.malformed
        }
    }
}

public struct DiagnosticError: DiagnosticPayload, Equatable {
    public let code: DiagnosticFailure

    public init(code: DiagnosticFailure) {
        self.code = code
    }

    public func validate() throws {}
}

public enum DiagnosticWire {
    public static func payload<T: DiagnosticPayload>(_ message: T) throws -> Data {
        try message.validate()
        let data = try JSONEncoder().encode(message)
        guard data.count <= DiagnosticConstants.maxFrameLength else {
            throw DiagnosticFailure.length
        }
        return data
    }

    public static func encode<T: DiagnosticPayload>(
        _ message: T,
        type: DiagnosticType,
        connectionID: Data,
        sequence: UInt64 = 0,
        mac: Data? = nil
    ) throws -> Data {
        try encode(
            payload: payload(message),
            type: type,
            connectionID: connectionID,
            sequence: sequence,
            mac: mac
        )
    }

    public static func encode(
        payload: Data,
        type: DiagnosticType,
        connectionID: Data,
        sequence: UInt64 = 0,
        mac: Data? = nil
    ) throws -> Data {
        try requireLength(connectionID, DiagnosticConstants.identifierLength)
        if let mac {
            try requireLength(mac, DiagnosticConstants.macLength)
        }
        let encoded = try JSONEncoder().encode(
            DiagnosticEnvelope(
                type: type,
                connectionID: connectionID,
                sequence: sequence,
                payload: payload,
                mac: mac
            )
        )
        guard !encoded.isEmpty, encoded.count <= DiagnosticConstants.maxFrameLength else {
            throw DiagnosticFailure.length
        }
        return encoded
    }

    public static func decode(_ data: Data) throws -> DiagnosticEnvelope {
        guard !data.isEmpty, data.count <= DiagnosticConstants.maxFrameLength else {
            throw DiagnosticFailure.length
        }
        let envelope: DiagnosticEnvelope
        do {
            envelope = try JSONDecoder().decode(DiagnosticEnvelope.self, from: data)
        } catch {
            throw DiagnosticFailure.malformed
        }
        guard envelope.version == DiagnosticConstants.version else {
            throw DiagnosticFailure.version
        }
        try requireLength(envelope.connectionID, DiagnosticConstants.identifierLength)
        guard envelope.payload.count <= DiagnosticConstants.maxFrameLength else {
            throw DiagnosticFailure.length
        }
        if let mac = envelope.mac {
            try requireLength(mac, DiagnosticConstants.macLength)
        }
        return envelope
    }

    public static func unpack<T: DiagnosticPayload>(
        _ envelope: DiagnosticEnvelope,
        as type: T.Type
    ) throws -> T {
        let message: T
        do {
            message = try JSONDecoder().decode(type, from: envelope.payload)
        } catch {
            throw DiagnosticFailure.malformed
        }
        try message.validate()
        return message
    }

    public static func frame(_ payload: Data) throws -> Data {
        guard !payload.isEmpty, payload.count <= DiagnosticConstants.maxFrameLength else {
            throw DiagnosticFailure.length
        }
        var frame = Data()
        let length = UInt32(payload.count)
        frame.append(UInt8((length >> 24) & 0xff))
        frame.append(UInt8((length >> 16) & 0xff))
        frame.append(UInt8((length >> 8) & 0xff))
        frame.append(UInt8(length & 0xff))
        frame.append(payload)
        return frame
    }

    public static func decodeFrameLength(_ header: Data) throws -> Int {
        guard header.count == 4 else {
            throw DiagnosticFailure.malformed
        }
        let length = header.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
        guard length > 0, length <= DiagnosticConstants.maxFrameLength else {
            throw DiagnosticFailure.length
        }
        return Int(length)
    }

    public static func unframe(_ frame: Data) throws -> Data {
        guard frame.count >= 4 else {
            throw DiagnosticFailure.malformed
        }
        let length = try decodeFrameLength(frame.prefix(4))
        guard frame.count == length + 4 else {
            throw DiagnosticFailure.length
        }
        return Data(frame.dropFirst(4))
    }
}
