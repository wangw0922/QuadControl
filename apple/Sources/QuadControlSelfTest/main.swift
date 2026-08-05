import Foundation
import QuadControlDiagnosticProtocol
import QuadControlDiagnosticServer
import QuadControlDiagnosticTransport

private final class TestRunner {
    private(set) var failures: [String] = []
    private(set) var assertions = 0

    func expect(_ condition: @autoclosure () -> Bool, _ name: String) {
        assertions += 1
        if !condition() {
            failures.append(name)
        }
    }

    func expectThrows(_ name: String, _ action: () throws -> Void) {
        assertions += 1
        do {
            try action()
            failures.append(name)
        } catch {}
    }

    func finish() -> Never {
        if failures.isEmpty {
            print("PASS: QuadControlSelfTest (\(assertions) assertions)")
            exit(0)
        }
        for failure in failures {
            fputs("FAIL: \(failure)\n", stderr)
        }
        exit(1)
    }
}

private final class LoopbackProbe: @unchecked Sendable {
    private let lock = NSLock()
    private let token: OneTimeToken
    private let server: DiagnosticServer
    private var client: DiagnosticClientSession?
    private var clientResult: Result<Void, DiagnosticFailure>?
    private var serverAuthenticated = false
    private var heartbeatObserved = false
    private let clientSemaphore = DispatchSemaphore(value: 0)
    private let heartbeatSemaphore = DispatchSemaphore(value: 0)

    init() throws {
        token = try OneTimeToken(value: Data(repeating: 0x5a, count: 32))
        server = DiagnosticServer(
            configuration: DiagnosticServerConfiguration(port: 0, mode: .loopback),
            token: token
        )
    }

    func run() throws -> Bool {
        try server.start { [weak self] event in
            self?.handle(event)
        }
        let connected = clientSemaphore.wait(timeout: .now() + 5) == .success
        let heartbeat = heartbeatSemaphore.wait(timeout: .now() + 12) == .success
        client?.close()
        server.stop()

        lock.lock()
        defer { lock.unlock() }
        guard connected,
              heartbeat,
              serverAuthenticated,
              heartbeatObserved,
              case .success? = clientResult else {
            return false
        }
        return true
    }

    private func handle(_ event: DiagnosticServerEvent) {
        switch event {
        case let .ready(port):
            do {
                let session = try DiagnosticClientSession(
                    host: "127.0.0.1",
                    port: port,
                    token: token.value
                )
                lock.lock()
                client = session
                lock.unlock()
                session.start { [weak self] result in
                    guard let self else { return }
                    self.lock.lock()
                    self.clientResult = result
                    self.lock.unlock()
                    self.clientSemaphore.signal()
                }
            } catch {
                clientSemaphore.signal()
            }
        case .authenticated:
            lock.lock()
            serverAuthenticated = true
            lock.unlock()
        case let .heartbeat(_, sequence) where sequence >= 2:
            lock.lock()
            heartbeatObserved = true
            lock.unlock()
            heartbeatSemaphore.signal()
        case .heartbeat:
            break
        case .failed:
            clientSemaphore.signal()
            heartbeatSemaphore.signal()
        default:
            break
        }
    }
}

private let runner = TestRunner()

runner.expectThrows("token must be exactly 32 bytes") {
    _ = try OneTimeToken(value: Data(repeating: 0, count: 31))
}
runner.expectThrows("empty frame rejected") {
    _ = try DiagnosticWire.frame(Data())
}
runner.expectThrows("oversized frame rejected") {
    _ = try DiagnosticWire.frame(Data(repeating: 0, count: 4_097))
}
runner.expectThrows("zero-length frame header rejected") {
    _ = try DiagnosticWire.decodeFrameLength(Data([0, 0, 0, 0]))
}
do {
    let payload = Data([1, 2, 3])
    let framed = try DiagnosticWire.frame(payload)
    let unframed = try DiagnosticWire.unframe(framed)
    runner.expect(unframed == payload, "frame round trip")
} catch {
    runner.expect(false, "frame round trip threw")
}

do {
    let tokenData = Data((0 ..< 32).map(UInt8.init))
    let display = try PairingToken.encode(tokenData)
    runner.expect(display.count == 43, "display token length")
    let decodedToken = try PairingToken.decode(display)
    runner.expect(decodedToken == tokenData, "display token round trip")
    runner.expectThrows("short display token rejected") {
        _ = try PairingToken.decode("123456")
    }

    let hello = DiagnosticHello(clientNonce: Data(repeating: 1, count: 32))
    let challenge = DiagnosticChallenge(
        serverNonce: Data(repeating: 2, count: 32),
        challengeID: Data(repeating: 3, count: 16),
        serverID: Data(repeating: 4, count: 16)
    )
    let vector = try DiagnosticTranscript.mac(
        key: tokenData,
        transcript: DiagnosticTranscript.clientProof(hello: hello, challenge: challenge)
    )
    runner.expect(
        vector.base64EncodedString() == "5ikhFFRax8k2NHL0s2DGFMYln5i9hP/g0Ct7pvAVEL0=",
        "canonical HMAC golden vector"
    )
    let sessionVector = try DiagnosticTranscript.sessionKey(
        token: tokenData,
        hello: hello,
        challenge: challenge
    )
    runner.expect(
        sessionVector.base64EncodedString() == "BabcAU5fo5YAWDIa24TWj14q0v7hHTbru2Wdmhea53k=",
        "HKDF session key golden vector"
    )

    let invalidVersion = try JSONEncoder().encode(
        DiagnosticEnvelope(
            version: 2,
            type: .hello,
            connectionID: Data(repeating: 9, count: 16),
            payload: try DiagnosticWire.payload(hello)
        )
    )
    runner.expectThrows("unknown protocol version rejected") {
        _ = try DiagnosticWire.decode(invalidVersion)
    }

    let schemaWire = try DiagnosticWire.encode(
        hello,
        type: .hello,
        connectionID: Data(repeating: 9, count: 16)
    )
    let schemaText = String(decoding: schemaWire, as: UTF8.self)
    let payloadText = String(decoding: try DiagnosticWire.payload(hello), as: UTF8.self)
    runner.expect(schemaText.contains("\"protocol_version\""), "envelope uses proto version key")
    runner.expect(schemaText.contains("\"connection_id\""), "envelope uses proto connection key")
    runner.expect(payloadText.contains("\"client_nonce\""), "payload uses proto field key")
    runner.expect(DiagnosticType.clientProof.rawValue == "client_proof", "type uses proto snake case")

    let wrongTokenServer = try ServerHandshake(
        token: OneTimeToken(value: tokenData, createdAtNanos: 1_000),
        serverID: challenge.serverID,
        nowNanos: 1_000
    )
    let wrongTokenClient = try ClientHandshake(
        token: Data(repeating: 0xff, count: 32),
        clientNonce: hello.clientNonce
    )
    let wrongChallenge = try wrongTokenServer.receiveHello(wrongTokenClient.hello, nowNanos: 1_001)
    let wrongProof = try wrongTokenClient.receiveChallenge(wrongChallenge)
    runner.expectThrows("wrong token proof rejected") {
        _ = try wrongTokenServer.receiveProof(wrongProof, nowNanos: 1_002)
    }

    let timedOutServer = try ServerHandshake(
        token: OneTimeToken(value: tokenData, createdAtNanos: 1_000),
        serverID: challenge.serverID,
        nowNanos: 1_000
    )
    runner.expectThrows("handshake deadline enforced") {
        _ = try timedOutServer.receiveHello(
            hello,
            nowNanos: 1_000 + DiagnosticConstants.handshakeTimeoutNanos + 1
        )
    }

    let oneTime = try OneTimeToken(value: tokenData, createdAtNanos: 1_000)
    let server = try ServerHandshake(
        token: oneTime,
        serverID: challenge.serverID,
        nowNanos: 1_000
    )
    let client = try ClientHandshake(token: tokenData, clientNonce: hello.clientNonce)
    let issued = try server.receiveHello(client.hello, nowNanos: 1_001)
    let proof = try client.receiveChallenge(issued)
    let result = try server.receiveProof(proof, nowNanos: 1_002)
    let clientResult = try client.receiveFinished(result.finished)
    runner.expect(clientResult.sessionKey == result.sessionKey, "mutual proof derives same session key")
    runner.expect(clientResult.sessionID == result.finished.sessionID, "server session id accepted")
    let heartbeatTranscript = try DiagnosticTranscript.clientHeartbeat(
        sessionID: clientResult.sessionID,
        sequence: 1,
        status: "alive"
    )
    var damagedHeartbeatMAC = try DiagnosticTranscript.mac(
        key: clientResult.sessionKey,
        transcript: heartbeatTranscript
    )
    damagedHeartbeatMAC[0] ^= 0xff
    runner.expect(
        !DiagnosticTranscript.isValid(
            mac: damagedHeartbeatMAC,
            key: clientResult.sessionKey,
            transcript: heartbeatTranscript
        ),
        "corrupted heartbeat MAC rejected"
    )
    let ackTranscript = try DiagnosticTranscript.serverAck(
        sessionID: clientResult.sessionID,
        sequence: 1,
        status: "ok"
    )
    let ackMAC = try DiagnosticTranscript.mac(key: clientResult.sessionKey, transcript: ackTranscript)
    runner.expect(
        !DiagnosticTranscript.isValid(
            mac: ackMAC,
            key: clientResult.sessionKey,
            transcript: heartbeatTranscript
        ),
        "ack MAC cannot authenticate as heartbeat"
    )
    runner.expectThrows("proof replay rejected") {
        _ = try server.receiveProof(proof, nowNanos: 1_003)
    }

    let expired = try OneTimeToken(value: tokenData, createdAtNanos: 0)
    runner.expectThrows("monotonic token expiry") {
        try expired.consumeIfValid(nowNanos: DiagnosticConstants.tokenTTLNanos + 1)
    }
} catch {
    runner.expect(false, "authentication vector setup threw")
}

do {
    let sequence = HeartbeatSequence()
    try sequence.accept(1)
    runner.expectThrows("duplicate sequence rejected") { try sequence.accept(1) }
    runner.expectThrows("backward sequence rejected") { try sequence.accept(0) }
    let firstOutgoing = try sequence.next()
    runner.expect(firstOutgoing == 1, "outgoing sequence starts at one")
} catch {
    runner.expect(false, "sequence setup threw")
}

let admission = DiagnosticAdmissionController()
var pending: [UUID] = []
for index in 0 ..< DiagnosticConstants.maxPending {
    let id = UUID()
    pending.append(id)
    runner.expect(
        admission.begin(connectionID: id, peerKey: "peer-\(index)", nowNanos: 1).isSuccess,
        "pending slot \(index)"
    )
}
runner.expect(
    admission.begin(connectionID: UUID(), peerKey: "overflow", nowNanos: 1).failure == .resourceLimit,
    "pending limit enforced"
)
for id in pending { admission.end(connectionID: id) }

let activeID = UUID()
runner.expect(
    admission.begin(connectionID: activeID, peerKey: "active", nowNanos: 1).isSuccess,
    "active candidate admitted"
)
runner.expect(admission.authenticationSucceeded(connectionID: activeID), "active session promoted")
runner.expect(
    admission.begin(connectionID: UUID(), peerKey: "second", nowNanos: 1).failure == .resourceLimit,
    "single active session enforced"
)
admission.end(connectionID: activeID)

for index in 0 ..< DiagnosticConstants.maxFailuresPerPeer {
    let id = UUID()
    _ = admission.begin(connectionID: id, peerKey: "limited", nowNanos: UInt64(index + 1))
    admission.recordFailure(connectionID: id, peerKey: "limited", nowNanos: UInt64(index + 1))
}
runner.expect(
    admission.begin(connectionID: UUID(), peerKey: "limited", nowNanos: 10).failure == .rateLimited,
    "peer failure rate enforced"
)

runner.expect(DiagnosticAddressPolicy.isPrivateOrLinkLocal(host: "10.0.0.1"), "RFC1918 accepted")
runner.expect(DiagnosticAddressPolicy.isPrivateOrLinkLocal(host: "169.254.1.2"), "IPv4 link-local accepted")
runner.expect(DiagnosticAddressPolicy.isPrivateOrLinkLocal(host: "fd00::1"), "IPv6 ULA accepted")
runner.expect(DiagnosticAddressPolicy.isAllowedClientHost("127.0.0.1"), "client loopback accepted")
runner.expect(!DiagnosticAddressPolicy.isAllowedClientHost("8.8.8.8"), "public client target rejected")
runner.expect(!DiagnosticAddressPolicy.isAllowedClientHost("example.test"), "client hostname rejected")
runner.expect(!DiagnosticAddressPolicy.isAllowedClientHost("[192.168.1.2]"), "bracketed IPv4 rejected")
runner.expect(!DiagnosticAddressPolicy.isAllowedClientHost("192.168.1.2%evil"), "IPv4 scope suffix rejected")
runner.expect(!DiagnosticAddressPolicy.isAllowedClientHost("fe80::1%en0"), "scoped IPv6 rejected")
runner.expect(!DiagnosticAddressPolicy.isPrivateOrLinkLocal(host: "fc-example.test"), "hostname prefix rejected")
runner.expect(
    DiagnosticAddressPolicy.peerKey(.hostPort(host: "192.168.1.2", port: 1000))
        == DiagnosticAddressPolicy.peerKey(.hostPort(host: "192.168.1.2", port: 2000)),
    "peer rate-limit key ignores source port"
)

do {
    let loopbackPassed = try LoopbackProbe().run()
    runner.expect(loopbackPassed, "real loopback auth, heartbeat, ack, and disconnect")
} catch {
    runner.expect(false, "loopback probe setup threw")
}

runner.finish()

private extension Result where Success == Void, Failure == DiagnosticFailure {
    var isSuccess: Bool {
        if case .success = self { return true }
        return false
    }

    var failure: DiagnosticFailure? {
        if case let .failure(value) = self { return value }
        return nil
    }
}
