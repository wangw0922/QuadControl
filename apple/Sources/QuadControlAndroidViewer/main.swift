import AppKit
import Foundation
import QuadControlAndroidVideo

/// Live Android mirror with mouse and keyboard control — layer one, official `adb`
/// commands only. No Android internal APIs, so it is expected to have a wider
/// compatibility range across devices with ADB debugging enabled.
///
/// Not implemented: clipboard, file transfer, audio, and screen-off. Screen-off is not
/// merely unbuilt — layer one cannot do it at all.

private struct Options {
    var maxDimension: Int?
    var bitRate: String?
    var serial: String?

    /// An adb serial of the form `host:port` means the device is attached over Wi-Fi.
    var isWireless: Bool {
        guard let serial else { return false }
        return serial.contains(":") || serial.contains("_adb-tls-connect._tcp")
    }

    /// Measured on this link: raw adb transfer sustained about 0.5 MB/s (~4 Mbps) over
    /// Wi-Fi against far more over USB. Asking for 8 Mbps on a 4 Mbps link does not drop
    /// frames — it queues them, so latency grows without bound while input, which is a
    /// few dozen bytes, stays responsive. That is exactly the "picture lags but taps feel
    /// fine" symptom. Defaults are therefore chosen per transport, and either can be
    /// overridden.
    /// Only used when the link probe cannot run; the measured value is preferred.
    var fallbackBitRate: String { bitRate ?? (isWireless ? "1500000" : "8M") }
    var effectiveMaxDimension: Int { maxDimension ?? (isWireless ? 720 : 1280) }

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
            case "--max-size":
                guard let parsed = Int(value), parsed > 0 else { return nil }
                options.maxDimension = parsed
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
private protocol MirrorViewDelegate: AnyObject {
    func mirrorView(_ view: MirrorView, tapAt point: CGPoint)
    func mirrorView(_ view: MirrorView, dragFrom start: CGPoint, to end: CGPoint, milliseconds: Int)
    func mirrorView(_ view: MirrorView, scrollFrom point: CGPoint, deltaY: CGFloat)
    func mirrorView(_ view: MirrorView, type text: String)
    func mirrorView(_ view: MirrorView, press key: AndroidKey)
}

@MainActor
private final class MirrorView: NSView {
    weak var delegate: MirrorViewDelegate?
    private var frameSize: CGSize?
    private var dragStart: CGPoint?
    private var dragStartedAt: Date?

    /// Android's origin is top-left; AppKit's is bottom-left. Flipping here means every
    /// coordinate downstream is already in Android's orientation.
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.black.cgColor
        layer?.contentsGravity = .resizeAspect
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    func present(_ frame: DecodedFrame) {
        frameSize = frame.size
        // The decoder's IOSurface goes straight to the compositor. Converting to a
        // CGImage first cost a GPU render and a readback for no benefit.
        layer?.contents = frame.surface
    }

    /// Where the picture actually sits inside the view.
    ///
    /// `resizeAspect` centres the image and leaves bars on one axis. Mapping against
    /// `bounds` instead would offset every tap by half the bar width.
    var pictureRect: CGRect {
        guard let frameSize, frameSize.width > 0, frameSize.height > 0 else { return bounds }
        let scale = min(bounds.width / frameSize.width, bounds.height / frameSize.height)
        let size = CGSize(width: frameSize.width * scale, height: frameSize.height * scale)
        return CGRect(
            x: bounds.midX - size.width / 2,
            y: bounds.midY - size.height / 2,
            width: size.width,
            height: size.height
        )
    }

    override func mouseDown(with event: NSEvent) {
        dragStart = convert(event.locationInWindow, from: nil)
        dragStartedAt = Date()
    }

    override func mouseUp(with event: NSEvent) {
        defer { dragStart = nil; dragStartedAt = nil }
        guard let start = dragStart, let startedAt = dragStartedAt else { return }
        let end = convert(event.locationInWindow, from: nil)
        let distance = hypot(end.x - start.x, end.y - start.y)
        // Below this a gesture is a click with a shaky hand, not a drag.
        if distance < 6 {
            delegate?.mirrorView(self, tapAt: end)
        } else {
            let elapsed = Int(Date().timeIntervalSince(startedAt) * 1000)
            delegate?.mirrorView(self, dragFrom: start, to: end, milliseconds: max(40, elapsed))
        }
    }

    override func scrollWheel(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        guard event.scrollingDeltaY != 0 else { return }
        delegate?.mirrorView(self, scrollFrom: point, deltaY: event.scrollingDeltaY)
    }

    override func keyDown(with event: NSEvent) {
        // Special keys first: they must not be typed as text.
        if let key = Self.specialKey(for: event) {
            delegate?.mirrorView(self, press: key)
            return
        }
        guard let characters = event.charactersIgnoringModifiers, !characters.isEmpty else { return }
        delegate?.mirrorView(self, type: characters)
    }

    private static func specialKey(for event: NSEvent) -> AndroidKey? {
        switch event.keyCode {
        case 51: return .delete      // Backspace maps to Android's DEL.
        case 36, 76: return .enter
        case 48: return .tab
        case 53: return .escape      // Escape is the natural stand-in for Back.
        case 126: return .dpadUp
        case 125: return .dpadDown
        case 123: return .dpadLeft
        case 124: return .dpadRight
        default: return nil
        }
    }
}

@MainActor
private final class Mirror: NSObject, NSApplicationDelegate, MirrorViewDelegate {
    private let options: Options
    private var window: NSWindow?
    private var view: MirrorView?
    private var stream: ScreenRecordStream?
    private var input: AndroidInput?
    private var display: AndroidDisplay?
    private var watcher: DisplayWatcher?
    private var sizedToStream = false
    private var rotationStartedAt: Date?
    private var chosenBitRate = "1500000"
    /// Latest frame waiting to be drawn. Live mirroring wants the newest picture, so a
    /// frame that arrives before the previous one is drawn simply replaces it — queueing
    /// every frame turns a slow link into ever-growing latency instead of dropped frames.
    private var pendingFrame: DecodedFrame?
    private var lastMissingSurfaceReport = Date.distantPast
    private var drawScheduled = false
    private var framesDecoded = 0
    private var framesDrawn = 0
    private var statsStartedAt = Date()

    init(options: Options) {
        self.options = options
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let display: AndroidDisplay
        do {
            display = try AndroidDisplay.query(serial: options.serial)
        } catch {
            report("could not read the device display size: \(error). Is a device authorized?")
            NSApp.terminate(nil)
            return
        }
        self.display = display
        // Matching the display's aspect ratio keeps screenrecord from letterboxing, which
        // would silently offset every tap.
        // Matching the display's aspect ratio keeps screenrecord from letterboxing, which
        // would both waste resolution and leave the window the wrong shape.
        let streamSize = display.streamSize(maxDimension: options.effectiveMaxDimension)
        // An explicit --bit-rate wins; otherwise measure the link and spend a small
        // fraction of it, because the lag is queueing rather than throughput.
        if options.bitRate == nil {
            let capacity = LinkProbe.measureBitsPerSecond(serial: options.serial)
            chosenBitRate = LinkProbe.recommendedBitRate(capacityBitsPerSecond: capacity)
            if let capacity {
                let target = capacity * LinkProbe.targetUtilisation
                if target < Double(LinkProbe.minimumBitsPerSecond) {
                    report("\(LinkProbe.describe(capacityBitsPerSecond: capacity)); using minimum \(chosenBitRate) for video")
                } else if target > Double(LinkProbe.maximumBitsPerSecond) {
                    report("\(LinkProbe.describe(capacityBitsPerSecond: capacity)); using maximum \(chosenBitRate) for video")
                } else {
                    report("\(LinkProbe.describe(capacityBitsPerSecond: capacity)); using \(chosenBitRate) for video (\(Int(LinkProbe.targetUtilisation * 100))% of measured capacity, with headroom against queueing)")
                }
            } else {
                report("link speed unknown; using conservative fallback \(chosenBitRate) for video")
            }
        } else {
            chosenBitRate = options.fallbackBitRate
        }
        report("device \(Int(display.currentSize.width))x\(Int(display.currentSize.height)), streaming \(Int(streamSize.width))x\(Int(streamSize.height)) over \(options.isWireless ? "Wi-Fi" : "USB")")

        buildWindow()

        let stream = ScreenRecordStream(
            onFrame: { frame in
                Task { @MainActor [weak self] in self?.enqueue(frame) }
            },
            onError: { message in
                Task { @MainActor [weak self] in self?.report(message) }
            }
        )
        self.stream = stream

        let input = AndroidInput(onError: { message in
            Task { @MainActor [weak self] in self?.report(message) }
        })
        self.input = input

        let watcher = DisplayWatcher(onChange: { rotated in
            Task { @MainActor [weak self] in self?.displayRotated(to: rotated) }
        })
        self.watcher = watcher

        do {
            try input.start(serial: options.serial)
            try watcher.start(initial: display, serial: options.serial)
            try stream.start(
                .init(
                    width: Int(streamSize.width),
                    height: Int(streamSize.height),
                    bitRate: chosenBitRate,
                    serial: options.serial
                )
            )
            report("click to tap, drag to swipe, scroll to scroll, type to enter text")
            report("esc = back; the buttons under the picture send home, recents and volume")
        } catch {
            report("could not start: \(error)")
            NSApp.terminate(nil)
        }
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationWillTerminate(_ notification: Notification) {
        watcher?.stop()
        stream?.stop()
        input?.stop()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    private func buildWindow() {
        let view = MirrorView(frame: NSRect(x: 0, y: 0, width: 420, height: 800))
        view.delegate = self

        // The buttons live in the window, not the menu bar. A SwiftPM executable is not
        // an .app bundle, and macOS does not reliably give one a menu bar — the first
        // version put these in a Device menu that never appeared.
        let buttons = NSStackView(views: [
            makeButton("← 返回", #selector(sendBack)),
            makeButton("⌂ 主页", #selector(sendHome)),
            makeButton("▤ 任务", #selector(sendRecents)),
            makeButton("音量 −", #selector(sendVolumeDown)),
            makeButton("音量 +", #selector(sendVolumeUp)),
        ])
        buttons.orientation = .horizontal
        buttons.distribution = .fillEqually
        buttons.spacing = 6
        buttons.edgeInsets = NSEdgeInsets(top: 6, left: 8, bottom: 6, right: 8)
        buttons.setContentHuggingPriority(.required, for: .vertical)
        buttons.setContentCompressionResistancePriority(.required, for: .vertical)

        let stack = NSStackView(views: [view, buttons])
        stack.orientation = .vertical
        stack.spacing = 0
        stack.distribution = .fill
        view.setContentHuggingPriority(.defaultLow, for: .vertical)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 420, height: 840),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "QuadControl — Android"
        window.contentView = stack
        window.center()
        window.makeKeyAndOrderFront(nil)
        // The mirror must own the keyboard, or typing goes to a button instead of the phone.
        window.makeFirstResponder(view)
        self.window = window
        self.view = view
    }

    private func makeButton(_ title: String, _ action: Selector) -> NSButton {
        let control = NSButton(title: title, target: self, action: action)
        control.bezelStyle = .rounded
        // Without this a click on a button steals focus and the next keystroke never
        // reaches the phone.
        control.refusesFirstResponder = true
        return control
    }

    @objc private func sendBack() { input?.key(.back) }
    @objc private func sendHome() { input?.key(.home) }
    @objc private func sendRecents() { input?.key(.appSwitch) }
    @objc private func sendVolumeUp() { input?.key(.volumeUp) }
    @objc private func sendVolumeDown() { input?.key(.volumeDown) }

    /// Rotation changes both the picture's shape and the coordinate basis, so the stream
    /// is rebuilt at the new aspect. That costs roughly 850 ms of held picture, so the
    /// window is reshaped immediately and animated rather than snapping — the last frame
    /// stays on screen, letterboxed, until the new stream arrives.
    private func displayRotated(to rotated: AndroidDisplay) {
        display = rotated
        let streamSize = rotated.streamSize(maxDimension: options.effectiveMaxDimension)
        rotationStartedAt = Date()
        report("rotated to \(Int(rotated.currentSize.width))x\(Int(rotated.currentSize.height)), restarting stream at \(Int(streamSize.width))x\(Int(streamSize.height))")
        applyWindowSize(streamSize, animated: true)
        sizedToStream = true
        do {
            try stream?.restart(
                .init(
                    width: Int(streamSize.width),
                    height: Int(streamSize.height),
                    bitRate: chosenBitRate,
                    serial: options.serial
                )
            )
        } catch {
            report("could not restart the stream after rotation: \(error)")
        }
    }

    /// Coalesces frames: only the most recent one is ever drawn.
    private func enqueue(_ frame: DecodedFrame) {
        guard frame.surface != nil else {
            let now = Date()
            if now.timeIntervalSince(lastMissingSurfaceReport) >= 5 {
                lastMissingSurfaceReport = now
                report("decoded frame has no IOSurface backing; skipping frame")
            }
            return
        }
        framesDecoded += 1
        pendingFrame = frame
        guard !drawScheduled else { return }
        drawScheduled = true
        // Draw on the next runloop turn so a burst collapses into a single draw.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.drawScheduled = false
            guard let frame = self.pendingFrame else { return }
            self.pendingFrame = nil
            self.framesDrawn += 1
            self.view?.present(frame)
            self.sizeWindowIfNeeded(frame.size)
            self.reportStatsIfDue()
        }
    }

    private func reportStatsIfDue() {
        let elapsed = Date().timeIntervalSince(statsStartedAt)
        guard elapsed >= 5 else { return }
        report(String(
            format: "  %.0f fps decoded, %.0f fps drawn, %d dropped",
            Double(framesDecoded) / elapsed,
            Double(framesDrawn) / elapsed,
            framesDecoded - framesDrawn
        ))
        framesDecoded = 0
        framesDrawn = 0
        statsStartedAt = Date()
    }

    private func sizeWindowIfNeeded(_ size: CGSize) {
        if let startedAt = rotationStartedAt {
            rotationStartedAt = nil
            report(String(format: "  first frame after rotation: %.0f ms", Date().timeIntervalSince(startedAt) * 1000))
        }
        guard !sizedToStream, size.height > 0 else { return }
        sizedToStream = true
        applyWindowSize(size, animated: false)
    }

    private func applyWindowSize(_ size: CGSize, animated: Bool = false) {
        guard size.width > 0, size.height > 0, let window else { return }
        let scale = max(max(size.height, size.width) / 900, 1)
        let content = NSSize(width: size.width / scale, height: size.height / scale)
        guard animated else {
            window.setContentSize(content)
            return
        }
        // Snapping between portrait and landscape reads as a glitch; easing the frame
        // makes the same 850 ms feel like a transition instead of a stall.
        var target = window.frameRect(forContentRect: NSRect(origin: .zero, size: content))
        // Keep the window's centre fixed so it does not appear to jump across the screen.
        let current = window.frame
        target.origin = NSPoint(
            x: current.midX - target.width / 2,
            y: current.midY - target.height / 2
        )
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.28
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            window.animator().setFrame(target, display: true)
        }
    }

    private func report(_ message: String) {
        FileHandle.standardError.write(Data((message + "\n").utf8))
    }

    private func devicePoint(_ point: CGPoint, in view: MirrorView) -> CGPoint? {
        display?.devicePoint(fromViewPoint: point, pictureRect: view.pictureRect)
    }

    // MARK: - MirrorViewDelegate

    func mirrorView(_ view: MirrorView, tapAt point: CGPoint) {
        guard let target = devicePoint(point, in: view) else { return }
        input?.tap(target)
    }

    func mirrorView(_ view: MirrorView, dragFrom start: CGPoint, to end: CGPoint, milliseconds: Int) {
        guard let from = devicePoint(start, in: view), let to = devicePoint(end, in: view) else { return }
        input?.swipe(from: from, to: to, milliseconds: milliseconds)
    }

    func mirrorView(_ view: MirrorView, scrollFrom point: CGPoint, deltaY: CGFloat) {
        guard let anchor = devicePoint(point, in: view), let display else { return }
        // A scroll is a swipe in the opposite direction: content follows the fingers.
        let travel = min(max(abs(deltaY) * 12, 120), display.currentSize.height / 2)
        let direction: CGFloat = deltaY > 0 ? 1 : -1
        let start = CGPoint(x: anchor.x, y: anchor.y - travel * direction / 2)
        let end = CGPoint(x: anchor.x, y: anchor.y + travel * direction / 2)
        input?.swipe(from: clamp(start, to: display), to: clamp(end, to: display), milliseconds: 120)
    }

    func mirrorView(_ view: MirrorView, type text: String) {
        input?.text(text)
    }

    func mirrorView(_ view: MirrorView, press key: AndroidKey) {
        input?.key(key)
    }

    private func clamp(_ point: CGPoint, to display: AndroidDisplay) -> CGPoint {
        CGPoint(
            x: min(max(point.x, 0), display.currentSize.width - 1),
            y: min(max(point.y, 0), display.currentSize.height - 1)
        )
    }
}

guard let options = Options.parse(Array(CommandLine.arguments.dropFirst())) else {
    FileHandle.standardError.write(
        Data("usage: QuadControlAndroidViewer [--max-size 1280] [--bit-rate 8M] [--serial S]\n".utf8)
    )
    exit(64)
}

private let application = NSApplication.shared
private let mirror = Mirror(options: options)
application.delegate = mirror
application.setActivationPolicy(.regular)
application.run()
