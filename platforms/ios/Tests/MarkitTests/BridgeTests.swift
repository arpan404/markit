import XCTest
@testable import Markit

final class BridgeTests: XCTestCase {
    func testUnicodeRangesAndHighlightingAcrossRealCBridge() throws {
        let document = try Markit.parse("# Hello 🦀\n\n**bold 😀**")
        XCTAssertEqual(document.version, 2)
        let bold = try XCTUnwrap(document.spans.first { $0.kind == "strong" })
        XCTAssertEqual((document.text as NSString).substring(with: bold.range), "bold 😀")
        let code = try Markit.highlight("let x = \"😀\";\n", language: "rust")
        XCTAssertEqual(code.text,"let x = \"😀\";\n")
        XCTAssertTrue(code.spans.contains { $0.kind == "syntax" })
    }
    func testMathHasNativeAttachmentBytesAndCopySource() throws {
        let document = try Markit.parse(#"Math \(\frac{x^2}{2}\)"#)
        let math = try XCTUnwrap(document.spans.first { $0.kind == "math" })
        XCTAssertEqual(math.raw,#"\(\frac{x^2}{2}\)"#)
        XCTAssertEqual(document.source().trimmingCharacters(in: .whitespacesAndNewlines), #"Math \(\frac{x^2}{2}\)"#)
        let data = try XCTUnwrap(math.png.flatMap { Data(base64Encoded: $0) })
        XCTAssertEqual(Array(data.prefix(4)),[137,80,78,71])
    }
    func testStreamKeepsPriorBlocksAndHandlesLateDefinitions() throws {
        let stream = MarkitStream()
        var documents: [MarkitDocument] = []
        for delta in ["[docs][ref]\n\nOpen tail", " continues\n\n[ref]: https://example.com\n"] {
            let patch = try stream.append(delta)
            documents.removeSubrange(patch.replace_from..<documents.count)
            documents.append(contentsOf: patch.blocks.map(\.document))
        }
        XCTAssertTrue(documents[0].spans.contains { $0.url == "https://example.com" })
        XCTAssertEqual(documents.map(\.text).joined(), "docs\nOpen tail continues\n")
    }
}
