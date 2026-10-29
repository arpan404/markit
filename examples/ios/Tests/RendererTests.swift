import XCTest
import UIKit
@testable import Markit

final class RendererTests: XCTestCase {
    @MainActor func testCodeCellsReuseSubviewsWithoutStaleText() throws {
        let lines = (0..<1000).map { "let value_\($0) = \"😀\";" }
        let document = try Markit.parse("```rust\n" + lines.joined(separator: "\n") + "\n```\n")
        let block = try XCTUnwrap(document.blocks.first { $0.kind == "code" })
        let pane = MarkitRowsPane(document: document, range: block.range, kind: "code", availableWidth: 390)
        pane.frame = CGRect(x: 0, y: 0, width: 390, height: 360); pane.layoutIfNeeded()
        func codeTable(_ view: UIView) -> UITableView? {
            if let table = view as? UITableView { return table }
            return view.subviews.lazy.compactMap { codeTable($0) }.first
        }
        let table = try XCTUnwrap(codeTable(pane))
        var contents: [ObjectIdentifier: UIView] = [:], reused = 0
        for offset in [CGFloat(0), max(0, table.contentSize.height - table.bounds.height), 0, max(0, table.contentSize.height - table.bounds.height), 0] {
            table.setContentOffset(CGPoint(x: 0, y: offset), animated: false); table.layoutIfNeeded()
            XCTAssertLessThan(table.visibleCells.count, 50)
            for cell in table.visibleCells {
                let row = try XCTUnwrap(table.indexPath(for: cell)).row
                let stack = try XCTUnwrap(cell.contentView.subviews.first as? UIStackView)
                if let previous = contents[ObjectIdentifier(cell)] { XCTAssertTrue(previous === stack); reused += 1 }
                contents[ObjectIdentifier(cell)] = stack
                XCTAssertEqual((stack.arrangedSubviews[0] as? UILabel)?.text, String(row + 1))
                XCTAssertEqual((stack.arrangedSubviews[1] as? UILabel)?.text, lines[row])
            }
        }
        XCTAssertGreaterThan(reused, 0)
    }

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
        view.setDocument(try Markit.parse("")); view.layoutIfNeeded()
        XCTAssertEqual(table.numberOfRows(inSection: 0),0)
        view.setDocument(first); view.layoutIfNeeded()
        XCTAssertEqual(table.numberOfRows(inSection: 0),first.blocks.count)
    }

    @MainActor func testReplacementsReconfigureCellsInPlace() throws {
        let view = MarkitView(frame: CGRect(x: 0, y: 0, width: 390, height: 800))
        let table = try XCTUnwrap(view.subviews.compactMap { $0 as? UITableView }.first)
        func text(_ cell: UITableViewCell?) -> String? {
            func find(_ node: UIView) -> UITextView? { (node as? UITextView) ?? node.subviews.lazy.compactMap(find).first }
            return cell.flatMap { find($0.contentView) }?.text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        func pane(_ node: UIView) -> MarkitRowsPane? { (node as? MarkitRowsPane) ?? node.subviews.lazy.compactMap(pane).first }
        view.setDocument(try Markit.parse("Hello\n\n```swift\nlet a = 1\n```\n")); view.layoutIfNeeded()
        let first = try XCTUnwrap(table.cellForRow(at: IndexPath(row: 0, section: 0)))
        XCTAssertEqual(text(first), "Hello")
        XCTAssertNotNil(pane(table))
        view.setDocument(try Markit.parse("Hello world\n\n```swift\nlet a = 2\n```\n")); view.layoutIfNeeded()
        let next = try XCTUnwrap(table.cellForRow(at: IndexPath(row: 0, section: 0)))
        XCTAssertTrue(first === next)
        XCTAssertEqual(text(next), "Hello world")
        // A row that changes kind gets the other cell type.
        view.setDocument(try Markit.parse("```swift\nlet b = 3\n```\n\nTail")); view.layoutIfNeeded()
        XCTAssertNotNil(pane(try XCTUnwrap(table.cellForRow(at: IndexPath(row: 0, section: 0)))))
        XCTAssertEqual(text(table.cellForRow(at: IndexPath(row: 1, section: 0))), "Tail")
    }
}
