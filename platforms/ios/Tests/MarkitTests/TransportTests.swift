import XCTest
@testable import Markit

final class TransportTests: XCTestCase {
    func testBinaryHighlightMatchesJSONHighlight() throws {
        let source = "fn main() {\n    let x = \"😀\"; // done\n}\n"
        let binary = try Markit.codeDocument(source, language: "rust", fullGrammar: false)
        let json = try Markit.highlight(source, language: "rust")
        XCTAssertEqual(binary.text, json.text)
        let syntax = { (document: MarkitDocument) in document.spans.filter { $0.kind == "syntax" }.map(\.range) }
        XCTAssertFalse(syntax(binary).isEmpty)
        XCTAssertEqual(syntax(binary), syntax(json))
    }

    func testReferenceLinksRoundTripThroughURLs() {
        let reference = MarkitReference(kind: "thread", target: "src/a b.swift?line=3&col=4#x é")
        XCTAssertEqual(MarkitReference(url: reference.url), reference)
        XCTAssertNil(MarkitReference(url: URL(string: "https://example.com/?target=x")!))
    }
}
