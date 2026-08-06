import CoreImage
import Foundation

/// Renders a QR code as terminal text using CoreImage, which ships with macOS — the
/// package still has no third-party dependencies.
///
/// Two vertical modules are packed into one character cell with half-block glyphs, so
/// the code stays roughly square in a terminal whose cells are about twice as tall as
/// they are wide. Colours are set explicitly rather than relying on the terminal theme,
/// because a QR scanner needs dark modules on a light background and a dark-themed
/// terminal would otherwise invert the code.
enum TerminalQRCode {
    private static let quietZone = 2
    private static let lightOnDark = "\u{1B}[47;30m"
    private static let reset = "\u{1B}[0m"

    static func lines(for message: Data) -> [String]? {
        guard let modules = modules(for: message) else { return nil }
        let size = modules.count
        var rendered: [String] = []
        rendered.reserveCapacity((size + 1) / 2)

        for topRow in stride(from: 0, to: size, by: 2) {
            var line = lightOnDark
            for column in 0 ..< size {
                let top = modules[topRow][column]
                let bottom = topRow + 1 < size ? modules[topRow + 1][column] : false
                switch (top, bottom) {
                case (false, false): line.append(" ")
                case (true, false): line.append("\u{2580}")   // ▀
                case (false, true): line.append("\u{2584}")   // ▄
                case (true, true): line.append("\u{2588}")    // █
                }
            }
            line.append(reset)
            rendered.append(line)
        }
        return rendered
    }

    /// `true` means a dark module.
    private static func modules(for message: Data) -> [[Bool]]? {
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { return nil }
        filter.setValue(message, forKey: "inputMessage")
        // Medium recovery keeps the code small enough for a terminal while tolerating
        // glare and camera angle.
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let output = filter.outputImage else { return nil }

        let extent = output.extent
        let width = Int(extent.width)
        let height = Int(extent.height)
        guard width > 0, height > 0, width == height else { return nil }

        let context = CIContext(options: [.useSoftwareRenderer: true])
        guard let cgImage = context.createCGImage(output, from: extent) else { return nil }

        var pixels = [UInt8](repeating: 0, count: width * height)
        guard let bitmap = CGContext(
            data: &pixels,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width,
            space: CGColorSpaceCreateDeviceGray(),
            bitmapInfo: CGImageAlphaInfo.none.rawValue
        ) else { return nil }
        bitmap.draw(cgImage, in: CGRect(x: 0, y: 0, width: width, height: height))

        let padded = width + quietZone * 2
        var result = [[Bool]](repeating: [Bool](repeating: false, count: padded), count: padded)
        for y in 0 ..< height {
            for x in 0 ..< width {
                // CoreImage emits the code with the origin at the bottom-left; a QR is
                // read top-down, so flip the rows back.
                let isDark = pixels[(height - 1 - y) * width + x] < 128
                result[y + quietZone][x + quietZone] = isDark
            }
        }
        return result
    }
}
