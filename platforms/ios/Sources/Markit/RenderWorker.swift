import Foundation

/// One pending replacement per view. A slow parse cannot queue every keystroke.
final class RenderWorker: @unchecked Sendable {
    struct Output: Sendable { let documents: [MarkitDocument]; let changedFrom: Int }
    struct Job { let source: String; let finalized: Bool; let completion: @Sendable (Result<Output, Error>) -> Void }
    private let lock = NSLock()
    private let queue = DispatchQueue(label: "dev.markit.render", qos: .userInitiated)
    // `pending` and `running` are guarded by `lock`. The stream state below is
    // touched only by whoever set `running`, so one render runs at a time.
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

    /// Renders on the calling thread when nothing is queued or running, so a
    /// view can show its first content in the same layout pass. Nil when busy.
    func renderNow(_ source: String, finalized: Bool) -> Result<Output, Error>? {
        lock.lock()
        guard !running, pending == nil else { lock.unlock(); return nil }
        running = true
        lock.unlock()
        let result = process(source, finalized: finalized)
        lock.lock()
        let restart = pending != nil
        if !restart { running = false }
        lock.unlock()
        if restart { queue.async { self.drain() } }
        return result
    }

    private func drain() {
        while true {
            lock.lock()
            guard let job = pending else { running = false; lock.unlock(); return }
            pending = nil
            lock.unlock()
            job.completion(process(job.source, finalized: job.finalized))
        }
    }

    private func process(_ source: String, finalized: Bool) -> Result<Output, Error> {
        do {
            if !source.hasPrefix(previous) { stream = MarkitStream(); previous = ""; documents = [] }
            // UTF-8 prefix removal preserves Unicode scalar boundaries.
            let delta = String(decoding: source.utf8.dropFirst(previous.utf8.count), as: UTF8.self)
            let patch = try stream.append(delta, finalized: finalized)
            documents.removeSubrange(patch.replace_from..<documents.count)
            documents.append(contentsOf: patch.blocks.map(\.document))
            previous = source
            return .success(Output(documents: documents, changedFrom: patch.replace_from))
        } catch {
            stream = MarkitStream(); previous = ""; documents = []
            return .failure(error)
        }
    }
}
