#if canImport(XCTest)
import XCTest
@testable import QuadControlDiagnosticProtocol

final class DiagnosticProtocolTests: XCTestCase {
    func testStrictTokenAndFrameBounds() throws {
        XCTAssertThrowsError(try OneTimeToken(value: Data(repeating: 0, count: 31)))
        XCTAssertThrowsError(try DiagnosticWire.frame(Data()))
        XCTAssertThrowsError(try DiagnosticWire.frame(Data(repeating: 0, count: 4_097)))
        let payload = Data([1, 2])
        XCTAssertEqual(try DiagnosticWire.unframe(DiagnosticWire.frame(payload)), payload)
    }

    func testAuthenticationAndAtomicConsumption() throws {
        let token = try OneTimeToken(value: Data((0 ..< 32).map(UInt8.init)), createdAtNanos: 100)
        let client = try ClientHandshake(token: token.value, clientNonce: Data(repeating: 1, count: 32))
        let server = try ServerHandshake(
            token: token,
            serverID: Data(repeating: 2, count: 16),
            nowNanos: 100
        )
        let challenge = try server.receiveHello(client.hello, nowNanos: 101)
        let proof = try client.receiveChallenge(challenge)
        let result = try server.receiveProof(proof, nowNanos: 102)
        XCTAssertEqual(try client.receiveFinished(result.finished).sessionKey, result.sessionKey)
        XCTAssertThrowsError(try server.receiveProof(proof, nowNanos: 103))
    }

    func testWrongTokenAndSequenceReplay() throws {
        let token = try OneTimeToken(value: Data(repeating: 3, count: 32))
        let good = try ClientHandshake(token: token.value)
        let bad = try ClientHandshake(token: Data(repeating: 4, count: 32))
        let server = try ServerHandshake(token: token)
        let challenge = try server.receiveHello(good.hello)
        XCTAssertThrowsError(try server.receiveProof(bad.receiveChallenge(challenge)))

        let sequence = HeartbeatSequence()
        try sequence.accept(1)
        XCTAssertThrowsError(try sequence.accept(1))
        XCTAssertThrowsError(try sequence.accept(0))
    }

    func testTokenUsesMonotonicExpiry() throws {
        let token = try OneTimeToken(value: Data(repeating: 0, count: 32), createdAtNanos: 0)
        XCTAssertThrowsError(
            try token.consumeIfValid(nowNanos: DiagnosticConstants.tokenTTLNanos + 1)
        )
    }
}
#endif
