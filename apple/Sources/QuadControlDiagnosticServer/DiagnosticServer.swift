import Foundation
import Network
import QuadControlDiagnosticProtocol
import QuadControlDiagnosticTransport

public enum DiagnosticListenerMode: Equatable, Sendable {
    case loopback
    case lanWiFi
    case lanWired
}

public struct DiagnosticServerConfiguration: Equatable, Sendable {
    public let port: UInt16
    public let mode: DiagnosticListenerMode

    public init(port: UInt16 = 47_100, mode: DiagnosticListenerMode = .loopback) {
        self.port = port
        self.mode = mode
    }
}

public enum DiagnosticServerEvent: Equatable, Sendable {
    case ready(port: UInt16)
    case connectionAccepted(UUID)
    case authenticated(UUID)
    case heartbeat(UUID, sequence: UInt64)
    case connectionClosed(UUID, DiagnosticFailure)
    case stopped
    case failed(DiagnosticFailure)
}

public final class DiagnosticServer: @unchecked Sendable {
    public let token: OneTimeToken

    private let configuration: DiagnosticServerConfiguration
    private let admission: DiagnosticAdmissionController
    private let queue = DispatchQueue(label: "QuadControl.DiagnosticServer")
    private var listener: NWListener?
    private var connections: [UUID: DiagnosticServerConnection] = [:]
    private var eventHandler: (@Sendable (DiagnosticServerEvent) -> Void)?
    private var hasStarted = false
    private var didEmitReady = false

    public init(
        configuration: DiagnosticServerConfiguration,
        token: OneTimeToken = OneTimeToken(),
        admission: DiagnosticAdmissionController = DiagnosticAdmissionController()
    ) {
        self.configuration = configuration
        self.token = token
        self.admission = admission
    }

    public func start(
        eventHandler: @escaping @Sendable (DiagnosticServerEvent) -> Void
    ) throws {
        guard !hasStarted else {
            throw DiagnosticFailure.state
        }
        hasStarted = true
        self.eventHandler = eventHandler

        let parameters = NWParameters.tcp
        let requestedPort: NWEndpoint.Port = configuration.port == 0
            ? .any
            : NWEndpoint.Port(rawValue: configuration.port)!
        switch configuration.mode {
        case .loopback:
            parameters.requiredLocalEndpoint = .hostPort(
                host: "127.0.0.1",
                port: requestedPort
            )
        case .lanWiFi:
            parameters.requiredInterfaceType = .wifi
        case .lanWired:
            parameters.requiredInterfaceType = .wiredEthernet
        }

        let listener = try NWListener(using: parameters, on: requestedPort)
        self.listener = listener
        listener.stateUpdateHandler = { [weak self, weak listener] state in
            guard let self else { return }
            switch state {
            case .ready:
                guard !self.didEmitReady else { return }
                guard let rawPort = listener?.port?.rawValue else {
                    self.eventHandler?(.failed(.network))
                    return
                }
                self.didEmitReady = true
                self.eventHandler?(.ready(port: rawPort))
            case .failed:
                self.eventHandler?(.failed(.network))
                self.stopInternal()
            case .cancelled:
                self.eventHandler?(.stopped)
            default:
                break
            }
        }
        listener.newConnectionHandler = { [weak self] connection in
            self?.accept(connection)
        }
        listener.start(queue: queue)
    }

    public func stop() {
        queue.async { [self] in
            stopInternal()
        }
    }

    private func accept(_ connection: NWConnection) {
        guard let peerKey = DiagnosticAddressPolicy.peerKey(connection.endpoint) else {
            connection.cancel()
            return
        }
        if configuration.mode != .loopback,
           !DiagnosticAddressPolicy.isPrivateOrLinkLocal(connection.endpoint) {
            connection.cancel()
            return
        }

        let connectionID = UUID()
        guard case .success = admission.begin(
            connectionID: connectionID,
            peerKey: peerKey
        ) else {
            connection.cancel()
            return
        }

        do {
            let handler = try DiagnosticServerConnection(
                connection: connection,
                token: token,
                connectionID: connectionID,
                peerKey: peerKey,
                queue: queue,
                admission: admission,
                eventHandler: { [weak self] event in
                    self?.eventHandler?(event)
                },
                closeHandler: { [weak self] closedID in
                    self?.connections.removeValue(forKey: closedID)
                }
            )
            connections[connectionID] = handler
            eventHandler?(.connectionAccepted(connectionID))
            handler.start()
        } catch {
            admission.recordFailure(connectionID: connectionID, peerKey: peerKey)
            connection.cancel()
        }
    }

    private func stopInternal() {
        listener?.cancel()
        listener = nil
        let activeConnections = Array(connections.values)
        connections.removeAll()
        for connection in activeConnections {
            connection.stop()
        }
    }
}

private final class DiagnosticServerConnection: @unchecked Sendable {
    private enum Phase: Equatable {
        case awaitingReady
        case awaitingHello
        case awaitingProof
        case authenticated
        case closed
    }

    private let transport: FramedConnection
    private let connectionID: UUID
    private let peerKey: String
    private let queue: DispatchQueue
    private let admission: DiagnosticAdmissionController
    private let eventHandler: @Sendable (DiagnosticServerEvent) -> Void
    private let closeHandler: @Sendable (UUID) -> Void
    private let handshake: ServerHandshake
    private let incomingSequence = HeartbeatSequence()
    private var phase = Phase.awaitingReady
    private var wireConnectionID: Data?
    private var sessionID: Data?
    private var sessionKey: Data?
    private var handshakeTimeout: DispatchWorkItem?
    private var heartbeatTimeout: DispatchWorkItem?

    init(
        connection: NWConnection,
        token: OneTimeToken,
        connectionID: UUID,
        peerKey: String,
        queue: DispatchQueue,
        admission: DiagnosticAdmissionController,
        eventHandler: @escaping @Sendable (DiagnosticServerEvent) -> Void,
        closeHandler: @escaping @Sendable (UUID) -> Void
    ) throws {
        transport = FramedConnection(connection: connection)
        self.connectionID = connectionID
        self.peerKey = peerKey
        self.queue = queue
        self.admission = admission
        self.eventHandler = eventHandler
        self.closeHandler = closeHandler
        handshake = try ServerHandshake(token: token)
    }

    func start() {
        scheduleHandshakeTimeout()
        transport.start(queue: queue) { [weak self] state in
            self?.handleConnectionState(state)
        }
    }

    func stop() {
        close(.network, recordFailure: false)
    }

    private func handleConnectionState(_ state: NWConnection.State) {
        switch state {
        case .ready:
            guard phase == .awaitingReady else { return }
            phase = .awaitingHello
            receive()
        case .failed, .cancelled:
            close(.network, recordFailure: false)
        default:
            break
        }
    }

    private func receive() {
        transport.receive { [weak self] result in
            guard let self else { return }
            self.queue.async {
                do {
                    try self.handle(result.get())
                } catch let failure as DiagnosticFailure {
                    self.close(failure, recordFailure: self.phase != .authenticated)
                } catch {
                    self.close(.malformed, recordFailure: self.phase != .authenticated)
                }
            }
        }
    }

    private func handle(_ body: Data) throws {
        let envelope = try DiagnosticWire.decode(body)
        if wireConnectionID == nil {
            guard phase == .awaitingHello,
                  envelope.type == .hello else {
                throw DiagnosticFailure.state
            }
            wireConnectionID = envelope.connectionID
        }
        guard envelope.connectionID == wireConnectionID else {
            throw DiagnosticFailure.authentication
        }

        switch phase {
        case .awaitingHello:
            try handleHello(envelope)
        case .awaitingProof:
            try handleProof(envelope)
        case .authenticated:
            try handleHeartbeat(envelope)
        default:
            throw DiagnosticFailure.state
        }
    }

    private func handleHello(_ envelope: DiagnosticEnvelope) throws {
        guard envelope.type == .hello,
              envelope.sequence == 0,
              envelope.mac == nil else {
            throw DiagnosticFailure.state
        }
        let hello = try DiagnosticWire.unpack(envelope, as: DiagnosticHello.self)
        let challenge = try handshake.receiveHello(hello)
        phase = .awaitingProof
        try send(challenge, type: .challenge) { [weak self] failure in
            guard let self else { return }
            if let failure {
                self.close(failure, recordFailure: true)
            } else {
                self.receive()
            }
        }
    }

    private func handleProof(_ envelope: DiagnosticEnvelope) throws {
        guard envelope.type == .clientProof,
              envelope.sequence == 0,
              envelope.mac == nil else {
            throw DiagnosticFailure.state
        }
        let proof = try DiagnosticWire.unpack(envelope, as: DiagnosticClientProof.self)
        let result = try handshake.receiveProof(proof)
        guard admission.authenticationSucceeded(connectionID: connectionID) else {
            throw DiagnosticFailure.resourceLimit
        }
        sessionID = result.finished.sessionID
        sessionKey = result.sessionKey
        phase = .authenticated
        handshakeTimeout?.cancel()
        eventHandler(.authenticated(connectionID))
        scheduleHeartbeatTimeout()
        try send(result.finished, type: .serverFinished) { [weak self] failure in
            guard let self else { return }
            if let failure {
                self.close(failure, recordFailure: false)
            } else {
                self.receive()
            }
        }
    }

    private func handleHeartbeat(_ envelope: DiagnosticEnvelope) throws {
        guard envelope.type == .heartbeat,
              let mac = envelope.mac,
              let sessionID,
              let sessionKey else {
            throw DiagnosticFailure.state
        }
        let heartbeat = try DiagnosticWire.unpack(envelope, as: DiagnosticHeartbeat.self)
        try heartbeat.validate()
        let transcript = try DiagnosticTranscript.clientHeartbeat(
            sessionID: sessionID,
            sequence: envelope.sequence,
            status: heartbeat.status
        )
        guard DiagnosticTranscript.isValid(
            mac: mac,
            key: sessionKey,
            transcript: transcript
        ) else {
            throw DiagnosticFailure.authentication
        }
        try incomingSequence.accept(envelope.sequence)
        heartbeatTimeout?.cancel()

        let ack = DiagnosticAck()
        let payload = try DiagnosticWire.payload(ack)
        let ackTranscript = try DiagnosticTranscript.serverAck(
            sessionID: sessionID,
            sequence: envelope.sequence,
            status: ack.status
        )
        let ackMac = try DiagnosticTranscript.mac(key: sessionKey, transcript: ackTranscript)
        eventHandler(.heartbeat(connectionID, sequence: envelope.sequence))
        scheduleHeartbeatTimeout()
        try send(
            payload: payload,
            type: .ack,
            sequence: envelope.sequence,
            mac: ackMac
        ) { [weak self] failure in
            guard let self else { return }
            if let failure {
                self.close(failure, recordFailure: false)
            } else {
                self.receive()
            }
        }
    }

    private func send<T: DiagnosticPayload>(
        _ message: T,
        type: DiagnosticType,
        completion: @escaping @Sendable (DiagnosticFailure?) -> Void
    ) throws {
        try send(payload: DiagnosticWire.payload(message), type: type, completion: completion)
    }

    private func send(
        payload: Data,
        type: DiagnosticType,
        sequence: UInt64 = 0,
        mac: Data? = nil,
        completion: @escaping @Sendable (DiagnosticFailure?) -> Void
    ) throws {
        guard let wireConnectionID else {
            throw DiagnosticFailure.state
        }
        let wire = try DiagnosticWire.encode(
            payload: payload,
            type: type,
            connectionID: wireConnectionID,
            sequence: sequence,
            mac: mac
        )
        transport.send(wire, completion: completion)
    }

    private func scheduleHandshakeTimeout() {
        let item = DispatchWorkItem { [weak self] in
            self?.close(.timeout, recordFailure: true)
        }
        handshakeTimeout = item
        queue.asyncAfter(
            deadline: .now() + DiagnosticConstants.handshakeTimeout,
            execute: item
        )
    }

    private func scheduleHeartbeatTimeout() {
        heartbeatTimeout?.cancel()
        let item = DispatchWorkItem { [weak self] in
            self?.close(.timeout, recordFailure: false)
        }
        heartbeatTimeout = item
        queue.asyncAfter(
            deadline: .now() + DiagnosticConstants.heartbeatTimeout,
            execute: item
        )
    }

    private func close(_ failure: DiagnosticFailure, recordFailure: Bool) {
        guard phase != .closed else { return }
        phase = .closed
        handshakeTimeout?.cancel()
        heartbeatTimeout?.cancel()
        sessionID = nil
        sessionKey = nil
        transport.cancel()
        if recordFailure {
            admission.recordFailure(connectionID: connectionID, peerKey: peerKey)
        } else {
            admission.end(connectionID: connectionID)
        }
        eventHandler(.connectionClosed(connectionID, failure))
        closeHandler(connectionID)
    }
}
