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
    /// GitHub flavour: `alert` kind (`note`, `tip`, `important`, `warning`, `caution`).
    public let alert: String?
    /// GitHub flavour: footnote label and number.
    public let label: String?
    public let number: Int?
    /// GitHub flavour: a `task` box's order and the UTF-16 source offset of
    /// the character inside its `[ ]`, and whether it is ticked.
    public let index: Int?
    public let offset: Int?
    public let checked: Bool?
    /// GitHub flavour: a `diagram` span's source (its `language` is set too).
    public let source: String?
    /// GitHub flavour: the element of an `html` span (`kbd`, `sub`, `details`, ...).
    public let tag: String?
    public var range: NSRange { NSRange(location: start, length: end - start) }
}

/// Which dialect a render uses. `.standard` is the plainest (raw HTML shows
/// as text); `.github` is for PR, issue and comment bodies; `.agent` is for
/// coding agents' answers: everything `.github` draws except `@mention`,
/// `#123` and commit references, which stay text.
public enum MarkitFlavor: UInt32, Sendable {
    case standard = 0
    case github = 1
    case agent = 2
}
/// A link into the host's world. `kind` is `file`, `thread` or `pull_request`
/// in any flavour; the GitHub flavour (not `.agent`) adds `mention` (`login` or
/// `org/team`), `issue` (`#123` or `owner/repo#123`), `commit` (a SHA or
/// `owner/repo@sha`) and, in inline views, `footnote` (the footnote's label).
public struct MarkitReference: Sendable, Equatable {
    public let kind: String
    public let target: String
    public init(kind: String, target: String) { self.kind = kind; self.target = target }
}
/// A GitHub-flavour task box the reader tapped. `offset` is the UTF-16 index
/// of the character inside its `[ ]` in the Markdown source, so the host
/// rewrites exactly that character (`" "` or `"x"`); `index` is the box's
/// order and `checked` is the state the reader asked for.
public struct MarkitTask: Sendable, Equatable {
    public let index: Int
    public let offset: Int
    public let checked: Bool
    public init(index: Int, offset: Int, checked: Bool) { self.index = index; self.offset = offset; self.checked = checked }
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
    public static func parse(_ source: String, flavor: MarkitFlavor = .standard) throws -> MarkitDocument {
        try request(Array(source.utf8), format: 0, flavor: flavor)
    }
    /// Colours the whole source (up to 8 MiB of UTF-8; larger sources stay plain).
    /// Unknown non-empty languages use a generic lexer; "" and "text" stay plain.
    /// Large files decode faster through `MarkitBinaryPlan.highlight`.
    public static func highlight(_ source: String, language: String, fullGrammar: Bool = false) throws -> MarkitDocument {
        let data = try JSONSerialization.data(withJSONObject: ["source": source, "language": language])
        return try request(Array(data), format: fullGrammar ? 3 : 2)
    }
    private static func request(_ bytes: [UInt8], format: UInt32, flavor: MarkitFlavor = .standard) throws -> MarkitDocument {
        guard bytes.count <= 16 * 1024 * 1024 else { throw MarkitError.documentTooLarge }
        let buffer = bytes.withUnsafeBufferPointer { markit_render_flavor($0.baseAddress, $0.count, format, flavor.rawValue) }
        return try decode(buffer)
    }
    static func decode<T: Decodable>(_ buffer: MarkitBuffer) throws -> T {
        defer { markit_buffer_free(buffer) }
        guard buffer.status == 0, let pointer = buffer.data else { throw MarkitError.renderFailed(status: buffer.status) }
        let data = Data(bytes: pointer, count: buffer.len)
        // Documents are dense span arrays; the manual bridge decoder avoids
        // JSONDecoder's per-value boxing (see BridgeDecoder). Anything else
        // keeps the generic path.
        if T.self == MarkitDocument.self {
            return try BridgeDecoder.document(from: data) as! T
        }
        return try JSONDecoder().decode(T.self, from: data)
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
    public let flavor: MarkitFlavor
    public init(flavor: MarkitFlavor = .standard) {
        self.flavor = flavor
        handle = markit_stream_new_flavor(flavor.rawValue)!
    }
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
