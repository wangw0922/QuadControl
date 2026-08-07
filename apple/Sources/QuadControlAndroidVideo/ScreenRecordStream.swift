import CoreGraphics
import CoreImage
import CoreVideo
import Foundation

/// A frame ready to be drawn.
///
/// `CVImageBuffer` is not `Sendable`, so the conversion to a `CGImage` happens on the
/// decode queue and this immutable box crosses to the UI instead. Doing it here also
/// keeps the expensive colour conversion off the main thread. `CGImage` is immutable
/// once created, which is what makes the unchecked conformance honest.
public struct DecodedFrame: @unchecked Sendable {
    public let image: CGImage
    public let size: CGSize

    public init(image: CGImage, size: CGSize) {
        self.image = image
        self.size = size
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
    private let context = CIContext()
    private let onFrame: @Sendable (DecodedFrame) -> Void
    private let onError: @Sendable (String) -> Void

    public init(
        onFrame: @escaping @Sendable (DecodedFrame) -> Void,
        onError: @escaping @Sendable (String) -> Void
    ) {
        self.onFrame = onFrame
        self.onError = onError
    }

    public func start(_ configuration: Configuration) throws {
        let decoder = H264Decoder { [weak self] imageBuffer in
            guard let self else { return }
            let image = CIImage(cvImageBuffer: imageBuffer)
            guard let rendered = self.context.createCGImage(image, from: image.extent) else { return }
            self.onFrame(DecodedFrame(image: rendered, size: image.extent.size))
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
        lock.unlock()
    }

    /// screenrecord keeps running on the device unless it is stopped explicitly.
    public func stop() {
        lock.lock()
        let running = process
        process = nil
        lock.unlock()
        running?.terminate()
    }

    public var presentationSize: CGSize? {
        lock.lock()
        defer { lock.unlock() }
        return decoder?.presentationSize
    }

    private func consume(_ chunk: Data) {
        lock.lock()
        let units = parser.append(chunk)
        let decoder = self.decoder
        lock.unlock()

        for unit in units {
            do {
                try decoder?.decode(unit)
            } catch {
                onError("decode: \(error)")
            }
        }
    }
}
