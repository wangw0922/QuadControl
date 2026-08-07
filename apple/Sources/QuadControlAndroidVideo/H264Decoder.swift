import CoreMedia
import Foundation
import VideoToolbox

public enum VideoDecodeFailure: String, Error, Equatable, Sendable {
    case missingParameterSets = "an picture arrived before SPS and PPS"
    case formatDescription = "could not build a format description from SPS/PPS"
    case sessionCreation = "could not create a decompression session"
    case sampleBuffer = "could not wrap the NAL unit in a sample buffer"
    case decodeFailed = "VideoToolbox rejected the frame"
}

/// Decodes the H.264 that `screenrecord` produces, using VideoToolbox.
///
/// VideoToolbox ships with macOS, so this adds no third-party dependency and decodes on
/// the GPU. It wants AVCC-framed samples plus a format description built from the
/// parameter sets, while Annex B carries start codes inline — the conversion between
/// the two is most of what this type does.
public final class H264Decoder {
    public typealias FrameHandler = @Sendable (CVImageBuffer) -> Void

    private var sps: Data?
    private var pps: Data?
    private var formatDescription: CMVideoFormatDescription?
    private var session: VTDecompressionSession?
    private let onFrame: FrameHandler

    public init(onFrame: @escaping FrameHandler) {
        self.onFrame = onFrame
    }

    deinit {
        if let session {
            VTDecompressionSessionInvalidate(session)
        }
    }

    /// Feeds one unit. Parameter sets are retained and applied; pictures are decoded.
    ///
    /// Pictures arriving before the parameter sets are dropped rather than treated as an
    /// error: a live stream can be joined mid-flight, and screenrecord repeats SPS/PPS
    /// before every keyframe, so the picture simply has to wait for the next one.
    public func decode(_ unit: NalUnit) throws {
        switch unit.kind {
        case .sps:
            if sps != unit.payload {
                sps = unit.payload
                try invalidateSession()
            }
        case .pps:
            if pps != unit.payload {
                pps = unit.payload
                try invalidateSession()
            }
        case .idrSlice, .nonIDRSlice:
            guard sps != nil, pps != nil else { return }
            try decodePicture(unit)
        case .accessUnitDelimiter, .other:
            break
        }
    }

    /// Dimensions become known once the parameter sets have been seen.
    public var presentationSize: CGSize? {
        formatDescription.map { description in
            let dimensions = CMVideoFormatDescriptionGetDimensions(description)
            return CGSize(width: Int(dimensions.width), height: Int(dimensions.height))
        }
    }

    private func invalidateSession() throws {
        if let session {
            VTDecompressionSessionInvalidate(session)
        }
        session = nil
        formatDescription = nil
    }

    private func ensureSession() throws {
        if session != nil, formatDescription != nil { return }
        guard let sps, let pps else { throw VideoDecodeFailure.missingParameterSets }

        var description: CMVideoFormatDescription?
        let created: OSStatus = sps.withUnsafeBytes { spsBytes in
            pps.withUnsafeBytes { ppsBytes in
                let pointers = [
                    spsBytes.bindMemory(to: UInt8.self).baseAddress!,
                    ppsBytes.bindMemory(to: UInt8.self).baseAddress!,
                ]
                let sizes = [sps.count, pps.count]
                return CMVideoFormatDescriptionCreateFromH264ParameterSets(
                    allocator: kCFAllocatorDefault,
                    parameterSetCount: 2,
                    parameterSetPointers: pointers,
                    parameterSetSizes: sizes,
                    // screenrecord uses four-byte lengths; this must match what
                    // `avccSample` writes below or every frame is misparsed.
                    nalUnitHeaderLength: 4,
                    formatDescriptionOut: &description
                )
            }
        }
        guard created == noErr, let description else {
            throw VideoDecodeFailure.formatDescription
        }
        formatDescription = description

        var callback = VTDecompressionOutputCallbackRecord(
            decompressionOutputCallback: { context, _, status, _, imageBuffer, _, _ in
                guard status == noErr, let imageBuffer, let context else { return }
                let decoder = Unmanaged<H264Decoder>.fromOpaque(context).takeUnretainedValue()
                decoder.onFrame(imageBuffer)
            },
            decompressionOutputRefCon: Unmanaged.passUnretained(self).toOpaque()
        )

        var newSession: VTDecompressionSession?
        let sessionStatus = VTDecompressionSessionCreate(
            allocator: kCFAllocatorDefault,
            formatDescription: description,
            decoderSpecification: nil,
            imageBufferAttributes: [
                kCVPixelBufferPixelFormatTypeKey as String:
                    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
                kCVPixelBufferMetalCompatibilityKey as String: true,
            ] as CFDictionary,
            outputCallback: &callback,
            decompressionSessionOut: &newSession
        )
        guard sessionStatus == noErr, let newSession else {
            throw VideoDecodeFailure.sessionCreation
        }
        session = newSession
    }

    private func decodePicture(_ unit: NalUnit) throws {
        try ensureSession()
        guard let session, let formatDescription else { throw VideoDecodeFailure.sessionCreation }

        let sample = Self.avccSample(unit.payload)
        // The block buffer must own memory that outlives this call, and `Data` is not a
        // stable buffer to hand out a raw pointer to. Malloc a copy and let CoreMedia
        // free it via kCFAllocatorMalloc.
        guard let memory = malloc(sample.count) else { throw VideoDecodeFailure.sampleBuffer }
        sample.copyBytes(to: memory.assumingMemoryBound(to: UInt8.self), count: sample.count)

        var blockBuffer: CMBlockBuffer?
        let blockStatus = CMBlockBufferCreateWithMemoryBlock(
            allocator: kCFAllocatorDefault,
            memoryBlock: memory,
            blockLength: sample.count,
            blockAllocator: kCFAllocatorMalloc,
            customBlockSource: nil,
            offsetToData: 0,
            dataLength: sample.count,
            flags: 0,
            blockBufferOut: &blockBuffer
        )
        guard blockStatus == kCMBlockBufferNoErr, let blockBuffer else {
            free(memory)
            throw VideoDecodeFailure.sampleBuffer
        }

        var sampleBuffer: CMSampleBuffer?
        var sampleSize = sample.count
        let sampleStatus = CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault,
            dataBuffer: blockBuffer,
            formatDescription: formatDescription,
            sampleCount: 1,
            sampleTimingEntryCount: 0,
            sampleTimingArray: nil,
            sampleSizeEntryCount: 1,
            sampleSizeArray: &sampleSize,
            sampleBufferOut: &sampleBuffer
        )
        guard sampleStatus == noErr, let sampleBuffer else {
            throw VideoDecodeFailure.sampleBuffer
        }

        var flags = VTDecodeInfoFlags()
        let decodeStatus = VTDecompressionSessionDecodeFrame(
            session,
            sampleBuffer: sampleBuffer,
            // Live mirroring wants the newest frame, not a reordered-perfect one.
            flags: [._EnableAsynchronousDecompression, ._EnableTemporalProcessing],
            frameRefcon: nil,
            infoFlagsOut: &flags
        )
        guard decodeStatus == noErr else { throw VideoDecodeFailure.decodeFailed }
    }

    /// Annex B marks unit boundaries with start codes; AVCC prefixes each unit with its
    /// big-endian length. VideoToolbox only accepts the latter.
    static func avccSample(_ payload: Data) -> Data {
        var sample = Data(capacity: payload.count + 4)
        let length = UInt32(payload.count).bigEndian
        withUnsafeBytes(of: length) { sample.append(contentsOf: $0) }
        sample.append(payload)
        return sample
    }
}
