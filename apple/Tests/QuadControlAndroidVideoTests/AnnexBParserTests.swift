import CoreMedia
import XCTest
@testable import QuadControlAndroidVideo

/// Captured from `adb exec-out screenrecord --output-format=h264` on a Samsung
/// SM-S9180 running Android 16. These are codec parameter sets only — no picture data,
/// so nothing from anyone's screen is stored here.
private enum RealStream {
    static let sps = Data([
        0x67, 0x64, 0x00, 0x1f, 0xac, 0xb4, 0x05, 0x01, 0x3f,
        0xc9, 0x35, 0x06, 0x06, 0x05, 0x06, 0xd0, 0xa1, 0x35,
    ])
    static let pps = Data([0x68, 0xee, 0x06, 0xf2, 0xc0])
}

private let fourByteStart = Data([0x00, 0x00, 0x00, 0x01])
private let threeByteStart = Data([0x00, 0x00, 0x01])

final class AnnexBParserTests: XCTestCase {
    func testSplitsRealParameterSets() {
        var parser = AnnexBParser()
        var stream = Data()
        stream.append(fourByteStart)
        stream.append(RealStream.sps)
        stream.append(fourByteStart)
        stream.append(RealStream.pps)
        stream.append(fourByteStart)
        stream.append(Data([0x65, 0x88, 0x84]))

        let units = parser.append(stream)

        XCTAssertEqual(units.count, 2, "the trailing unit is not complete until the next start code")
        XCTAssertEqual(units[0].kind, .sps)
        XCTAssertEqual(units[0].payload, RealStream.sps)
        XCTAssertEqual(units[1].kind, .pps)
        XCTAssertEqual(units[1].payload, RealStream.pps)
    }

    func testEmitsTrailingUnitOnlyOnFinish() {
        var parser = AnnexBParser()
        _ = parser.append(fourByteStart + RealStream.sps)

        let flushed = parser.finish()

        XCTAssertEqual(flushed.count, 1)
        XCTAssertEqual(flushed[0].kind, .sps)
    }

    /// A pipe splits wherever it likes, including through a start code. This is the
    /// failure mode that would corrupt frames if the parser held no state.
    func testStartCodeSplitAcrossChunks() {
        var whole = AnnexBParser()
        var stream = Data()
        stream.append(fourByteStart)
        stream.append(RealStream.sps)
        stream.append(fourByteStart)
        stream.append(RealStream.pps)
        stream.append(fourByteStart)
        let expected = whole.append(stream)
        XCTAssertEqual(expected.count, 2)

        for splitPoint in 1 ..< stream.count {
            var parser = AnnexBParser()
            var units = parser.append(stream.prefix(splitPoint))
            units += parser.append(stream.suffix(from: splitPoint))
            XCTAssertEqual(units, expected, "split at byte \(splitPoint) changed the result")
        }
    }

    func testByteAtATimeMatchesWholeStream() {
        var stream = Data()
        stream.append(threeByteStart)
        stream.append(RealStream.sps)
        stream.append(fourByteStart)
        stream.append(RealStream.pps)
        stream.append(threeByteStart)

        var reference = AnnexBParser()
        let expected = reference.append(stream)

        var drip = AnnexBParser()
        var units: [NalUnit] = []
        for byte in stream {
            units += drip.append(Data([byte]))
        }
        XCTAssertEqual(units, expected)
    }

    /// screenrecord emits both start-code lengths; treating only the four-byte form as a
    /// boundary would silently merge two units into one corrupt slice.
    func testHandlesThreeAndFourByteStartCodes() {
        var parser = AnnexBParser()
        var stream = Data()
        stream.append(threeByteStart)
        stream.append(RealStream.sps)
        stream.append(fourByteStart)
        stream.append(RealStream.pps)
        stream.append(threeByteStart)

        let units = parser.append(stream)

        XCTAssertEqual(units.map(\.kind), [.sps, .pps])
        XCTAssertEqual(units[0].payload, RealStream.sps, "three-byte code must not leak a byte into the payload")
    }

    func testClassifiesSliceTypes() {
        var parser = AnnexBParser()
        var stream = Data()
        for header in [UInt8(0x65), UInt8(0x41), UInt8(0x09), UInt8(0x06)] {
            stream.append(fourByteStart)
            stream.append(header)
        }
        stream.append(fourByteStart)

        let units = parser.append(stream)

        XCTAssertEqual(units.map(\.kind), [.idrSlice, .nonIDRSlice, .accessUnitDelimiter, .other(6)])
        XCTAssertTrue(units[0].isPicture)
        XCTAssertTrue(units[1].isPicture)
        XCTAssertFalse(units[2].isPicture)
    }

    func testRejectsUnitWithForbiddenBitSet() {
        var parser = AnnexBParser()
        var stream = Data()
        stream.append(fourByteStart)
        stream.append(Data([0x87]))  // forbidden_zero_bit set
        stream.append(fourByteStart)

        XCTAssertTrue(parser.append(stream).isEmpty)
    }

    func testGarbageWithoutStartCodesIsBounded() {
        var parser = AnnexBParser()
        for _ in 0 ..< 200 {
            XCTAssertTrue(parser.append(Data(repeating: 0xAB, count: 1024)).isEmpty)
        }
        // Nothing resolvable arrived, so nothing may be emitted — and the parser must not
        // have grown without bound while waiting.
        XCTAssertTrue(parser.finish().count <= 1)
    }
}

final class H264SampleFramingTests: XCTestCase {
    /// VideoToolbox is configured with a four-byte NAL length, so the prefix this writes
    /// has to be exactly four bytes big-endian or every frame is misparsed.
    func testAvccPrefixIsFourByteBigEndianLength() {
        let payload = Data([0x65, 0x01, 0x02])
        let sample = H264Decoder.avccSample(payload)

        XCTAssertEqual(sample.count, payload.count + 4)
        XCTAssertEqual(Array(sample.prefix(4)), [0x00, 0x00, 0x00, 0x03])
        XCTAssertEqual(sample.suffix(from: 4), payload)
    }

    func testRealParameterSetsProduceAFormatDescription() throws {
        var description: CMVideoFormatDescription?
        let status: OSStatus = RealStream.sps.withUnsafeBytes { spsBytes in
            RealStream.pps.withUnsafeBytes { ppsBytes in
                let pointers = [
                    spsBytes.bindMemory(to: UInt8.self).baseAddress!,
                    ppsBytes.bindMemory(to: UInt8.self).baseAddress!,
                ]
                let sizes = [RealStream.sps.count, RealStream.pps.count]
                return CMVideoFormatDescriptionCreateFromH264ParameterSets(
                    allocator: kCFAllocatorDefault,
                    parameterSetCount: 2,
                    parameterSetPointers: pointers,
                    parameterSetSizes: sizes,
                    nalUnitHeaderLength: 4,
                    formatDescriptionOut: &description
                )
            }
        }

        XCTAssertEqual(status, noErr)
        let dimensions = try CMVideoFormatDescriptionGetDimensions(XCTUnwrap(description))
        // The stream was requested at 640x298; the encoder rounds to macroblocks.
        XCTAssertEqual(dimensions.width, 640)
        XCTAssertGreaterThanOrEqual(dimensions.height, 288)
        XCTAssertLessThanOrEqual(dimensions.height, 304)
    }
}
