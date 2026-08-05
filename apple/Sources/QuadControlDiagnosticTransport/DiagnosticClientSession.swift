import Foundation
import Network
import QuadControlDiagnosticProtocol

public enum DiagnosticClientState: Equatable, Sendable {
    case connecting
    case authenticating
    case connected
    case disconnected
    case failed(DiagnosticFailure)
}

public final class DiagnosticClientSession: @unchecked Sendable {
    private enum Phase: Equatable {
        case idle
        case awaitingChallenge
        case awaitingFinished
        case authenticated
        case closed
    }

    private let queue = DispatchQueue(label: "QuadControl.DiagnosticClient")
    private let transport: FramedConnection
    private let connectionID = secureRandomData(count: DiagnosticConstants.identifierLength)
    private let stateHandler: @Sendable (DiagnosticClientState) -> Void
    private var handshake: ClientHandshake?
    private var sessionID: Data?
    private var sessionKey: Data?
    private var phase = Phase.idle
    private var startCompletion: (@Sendable (Result<Void, DiagnosticFailure>) -> Void)?
    private var handshakeTimeout: DispatchWorkItem?
    private var heartbeatTimeout: DispatchWorkItem?
    private var heartbeatSchedule: DispatchWorkItem?
    private let outgoingSequence = HeartbeatSequence()
    private let incomingSequence = HeartbeatSequence()

    public init(
        host: String,
        port: UInt16,
        token: Data,
        stateHandler: @escaping @Sendable (DiagnosticClientState) -> Void = { _ in }
    ) throws {
        guard let allowedHost = DiagnosticAddressPolicy.allowedClientHost(host), port > 0,
              let networkPort = NWEndpoint.Port(rawValue: port) else {
            throw DiagnosticFailure.malformed
        }
        handshake = try ClientHandshake(token: token)
        transport = FramedConnection(
            connection: NWConnection(
                host: allowedHost,
                port: networkPort,
                using: .tcp
            )
        )
        self.stateHandler = stateHandler
    }

    public func start(
        completion: @escaping @Sendable (Result<Void, DiagnosticFailure>) -> Void
    ) {
        queue.async { [self] in
            guard phase == .idle else {
                completion(.failure(.state))
                return
            }
            startCompletion = completion
            phase = .awaitingChallenge
            stateHandler(.connecting)
            scheduleHandshakeTimeout()
            transport.start(queue: queue) { [weak self] state in
                self?.handleConnectionState(state)
            }
        }
    }

    public func close() {
        queue.async { [self] in
            closeInternal(state: .disconnected, initialFailure: .network)
        }
    }

    private func handleConnectionState(_ state: NWConnection.State) {
        switch state {
        case .ready:
            beginHandshake()
        case .failed:
            fail(.network)
        case .waiting:
            break
        case .cancelled:
            if phase != .closed {
                closeInternal(state: .disconnected, initialFailure: .network)
            }
        default:
            break
        }
    }

    private func beginHandshake() {
        guard phase == .awaitingChallenge,
              let handshake else {
            return
        }
        stateHandler(.authenticating)
        do {
            let wire = try DiagnosticWire.encode(
                handshake.hello,
                type: .hello,
                connectionID: connectionID
            )
            transport.send(wire) { [weak self] failure in
                guard let self else { return }
                self.queue.async {
                    if let failure {
                        self.fail(failure)
                    } else {
                        self.receiveChallenge()
                    }
                }
            }
        } catch let failure as DiagnosticFailure {
            fail(failure)
        } catch {
            fail(.malformed)
        }
    }

    private func receiveChallenge() {
        transport.receive { [weak self] result in
            guard let self else { return }
            self.queue.async {
                do {
                    guard self.phase == .awaitingChallenge,
                          let handshake = self.handshake else {
                        throw DiagnosticFailure.state
                    }
                    let envelope = try DiagnosticWire.decode(result.get())
                    guard envelope.type == .challenge,
                          envelope.connectionID == self.connectionID,
                          envelope.sequence == 0,
                          envelope.mac == nil else {
                        throw DiagnosticFailure.state
                    }
                    let challenge = try DiagnosticWire.unpack(
                        envelope,
                        as: DiagnosticChallenge.self
                    )
                    let proof = try handshake.receiveChallenge(challenge)
                    self.phase = .awaitingFinished
                    try self.sendProof(proof)
                } catch let failure as DiagnosticFailure {
                    self.fail(failure)
                } catch {
                    self.fail(.malformed)
                }
            }
        }
    }

    private func sendProof(_ proof: DiagnosticClientProof) throws {
        let wire = try DiagnosticWire.encode(
            proof,
            type: .clientProof,
            connectionID: connectionID
        )
        transport.send(wire) { [weak self] failure in
            guard let self else { return }
            self.queue.async {
                if let failure {
                    self.fail(failure)
                } else {
                    self.receiveFinished()
                }
            }
        }
    }

    private func receiveFinished() {
        transport.receive { [weak self] result in
            guard let self else { return }
            self.queue.async {
                do {
                    guard self.phase == .awaitingFinished,
                          let handshake = self.handshake else {
                        throw DiagnosticFailure.state
                    }
                    let envelope = try DiagnosticWire.decode(result.get())
                    guard envelope.type == .serverFinished,
                          envelope.connectionID == self.connectionID,
                          envelope.sequence == 0,
                          envelope.mac == nil else {
                        throw DiagnosticFailure.state
                    }
                    let finished = try DiagnosticWire.unpack(
                        envelope,
                        as: DiagnosticServerFinished.self
                    )
                    let result = try handshake.receiveFinished(finished)
                    self.sessionID = result.sessionID
                    self.sessionKey = result.sessionKey
                    self.handshake = nil
                    self.phase = .authenticated
                    self.handshakeTimeout?.cancel()
                    self.finishStart(.success(()))
                    self.stateHandler(.connected)
                    self.scheduleHeartbeat(after: 0)
                } catch let failure as DiagnosticFailure {
                    self.fail(failure)
                } catch {
                    self.fail(.malformed)
                }
            }
        }
    }

    private func scheduleHeartbeat(after delay: TimeInterval) {
        heartbeatSchedule?.cancel()
        let item = DispatchWorkItem { [weak self] in
            self?.sendHeartbeat()
        }
        heartbeatSchedule = item
        queue.asyncAfter(deadline: .now() + delay, execute: item)
    }

    private func sendHeartbeat() {
        do {
            guard phase == .authenticated,
                  let sessionID,
                  let sessionKey else {
                throw DiagnosticFailure.state
            }
            let sequence = try outgoingSequence.next()
            let message = DiagnosticHeartbeat()
            let payload = try DiagnosticWire.payload(message)
            let transcript = try DiagnosticTranscript.clientHeartbeat(
                sessionID: sessionID,
                sequence: sequence,
                status: message.status
            )
            let mac = try DiagnosticTranscript.mac(key: sessionKey, transcript: transcript)
            let wire = try DiagnosticWire.encode(
                payload: payload,
                type: .heartbeat,
                connectionID: connectionID,
                sequence: sequence,
                mac: mac
            )
            scheduleHeartbeatTimeout()
            transport.send(wire) { [weak self] failure in
                guard let self else { return }
                self.queue.async {
                    if let failure {
                        self.fail(failure)
                    } else {
                        self.awaitAck(sequence: sequence)
                    }
                }
            }
        } catch let failure as DiagnosticFailure {
            fail(failure)
        } catch {
            fail(.malformed)
        }
    }

    private func awaitAck(sequence: UInt64) {
        transport.receive { [weak self] result in
            guard let self else { return }
            self.queue.async {
                do {
                    guard self.phase == .authenticated,
                          let sessionID = self.sessionID,
                          let sessionKey = self.sessionKey else {
                        throw DiagnosticFailure.state
                    }
                    let envelope = try DiagnosticWire.decode(result.get())
                    guard envelope.type == .ack,
                          envelope.connectionID == self.connectionID,
                          envelope.sequence == sequence,
                          let mac = envelope.mac else {
                        throw DiagnosticFailure.state
                    }
                    let ack = try DiagnosticWire.unpack(envelope, as: DiagnosticAck.self)
                    try ack.validate()
                    let transcript = try DiagnosticTranscript.serverAck(
                        sessionID: sessionID,
                        sequence: sequence,
                        status: ack.status
                    )
                    guard DiagnosticTranscript.isValid(
                        mac: mac,
                        key: sessionKey,
                        transcript: transcript
                    ) else {
                        throw DiagnosticFailure.authentication
                    }
                    try self.incomingSequence.accept(sequence)
                    self.heartbeatTimeout?.cancel()
                    self.scheduleHeartbeat(after: DiagnosticConstants.heartbeatInterval)
                } catch let failure as DiagnosticFailure {
                    self.fail(failure)
                } catch {
                    self.fail(.malformed)
                }
            }
        }
    }

    private func scheduleHandshakeTimeout() {
        let item = DispatchWorkItem { [weak self] in
            self?.fail(.timeout)
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
            self?.fail(.timeout)
        }
        heartbeatTimeout = item
        queue.asyncAfter(
            deadline: .now() + DiagnosticConstants.heartbeatTimeout,
            execute: item
        )
    }

    private func fail(_ failure: DiagnosticFailure) {
        closeInternal(state: .failed(failure), initialFailure: failure)
    }

    private func closeInternal(
        state: DiagnosticClientState,
        initialFailure: DiagnosticFailure
    ) {
        guard phase != .closed else { return }
        phase = .closed
        handshakeTimeout?.cancel()
        heartbeatTimeout?.cancel()
        heartbeatSchedule?.cancel()
        handshake = nil
        sessionID = nil
        sessionKey = nil
        transport.cancel()
        finishStart(.failure(initialFailure))
        stateHandler(state)
    }

    private func finishStart(_ result: Result<Void, DiagnosticFailure>) {
        guard let completion = startCompletion else { return }
        startCompletion = nil
        completion(result)
    }
}
