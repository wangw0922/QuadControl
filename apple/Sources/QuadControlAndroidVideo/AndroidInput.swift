import CoreGraphics
import Foundation

public enum AndroidInputFailure: String, Error, Equatable, Sendable {
    case displayUnavailable = "could not read the device display size"
    case shellUnavailable = "could not start a persistent adb shell"
}

/// Android key codes this mirror sends. Named rather than numeric so a typo cannot
/// silently become a different key.
public enum AndroidKey: String, Sendable, CaseIterable {
    case back = "KEYCODE_BACK"
    case home = "KEYCODE_HOME"
    case appSwitch = "KEYCODE_APP_SWITCH"
    case enter = "KEYCODE_ENTER"
    case delete = "KEYCODE_DEL"
    case tab = "KEYCODE_TAB"
    case escape = "KEYCODE_ESCAPE"
    case volumeUp = "KEYCODE_VOLUME_UP"
    case volumeDown = "KEYCODE_VOLUME_DOWN"
    case dpadUp = "KEYCODE_DPAD_UP"
    case dpadDown = "KEYCODE_DPAD_DOWN"
    case dpadLeft = "KEYCODE_DPAD_LEFT"
    case dpadRight = "KEYCODE_DPAD_RIGHT"
}

/// Sends input through the official `adb shell input` command.
///
/// Layer one: no Android internal APIs, so this works on any device with ADB debugging
/// on. Commands go through one persistent `adb shell` rather than a new `adb` process
/// per event — measured on a Samsung SM-S9180, that takes a keyevent from 103 ms to
/// 53 ms. The remaining ~51 ms is the device starting a fresh `input` process, which
/// layer one cannot avoid.
///
/// Because of that cost, gestures are expressed as single commands wherever possible:
/// a drag is one `input swipe` with a duration rather than a stream of move events, and
/// a string is one `input text` rather than one command per character.
public final class AndroidInput: @unchecked Sendable {
    private let lock = NSLock()
    private var shell: Process?
    private var stdin: FileHandle?
    private let onError: @Sendable (String) -> Void

    public init(onError: @escaping @Sendable (String) -> Void) {
        self.onError = onError
    }

    public func start(serial: String? = nil) throws {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        var arguments = ["adb"]
        if let serial { arguments += ["-s", serial] }
        arguments += ["shell"]
        process.arguments = arguments

        let input = Pipe()
        process.standardInput = input
        process.standardOutput = Pipe()
        let errors = Pipe()
        process.standardError = errors
        errors.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            guard !chunk.isEmpty,
                  let text = String(data: chunk, encoding: .utf8)?
                      .trimmingCharacters(in: .whitespacesAndNewlines),
                  !text.isEmpty else { return }
            self?.onError("adb shell: \(text)")
        }

        do {
            try process.run()
        } catch {
            throw AndroidInputFailure.shellUnavailable
        }

        lock.lock()
        shell = process
        stdin = input.fileHandleForWriting
        lock.unlock()
    }

    public func stop() {
        lock.lock()
        let process = shell
        let handle = stdin
        shell = nil
        stdin = nil
        lock.unlock()
        try? handle?.close()
        process?.terminate()
    }

    public func tap(_ point: CGPoint) {
        send("input tap \(Int(point.x)) \(Int(point.y))")
    }

    /// One command for the whole gesture. Sending discrete move events would cost 53 ms
    /// each and never look like a drag.
    public func swipe(from start: CGPoint, to end: CGPoint, milliseconds: Int) {
        let duration = max(1, milliseconds)
        send("input swipe \(Int(start.x)) \(Int(start.y)) \(Int(end.x)) \(Int(end.y)) \(duration)")
    }

    public func key(_ key: AndroidKey) {
        send("input keyevent \(key.rawValue)")
    }

    /// Sends a literal string.
    ///
    /// `input text` is ASCII-only in practice: it maps the string onto key events, so
    /// characters outside that range are dropped by the device rather than typed. This
    /// filters them out and reports it, instead of letting text silently go missing.
    /// Entering non-ASCII needs a different mechanism and is not implemented.
    public func text(_ value: String) {
        let ascii = value.filter { $0.isASCII }
        if ascii.count != value.count {
            onError("input text dropped \(value.count - ascii.count) non-ASCII character(s); `input text` cannot send them")
        }
        guard !ascii.isEmpty else { return }
        send("input text \(Self.shellQuoted(ascii))")
    }

    /// Single-quote for the device shell, closing and reopening around embedded quotes.
    /// `input text` additionally treats a literal space as an argument separator, so
    /// spaces become `%s` — its documented escape.
    static func shellQuoted(_ value: String) -> String {
        let spaced = value.replacingOccurrences(of: " ", with: "%s")
        return "'" + spaced.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    private func send(_ command: String) {
        lock.lock()
        let handle = stdin
        lock.unlock()
        guard let handle else { return }
        do {
            try handle.write(contentsOf: Data((command + "\n").utf8))
        } catch {
            onError("input write failed: \(error)")
        }
    }
}
