import Foundation
import CMarkit

public enum MarkitBinaryError: Error { case malformed, unsupportedVersion }

/// Direct v1 reader. Owns the Rust allocation until deinit; records and blobs are
/// read on demand. UTF-16 ranges index text. No JSON or base64 decoding occurs.
public final class MarkitBinaryPlan {
    private let buffer: MarkitBuffer
    public let text: String
    public let documentVersion: UInt32
    public let spanCount: Int
    public let blockCount: Int
    private let spansAt: Int
    private let blocksAt: Int
    private let tableAt: Int
    private let blobsAt: Int
    private let entryCount: Int

    private init(_ buffer: MarkitBuffer) throws {
        guard buffer.status == 0, let pointer = buffer.data else {
            markit_buffer_free(buffer)
            throw MarkitError.renderFailed(status: buffer.status)
        }
        let bytes = UnsafeRawBufferPointer(start: pointer, count: buffer.len)
        do {
            guard bytes.count >= 32, Array(bytes[0..<4]) == [77, 75, 66, 80] else { throw MarkitBinaryError.malformed }
            func word(_ at: Int) -> UInt32 {
                UInt32(bytes[at]) | UInt32(bytes[at + 1]) << 8 | UInt32(bytes[at + 2]) << 16 | UInt32(bytes[at + 3]) << 24
            }
            guard word(4) == 1 else { throw MarkitBinaryError.unsupportedVersion }
            let textLength = Int(word(12)), spans = Int(word(16)), blocks = Int(word(20)), entries = Int(word(24))
            let s = 32 + textLength, b = s + spans * 40, t = b + blocks * 12, blob = t + entries * 8
            guard blob <= bytes.count, Int(word(28)) == bytes.count - blob,
                  let text = String(bytes: bytes[32..<s], encoding: .utf8) else { throw MarkitBinaryError.malformed }
            for i in 0..<entries {
                let at = t + i * 8, offset = Int(word(at)), length = Int(word(at + 4))
                guard offset <= bytes.count - blob, length <= bytes.count - blob - offset else { throw MarkitBinaryError.malformed }
            }
            self.buffer = buffer
            self.text = text
            self.documentVersion = word(8)
            self.spanCount = spans; self.blockCount = blocks; self.entryCount = entries
            self.spansAt = s; self.blocksAt = b; self.tableAt = t; self.blobsAt = blob
        } catch {
            markit_buffer_free(buffer)
            throw error
        }
    }
    deinit { markit_buffer_free(buffer) }
    private func word(_ at: Int) -> UInt32 {
        let p = buffer.data!
        return UInt32(p[at]) | UInt32(p[at + 1]) << 8 | UInt32(p[at + 2]) << 16 | UInt32(p[at + 3]) << 24
    }
    public struct Record {
        public let start: Int
        public let end: Int
        public let tag: UInt32
        /// Payload interpretation is specified in docs/binary-transport.md.
        public let arguments: [UInt32]
    }
    public func span(at index: Int) throws -> Record {
        guard index >= 0, index < spanCount else { throw MarkitBinaryError.malformed }
        let at = spansAt + index * 40
        return Record(start: Int(word(at)), end: Int(word(at + 4)), tag: word(at + 8),
                      arguments: (0..<7).map { word(at + 12 + $0 * 4) })
    }
    public func block(at index: Int) throws -> Record {
        guard index >= 0, index < blockCount else { throw MarkitBinaryError.malformed }
        let at = blocksAt + index * 12
        return Record(start: Int(word(at)), end: Int(word(at + 4)), tag: 0, arguments: [word(at + 8)])
    }
    /// Copies just this entry. Math PNG entries contain raw PNG bytes.
    public func blob(_ id: UInt32) throws -> Data {
        guard Int(id) < entryCount else { throw MarkitBinaryError.malformed }
        let at = tableAt + Int(id) * 8
        return Data(bytes: buffer.data! + blobsAt + Int(word(at)), count: Int(word(at + 4)))
    }
    public func string(_ id: UInt32) throws -> String {
        guard let text = String(data: try blob(id), encoding: .utf8) else { throw MarkitBinaryError.malformed }
        return text
    }
    public static func parse(_ source: String) throws -> MarkitBinaryPlan {
        let bytes = Array(source.utf8)
        guard bytes.count <= 16 * 1024 * 1024 else { throw MarkitError.documentTooLarge }
        return try bytes.withUnsafeBufferPointer { try MarkitBinaryPlan(markit_render_binary($0.baseAddress, $0.count)) }
    }
    public static func highlight(_ source: String, language: String, fullGrammar: Bool = false) throws -> MarkitBinaryPlan {
        let source = Array(source.utf8), language = Array(language.utf8)
        guard source.count <= 16 * 1024 * 1024, language.count <= 16 * 1024 * 1024 else { throw MarkitError.documentTooLarge }
        return try source.withUnsafeBufferPointer { s in
            try language.withUnsafeBufferPointer { l in
                var request = MarkitCodeRequest(source: s.baseAddress, source_len: s.count,
                                               language: l.baseAddress, language_len: l.count, mode: fullGrammar ? 1 : 0)
                return try MarkitBinaryPlan(markit_code_binary(&request))
            }
        }
    }
}
