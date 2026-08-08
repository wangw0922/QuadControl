import CoreGraphics
import Foundation

/// Watches for the device rotating.
///
/// Rotation is not cosmetic here. `screenrecord`'s frame size is fixed when it starts,
/// so after a rotation the stream keeps the old aspect and the picture is letterboxed
/// inside it — and worse, `AndroidDisplay` goes stale, so every tap is mapped against
/// the wrong dimensions and lands somewhere the user did not click. Both the stream and
/// the coordinate basis have to be rebuilt.
///
/// Android offers no push notification for this to a shell client, so it is polled.
/// `dumpsys window displays | grep -m1 cur=` measured 25 ms on a persistent shell
/// against 51–67 ms for the alternatives, and it is the only one that reports the
/// rotated size rather than the natural one.
public final class DisplayWatcher: @unchecked Sendable {
    private let lock = NSLock()
    private var shell: Process?
    private var stdin: FileHandle?
    private var reader: Thread?
    private var lastSeen: CGSize?
    private var stopping = false
    private let interval: TimeInterval
    private let onChange: @Sendable (AndroidDisplay) -> Void

    public init(
        pollInterval: TimeInterval = 0.35,
        onChange: @escaping @Sendable (AndroidDisplay) -> Void
    ) {
        self.interval = pollInterval
        self.onChange = onChange
    }

    public func start(initial: AndroidDisplay, serial: String? = nil) throws {
        lock.lock()
        lastSeen = initial.currentSize
        lock.unlock()

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        var arguments = ["adb"]
        if let serial { arguments += ["-s", serial] }
        arguments += ["shell"]
        process.arguments = arguments

        let input = Pipe()
        let output = Pipe()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = Pipe()

        // A dedicated shell rather than sharing the input one: interleaving polls with
        // taps would make both harder to reason about, and a poll must never delay input.
        do {
            try process.run()
        } catch {
            throw AndroidInputFailure.shellUnavailable
        }

        lock.lock()
        shell = process
        stdin = input.fileHandleForWriting
        stopping = false
        lock.unlock()

        let thread = Thread { [weak self] in
            self?.poll(reading: output.fileHandleForReading)
        }
        thread.name = "QuadControl.DisplayWatcher"
        thread.start()
        lock.lock()
        reader = thread
        lock.unlock()
    }

    public func stop() {
        lock.lock()
        stopping = true
        let process = shell
        let handle = stdin
        shell = nil
        stdin = nil
        lock.unlock()
        try? handle?.close()
        process?.terminate()
    }

    private func poll(reading handle: FileHandle) {
        var pending = Data()
        while true {
            lock.lock()
            let done = stopping
            let writer = stdin
            lock.unlock()
            if done || writer == nil { return }

            do {
                try writer?.write(contentsOf: Data("dumpsys window displays | grep -m1 cur=\n".utf8))
            } catch {
                return
            }

            let chunk = handle.availableData
            if chunk.isEmpty { return }
            pending.append(chunk)
            if let text = String(data: pending, encoding: .utf8) {
                pending.removeAll(keepingCapacity: true)
                if let display = AndroidDisplay.parse(text) {
                    notifyIfChanged(display)
                }
            }
            Thread.sleep(forTimeInterval: interval)
        }
    }

    private func notifyIfChanged(_ display: AndroidDisplay) {
        lock.lock()
        let changed = lastSeen != display.currentSize
        if changed { lastSeen = display.currentSize }
        lock.unlock()
        guard changed else { return }
        onChange(display)
    }
}
