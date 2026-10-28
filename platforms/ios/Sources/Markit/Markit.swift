import CMarkit
import Foundation

public struct MarkitDocument: Decodable, Sendable {
    public let version: Int
    public let text: String
    public let spans: [MarkitSpan]
    public let blocks: [MarkitBlock]
    /// Plain source for a rendered selection, including original math delimiters.
    public func source(in requested: NSRange? = nil) -> String {
        let text = self.text as NSString
        let range = NSIntersectionRange(requested ?? NSRange(location: 0, length: text.length), NSRange(location: 0, length: text.length))
        var result = "", at = range.location
        for span in spans where span.kind == "math" && span.start >= range.location && span.end <= NSMaxRange(range) {
            result += text.substring(with: NSRange(location: at, length: span.start - at))
            result += span.raw ?? span.tex ?? ""; at = span.end
        }
        result += text.substring(with: NSRange(location: at, length: NSMaxRange(range) - at))
        return result
    }
}
public struct MarkitBlock: Decodable, Sendable {
    public let start: Int
    public let end: Int
    public let kind: String
    public var range: NSRange { NSRange(location: start, length: end - start) }
}
public struct MarkitSpan: Decodable, Sendable {
    public let start: Int
    public let end: Int
    public let kind: String
    public let level: Int?
    public let depth: Int?
    public let language: String?
    public let url: String?
    public let file: String?
    public let diff: Bool?
    public let light: String?
    public let dark: String?
    public let change: String?
    public let tex: String?
    public let raw: String?
    public let display: Bool?
    public let png: String?
    public let width: Int?
    public let height: Int?
    public let row: Int?
    public let column: Int?
    public let header: Bool?
    public let alignment: String?
    public let target: String?
    public let reference_kind: String?
    public var range: NSRange { NSRange(location: start, length: end - start) }
}
public struct MarkitReference: Sendable {
    public let kind: String
    public let target: String
}
public struct MarkitPatch: Decodable, Sendable {
    public struct Block: Decodable, Sendable { public let document: MarkitDocument }
    public let replace_from: Int
    public let blocks: [Block]
    public let finalized: Bool
    public let source_bytes: Int
    public let parsed_bytes: Int
}
public enum MarkitError: Error { case renderFailed(status: UInt32), documentTooLarge }

public enum Markit {
    public static func parse(_ source: String) throws -> MarkitDocument {
        try request(Array(source.utf8), format: 0)
    }
    public static func highlight(_ source: String, language: String, fullGrammar: Bool = false) throws -> MarkitDocument {
        let data = try JSONSerialization.data(withJSONObject: ["source": source, "language": language])
        return try request(Array(data), format: fullGrammar ? 3 : 2)
    }
    private static func request(_ bytes: [UInt8], format: UInt32) throws -> MarkitDocument {
        guard bytes.count <= 16 * 1024 * 1024 else { throw MarkitError.documentTooLarge }
        let buffer = bytes.withUnsafeBufferPointer { markit_render($0.baseAddress, $0.count, format) }
        return try decode(buffer)
    }
    static func decode<T: Decodable>(_ buffer: MarkitBuffer) throws -> T {
        defer { markit_buffer_free(buffer) }
        guard buffer.status == 0, let pointer = buffer.data else { throw MarkitError.renderFailed(status: buffer.status) }
        return try JSONDecoder().decode(T.self, from: Data(bytes: pointer, count: buffer.len))
    }
}

/// Each stream serializes access to its Rust handle. Release occurs in deinit.
/// Patches cross the bridge as MKNP v2 binary, not JSON: an open code fence
/// sends only its appended text and changed spans, applied to the block this
/// stream already holds. The returned `MarkitPatch` is unchanged.
public final class MarkitStream: @unchecked Sendable {
    private let handle: OpaquePointer
    private let lock = NSLock()
    /// Current blocks, the base for fence extensions.
    private var documents: [MarkitDocument] = []
    private var failed = false
    public init() { handle = markit_stream_new()! }
    deinit { markit_stream_free(handle) }
    public func append(_ delta: String, finalized: Bool = false) throws -> MarkitPatch {
        lock.lock(); defer { lock.unlock() }
        // A rejected patch leaves Rust ahead of `documents`; later extensions
        // would apply to the wrong base, so the stream stays failed.
        guard !failed else { throw MarkitBinaryError.malformed }
        let bytes = Array(delta.utf8)
        let buffer = bytes.withUnsafeBufferPointer { markit_stream_append_patch(handle, $0.baseAddress, $0.count, finalized ? 1 : 0) }
        do {
            return try NativePatchReader.apply(buffer, to: &documents)
        } catch {
            // Status 1/2 reject the input before Rust changes state.
            if buffer.status == 0 || buffer.status == 3 { failed = true }
            throw error
        }
    }
}
