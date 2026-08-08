import CoreGraphics
import CoreVideo
import Foundation

/// A frame ready to be drawn.
///
/// `CVImageBuffer` is not `Sendable`, so this immutable box crosses to the UI instead.
/// VideoToolbox decodes into an IOSurface-backed pixel buffer, and CALayer can display
/// an IOSurface directly.
///
/// `CVPixelBuffer` is a CoreFoundation type and not `Sendable`; the buffer is treated as
/// immutable from here on, which is what makes the unchecked conformance honest.
public struct DecodedFrame: @unchecked Sendable {
    public let pixelBuffer: CVPixelBuffer
    public let size: CGSize

    public init(pixelBuffer: CVPixelBuffer, size: CGSize) {
        self.pixelBuffer = pixelBuffer
        self.size = size
    }

    /// The surface CALayer can show without any conversion.
    public var surface: IOSurface? {
        CVPixelBufferGetIOSurface(pixelBuffer)?.takeUnretainedValue()
    }
}

/// Runs `adb exec-out screenrecord` and turns its stdout into decoded frames.
///
/// Parsing and decoding stay off the main thread; only finished frames are handed
/// onward. Keeping this separate from the window means the whole capture path can be
/// reasoned about — and later reused by a different front end — without any UI type.
public final class ScreenRecordStream: @unchecked Sendable {
    public struct Configuration: Sendable {
        public var width: Int
        public var height: Int
        public var bitRate: String
        public var serial: String?

        public init(width: Int = 1280, height: Int = 2560, bitRate: String = "8M", serial: String? = nil) {
            self.width = width
            self.height = height
            self.bitRate = bitRate
            self.serial = serial
        }

        var adbArguments: [String] {
            var arguments = ["adb"]
            if let serial {
                arguments += ["-s", serial]
            }
            arguments += [
                "exec-out", "screenrecord",
                "--output-format=h264",
                // Without this the device stops after 180 seconds and the picture just
                // freezes with no error. 0 is the documented way to remove the limit.
                "--time-limit", "0",
                "--size", "\(width)x\(height)",
                "--bit-rate", bitRate,
                "-",
            ]
            return arguments
        }
    }

    private let lock = NSLock()
    private var parser = AnnexBParser()
    private var decoder: H264Decoder?
    private var process: Process?
    private var activeSerial: String?
    private let onFrame: @Sendable (DecodedFrame) -> Void
    private let onError: @Sendable (String) -> Void

    public init(
        onFrame: @escaping @Sendable (DecodedFrame) -> Void,
        onError: @escaping @Sendable (String) -> Void
    ) {
        self.onFrame = onFrame
        self.onError = onError
    }

    /// Tears the stream down and brings it back at a new size.
    ///
    /// `screenrecord` fixes its frame size when it starts, so a rotation cannot be
    /// followed by reconfiguring — the process has to be replaced. The parser and
    /// decoder are rebuilt too: the new stream begins with its own parameter sets, and
    /// feeding a half-parsed unit from the old one across the boundary would corrupt the
    /// first frames.
    public func restart(_ configuration: Configuration) throws {
        stop()
        lock.lock()
        parser = AnnexBParser()
        decoder = nil
        lock.unlock()
        try start(configuration)
    }

    public func start(_ configuration: Configuration) throws {
        // A previous run that was killed rather than closed leaves a recorder behind, and
        // a second encoder alongside it starves both.
        Self.killOrphanedRecorders(serial: configuration.serial)

        let decoder = H264Decoder { [weak self] imageBuffer in
            guard let self else { return }
            let size = CGSize(
                width: CVPixelBufferGetWidth(imageBuffer),
                height: CVPixelBufferGetHeight(imageBuffer)
            )
            self.onFrame(DecodedFrame(pixelBuffer: imageBuffer, size: size))
        }
        lock.lock()
        self.decoder = decoder
        lock.unlock()

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = configuration.adbArguments

        let output = Pipe()
        process.standardOutput = output
        let errors = Pipe()
        process.standardError = errors

        output.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            guard !chunk.isEmpty else { return }
            self?.consume(chunk)
        }
        // screenrecord reports device-side problems here — an unsupported size, or a
        // secure surface it refuses to record.
        errors.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            guard !chunk.isEmpty,
                  let text = String(data: chunk, encoding: .utf8)?
                      .trimmingCharacters(in: .whitespacesAndNewlines),
                  !text.isEmpty else { return }
            self?.onError("screenrecord: \(text)")
        }
        process.terminationHandler = { [weak self] finished in
            guard finished.terminationStatus != 0 else { return }
            self?.onError("screenrecord exited with status \(finished.terminationStatus)")
        }

        try process.run()
        lock.lock()
        self.process = process
        self.activeSerial = configuration.serial
        lock.unlock()
    }

    /// screenrecord keeps running on the device unless it is stopped explicitly.
    ///
    /// Terminating the local `adb` process is not enough on its own, and a hard kill of
    /// this app skips it entirely — orphans then pile up on the phone. Four of them
    /// running at once were measured competing for the hardware encoder and starving the
    /// stream to 0–4 fps, which looked exactly like a bandwidth problem and sent an
    /// investigation down the wrong path. So the device is swept as well.
    public func stop() {
        lock.lock()
        let running = process
        let serial = activeSerial
        process = nil
        lock.unlock()
        running?.terminate()
        Self.killOrphanedRecorders(serial: serial)
    }

    /// Kills any `screenrecord` the device is still running for us.
    ///
    /// Safe to call before starting too: a previous run that was killed rather than
    /// closed leaves one behind, and starting a second encoder alongside it is what
    /// causes the starvation described above.
    public static func killOrphanedRecorders(serial: String?) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        var arguments = ["adb"]
        if let serial { arguments += ["-s", serial] }
        arguments += ["shell", "pkill", "-f", "screenrecord"]
        process.arguments = arguments
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try? process.run()
        process.waitUntilExit()
    }

    public var presentationSize: CGSize? {
        lock.lock()
        defer { lock.unlock() }
        return decoder?.presentationSize
    }

    private func consume(_ chunk: Data) {
        let arrivedAt = DispatchTime.now().uptimeNanoseconds
        lock.lock()
        let units = parser.append(chunk)
        let decoder = self.decoder
        lock.unlock()
        let parsedAt = DispatchTime.now().uptimeNanoseconds

        for unit in units {
            do {
                try decoder?.decode(unit)
            } catch {
                onError("decode: \(error)")
            }
        }
        let decodedAt = DispatchTime.now().uptimeNanoseconds
        recordStage(parseNanos: parsedAt - arrivedAt, decodeNanos: decodedAt - parsedAt)
    }

    // MARK: - Stage timing
    //
    // Guessing at where latency lives already cost one wrong diagnosis, so each stage is
    // measured instead. Reported as medians: a single slow frame says less than the
    // typical one.
    private var parseSamples: [UInt64] = []
    private var decodeSamples: [UInt64] = []
    private var lastStageReport = DispatchTime.now().uptimeNanoseconds

    private func recordStage(parseNanos: UInt64 = 0, decodeNanos: UInt64 = 0) {
        lock.lock()
        if parseNanos > 0 { parseSamples.append(parseNanos) }
        if decodeNanos > 0 { decodeSamples.append(decodeNanos) }
        let now = DispatchTime.now().uptimeNanoseconds
        guard now - lastStageReport > 5_000_000_000, (!parseSamples.isEmpty || !decodeSamples.isEmpty) else {
            lock.unlock()
            return
        }
        lastStageReport = now
        func median(_ values: [UInt64]) -> Double {
            guard !values.isEmpty else { return 0 }
            return Double(values.sorted()[values.count / 2]) / 1_000_000
        }
        let summary = String(
            format: "  stage medians: parse %.2f ms, decode-submit %.2f ms (n=%d)",
            median(parseSamples), median(decodeSamples), max(parseSamples.count, decodeSamples.count)
        )
        parseSamples.removeAll(); decodeSamples.removeAll()
        lock.unlock()
        onError(summary)
    }
}
