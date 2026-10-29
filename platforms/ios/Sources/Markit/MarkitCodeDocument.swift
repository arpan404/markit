import CMarkit
import Foundation

extension Markit {
    /// Highlights `source` through the binary transport (no JSON), for source
    /// files that can hold tens of thousands of syntax runs. Same document as
    /// `highlight(_:language:fullGrammar:)`.
    static func codeDocument(_ source: String, language: String, fullGrammar: Bool) throws -> MarkitDocument {
        let source = Array(source.utf8), language = Array(language.utf8)
        guard source.count <= 16 * 1024 * 1024, language.count <= 16 * 1024 * 1024 else { throw MarkitError.documentTooLarge }
        let buffer = source.withUnsafeBufferPointer { s in
            language.withUnsafeBufferPointer { l in
                var request = MarkitCodeRequest(source: s.baseAddress, source_len: s.count,
                                               language: l.baseAddress, language_len: l.count, mode: fullGrammar ? 1 : 0)
                return markit_code_binary(&request)
            }
        }
        defer { markit_buffer_free(buffer) }
        guard buffer.status == 0, let pointer = buffer.data else { throw MarkitError.renderFailed(status: buffer.status) }
        return try NativePatchReader.document(UnsafeRawBufferPointer(start: pointer, count: buffer.len))
    }
}
