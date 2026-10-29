#if canImport(UIKit)
import XCTest
@testable import Markit

/// GitHub flavour in the inline view: what a reader sees and what a tap reports.
final class GithubInlineTests: XCTestCase {
    @MainActor func testTappingATaskBoxReportsItsSourceOffsetAndTheRequestedState() throws {
        let source = "Plan:\n\n- [ ] Rebase\n- [x] Lint\n"
        let view = MarkitInlineView(flavor: .github)
        var tasks: [MarkitTask] = []
        view.onTask = { tasks.append($0) }
        view.render(source)
        let text = try XCTUnwrap(textViews(in: view).first { $0.attributedText.string.contains("Rebase") })

        tapLink(at: "☐", in: text)
        tapLink(at: "☑", in: text)

        XCTAssertEqual(tasks.map(\.index), [0, 1])
        XCTAssertEqual(tasks.map(\.checked), [true, false])
        let utf16 = Array(source.utf16)
        for task in tasks {
            XCTAssertEqual(utf16[task.offset - 1], UInt16(UInt8(ascii: "[")), "offset \(task.offset) must sit inside a box")
        }
        XCTAssertEqual(utf16[tasks[0].offset], UInt16(UInt8(ascii: " ")))
        XCTAssertEqual(utf16[tasks[1].offset], UInt16(UInt8(ascii: "x")))
    }

    @MainActor func testTaskBoxesAreNotTappableWithoutAHost() throws {
        let view = MarkitInlineView(flavor: .github)
        view.render("- [ ] Rebase\n")
        let text = try XCTUnwrap(textViews(in: view).first)
        let box = (text.attributedText.string as NSString).range(of: "☐")
        XCTAssertNotEqual(box.location, NSNotFound)
        XCTAssertNil(text.attributedText.attribute(.link, at: box.location, effectiveRange: nil))

        var tasks: [MarkitTask] = []
        view.onTask = { tasks.append($0) }
        let enabled = try XCTUnwrap(textViews(in: view).first)
        XCTAssertNotNil(enabled.attributedText.attribute(.link, at: box.location, effectiveRange: nil))
    }

    @MainActor func testAlertShowsItsTitleInTheAlertAccent() throws {
        var theme = MarkitTheme()
        theme.alertWarning = .systemPink
        let view = MarkitInlineView(theme: theme, flavor: .github)
        view.render("> [!WARNING]\n> Deleting is permanent.\n")
        let text = try XCTUnwrap(textViews(in: view).first)
        XCTAssertTrue(text.attributedText.string.hasPrefix("Warning\nDeleting is permanent."), text.attributedText.string)
        let color = text.attributedText.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor
        XCTAssertEqual(color, .systemPink)
        XCTAssertEqual(text.attributedText.attribute(.markitAlert, at: 0, effectiveRange: nil) as? String, "warning")
    }

    @MainActor func testStandardFlavourLeavesAlertSyntaxAsText() throws {
        let view = MarkitInlineView()
        view.render("> [!WARNING]\n> Deleting is permanent.\n")
        let text = try XCTUnwrap(textViews(in: view).first)
        XCTAssertTrue(text.attributedText.string.contains("[!WARNING]"), text.attributedText.string)
    }

    @MainActor func testChangingTheFlavourRendersTheTextAgain() throws {
        let view = MarkitInlineView()
        view.render("> [!TIP]\n> Use the flavour.\n")
        XCTAssertTrue(textViews(in: view).first?.attributedText.string.contains("[!TIP]") == true)
        view.flavor = .github
        // A fresh stream renders its first text synchronously, like a first render.
        let text = try XCTUnwrap(textViews(in: view).first)
        XCTAssertTrue(text.attributedText.string.hasPrefix("Tip\nUse the flavour."), text.attributedText.string)
    }

    @MainActor func testMentionsIssuesAndFootnotesReachTheHostWithTheirKinds() throws {
        let view = MarkitInlineView(flavor: .github)
        var references: [MarkitReference] = []
        view.onReference = { references.append($0) }
        view.render("Thanks @octocat for #12.[^n]\n\n[^n]: The note.\n")
        let text = try XCTUnwrap(textViews(in: view).first { $0.attributedText.string.contains("@octocat") })
        tapLink(at: "@octocat", in: text)
        tapLink(at: "#12", in: text)
        tapLink(at: "[1]", in: text)
        XCTAssertEqual(references, [
            MarkitReference(kind: "mention", target: "octocat"),
            MarkitReference(kind: "issue", target: "#12"),
            MarkitReference(kind: "footnote", target: "n"),
        ])
        let marker = (text.attributedText.string as NSString).range(of: "[1]")
        let markerFont = text.attributedText.attribute(.font, at: marker.location, effectiveRange: nil) as? UIFont
        let bodyFont = text.attributedText.attribute(.font, at: 0, effectiveRange: nil) as? UIFont
        XCTAssertLessThan(markerFont?.pointSize ?? 0, bodyFont?.pointSize ?? 0, "footnote references are superscript")
        XCTAssertGreaterThan(text.attributedText.attribute(.baselineOffset, at: marker.location, effectiveRange: nil) as? CGFloat ?? 0, 0)
        let note = try XCTUnwrap(textViews(in: view).first { $0.attributedText.string.contains("The note.") })
        let font = note.attributedText.attribute(.font, at: 0, effectiveRange: nil) as? UIFont
        XCTAssertEqual(font?.pointSize, MarkitTheme.standard.footnoteFont.font(compatibleWith: view.traitCollection).pointSize)
    }

    @MainActor func testDiagramStaysACodeBlockWhenTheHostDrawsNothing() async throws {
        let source = "```mermaid\ngraph TD; A-->B\n```\n"
        let plain = MarkitInlineView(flavor: .github)
        plain.render(source)
        let codeHeight = plain.height(for: 320)

        let view = MarkitInlineView(flavor: .github)
        let asked = Recorder()
        view.renderDiagram = { language, source in asked.values.append("\(language):\(source)"); return nil }
        view.render(source)
        let diagram = try XCTUnwrap(view.blocks.first as? InlineDiagramBlockView)
        for _ in 0..<100 where asked.values.isEmpty { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertEqual(asked.values, ["mermaid:graph TD; A-->B\n"])
        XCTAssertFalse(diagram.isDrawn)
        XCTAssertEqual(view.height(for: 320), codeHeight)
        XCTAssertGreaterThan(codeHeight, 0)
        XCTAssertTrue(textViews(in: view).contains { $0.attributedText.string.contains("graph TD") && !$0.isHiddenInTree })
    }

    @MainActor func testDiagramDrawnByTheHostReplacesTheCodeAndSurvivesARender() async throws {
        let source = "```mermaid\ngraph TD; A-->B\n```\n"
        let view = MarkitInlineView(flavor: .github)
        let drawing = UIView(frame: CGRect(x: 0, y: 0, width: 200, height: 140))
        let calls = Recorder()
        view.renderDiagram = { language, _ in calls.values.append(language); return drawing }
        view.render(source, finalized: false)
        let diagram = try XCTUnwrap(view.blocks.first as? InlineDiagramBlockView)
        for _ in 0..<100 where !diagram.isDrawn { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(diagram.isDrawn)
        XCTAssertTrue(drawing.isDescendant(of: view))
        XCTAssertEqual(view.height(for: 320), 140)

        view.render(source + "\nMore text.\n")
        for _ in 0..<100 where view.blocks.count < 2 { try await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(drawing.isDescendant(of: view))
        XCTAssertEqual(calls.values, ["mermaid"])
    }

    // MARK: Helpers

    @MainActor private func textViews(in view: UIView) -> [UITextView] {
        view.subviews.flatMap { subview -> [UITextView] in
            if let text = subview as? UITextView { return [text] }
            return textViews(in: subview)
        }
    }

    /// Performs what a tap on the link at `needle` does: the text view asks its delegate.
    @MainActor private func tapLink(at needle: String, in text: UITextView, file: StaticString = #filePath, line: UInt = #line) {
        let range = (text.attributedText.string as NSString).range(of: needle)
        guard range.location != NSNotFound,
              let url = text.attributedText.attribute(.link, at: range.location, effectiveRange: nil) as? URL else {
            return XCTFail("no link at \(needle)", file: file, line: line)
        }
        let opened = text.delegate?.textView?(text, shouldInteractWith: url, in: range, interaction: .invokeDefaultAction)
        XCTAssertEqual(opened, false, "the host handles \(needle), not the system", file: file, line: line)
    }
}

/// Records what a main-actor host hook was asked.
@MainActor private final class Recorder {
    var values: [String] = []
}

private extension UIView {
    var isHiddenInTree: Bool {
        var view: UIView? = self
        while let current = view {
            if current.isHidden { return true }
            view = current.superview
        }
        return false
    }
}
#endif
