import XCTest
@testable import Markit

final class InlinePlannerTests: XCTestCase {
    func testLongTextRootSplitsAtLineEndsSoOnlyTheLastPieceChanges() throws {
        let line = String(repeating: "a", count: 99)
        let text = Array(repeating: line, count: 40).joined(separator: "\n") + "\n"
        let document = try makeDocument(#"{"version":2,"text":\#(jsonString(text)),"spans":[],"blocks":[{"start":0,"end":4000,"kind":"text"}]}"#)
        let plans = InlinePlanner.plan(document, index: 0)
        XCTAssertGreaterThan(plans.count, 1)
        XCTAssertTrue(plans.allSatisfy { $0.kind == .text })
        XCTAssertEqual(plans.map(\.continuation), [false] + Array(repeating: true, count: plans.count - 1))
        let source = text as NSString
        // Pieces are whole lines and together cover the text.
        XCTAssertEqual(plans.map { source.substring(with: $0.range) }.joined(separator: "\n") + "\n", text)
        XCTAssertTrue(plans.allSatisfy { $0.range.length <= InlinePlanner.pieceLimit })
    }

    func testOneLongLineIsNeverSplit() throws {
        let text = String(repeating: "word ", count: 1200)
        let document = try makeDocument(#"{"version":2,"text":"\#(text)","spans":[],"blocks":[{"start":0,"end":6000,"kind":"text"}]}"#)
        let plans = InlinePlanner.plan(document, index: 0)
        XCTAssertEqual(plans.count, 1)
        XCTAssertEqual(plans.first?.range, NSRange(location: 0, length: 6000))
    }

    func testCodeInsideAListItemIsIndentedByItsDepth() throws {
        let document = try makeDocument(#"""
        {"version":2,"text":"• item\nlet x\n","spans":[{"start":0,"end":13,"kind":"list_item","depth":2},{"start":7,"end":13,"kind":"code_block","language":"swift"}],
         "blocks":[{"start":0,"end":7,"kind":"text"},{"start":7,"end":13,"kind":"code"}]}
        """#)
        let plans = InlinePlanner.plan(document, index: 3)
        XCTAssertEqual(plans, [
            InlineBlockPlan(document: 3, range: NSRange(location: 0, length: 6), kind: .text, depth: 0, continuation: false),
            InlineBlockPlan(document: 3, range: NSRange(location: 7, length: 5), kind: .code, depth: 2, continuation: false),
        ])
    }

    func testUnknownBlockKindsRenderAsText() throws {
        let document = try makeDocument(#"{"version":2,"text":"graph TD","spans":[{"start":0,"end":8,"kind":"diagram"}],"blocks":[{"start":0,"end":8,"kind":"diagram"}]}"#)
        XCTAssertEqual(InlinePlanner.plan(document, index: 0).map(\.kind), [.text])
    }

    func testImagesFollowTheTextThatHoldsThem() throws {
        let document = try makeDocument(#"{"version":2,"text":"see alt here","spans":[{"start":4,"end":7,"kind":"image","url":"a.png"}],"blocks":[{"start":0,"end":12,"kind":"text"}]}"#)
        let plans = InlinePlanner.plan(document, index: 0)
        XCTAssertEqual(plans.map(\.kind), [.text, .image(url: "a.png")])
        XCTAssertEqual(plans.last?.range, NSRange(location: 4, length: 3))
    }

    func testPlanningFromAChangedRootLeavesSettledRootsOut() throws {
        let first = try makeDocument(#"{"version":2,"text":"one","spans":[],"blocks":[{"start":0,"end":3,"kind":"text"}]}"#)
        let second = try makeDocument(#"{"version":2,"text":"two","spans":[],"blocks":[{"start":0,"end":3,"kind":"text"}]}"#)
        let plans = InlinePlanner.plan([first, second], from: 1)
        XCTAssertEqual(plans.map(\.document), [1])
        XCTAssertTrue(InlinePlanner.plan([first, second], from: 2).isEmpty)
    }
}
