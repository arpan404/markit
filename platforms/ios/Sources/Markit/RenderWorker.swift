import Foundation

/// One pending replacement per view. A slow parse cannot queue every keystroke.
final class RenderWorker: @unchecked Sendable {
    struct Output { let documents: [MarkitDocument]; let changedFrom: Int }
    struct Job { let source: String; let finalized: Bool; let completion: @Sendable (Result<Output, Error>) -> Void }
    private let lock = NSLock()
    private let queue = DispatchQueue(label: "dev.markit.render", qos: .userInitiated)
    private var pending: Job?
    private var running = false
    private var stream = MarkitStream()
    private var previous = ""
    private var documents: [MarkitDocument] = []

    func submit(_ source: String, finalized: Bool, completion: @escaping @Sendable (Result<Output, Error>) -> Void) {
        lock.lock()
        pending = Job(source: source, finalized: finalized, completion: completion)
        let start = !running
        running = true
        lock.unlock()
        if start { queue.async { self.drain() } }
    }
    private func drain() {
        while true {
            lock.lock()
            guard let job = pending else { running = false; lock.unlock(); return }
            pending = nil
            lock.unlock()
            do {
                if !job.source.hasPrefix(previous) { stream = MarkitStream(); previous = ""; documents = [] }
                // UTF-8 prefix removal preserves Unicode scalar boundaries.
                let delta = String(decoding: job.source.utf8.dropFirst(previous.utf8.count), as: UTF8.self)
                let patch = try stream.append(delta, finalized: job.finalized)
                documents.removeSubrange(patch.replace_from..<documents.count)
                documents.append(contentsOf: patch.blocks.map(\.document))
                previous = job.source
                job.completion(.success(Output(documents: documents, changedFrom: patch.replace_from)))
            } catch { stream = MarkitStream(); previous = ""; documents = []; job.completion(.failure(error)) }
        }
    }
}
