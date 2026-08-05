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

@MainActor
@Observable
final class ConnectionModel {
    var host = ""
    var port = "47100"
    var token = ""
    var status = "Not connected"
    var isConnected = false
    var isSessionActive = false

    private var session: DiagnosticClientSession?
    private var attemptID: UUID?

    func connect() {
        guard session == nil,
              !host.isEmpty,
              let port = UInt16(port),
              port > 0 else {
            status = "Enter a valid private host and port"
            clearToken()
            return
        }

        let enteredToken: Data
        do {
            enteredToken = try PairingToken.decode(token)
        } catch {
            status = "Temporary token must be the 43-character value shown by the Mac"
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
            status = "Connecting…"
            next.start { [weak self] result in
                Task { @MainActor in
                    guard self?.attemptID == nextAttemptID else { return }
                    if case let .failure(failure) = result {
                        self?.attemptID = nil
                        self?.session = nil
                        self?.isSessionActive = false
                        self?.isConnected = false
                        self?.status = "Connection failed (\(failure.rawValue))"
                    }
                    self?.clearToken()
                }
            }
        } catch {
            attemptID = nil
            session = nil
            isSessionActive = false
            isConnected = false
            status = "Connection setup failed"
            clearToken()
        }
    }

    func disconnect() {
        attemptID = nil
        session?.close()
        session = nil
        isSessionActive = false
        isConnected = false
        status = "Disconnected"
        clearToken()
    }

    private func apply(_ state: DiagnosticClientState, attemptID: UUID) {
        guard self.attemptID == attemptID else { return }
        switch state {
        case .connecting:
            status = "Connecting…"
        case .authenticating:
            status = "Authenticating diagnostic session…"
        case .connected:
            isConnected = true
            status = "Connected; authenticated heartbeats active"
        case .disconnected:
            self.attemptID = nil
            isConnected = false
            isSessionActive = false
            session = nil
            status = "Disconnected"
        case let .failed(failure):
            self.attemptID = nil
            isConnected = false
            isSessionActive = false
            session = nil
            status = "Connection failed (\(failure.rawValue))"
        }
        clearToken()
    }

    private func clearToken() {
        token = ""
    }
}

private struct ConnectionScreen: View {
    @Bindable var model: ConnectionModel

    var body: some View {
        Form {
            Section("Mac diagnostic listener") {
                TextField("Private host", text: $model.host)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                TextField("Port", text: $model.port)
                    .keyboardType(.numberPad)
                SecureField("43-character temporary token", text: $model.token)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
            }

            Section {
                Button("Connect diagnostic session") {
                    model.connect()
                }
                .disabled(
                    model.isSessionActive
                        || model.host.isEmpty
                        || model.port.isEmpty
                        || model.token.isEmpty
                )

                Button("Disconnect", role: .destructive) {
                    model.disconnect()
                }
                .disabled(!model.isSessionActive)
            }

            Section("Status") {
                Text(model.status)
                    .accessibilityIdentifier("connection-status")
            }

            Section("Safety boundary") {
                Text(
                    "Development connection diagnostics only. No screen sharing, remote control, clipboard, files, relay, or production encrypted control session."
                )
                .font(.footnote)
            }
        }
        .navigationTitle("QuadControl")
    }
}
