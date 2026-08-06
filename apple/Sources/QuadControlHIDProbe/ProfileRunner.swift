import Foundation

/// Injectable so the probe's assembly can be tested without running a subprocess.
public protocol ProfileRunner: Sendable {
    func run() throws -> Data
}

/// Runs the one whitelisted read-only command with a timeout and an output bound,
/// matching the ADB probe's contract. `system_profiler` is a reporting tool: it reads
/// system state and changes nothing.
public struct SystemProfileRunner: ProfileRunner {
    public static let executable = "/usr/sbin/system_profiler"
    public static let arguments = ["SPBluetoothDataType", "-json"]
    public static let timeout: TimeInterval = 15
    public static let outputLimit = 1_048_576

    public init() {}

    public func run() throws -> Data {
        guard FileManager.default.isExecutableFile(atPath: Self.executable) else {
            throw HIDProbeError.profileCommandMissing
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: Self.executable)
        process.arguments = Self.arguments
        let stdout = Pipe()
        process.standardOutput = stdout
        process.standardError = Pipe()

        do {
            try process.run()
        } catch {
            throw HIDProbeError.profileCommandMissing
        }

        // Read on a worker so a stalled child cannot deadlock the pipe, and stop at the
        // output bound rather than buffering whatever the child decides to emit.
        let collector = OutputCollector(limit: Self.outputLimit)
        let handle = stdout.fileHandleForReading
        let reader = Thread {
            while let chunk = try? handle.read(upToCount: 64 * 1024), !chunk.isEmpty {
                if !collector.append(chunk) {
                    process.terminate()
                    return
                }
            }
        }
        reader.start()

        let deadline = Date().addingTimeInterval(Self.timeout)
        while process.isRunning, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.05)
        }
        if process.isRunning {
            process.terminate()
            throw HIDProbeError.profileCommandTimedOut
        }
        process.waitUntilExit()

        if collector.exceededLimit {
            throw HIDProbeError.profileOutputTooLarge
        }
        guard process.terminationStatus == 0 else {
            throw HIDProbeError.profileCommandFailed
        }
        return collector.data
    }
}

private final class OutputCollector: @unchecked Sendable {
    private let lock = NSLock()
    private let limit: Int
    private var storage = Data()
    private var overflowed = false

    init(limit: Int) {
        self.limit = limit
    }

    /// Returns `false` once the bound is hit so the caller stops reading.
    func append(_ chunk: Data) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard storage.count + chunk.count <= limit else {
            overflowed = true
            return false
        }
        storage.append(chunk)
        return true
    }

    var data: Data {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    var exceededLimit: Bool {
        lock.lock()
        defer { lock.unlock() }
        return overflowed
    }
}
