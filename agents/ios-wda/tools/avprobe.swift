// avprobe: capture the iPhone screen via Apple's CoreMediaIO/AVFoundation path
// (the same USB "Valeria" interface QuickTime Player uses).
//
// Two modes:
//   avprobe <seconds> <frames.tsv>          stamp every frame (wall clock, PTS, luma diff)
//   avprobe --serve <port> [--quality q]    JPEG-encode every frame and serve it as an
//                                           MJPEG stream shaped like WDA's (HTTP/1.0,
//                                           boundary --BoundaryString, Content-Length per
//                                           part); prints "port=<n>" on stdout once bound.
//
// Never prints the device name or unique id. Run from Terminal.app: camera permission is
// granted per launching app and a probe started from an agent shell is denied silently.
import AVFoundation
import CoreImage
import CoreMediaIO
import Darwin
import Foundation

func err(_ s: String) { FileHandle.standardError.write((s + "\n").data(using: .utf8)!) }

let args = CommandLine.arguments
var serveMode = false
var servePort: UInt16 = 0
var quality = 0.5
var seconds = 30.0
var outPath = "frames.tsv"
var i = 1
while i < args.count {
    switch args[i] {
    case "--serve": serveMode = true; servePort = UInt16(args[i + 1]) ?? 0; i += 2
    case "--quality": quality = Double(args[i + 1]) ?? 0.5; i += 2
    default:
        if Double(args[i]) != nil { seconds = Double(args[i])! } else { outPath = args[i] }
        i += 1
    }
}

var prop = CMIOObjectPropertyAddress(
    mSelector: CMIOObjectPropertySelector(kCMIOHardwarePropertyAllowScreenCaptureDevices),
    mScope: CMIOObjectPropertyScope(kCMIOObjectPropertyScopeGlobal),
    mElement: CMIOObjectPropertyElement(kCMIOObjectPropertyElementMain))
var allow: UInt32 = 1
CMIOObjectSetPropertyData(CMIOObjectID(kCMIOObjectSystemObject), &prop, 0, nil, UInt32(MemoryLayout<UInt32>.size), &allow)

let auth = AVCaptureDevice.authorizationStatus(for: .video)
err("camera auth status: \(auth.rawValue) (3=authorized)")
if auth == .notDetermined {
    let sem = DispatchSemaphore(value: 0)
    AVCaptureDevice.requestAccess(for: .video) { ok in err("requestAccess -> \(ok)"); sem.signal() }
    _ = sem.wait(timeout: .now() + 120)
}
if AVCaptureDevice.authorizationStatus(for: .video) != .authorized {
    err("capture_permission_denied"); exit(3)
}

func findScreenDevice() -> AVCaptureDevice? {
    let ds = AVCaptureDevice.DiscoverySession(deviceTypes: [.external], mediaType: nil, position: .unspecified)
    return ds.devices.first { $0.hasMediaType(.muxed) }
}
var device: AVCaptureDevice? = nil
for _ in 0..<40 {
    device = findScreenDevice()
    if device != nil { break }
    RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.5))
}
guard let dev = device else { err("capture_device_not_found"); exit(2) }
err("using an iOS screen device")

// MARK: - frame sinks

final class LatestFrame {
    private let lock = NSCondition()
    private var data = Data()
    private var seq = 0
    func publish(_ d: Data) { lock.lock(); data = d; seq += 1; lock.broadcast(); lock.unlock() }
    /// Blocks until a frame newer than `after` exists; returns (seq, bytes).
    func wait(after: Int) -> (Int, Data) {
        lock.lock(); defer { lock.unlock() }
        while seq <= after { lock.wait() }
        return (seq, data)
    }
}

final class Sink: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    var idx = 0
    var prev: [UInt8]? = nil
    let fh: FileHandle?
    let latest: LatestFrame?
    let ci = CIContext(options: [.useSoftwareRenderer: false])
    let cs = CGColorSpace(name: CGColorSpace.sRGB)!
    var encodeNs: Int64 = 0
    var encoded = 0
    var lastReport = Date()
    init(fh: FileHandle?, latest: LatestFrame?) { self.fh = fh; self.latest = latest }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        let wall = Date().timeIntervalSince1970
        guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        if let latest = latest {
            let t0 = DispatchTime.now().uptimeNanoseconds
            let image = CIImage(cvPixelBuffer: pb)
            if let jpeg = ci.jpegRepresentation(of: image, colorSpace: cs, options: [kCGImageDestinationLossyCompressionQuality as CIImageRepresentationOption: quality]) {
                latest.publish(jpeg)
                encodeNs += Int64(DispatchTime.now().uptimeNanoseconds - t0)
                encoded += 1
            }
            idx += 1
            if Date().timeIntervalSince(lastReport) >= 5 {
                let ms = encoded > 0 ? Double(encodeNs) / Double(encoded) / 1e6 : 0
                err(String(format: "capture %.1f fps, jpeg encode %.1f ms/frame, last frame %d bytes", Double(idx) / Date().timeIntervalSince(lastReport), ms, latest.wait(after: -1).1.count))
                idx = 0; encodeNs = 0; encoded = 0; lastReport = Date()
            }
            return
        }
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer).seconds
        CVPixelBufferLockBaseAddress(pb, .readOnly)
        let w = CVPixelBufferGetWidthOfPlane(pb, 0), h = CVPixelBufferGetHeightOfPlane(pb, 0)
        let stride = CVPixelBufferGetBytesPerRowOfPlane(pb, 0)
        let base = CVPixelBufferGetBaseAddressOfPlane(pb, 0)!.assumingMemoryBound(to: UInt8.self)
        var cur = [UInt8](); cur.reserveCapacity((w / 8 + 1) * (h / 8 + 1))
        var y = 0
        while y < h { var x = 0; while x < w { cur.append(base[y * stride + x]); x += 8 }; y += 8 }
        CVPixelBufferUnlockBaseAddress(pb, .readOnly)
        var diff = 0.0
        if let p = prev, p.count == cur.count {
            var acc = 0; for k in 0..<cur.count { acc += abs(Int(cur[k]) - Int(p[k])) }
            diff = Double(acc) / Double(cur.count)
        }
        prev = cur
        fh?.write(String(format: "%d\t%.6f\t%.6f\t%d\t%d\t%.3f\n", idx, wall, pts, w, h, diff).data(using: .utf8)!)
        idx += 1
    }
    func captureOutput(_ output: AVCaptureOutput, didDrop sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        fh?.write("# dropped at \(Date().timeIntervalSince1970)\n".data(using: .utf8)!)
    }
}

// MARK: - MJPEG server (WDA-shaped)

func serve(latest: LatestFrame, port: UInt16) {
    let fd = socket(AF_INET, SOCK_STREAM, 0)
    var one: Int32 = 1
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, socklen_t(MemoryLayout<Int32>.size))
    var addr = sockaddr_in()
    addr.sin_family = sa_family_t(AF_INET)
    addr.sin_port = port.bigEndian
    addr.sin_addr.s_addr = inet_addr("127.0.0.1")
    let bound = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
    guard bound == 0, listen(fd, 4) == 0 else { err("bind/listen failed: \(errno)"); exit(4) }
    var bound_addr = sockaddr_in(); var len = socklen_t(MemoryLayout<sockaddr_in>.size)
    withUnsafeMutablePointer(to: &bound_addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { _ = getsockname(fd, $0, &len) } }
    let actual = UInt16(bigEndian: bound_addr.sin_port)
    print("port=\(actual)"); fflush(stdout)
    while true {
        let client = accept(fd, nil, nil)
        if client < 0 { continue }
        Thread.detachNewThread {
            var nosig: Int32 = 1
            setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &nosig, socklen_t(MemoryLayout<Int32>.size))
            // Wait for the request line + headers (the proxy sends GET / HTTP/1.1 first).
            var req = Data(); var buf = [UInt8](repeating: 0, count: 1024)
            while req.range(of: Data("\r\n\r\n".utf8)) == nil {
                let n = read(client, &buf, buf.count)
                if n <= 0 { close(client); return }
                req.append(buf, count: n)
                if req.count > 8192 { close(client); return }
            }
            func send(_ d: Data) -> Bool {
                var off = 0
                while off < d.count {
                    let n = d.withUnsafeBytes { write(client, $0.baseAddress! + off, d.count - off) }
                    if n <= 0 { return false }
                    off += n
                }
                return true
            }
            let head = "HTTP/1.0 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary=--BoundaryString\r\n\r\n"
            guard send(Data(head.utf8)) else { close(client); return }
            var seq = -1
            var sent = 0
            let started = Date()
            while true {
                let (s, frame) = latest.wait(after: seq)
                seq = s
                let part = "--BoundaryString\r\nContent-type: image/jpeg\r\nContent-Length: \(frame.count)\r\n\r\n"
                guard send(Data(part.utf8)), send(frame), send(Data("\r\n".utf8)) else { break }
                sent += 1
                if sent % 250 == 0 { err(String(format: "client: %d parts sent, %.1f parts/s", sent, Double(sent) / Date().timeIntervalSince(started))) }
            }
            close(client)
            err("client disconnected after \(sent) parts")
        }
    }
}

// MARK: - session

let latest: LatestFrame? = serveMode ? LatestFrame() : nil
var fh: FileHandle? = nil
if !serveMode {
    FileManager.default.createFile(atPath: outPath, contents: nil)
    fh = FileHandle(forWritingAtPath: outPath)
}
let session = AVCaptureSession()
let input = try AVCaptureDeviceInput(device: dev)
session.addInput(input)
let out = AVCaptureVideoDataOutput()
out.alwaysDiscardsLateVideoFrames = serveMode
out.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: serveMode ? kCVPixelFormatType_32BGRA : kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange]
let sink = Sink(fh: fh, latest: latest)
out.setSampleBufferDelegate(sink, queue: DispatchQueue(label: "sink"))
session.addOutput(out)
fh?.write("# start wall=\(Date().timeIntervalSince1970)\n".data(using: .utf8)!)
session.startRunning()

signal(SIGINT) { _ in exit(0) }
signal(SIGTERM) { _ in exit(0) }
if serveMode {
    // Exit when stdin closes (the supervisor's pipe), so a killed parent takes us down.
    Thread.detachNewThread {
        while let _ = readLine() {}
        err("stdin closed, exiting"); exit(0)
    }
    serve(latest: latest!, port: servePort)
} else {
    RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds))
    session.stopRunning()
    err("done: \(sink.idx) frames")
}
