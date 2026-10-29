import XCTest
import UIKit
@testable import Markit

final class RendererTests: XCTestCase {
    @MainActor func testIndexedCodeStylesMatchFullSpanScan() throws {
        let document = try Markit.parse("```rust\nlet café = \"😀\";\n/* multi\nline */\n\n```\n")
        let source = document.text as NSString
        var lines: [NSRange] = [], start = 0
        while start < source.length {
            let next = source.range(of: "\n", range: NSRange(location: start, length: source.length - start))
            let end = next.location == NSNotFound ? source.length : next.location
            lines.append(NSRange(location: start, length: end - start)); start = end + 1
        }
        let buckets = spansByLine(document.spans, lines: lines)
        for traits in [UITraitCollection(userInterfaceStyle: .light), UITraitCollection(userInterfaceStyle: .dark)] {
            for (index, line) in lines.enumerated() {
                XCTAssertTrue(attributed(document, range: line, traits: traits).isEqual(to: attributed(document, range: line, traits: traits, candidates: buckets[index])))
            }
        }
    }
    @MainActor func testDocumentsCanBeSetBeforeFirstLayoutAndReplaced() throws {
        let view = MarkitView(frame: CGRect(x: 0, y: 0, width: 390, height: 800))
        let first = try Markit.parse("First paragraph.\n\nSecond paragraph.")
        view.setDocument(first)
        view.layoutIfNeeded()
        let table = try XCTUnwrap(view.subviews.compactMap { $0 as? UITableView }.first)
        XCTAssertEqual(table.numberOfRows(inSection: 0), first.blocks.count)
        let next = try Markit.parse("Replacement.")
        view.setDocument(next)
        view.layoutIfNeeded()
        XCTAssertEqual(table.numberOfRows(inSection: 0), next.blocks.count)
    }
}
