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
        XCTAssertEqual(model.status, "Temporary token must be the 43-character value shown by the Mac")
    }
}
