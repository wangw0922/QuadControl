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
            let host = options.configuration.mode == .loopback
                ? "127.0.0.1"
                : "private address of the selected interface"
            do {
                let displayToken = try PairingToken.encode(token.value)
                try terminal.write(
                    """
                    QuadControl diagnostic listener ready
                    Host: \(host)
                    Port: \(port)
                    Temporary token (shown once; expires in 180 seconds):
                    \(displayToken)
                    No screen, input, clipboard, files, relay, or unattended access.
                    Terminal scrollback and screenshots can expose this token. Ctrl-C stops.
                    """
                )
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
