import Observation
import QuadControlDiagnosticProtocol
import QuadControlDiagnosticTransport
import SwiftUI

@main
struct QuadControlIOSApp: App {
    @Environment(\.scenePhase) private var scenePhase
    @State private var connection = ConnectionModel()

    var body: some Scene {
        WindowGroup {
            NavigationStack {
                ConnectionScreen(model: connection)
            }
            .onChange(of: scenePhase) { _, phase in
                if phase != .active {
                    connection.disconnect()
                }
            }
        }
    }
}

/// User-visible connection state. Kept as a value rather than a rendered string so
/// the model stays free of copy and every case has one localized presentation.
enum ConnectionStatus: Equatable {
    case notConnected
    case invalidHostOrPort
    case invalidToken
    case connecting
    case authenticating
    case connected
    case disconnected
    case setupFailed
    case invalidScannedCode
    case rejectedScannedHost
    case cameraUnavailable
    case failed(DiagnosticFailure)

    var localized: String {
        switch self {
        case .notConnected:
            return String(localized: "status.not_connected")
        case .invalidHostOrPort:
            return String(localized: "status.invalid_host_or_port")
        case .invalidToken:
            return String(localized: "status.invalid_token")
        case .connecting:
            return String(localized: "status.connecting")
        case .authenticating:
            return String(localized: "status.authenticating")
        case .connected:
            return String(localized: "status.connected")
        case .disconnected:
            return String(localized: "status.disconnected")
        case .setupFailed:
            return String(localized: "status.setup_failed")
        case .invalidScannedCode:
            return String(localized: "status.invalid_scanned_code")
        case .rejectedScannedHost:
            return String(localized: "status.rejected_scanned_host")
        case .cameraUnavailable:
            return String(localized: "status.camera_unavailable")
        case let .failed(failure):
            return String(
                format: String(localized: "status.failed"),
                failure.rawValue
            )
        }
    }
}

@MainActor
@Observable
final class ConnectionModel {
    var host = ""
    var port = "47100"
    var token = ""
    var status = ConnectionStatus.notConnected
    var isConnected = false
    var isSessionActive = false

    private var session: DiagnosticClientSession?
    private var attemptID: UUID?

    func connect() {
        guard session == nil,
              !host.isEmpty,
              let port = UInt16(port),
              port > 0 else {
            status = .invalidHostOrPort
            clearToken()
            return
        }

        let enteredToken: Data
        do {
            enteredToken = try PairingToken.decode(token)
        } catch {
            status = .invalidToken
            clearToken()
            return
        }
        clearToken()

        do {
            let nextAttemptID = UUID()
            attemptID = nextAttemptID
            let next = try DiagnosticClientSession(
                host: host,
                port: port,
                token: enteredToken,
                stateHandler: { [weak self] state in
                    Task { @MainActor in
                        self?.apply(state, attemptID: nextAttemptID)
                    }
                }
            )
            session = next
            isSessionActive = true
            status = .connecting
            next.start { [weak self] result in
                Task { @MainActor in
                    guard self?.attemptID == nextAttemptID else { return }
                    if case let .failure(failure) = result {
                        self?.attemptID = nil
                        self?.session = nil
                        self?.isSessionActive = false
                        self?.isConnected = false
                        self?.status = .failed(failure)
                    }
                    self?.clearToken()
                }
            }
        } catch {
            attemptID = nil
            session = nil
            isSessionActive = false
            isConnected = false
            status = .setupFailed
            clearToken()
        }
    }

    /// Consumes a scanned pairing code. The payload is decoded and validated exactly
    /// like typed input: the host must still pass the private-address policy, so a
    /// hostile QR cannot redirect this client at a public listener, and the token still
    /// goes through `PairingToken.decode`. The payload is never treated as a URL and is
    /// never persisted.
    func apply(scannedPayload data: Data) {
        let payload: DiagnosticPairingPayload
        do {
            payload = try DiagnosticPairingPayload.decode(data)
        } catch {
            status = .invalidScannedCode
            clearToken()
            return
        }
        guard DiagnosticAddressPolicy.isAllowedClientHost(payload.host) else {
            status = .rejectedScannedHost
            clearToken()
            return
        }
        host = payload.host
        port = String(payload.port)
        token = payload.token
        connect()
    }

    func disconnect() {
        attemptID = nil
        session?.close()
        session = nil
        isSessionActive = false
        isConnected = false
        status = .disconnected
        clearToken()
    }

    private func apply(_ state: DiagnosticClientState, attemptID: UUID) {
        guard self.attemptID == attemptID else { return }
        switch state {
        case .connecting:
            status = .connecting
        case .authenticating:
            status = .authenticating
        case .connected:
            isConnected = true
            status = .connected
        case .disconnected:
            self.attemptID = nil
            isConnected = false
            isSessionActive = false
            session = nil
            status = .disconnected
        case let .failed(failure):
            self.attemptID = nil
            isConnected = false
            isSessionActive = false
            session = nil
            status = .failed(failure)
        }
        clearToken()
    }

    private func clearToken() {
        token = ""
    }
}

private struct ConnectionScreen: View {
    @Bindable var model: ConnectionModel
    @State private var isScanning = false

    var body: some View {
        Form {
            Section {
                Button("button.scan") {
                    isScanning = true
                }
                .disabled(model.isSessionActive)
            } footer: {
                Text("text.scan_hint")
            }

            Section("section.listener") {
                TextField("field.host", text: $model.host)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("field.port", text: $model.port)
                    .keyboardType(.numberPad)
                SecureField("field.token", text: $model.token)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }

            Section {
                Button("button.connect") {
                    model.connect()
                }
                .disabled(
                    model.isSessionActive
                        || model.host.isEmpty
                        || model.port.isEmpty
                        || model.token.isEmpty
                )

                Button("button.disconnect", role: .destructive) {
                    model.disconnect()
                }
                .disabled(!model.isSessionActive)
            }

            Section("section.status") {
                Text(model.status.localized)
                    .accessibilityIdentifier("connection-status")
            }

            Section("section.safety") {
                Text("text.safety_boundary")
                    .font(.footnote)
            }
        }
        .navigationTitle("app.title")
        .sheet(isPresented: $isScanning) {
            NavigationStack {
                QRScannerView(
                    onScan: { data in
                        isScanning = false
                        model.apply(scannedPayload: data)
                    },
                    onUnavailable: {
                        isScanning = false
                        model.status = .cameraUnavailable
                    }
                )
                .ignoresSafeArea(edges: .bottom)
                .navigationTitle("scanner.title")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("button.cancel") { isScanning = false }
                    }
                }
            }
        }
    }
}
