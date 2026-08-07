import AppKit
import Foundation
import QuadControlAndroidVideo

/// Live mirror of an Android screen — layer one, official `adb` commands only.
///
/// `screenrecord` writes raw Annex B H.264 to stdout; `ScreenRecordStream` parses and
/// decodes it, and this draws the frames. Nothing here touches an Android internal API,
/// so it behaves the same on any device with ADB debugging enabled.
///
/// Mirroring only. Input, clipboard, files, and screen-off are not implemented.

private struct Options {
    var width = 1280
    var height = 2560
    var bitRate = "8M"
    var serial: String?

    static func parse(_ arguments: [String]) -> Options? {
        var options = Options()
        var index = 0
        while index < arguments.count {
            let flag = arguments[index]
            index += 1
            guard index < arguments.count else { return nil }
            let value = arguments[index]
            index += 1
            switch flag {
            case "--width":
                guard let parsed = Int(value), parsed > 0 else { return nil }
                options.width = parsed
            case "--height":
                guard let parsed = Int(value), parsed > 0 else { return nil }
                options.height = parsed
            case "--bit-rate":
                options.bitRate = value
            case "--serial":
                options.serial = value
            default:
                return nil
            }
        }
        return options
    }
}

@MainActor
private final class MirrorView: NSView {
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        layer?.contentsGravity = .resizeAspect
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    func present(_ frame: DecodedFrame) {
        layer?.contents = frame.image
    }
}

@MainActor
private final class Mirror: NSObject, NSApplicationDelegate {
    private let options: Options
    private var window: NSWindow?
    private var view: MirrorView?
    private var stream: ScreenRecordStream?
    private var sizedToStream = false

    init(options: Options) {
        self.options = options
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let view = MirrorView(frame: NSRect(x: 0, y: 0, width: 420, height: 840))
        let window = NSWindow(
            contentRect: view.frame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "QuadControl — Android mirror"
        window.contentView = view
        window.center()
        window.makeKeyAndOrderFront(nil)
        self.window = window
        self.view = view

        // Both callbacks arrive off the main thread, so everything touching AppKit hops
        // back explicitly.
        let stream = ScreenRecordStream(
            onFrame: { frame in
                Task { @MainActor [weak self] in
                    self?.view?.present(frame)
                    self?.sizeToStreamIfNeeded(frame.size)
                }
            },
            onError: { message in
                Task { @MainActor in
                    FileHandle.standardError.write(Data((message + "\n").utf8))
                }
            }
        )
        self.stream = stream

        do {
            try stream.start(
                .init(
                    width: options.width,
                    height: options.height,
                    bitRate: options.bitRate,
                    serial: options.serial
                )
            )
            FileHandle.standardError.write(Data("mirroring; close the window to stop\n".utf8))
        } catch {
            FileHandle.standardError.write(Data("could not start screenrecord: \(error)\n".utf8))
            NSApp.terminate(nil)
        }
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationWillTerminate(_ notification: Notification) {
        stream?.stop()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    /// The encoder picks the real dimensions, which may not match what was requested, so
    /// the window is matched to the first decoded frame rather than to the request.
    private func sizeToStreamIfNeeded(_ size: CGSize) {
        guard !sizedToStream, size.height > 0 else { return }
        sizedToStream = true
        FileHandle.standardError.write(
            Data("first frame \(Int(size.width))x\(Int(size.height))\n".utf8)
        )
        let scale = max(size.height / 840, 1)
        window?.setContentSize(NSSize(width: size.width / scale, height: size.height / scale))
    }
}

guard let options = Options.parse(Array(CommandLine.arguments.dropFirst())) else {
    FileHandle.standardError.write(
        Data("usage: QuadControlAndroidViewer [--width N] [--height N] [--bit-rate 8M] [--serial S]\n".utf8)
    )
    exit(64)
}

private let application = NSApplication.shared
private let mirror = Mirror(options: options)
application.delegate = mirror
application.setActivationPolicy(.regular)
application.run()
