import Foundation

/// Measures how fast this adb link actually carries bytes, and picks a video bitrate
/// that leaves it mostly idle.
///
/// Sustained *growth* in latency here is a queueing problem, not a throughput one (the
/// fixed device-side floor is a separate story — see docs/CONTROL_ARCHITECTURE.md). The
/// encoder hands adb more bytes than the link drains and TCP buffers the difference, so
/// the picture falls steadily further behind while input — a few dozen bytes — stays
/// responsive. The same physical link measured 4–6 Mbps in a sick Wi-Fi association and
/// 33–43 Mbps after re-associating, which is why capacity is measured at every start
/// rather than hardcoded — a fixed default is wrong on every link but the one it was
/// tuned on, and sometimes wrong even there.
public enum LinkProbe {
    /// Fraction of measured capacity to spend on video.
    ///
    /// Deliberately small. H.264 overshoots its target during motion, and the headroom
    /// is what keeps those bursts from turning into queue.
    public static let targetUtilisation = 0.2

    public static let minimumBitsPerSecond = 800_000
    public static let maximumBitsPerSecond = 12_000_000

    /// Transfers a few megabytes of zeros from the device and times it.
    ///
    /// Zeros compress well on some transports, so this is an optimistic bound — which is
    /// fine, because it is scaled down hard afterwards. Returns nil if the probe cannot
    /// run; callers should then fall back to a conservative fixed value rather than
    /// assuming a fast link.
    public static func measureBitsPerSecond(serial: String?, megabytes: Int = 8) -> Double? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        var arguments = ["adb"]
        if let serial { arguments += ["-s", serial] }
        arguments += ["exec-out", "dd if=/dev/zero bs=1M count=\(megabytes) 2>/dev/null"]
        process.arguments = arguments

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()

        let started = DispatchTime.now().uptimeNanoseconds
        do {
            try process.run()
        } catch {
            return nil
        }
        var received = 0
        while true {
            let chunk = pipe.fileHandleForReading.availableData
            if chunk.isEmpty { break }
            received += chunk.count
        }
        process.waitUntilExit()
        let elapsed = Double(DispatchTime.now().uptimeNanoseconds - started) / 1_000_000_000

        guard Self.probeSucceeded(
            receivedBytes: received,
            expectedBytes: megabytes * 1_048_576,
            terminationStatus: process.terminationStatus,
            elapsed: elapsed
        ) else { return nil }
        return Double(received) * 8 / elapsed
    }

    static func probeSucceeded(
        receivedBytes: Int,
        expectedBytes: Int,
        terminationStatus: Int32,
        elapsed: Double
    ) -> Bool {
        receivedBytes == expectedBytes && terminationStatus == 0 && elapsed > 0.05
    }

    /// The bitrate to ask `screenrecord` for, as an `adb`-ready string.
    public static func recommendedBitRate(capacityBitsPerSecond: Double?) -> String {
        guard let capacity = capacityBitsPerSecond else {
            // No measurement: assume a slow link rather than a fast one. Guessing high
            // reintroduces exactly the queueing this exists to avoid.
            return "1500000"
        }
        let target = capacity * targetUtilisation
        let clamped = min(max(target, Double(minimumBitsPerSecond)), Double(maximumBitsPerSecond))
        return String(Int(clamped))
    }

    public static func describe(capacityBitsPerSecond: Double?) -> String {
        guard let capacity = capacityBitsPerSecond else { return "link speed unknown" }
        return String(format: "link ~%.1f Mbps", capacity / 1_000_000)
    }
}
