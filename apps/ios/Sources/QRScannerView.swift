import AVFoundation
import SwiftUI
import UIKit

/// Camera view that reports the raw bytes of the first QR code it sees.
///
/// It does no interpretation of its own: the bytes go straight to `ConnectionModel`,
/// which applies the same payload validation and private-address policy as typed input.
/// Nothing scanned is persisted, logged, or opened as a URL.
struct QRScannerView: UIViewControllerRepresentable {
    let onScan: (Data) -> Void
    let onUnavailable: () -> Void

    func makeUIViewController(context: Context) -> QRScannerViewController {
        let controller = QRScannerViewController()
        controller.onScan = onScan
        controller.onUnavailable = onUnavailable
        return controller
    }

    func updateUIViewController(_ controller: QRScannerViewController, context: Context) {}
}

final class QRScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onScan: ((Data) -> Void)?
    var onUnavailable: (() -> Void)?

    private let session = AVCaptureSession()
    private let sessionQueue = DispatchQueue(label: "QuadControl.QRScanner")
    private var previewLayer: AVCaptureVideoPreviewLayer?
    private var hasDelivered = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        requestAccessThenConfigure()
    }

    private func requestAccessThenConfigure() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureSession()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                DispatchQueue.main.async {
                    guard let self else { return }
                    if granted {
                        self.configureSession()
                        self.startSession()
                    } else {
                        self.onUnavailable?()
                    }
                }
            }
        default:
            onUnavailable?()
        }
    }

    private func configureSession() {
        guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .back),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input) else {
            onUnavailable?()
            return
        }
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else {
            onUnavailable?()
            return
        }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]

        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.addSublayer(layer)
        previewLayer = layer
    }

    private func startSession() {
        guard !session.isRunning, session.inputs.isEmpty == false else { return }
        // `startRunning()` blocks until the camera is configured, so keep it off main.
        sessionQueue.async { [session] in session.startRunning() }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        startSession()
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        sessionQueue.async { [session] in
            if session.isRunning { session.stopRunning() }
        }
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard !hasDelivered,
              let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
              object.type == .qr,
              let text = object.stringValue,
              let data = text.data(using: .utf8) else {
            return
        }
        hasDelivered = true
        sessionQueue.async { [session] in
            if session.isRunning { session.stopRunning() }
        }
        onScan?(data)
    }
}
