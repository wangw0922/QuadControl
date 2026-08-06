import XCTest
@testable import QuadControlHIDProbe

private struct StubRunner: ProfileRunner {
    let payload: Result<Data, HIDProbeError>

    init(_ json: String) { payload = .success(Data(json.utf8)) }
    init(failure: HIDProbeError) { payload = .failure(failure) }

    func run() throws -> Data {
        switch payload {
        case let .success(data): return data
        case let .failure(error): throw error
        }
    }
}

private let realWorldOutput = """
{"SPBluetoothDataType":[{"controller_properties":{
"controller_address":"00:11:22:33:44:55",
"controller_chipset":"BCM_4387",
"controller_discoverable":"attrib_off",
"controller_state":"attrib_on",
"controller_supportedServices":"0x392039 < HFP AVRCP A2DP HID Braille LEA AACP GATT SerialPort >",
"controller_transport":"PCIe"}}]}
"""

final class ControllerParserTests: XCTestCase {
    func testParsesRealSystemProfilerOutput() throws {
        let report = try XCTUnwrap(ControllerParser.parse(Data(realWorldOutput.utf8)))
        XCTAssertEqual(report.powered, .yes)
        XCTAssertEqual(report.discoverable, .no)
        XCTAssertEqual(report.chipset, "BCM_4387")
        XCTAssertEqual(report.transport, "PCIe")
        XCTAssertEqual(report.hidProfileSupported, .yes)
        XCTAssertTrue(report.supportedServices.contains("A2DP"))
        XCTAssertEqual(report.supportedServices.count, 9)
    }

    func testControllerAddressIsNeverReported() throws {
        let report = try XCTUnwrap(ControllerParser.parse(Data(realWorldOutput.utf8)))
        let encoded = String(decoding: try JSONEncoder().encode(report), as: UTF8.self)
        XCTAssertFalse(encoded.contains("00:11:22"))
        XCTAssertFalse(encoded.lowercased().contains("address"))
    }

    func testServiceListWithoutHIDReportsNo() {
        let services = ControllerParser.supportedServices(from: "0x1 < HFP A2DP >")
        XCTAssertEqual(services, ["HFP", "A2DP"])
    }

    func testMalformedServiceStringYieldsEmptyList() {
        XCTAssertTrue(ControllerParser.supportedServices(from: "0x392039").isEmpty)
        XCTAssertTrue(ControllerParser.supportedServices(from: "> backwards <").isEmpty)
        XCTAssertTrue(ControllerParser.supportedServices(from: "").isEmpty)
    }

    func testUnreadableServiceListStaysUnknownRatherThanNo() throws {
        let json = """
        {"SPBluetoothDataType":[{"controller_properties":{"controller_state":"attrib_on"}}]}
        """
        let report = try XCTUnwrap(ControllerParser.parse(Data(json.utf8)))
        XCTAssertEqual(report.hidProfileSupported, .unknown)
        XCTAssertEqual(report.discoverable, .unknown)
    }

    func testUnknownAttributeValueIsNotCoercedToBoolean() {
        XCTAssertEqual(ControllerParser.attribute("attrib_unknown"), .unknown)
        XCTAssertEqual(ControllerParser.attribute(nil), .unknown)
    }

    func testHostWithoutControllerParsesToNil() throws {
        XCTAssertNil(try ControllerParser.parse(Data(#"{"SPBluetoothDataType":[]}"#.utf8)))
        XCTAssertNil(try ControllerParser.parse(Data("{}".utf8)))
    }

    func testMalformedJSONThrows() {
        XCTAssertThrowsError(try ControllerParser.parse(Data("not json".utf8))) { error in
            XCTAssertEqual(error as? HIDProbeError, .profileOutputMalformed)
        }
        XCTAssertThrowsError(try ControllerParser.parse(Data("[1,2,3]".utf8))) { error in
            XCTAssertEqual(error as? HIDProbeError, .profileOutputMalformed)
        }
    }
}

final class HIDProbeAssemblyTests: XCTestCase {
    func testPeripheralRoleIsAlwaysUnknown() throws {
        let result = HIDProbe.run(runner: StubRunner(realWorldOutput), osVersion: "Version 26.5.1")
        let report = try XCTUnwrap(try? result.get())
        // The whole point of the probe: a supported HID profile must never be reported
        // as "this Mac can act as a keyboard".
        XCTAssertEqual(report.controller?.hidProfileSupported, .yes)
        XCTAssertEqual(report.canActAsHIDPeripheral, .unknown)
        XCTAssertFalse(report.canActAsHIDPeripheralNote.isEmpty)
    }

    func testRunnerFailurePropagates() {
        let result = HIDProbe.run(runner: StubRunner(failure: .profileCommandTimedOut))
        XCTAssertEqual(HIDProbe.exitCode(for: result), 1)
        guard case let .failure(error) = result else { return XCTFail("expected failure") }
        XCTAssertEqual(error, .profileCommandTimedOut)
    }

    func testHostWithoutControllerStillProducesAReport() throws {
        let result = HIDProbe.run(runner: StubRunner(#"{"SPBluetoothDataType":[]}"#))
        let report = try XCTUnwrap(try? result.get())
        XCTAssertNil(report.controller)
        XCTAssertEqual(HIDProbe.exitCode(for: result), 0)
        XCTAssertTrue(report.notes.contains { $0.contains("No Bluetooth controller") })
    }

    func testReportEncodesWithSchemaKeys() throws {
        let result = HIDProbe.run(runner: StubRunner(realWorldOutput))
        let json = try XCTUnwrap(try? result.get()).json()
        XCTAssertTrue(json.contains("\"can_act_as_hid_peripheral\""))
        XCTAssertTrue(json.contains("\"hid_profile_supported\""))
        XCTAssertTrue(json.contains("\"peripheral_role_apis\""))
    }

    func testCLIRejectsUnknownArguments() {
        XCTAssertTrue(HIDProbe.argumentsAreValid([]))
        XCTAssertTrue(HIDProbe.argumentsAreValid(["--format", "json"]))
        XCTAssertFalse(HIDProbe.argumentsAreValid(["--publish-record"]))
        XCTAssertFalse(HIDProbe.argumentsAreValid(["--format", "xml"]))
    }

    func testEveryPeripheralRoleRequirementIsDescribed() {
        for requirement in PeripheralRoleAPIs.requirements {
            XCTAssertFalse(requirement.purpose.isEmpty, "\(requirement.name) has no stated purpose")
            XCTAssertTrue(requirement.name.contains(requirement.className))
        }
    }

    func testAvailabilityIsUnknownNotAbsentWhenFrameworkIsMissing() {
        let entries = PeripheralRoleAPIs.report(frameworkLoaded: false)
        XCTAssertEqual(entries.count, PeripheralRoleAPIs.requirements.count)
        XCTAssertTrue(entries.allSatisfy { $0.present == .unknown })
    }
}
