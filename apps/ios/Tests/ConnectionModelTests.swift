import QuadControlDiagnosticProtocol
import XCTest
@testable import QuadControlIOS

@MainActor
final class ConnectionModelTests: XCTestCase {
    func testDisconnectClearsTemporaryToken() {
        let model = ConnectionModel()
        model.token = "temporary"
        model.disconnect()
        XCTAssertEqual(model.token, "")
        XCTAssertFalse(model.isSessionActive)
    }

    func testInvalidTokenIsClearedWithoutCreatingSession() {
        let model = ConnectionModel()
        model.host = "192.168.1.2"
        model.token = "short"

        model.connect()

        XCTAssertEqual(model.token, "")
        XCTAssertFalse(model.isSessionActive)
        XCTAssertFalse(model.isConnected)
        XCTAssertEqual(model.status, .invalidToken)
    }

    func testEveryStatusResolvesToNonEmptyLocalizedText() {
        let statuses: [ConnectionStatus] = [
            .notConnected, .invalidHostOrPort, .invalidToken, .connecting,
            .authenticating, .connected, .disconnected, .setupFailed,
            .failed(.network),
        ]
        for status in statuses {
            let text = status.localized
            XCTAssertFalse(text.isEmpty, "\(status) has no localized text")
            XCTAssertFalse(
                text.hasPrefix("status."),
                "\(status) fell through to its raw key — the strings file is missing an entry"
            )
        }
    }

    func testFailureStatusInterpolatesTheProtocolCode() {
        XCTAssertTrue(ConnectionStatus.failed(.timeout).localized.contains("timeout"))
    }

    // MARK: - Scanned pairing codes

    private func payloadData(host: String, port: UInt16 = 47_100) throws -> Data {
        let token = try PairingToken.encode(Data(repeating: 0x7a, count: 32))
        return try DiagnosticPairingPayload(host: host, port: port, token: token).encoded()
    }

    func testScannedPrivateHostFillsEveryField() throws {
        let model = ConnectionModel()
        model.apply(scannedPayload: try payloadData(host: "192.168.1.23", port: 47_101))

        XCTAssertEqual(model.host, "192.168.1.23")
        XCTAssertEqual(model.port, "47101")
        // The token is consumed by connect() and cleared, never left in the field.
        XCTAssertEqual(model.token, "")
        model.disconnect()
    }

    func testScannedPublicHostIsRejected() throws {
        let model = ConnectionModel()
        model.apply(scannedPayload: try payloadData(host: "8.8.8.8"))

        XCTAssertEqual(model.status, .rejectedScannedHost)
        XCTAssertFalse(model.isSessionActive)
        XCTAssertEqual(model.host, "")
        XCTAssertEqual(model.token, "")
    }

    func testScannedHostnameIsRejected() throws {
        let model = ConnectionModel()
        model.apply(scannedPayload: try payloadData(host: "listener.example.test"))

        XCTAssertEqual(model.status, .rejectedScannedHost)
        XCTAssertFalse(model.isSessionActive)
    }

    func testArbitraryQRContentIsRejected() {
        let model = ConnectionModel()
        model.apply(scannedPayload: Data("https://example.test/pair?token=abc".utf8))

        XCTAssertEqual(model.status, .invalidScannedCode)
        XCTAssertFalse(model.isSessionActive)
    }

    func testOversizedPayloadIsRejectedBeforeDecoding() {
        let model = ConnectionModel()
        model.apply(scannedPayload: Data(repeating: 0x41, count: 4096))

        XCTAssertEqual(model.status, .invalidScannedCode)
        XCTAssertFalse(model.isSessionActive)
    }
}
