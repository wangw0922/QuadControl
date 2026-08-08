import Foundation

/// One H.264 NAL unit, without its Annex B start code.
public struct NalUnit: Equatable, Sendable {
    /// H.264 NAL unit types this pipeline cares about. Anything else passes through as
    /// `other` rather than being dropped, so an unexpected stream is visible instead of
    /// silently losing data.
    public enum Kind: Equatable, Sendable {
        case nonIDRSlice
        case idrSlice
        case sps
        case pps
        case accessUnitDelimiter
        case other(UInt8)

        init(rawType: UInt8) {
            switch rawType {
            case 1: self = .nonIDRSlice
            case 5: self = .idrSlice
            case 7: self = .sps
            case 8: self = .pps
            case 9: self = .accessUnitDelimiter
            default: self = .other(rawType)
            }
        }
    }

    public let kind: Kind
    /// Payload without the start code; the first byte is still the NAL header.
    public let payload: Data

    public init(kind: Kind, payload: Data) {
        self.kind = kind
        self.payload = payload
    }

    /// A slice that decoders must have parameter sets for before it means anything.
    public var isPicture: Bool {
        kind == .idrSlice || kind == .nonIDRSlice
    }
}

/// Splits an Annex B byte stream into NAL units across chunk boundaries.
///
/// `adb exec-out screenrecord --output-format=h264` writes a raw Annex B stream to a
/// pipe, so units arrive split at arbitrary points — a start code can straddle two
/// reads. This buffers whatever cannot yet be resolved and emits only complete units.
///
/// A unit is only complete once the *next* start code is seen, so the final unit stays
/// buffered until the stream ends. That is correct for live streaming: emitting a
/// truncated trailing unit would hand the decoder a corrupt slice.
public struct AnnexBParser: Sendable {
    private var buffer = Data()

    public init() {}

    public mutating func append(_ chunk: Data) -> [NalUnit] {
        buffer.append(chunk)
        return drain(flushTail: false)
    }

    /// Emits the trailing unit. Only valid once the producer has closed the stream.
    public mutating func finish() -> [NalUnit] {
        drain(flushTail: true)
    }

    private mutating func drain(flushTail: Bool) -> [NalUnit] {
        var starts: [(index: Int, codeLength: Int)] = []
        var index = buffer.startIndex

        while index < buffer.endIndex {
            guard let found = Self.nextStartCode(in: buffer, from: index) else { break }
            starts.append(found)
            index = found.index + found.codeLength
        }

        guard !starts.isEmpty else {
            // Keep at most three bytes: a start code is four bytes at most, so nothing
            // longer can still be the prefix of one.
            if !flushTail, buffer.count > 3 {
                buffer.removeFirst(buffer.count - 3)
            }
            return []
        }

        var units: [NalUnit] = []
        for (position, start) in starts.enumerated() {
            let payloadStart = start.index + start.codeLength
            let payloadEnd = position + 1 < starts.count
                ? starts[position + 1].index
                : (flushTail ? buffer.endIndex : nil)
            guard let payloadEnd else { break }
            if let unit = Self.makeUnit(buffer[payloadStart ..< payloadEnd]) {
                units.append(unit)
            }
        }

        if flushTail {
            buffer.removeAll(keepingCapacity: true)
        } else if let last = starts.last {
            buffer.removeSubrange(buffer.startIndex ..< last.index)
        }
        return units
    }

    private static func makeUnit(_ slice: Data) -> NalUnit? {
        guard let header = slice.first else { return nil }
        // Bit 7 is forbidden_zero_bit; a set bit means the stream is corrupt.
        guard header & 0x80 == 0 else { return nil }
        return NalUnit(kind: NalUnit.Kind(rawType: header & 0x1F), payload: Data(slice))
    }

    /// Annex B allows both three-byte and four-byte start codes, and screenrecord emits
    /// both, so matching only `00 00 00 01` would merge units.
    private static func nextStartCode(in data: Data, from start: Int) -> (index: Int, codeLength: Int)? {
        var index = start
        while index + 2 < data.endIndex {
            if data[index] == 0, data[index + 1] == 0 {
                if data[index + 2] == 1 {
                    return (index, 3)
                }
                if index + 3 < data.endIndex, data[index + 2] == 0, data[index + 3] == 1 {
                    return (index, 4)
                }
            }
            index += 1
        }
        return nil
    }
}
