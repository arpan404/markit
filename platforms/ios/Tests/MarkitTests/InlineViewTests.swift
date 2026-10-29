#if canImport(UIKit)
import XCTest
@testable import Markit

final class InlineViewTests: XCTestCase {
    @MainActor func testHeightGrowsWithContent() throws {
        let view = MarkitInlineView()
        view.setDocument(try Markit.parse("One line"))
        let short = view.height(for: 320)
        view.setDocument(try Markit.parse("One line\n\nTwo\n\n- three\n- four\n\n```swift\nlet x = 1\n```\n\n| a | b |\n| - | - |\n| 1 | 2 |"))
        let long = view.height(for: 320)
        XCTAssertGreaterThan(short, 0)
        XCTAssertGreaterThan(long, short)
        XCTAssertEqual(view.sizeThatFits(CGSize(width: 320, height: 0)).height, long)
    }

    @MainActor func testNarrowerWidthWrapsTextTaller() throws {
        let view = MarkitInlineView()
        view.setDocument(try Markit.parse(String(repeating: "words that wrap ", count: 40)))
        XCTAssertGreaterThan(view.height(for: 160), view.height(for: 480))
    }

    @MainActor func testSettledBlockViewsSurviveStreamingAppends() throws {
        let view = MarkitInlineView()
        let stream = MarkitStream()
        var documents: [MarkitDocument] = []
        func append(_ delta: String) throws -> Int {
            let patch = try stream.append(delta)
            documents.removeSubrange(patch.replace_from..<documents.count)
            documents.append(contentsOf: patch.blocks.map(\.document))
            view.apply(documents, changedFrom: patch.replace_from)
            return patch.replace_from
        }
        _ = try append("Alpha paragraph.\n\nBeta paragraph.\n\nGamma ")
        let before = view.blocks
        let settledHeight = view.height(for: 320)
        let changedFrom = try append("continues.\n\nDelta arrives.\n\nEpsilon")
        XCTAssertGreaterThan(changedFrom, 0, "the first root should be settled")
        let settled = before.filter { $0.plan.document < changedFrom }
        XCTAssertFalse(settled.isEmpty)
        for block in settled { XCTAssertTrue(view.blocks.contains { $0 === block }) }
        XCTAssertGreaterThan(view.blocks.count, before.count)
        XCTAssertGreaterThan(view.height(for: 320), settledHeight)
    }

    @MainActor func testStreamedRenderReportsItsHeight() {
        let view = MarkitInlineView()
        view.synchronousRenderLimit = 0
        view.frame = CGRect(x: 0, y: 0, width: 320, height: 10)
        let reported = expectation(description: "height reported")
        reported.assertForOverFulfill = false
        view.onContentHeightChange = { height in if height > 0 { reported.fulfill() } }
        view.render("Hello **there**", finalized: false)
        wait(for: [reported], timeout: 10)
        XCTAssertGreaterThan(view.height(for: 320), 0)
    }

    @MainActor func testFirstRenderIsSizedSynchronously() {
        let view = MarkitInlineView()
        view.render("Ready **now**")
        XCTAssertGreaterThan(view.height(for: 320), 0)
    }

    @MainActor func testLongCodeCollapsesAndExpands() throws {
        let view = MarkitInlineView()
        view.codeCollapse = MarkitCodeCollapse(collapseAbove: 3, visibleLines: 2)
        view.setDocument(try Markit.parse("```\n1\n2\n3\n4\n5\n6\n```"))
        view.frame = CGRect(x: 0, y: 0, width: 320, height: view.height(for: 320))
        view.layoutIfNeeded()
        let collapsed = view.height(for: 320)
        let toggle = try XCTUnwrap(buttons(in: view).first { $0.title(for: .normal) == "Show all 6 lines" })
        toggle.sendActions(for: .touchUpInside)
        XCTAssertGreaterThan(view.height(for: 320), collapsed)
        XCTAssertEqual(toggle.title(for: .normal), "Collapse")
    }

    @MainActor func testUnknownBlockKindsShowTheirText() throws {
        let view = MarkitInlineView()
        view.setDocument(try makeDocument(#"{"version":2,"text":"graph TD; A-->B","spans":[{"start":0,"end":15,"kind":"diagram"}],"blocks":[{"start":0,"end":15,"kind":"diagram"}]}"#))
        XCTAssertGreaterThan(view.height(for: 320), 0)
        XCTAssertTrue(view.blocks.first is InlineTextBlockView)
    }

    @MainActor func testReferencesGoToTheHostNotTheSystem() throws {
        let view = MarkitInlineView()
        var opened: [MarkitReference] = []
        view.onReference = { opened.append($0) }
        let reference = MarkitReference(kind: "file", target: "src/main.rs")
        let handled = view.open(reference.url, attributes: [.markitReference: reference.target, .markitReferenceKind: reference.kind])
        XCTAssertFalse(handled)
        XCTAssertEqual(opened, [reference])
    }

    @MainActor private func buttons(in view: UIView) -> [UIButton] {
        var result: [UIButton] = []
        for subview in view.subviews {
            if let button = subview as? UIButton { result.append(button) } else { result += buttons(in: subview) }
        }
        return result
    }
}
#endif
