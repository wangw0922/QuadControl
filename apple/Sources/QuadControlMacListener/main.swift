import Foundation
import QuadControlDiagnosticProtocol
import QuadControlDiagnosticServer

private struct ListenerOptions {
    let configuration: DiagnosticServerConfiguration

    static func parse(_ arguments: [String]) throws -> ListenerOptions {
        var port: UInt16 = 47_100
        var lan = false
        var interface: String?
        var index = 0

        while index < arguments.count {
            switch arguments[index] {
            case "--lan":
                lan = true
                index += 1
            case "--interface":
                guard index + 1 < arguments.count else { throw DiagnosticFailure.malformed }
                interface = arguments[index + 1]
                index += 2
            case "--port":
                guard index + 1 < arguments.count,
                      let parsed = UInt16(arguments[index + 1]),
                      parsed > 0 else {
                    throw DiagnosticFailure.malformed
                }
                port = parsed
                index += 2
            default:
                throw DiagnosticFailure.malformed
            }
        }

        let mode: DiagnosticListenerMode
        if lan {
            switch interface {
            case "wifi": mode = .lanWiFi
            case "wired": mode = .lanWired
            default: throw DiagnosticFailure.malformed
            }
        } else {
            guard interface == nil else { throw DiagnosticFailure.malformed }
            mode = .loopback
        }
        return ListenerOptions(
            configuration: DiagnosticServerConfiguration(port: port, mode: mode)
        )
    }
}

private final class TerminalWriter: @unchecked Sendable {
    private let handle: FileHandle
    private let lock = NSLock()

    init?() {
        guard let handle = FileHandle(forWritingAtPath: "/dev/tty") else { return nil }
        self.handle = handle
    }

    func write(_ text: String) throws {
        guard let data = "\(text)\n".data(using: .utf8) else {
            throw DiagnosticFailure.malformed
        }
        lock.lock()
        defer { lock.unlock() }
        try handle.write(contentsOf: data)
    }
}

private final class SanitizedLogger: @unchecked Sendable {
    private let lock = NSLock()

    func write(_ text: String) {
        lock.lock()
        defer { lock.unlock() }
        fputs("\(text)\n", stderr)
    }
}

private func shortID(_ id: UUID) -> String {
    String(id.uuidString.prefix(8))
}

/// A command-line tool has no application bundle, so CFBundle resolves `Bundle.module`
/// strings against the development region rather than the user's languages — every
/// string silently comes back English. Resolving the best-matching `.lproj` explicitly
/// makes the token block follow the system language.
private let localizationBundle: Bundle = {
    let preferred = Bundle.preferredLocalizations(
        from: Bundle.module.localizations,
        forPreferences: Locale.preferredLanguages
    )
    guard let code = preferred.first,
          let path = Bundle.module.path(forResource: code, ofType: "lproj"),
          let bundle = Bundle(path: path) else {
        return .module
    }
    return bundle
}()

/// Only the one-time token block shown on `/dev/tty` is localized. Every stderr log
/// line stays in English because those are machine-readable event identifiers that
/// the troubleshooting docs and tests match on.
private func localized(_ key: String) -> String {
    localizationBundle.localizedString(forKey: key, value: nil, table: nil)
}

guard let terminal = TerminalWriter() else {
    fputs("refusing launch without an interactive terminal for one-time token display\n", stderr)
    exit(64)
}

private let options: ListenerOptions
do {
    options = try ListenerOptions.parse(Array(CommandLine.arguments.dropFirst()))
} catch {
    fputs(
        "usage: QuadControlMacListener [--port 47100] [--lan --interface wifi|wired]\n",
        stderr
    )
    exit(64)
}

private let logger = SanitizedLogger()
let token = OneTimeToken()
let server = DiagnosticServer(configuration: options.configuration, token: token)

do {
    try server.start { event in
        switch event {
        case let .ready(port):
            let resolvedHost: String?
            switch options.configuration.mode {
            case .loopback: resolvedHost = "127.0.0.1"
            case .lanWiFi: resolvedHost = InterfaceAddress.privateIPv4(for: .wifi)
            case .lanWired: resolvedHost = InterfaceAddress.privateIPv4(for: .wiredEthernet)
            }
            do {
                let displayToken = try PairingToken.encode(token.value)
                var block = """
                \(localized("listener.ready.title"))
                \(String(format: localized("listener.ready.host"), resolvedHost ?? localized("listener.host.unresolved")))
                \(String(format: localized("listener.ready.port"), String(port)))
                \(localized("listener.ready.token_caption"))
                \(displayToken)
                """
                // The QR is a convenience for the same values printed above; if the
                // address cannot be resolved or the code cannot be rendered, the
                // listener still starts and the values can be entered by hand.
                if let resolvedHost {
                    let payload = DiagnosticPairingPayload(
                        host: resolvedHost,
                        port: port,
                        token: displayToken
                    )
                    if let encoded = try? payload.encoded(),
                       let qrLines = TerminalQRCode.lines(for: encoded) {
                        block += "\n\(localized("listener.ready.qr_caption"))\n"
                        block += qrLines.joined(separator: "\n")
                    }
                }
                block += "\n\(localized("listener.ready.scope"))"
                block += "\n\(localized("listener.ready.warning"))"
                try terminal.write(block)
            } catch {
                logger.write("listener failed: terminal-output")
                exit(1)
            }
            logger.write("listener ready")
        case let .connectionAccepted(id):
            logger.write("connection \(shortID(id)): handshake")
        case let .authenticated(id):
            logger.write("connection \(shortID(id)): authenticated")
        case let .heartbeat(id, sequence):
            logger.write("connection \(shortID(id)): heartbeat \(sequence)")
        case let .connectionClosed(id, reason):
            logger.write("connection \(shortID(id)): closed \(reason.rawValue)")
        case .stopped:
            logger.write("listener stopped")
        case .failed:
            logger.write("listener failed: network")
            exit(1)
        }
    }
} catch {
    logger.write("listener failed: startup")
    exit(1)
}

dispatchMain()
