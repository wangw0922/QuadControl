import XCTest
@testable import QuadControlAndroidVideo

/// Trimmed from real `adb shell dumpsys window displays` output on a Samsung SM-S9180
/// held sideways: `init` stays in the natural orientation while `cur` follows rotation.
private let rotatedDumpsys = """
Display: mDisplayId=0 mLayerStack=0
  init=1440x3088 320dpi cur=3088x1440 app=3088x1440 rng=1440x1287-3088x2935
  deferred=false mLayoutSeq=812
"""

final class AndroidDisplayTests: XCTestCase {
    func testUsesRotatedCurrentSizeNotNaturalSize() throws {
        let display = try XCTUnwrap(AndroidDisplay.parse(rotatedDumpsys))

        // Reading `init` here — which is what `wm size` reports — would transpose every
        // tap whenever the phone is sideways.
        XCTAssertEqual(display.currentSize, CGSize(width: 3088, height: 1440))
        XCTAssertEqual(display.naturalSize, CGSize(width: 1440, height: 3088))
    }

    func testRejectsOutputWithoutCurrentSize() {
        XCTAssertNil(AndroidDisplay.parse("Display: mDisplayId=0\n  init=1440x3088"))
        XCTAssertNil(AndroidDisplay.parse(""))
    }

    func testStreamSizeKeepsAspectRatioSoNothingIsLetterboxed() throws {
        let display = try XCTUnwrap(AndroidDisplay.parse(rotatedDumpsys))

        let stream = display.streamSize(maxDimension: 1280)

        XCTAssertEqual(max(stream.width, stream.height), 1280)
        let sourceAspect = display.currentSize.width / display.currentSize.height
        let streamAspect = stream.width / stream.height
        XCTAssertEqual(streamAspect, sourceAspect, accuracy: 0.01, "letterboxing would offset every tap")
        // H.264 needs even dimensions.
        XCTAssertEqual(Int(stream.width) % 2, 0)
        XCTAssertEqual(Int(stream.height) % 2, 0)
    }

    func testStreamSizeLeavesSmallDisplaysAlone() throws {
        let display = try XCTUnwrap(AndroidDisplay.parse(rotatedDumpsys))
        XCTAssertEqual(display.streamSize(maxDimension: 4000), display.currentSize)
    }

    func testMapsCornersAndCentreOfThePicture() throws {
        let display = try XCTUnwrap(AndroidDisplay.parse(rotatedDumpsys))
        // A picture inset inside a larger view, as `resizeAspect` produces.
        let picture = CGRect(x: 10, y: 20, width: 400, height: 200)

        XCTAssertEqual(
            display.devicePoint(fromViewPoint: CGPoint(x: 10, y: 20), pictureRect: picture),
            CGPoint(x: 0, y: 0)
        )
        XCTAssertEqual(
            display.devicePoint(fromViewPoint: CGPoint(x: 210, y: 120), pictureRect: picture),
            CGPoint(x: 1544, y: 720)
        )
        // The far edge maps onto the last valid pixel, not one past it — the device
        // rejects a tap at 3088 on a 3088-wide display.
        XCTAssertEqual(
            display.devicePoint(fromViewPoint: CGPoint(x: 410, y: 220), pictureRect: picture),
            CGPoint(x: 3087, y: 1439)
        )
    }

    /// Clicks on the letterbox bars are not on the phone at all, so they must not be
    /// clamped onto an edge — that would fire a real tap the user did not aim at.
    func testPointsOutsideThePictureAreRejected() throws {
        let display = try XCTUnwrap(AndroidDisplay.parse(rotatedDumpsys))
        let picture = CGRect(x: 10, y: 20, width: 400, height: 200)

        XCTAssertNil(display.devicePoint(fromViewPoint: CGPoint(x: 5, y: 120), pictureRect: picture))
        XCTAssertNil(display.devicePoint(fromViewPoint: CGPoint(x: 210, y: 500), pictureRect: picture))
    }
}

final class AndroidInputEscapingTests: XCTestCase {
    /// `input text` splits on spaces, so a literal space would become a second argument
    /// and be lost. `%s` is its documented escape.
    func testSpacesBecomePercentS() {
        XCTAssertEqual(AndroidInput.shellQuoted("hello world"), "'hello%sworld'")
    }

    func testSingleQuotesAreClosedAndReopened() {
        XCTAssertEqual(AndroidInput.shellQuoted("it's"), "'it'\\''s'")
    }

    func testShellMetacharactersStayInsideQuotes() {
        // The point of quoting: none of this may reach the device shell as syntax.
        for dangerous in ["a;rm -rf /", "$(id)", "`id`", "a && b", "a|b", "a>b"] {
            let quoted = AndroidInput.shellQuoted(dangerous)
            XCTAssertTrue(quoted.hasPrefix("'"))
            XCTAssertTrue(quoted.hasSuffix("'"))
            // Every embedded quote is escaped, so the string cannot terminate early.
            let interior = quoted.dropFirst().dropLast()
            XCTAssertFalse(interior.contains("'") && !quoted.contains("'\\''"))
        }
    }

    func testKeyCodesAreNamedNotNumeric() {
        // A numeric typo silently presses a different key; a named one fails loudly.
        for key in AndroidKey.allCases {
            XCTAssertTrue(key.rawValue.hasPrefix("KEYCODE_"), "\(key) is not a named key code")
        }
        XCTAssertEqual(AndroidKey.back.rawValue, "KEYCODE_BACK")
        XCTAssertEqual(AndroidKey.appSwitch.rawValue, "KEYCODE_APP_SWITCH")
    }
}

final class LinkProbeTests: XCTestCase {
    /// The lag was queueing: the encoder outran the link and TCP buffered the difference.
    /// Spending only a fraction of measured capacity is what keeps that queue empty.
    func testSpendsOnlyAFractionOfMeasuredCapacity() {
        // The link measured on real hardware.
        let recommended = LinkProbe.recommendedBitRate(capacityBitsPerSecond: 8_900_000)
        let bits = Int(recommended) ?? 0

        XCTAssertEqual(Double(bits), 8_900_000 * LinkProbe.targetUtilisation, accuracy: 1)
        // 6 Mbps on this link was visibly laggy; 1.5 Mbps was not.
        XCTAssertLessThan(bits, 6_000_000)
    }

    /// Guessing high when the probe fails would reintroduce the exact problem this
    /// exists to avoid, so an unknown link is treated as a slow one.
    func testUnknownCapacityFallsBackToAConservativeRate() {
        let bits = Int(LinkProbe.recommendedBitRate(capacityBitsPerSecond: nil)) ?? 0
        XCTAssertLessThanOrEqual(bits, 2_000_000)
        XCTAssertGreaterThanOrEqual(bits, LinkProbe.minimumBitsPerSecond)
    }

    func testStaysWithinBoundsForVerySlowAndVeryFastLinks() {
        let slow = Int(LinkProbe.recommendedBitRate(capacityBitsPerSecond: 200_000)) ?? 0
        XCTAssertEqual(slow, LinkProbe.minimumBitsPerSecond, "an unusably low rate helps nobody")

        let fast = Int(LinkProbe.recommendedBitRate(capacityBitsPerSecond: 900_000_000)) ?? 0
        XCTAssertEqual(fast, LinkProbe.maximumBitsPerSecond, "more bitrate stops buying quality")
    }

    func testRateIsPlainDigitsForAdb() {
        // `screenrecord` accepts "4M", but a computed value must not be emitted in
        // scientific notation or with a decimal point.
        let rate = LinkProbe.recommendedBitRate(capacityBitsPerSecond: 8_900_000)
        XCTAssertTrue(rate.allSatisfy(\.isNumber), "adb would reject \(rate)")
    }

    func testProbeRejectsShortOutput() {
        XCTAssertFalse(LinkProbe.probeSucceeded(
            receivedBytes: 8 * 1_048_576 - 1,
            expectedBytes: 8 * 1_048_576,
            terminationStatus: 0,
            elapsed: 1
        ))
    }

    func testProbeRejectsNonZeroExitStatus() {
        XCTAssertFalse(LinkProbe.probeSucceeded(
            receivedBytes: 8 * 1_048_576,
            expectedBytes: 8 * 1_048_576,
            terminationStatus: 1,
            elapsed: 1
        ))
    }
}
