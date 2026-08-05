import Foundation
import QuadControlDiagnosticProtocol

public final class DiagnosticAdmissionController: @unchecked Sendable {
    private let lock = NSLock()
    private var pending = Set<UUID>()
    private var active: UUID?
    private var globalFailures: [UInt64] = []
    private var peerFailures: [String: [UInt64]] = [:]

    public init() {}

    public func begin(
        connectionID: UUID,
        peerKey: String,
        nowNanos: UInt64 = monotonicNowNanos()
    ) -> Result<Void, DiagnosticFailure> {
        lock.lock()
        defer { lock.unlock() }
        prune(nowNanos: nowNanos)
        guard active == nil,
              pending.count < DiagnosticConstants.maxPending else {
            return .failure(.resourceLimit)
        }
        guard globalFailures.count < DiagnosticConstants.maxFailuresGlobal,
              peerFailures[peerKey, default: []].count < DiagnosticConstants.maxFailuresPerPeer else {
            return .failure(.rateLimited)
        }
        pending.insert(connectionID)
        return .success(())
    }

    public func authenticationSucceeded(connectionID: UUID) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard pending.remove(connectionID) != nil,
              active == nil else {
            return false
        }
        active = connectionID
        return true
    }

    public func recordFailure(
        connectionID: UUID,
        peerKey: String,
        nowNanos: UInt64 = monotonicNowNanos()
    ) {
        lock.lock()
        defer { lock.unlock() }
        pending.remove(connectionID)
        prune(nowNanos: nowNanos)
        globalFailures.append(nowNanos)
        peerFailures[peerKey, default: []].append(nowNanos)
    }

    public func end(connectionID: UUID) {
        lock.lock()
        defer { lock.unlock() }
        pending.remove(connectionID)
        if active == connectionID {
            active = nil
        }
    }

    public func counts() -> (pending: Int, active: Int) {
        lock.lock()
        defer { lock.unlock() }
        return (pending.count, active == nil ? 0 : 1)
    }

    private func prune(nowNanos: UInt64) {
        let cutoff = nowNanos > DiagnosticConstants.failureWindowNanos
            ? nowNanos - DiagnosticConstants.failureWindowNanos
            : 0
        globalFailures.removeAll { $0 < cutoff }
        peerFailures = peerFailures.reduce(into: [:]) { result, entry in
            let recent = entry.value.filter { $0 >= cutoff }
            if !recent.isEmpty {
                result[entry.key] = recent
            }
        }
    }
}
