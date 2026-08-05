import Foundation
import Network
import QuadControlDiagnosticProtocol

public final class FramedConnection: @unchecked Sendable {
    public let connection: NWConnection

    public init(connection: NWConnection) {
        self.connection = connection
    }

    public func start(
        queue: DispatchQueue,
        stateHandler: @escaping @Sendable (NWConnection.State) -> Void
    ) {
        connection.stateUpdateHandler = stateHandler
        connection.start(queue: queue)
    }

    public func cancel() {
        connection.cancel()
    }

    public func send(
        _ wirePayload: Data,
        completion: @escaping @Sendable (DiagnosticFailure?) -> Void
    ) {
        do {
            let frame = try DiagnosticWire.frame(wirePayload)
            connection.send(content: frame, completion: .contentProcessed { error in
                completion(error == nil ? nil : .network)
            })
        } catch let failure as DiagnosticFailure {
            completion(failure)
        } catch {
            completion(.malformed)
        }
    }

    public func receive(
        completion: @escaping @Sendable (Result<Data, DiagnosticFailure>) -> Void
    ) {
        readExactly(4) { [weak self] result in
            guard let self else {
                completion(.failure(.network))
                return
            }
            do {
                let header = try result.get()
                let length = try DiagnosticWire.decodeFrameLength(header)
                self.readExactly(length, completion: completion)
            } catch let failure as DiagnosticFailure {
                self.cancel()
                completion(.failure(failure))
            } catch {
                self.cancel()
                completion(.failure(.malformed))
            }
        }
    }

    private func readExactly(
        _ length: Int,
        accumulated: Data = Data(),
        completion: @escaping @Sendable (Result<Data, DiagnosticFailure>) -> Void
    ) {
        let remaining = length - accumulated.count
        guard remaining > 0 else {
            completion(.success(accumulated))
            return
        }

        connection.receive(
            minimumIncompleteLength: 1,
            maximumLength: remaining
        ) { [weak self] content, _, isComplete, error in
            guard let self else {
                completion(.failure(.network))
                return
            }
            let next = accumulated + (content ?? Data())
            if next.count == length {
                completion(.success(next))
            } else if error != nil || isComplete || content?.isEmpty != false {
                self.cancel()
                completion(.failure(.network))
            } else {
                self.readExactly(length, accumulated: next, completion: completion)
            }
        }
    }
}
