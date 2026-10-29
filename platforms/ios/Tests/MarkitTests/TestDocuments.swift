import Foundation
@testable import Markit

/// Builds a document from native JSON, the same shape the renderer exports.
func makeDocument(_ json: String) throws -> MarkitDocument {
    try JSONDecoder().decode(MarkitDocument.self, from: Data(json.utf8))
}

/// JSON string literal for `text`.
func jsonString(_ text: String) -> String {
    let data = try? JSONSerialization.data(withJSONObject: [text], options: [])
    let array = data.flatMap { String(data: $0, encoding: .utf8) } ?? "[\"\"]"
    return String(array.dropFirst().dropLast())
}
