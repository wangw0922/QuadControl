import CoreGraphics
import Foundation

/// The device's current display geometry, which is what input coordinates are in.
public struct AndroidDisplay: Equatable, Sendable {
    /// Rotation-aware size. A phone held sideways reports 3088×1440, not 1440×3088.
    public let currentSize: CGSize
    /// Natural size, unaffected by rotation. Kept for diagnostics only.
    public let naturalSize: CGSize

    public init(currentSize: CGSize, naturalSize: CGSize) {
        self.currentSize = currentSize
        self.naturalSize = naturalSize
    }

    /// `dumpsys window displays` reports `init=` for the natural size and `cur=` for the
    /// rotated one. `wm size` only gives the natural size, which silently produces
    /// transposed taps whenever the phone is sideways — so it is not used here.
    public static func parse(_ dumpsys: String) -> AndroidDisplay? {
        func size(prefix: String) -> CGSize? {
            guard let range = dumpsys.range(of: "\(prefix)=\\d+x\\d+", options: .regularExpression) else {
                return nil
            }
            let pair = dumpsys[range].dropFirst(prefix.count + 1).split(separator: "x")
            guard pair.count == 2, let width = Int(pair[0]), let height = Int(pair[1]),
                  width > 0, height > 0 else { return nil }
            return CGSize(width: width, height: height)
        }
        guard let current = size(prefix: "cur") else { return nil }
        return AndroidDisplay(currentSize: current, naturalSize: size(prefix: "init") ?? current)
    }

    public static func query(serial: String? = nil) throws -> AndroidDisplay {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        var arguments = ["adb"]
        if let serial { arguments += ["-s", serial] }
        arguments += ["shell", "dumpsys", "window", "displays"]
        process.arguments = arguments

        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        try process.run()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        guard let display = parse(String(decoding: data, as: UTF8.self)) else {
            throw AndroidInputFailure.displayUnavailable
        }
        return display
    }

    /// A stream size that keeps the display's aspect ratio, so the encoder never
    /// letterboxes. Black bars would silently offset every tap.
    public func streamSize(maxDimension: Int) -> CGSize {
        let longest = max(currentSize.width, currentSize.height)
        guard longest > CGFloat(maxDimension) else { return currentSize }
        let scale = CGFloat(maxDimension) / longest
        // H.264 wants even dimensions; rounding down avoids exceeding the cap.
        func even(_ value: CGFloat) -> CGFloat { (value * scale / 2).rounded(.down) * 2 }
        return CGSize(width: even(currentSize.width), height: even(currentSize.height))
    }

    /// Maps a point in a view onto device coordinates.
    ///
    /// `view` is the drawn picture's rectangle, not the window: the layer letterboxes
    /// with `resizeAspect`, and using the window would drift by the bar width.
    public func devicePoint(fromViewPoint point: CGPoint, pictureRect: CGRect) -> CGPoint? {
        guard pictureRect.width > 0, pictureRect.height > 0 else { return nil }
        // `CGRect.contains` excludes the max edge, which would reject a click on the
        // picture's last pixel column. Those pixels are part of the picture.
        guard point.x >= pictureRect.minX, point.x <= pictureRect.maxX,
              point.y >= pictureRect.minY, point.y <= pictureRect.maxY else { return nil }

        let fractionX = (point.x - pictureRect.minX) / pictureRect.width
        let fractionY = (point.y - pictureRect.minY) / pictureRect.height
        // Valid pixels run 0...size-1, so a click on the far edge must land on the last
        // pixel rather than one past it, which the device would reject.
        return CGPoint(
            x: min((fractionX * currentSize.width).rounded(), currentSize.width - 1),
            y: min((fractionY * currentSize.height).rounded(), currentSize.height - 1)
        )
    }
}
