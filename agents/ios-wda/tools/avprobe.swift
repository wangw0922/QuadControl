// avprobe: capture the iPhone screen via Apple's CoreMediaIO/AVFoundation path
// (the same USB "Valeria" interface qvh uses) and stamp every frame with the
// host wall clock plus a luma-difference score against the previous frame.
import AVFoundation
import CoreMediaIO
import Foundation

let args = CommandLine.arguments
let seconds = args.count > 1 ? Double(args[1]) ?? 30 : 30
let outPath = args.count > 2 ? args[2] : "frames.tsv"
func err(_ s: String) { FileHandle.standardError.write((s + "\n").data(using: .utf8)!) }

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

func findScreenDevice() -> AVCaptureDevice? {
    let ds = AVCaptureDevice.DiscoverySession(deviceTypes: [.external], mediaType: nil, position: .unspecified)
    for d in ds.devices {
        err("device: muxed=\(d.hasMediaType(.muxed)) video=\(d.hasMediaType(.video)) model=\(d.modelID)")
        if d.hasMediaType(.muxed) { return d }
    }
    return nil
}

var device: AVCaptureDevice? = nil
for _ in 0..<40 {
    device = findScreenDevice()
    if device != nil { break }
    RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.5))
}
guard let dev = device else { err("no muxed (screen) device found"); exit(2) }
err("using screen device model=\(dev.modelID)")
for f in dev.formats {
    let dims = CMVideoFormatDescriptionGetDimensions(f.formatDescription)
    let codec = CMFormatDescriptionGetMediaSubType(f.formatDescription)
    let rates = f.videoSupportedFrameRateRanges.map { "\($0.minFrameRate)-\($0.maxFrameRate)" }
    err("format: \(dims.width)x\(dims.height) subtype=\(String(format: "%08x", codec)) rates=\(rates)")
}

final class Sink: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    var idx = 0
    var prev: [UInt8]? = nil
    let fh: FileHandle
    init(fh: FileHandle) { self.fh = fh }
    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        let wall = Date().timeIntervalSince1970
        let pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer).seconds
        guard let pb = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
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
            var acc = 0; for i in 0..<cur.count { acc += abs(Int(cur[i]) - Int(p[i])) }
            diff = Double(acc) / Double(cur.count)
        }
        prev = cur
        fh.write(String(format: "%d\t%.6f\t%.6f\t%d\t%d\t%.3f\n", idx, wall, pts, w, h, diff).data(using: .utf8)!)
        idx += 1
    }
    func captureOutput(_ output: AVCaptureOutput, didDrop sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        fh.write("# dropped at \(Date().timeIntervalSince1970)\n".data(using: .utf8)!)
    }
}

FileManager.default.createFile(atPath: outPath, contents: nil)
let fh = FileHandle(forWritingAtPath: outPath)!
let session = AVCaptureSession()
let input = try AVCaptureDeviceInput(device: dev)
session.addInput(input)
let out = AVCaptureVideoDataOutput()
out.alwaysDiscardsLateVideoFrames = false
out.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange]
let sink = Sink(fh: fh)
out.setSampleBufferDelegate(sink, queue: DispatchQueue(label: "sink"))
session.addOutput(out)
fh.write("# start wall=\(Date().timeIntervalSince1970)\n".data(using: .utf8)!)
session.startRunning()
RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds))
session.stopRunning()
err("done: \(sink.idx) frames")
