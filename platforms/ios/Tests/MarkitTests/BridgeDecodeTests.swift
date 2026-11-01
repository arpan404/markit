import CMarkit
import Foundation
import XCTest

@testable import Markit

/// The bridge decoder (`BridgeDecoder`) must agree with `JSONDecoder` on
/// identical bridge bytes, field by field, for every span kind the renderer
/// emits. `JSONDecoder` plus the `Decodable` models is ground truth; these
/// tests pin the fast path to it.
final class BridgeDecodeTests: XCTestCase {
    /// Renders `source` through the real Rust bridge and returns its raw
    /// JSON bytes, the same bytes `Markit.decode` receives.
    private func bridgeBytes(
        _ source: String, flavor: MarkitFlavor = .standard
    ) throws -> Data {
        let bytes = Array(source.utf8)
        let buffer = bytes.withUnsafeBufferPointer {
            markit_render_flavor(
                $0.baseAddress, $0.count, 0, flavor.rawValue)
        }
        defer { markit_buffer_free(buffer) }
        guard buffer.status == 0, let pointer = buffer.data else {
            throw MarkitError.renderFailed(status: buffer.status)
        }
        return Data(bytes: pointer, count: buffer.len)
    }

    private func checkSameDocument(
        _ data: Data, file: StaticString = #filePath, line: UInt = #line
    ) throws -> MarkitDocument {
        let expected = try JSONDecoder().decode(MarkitDocument.self, from: data)
        let actual = try BridgeDecoder.document(from: data)
        XCTAssertEqual(
            actual.version, expected.version, file: file, line: line)
        XCTAssertEqual(actual.text, expected.text, file: file, line: line)
        XCTAssertEqual(
            actual.blocks.count, expected.blocks.count,
            "block count", file: file, line: line)
        for (i, (a, e)) in zip(actual.blocks, expected.blocks).enumerated() {
            XCTAssertEqual(a.start, e.start, "block[\(i)].start", file: file, line: line)
            XCTAssertEqual(a.end, e.end, "block[\(i)].end", file: file, line: line)
            XCTAssertEqual(a.kind, e.kind, "block[\(i)].kind", file: file, line: line)
        }
        XCTAssertEqual(
            actual.spans.count, expected.spans.count,
            "span count", file: file, line: line)
        for (i, (a, e)) in zip(actual.spans, expected.spans).enumerated() {
            let where_ = "span[\(i)] kind=\(e.kind)"
            XCTAssertEqual(a.start, e.start, "\(where_).start", file: file, line: line)
            XCTAssertEqual(a.end, e.end, "\(where_).end", file: file, line: line)
            XCTAssertEqual(a.kind, e.kind, "\(where_).kind", file: file, line: line)
            XCTAssertEqual(a.level, e.level, "\(where_).level", file: file, line: line)
            XCTAssertEqual(a.depth, e.depth, "\(where_).depth", file: file, line: line)
            XCTAssertEqual(a.language, e.language, "\(where_).language", file: file, line: line)
            XCTAssertEqual(a.url, e.url, "\(where_).url", file: file, line: line)
            XCTAssertEqual(a.file, e.file, "\(where_).file", file: file, line: line)
            XCTAssertEqual(a.diff, e.diff, "\(where_).diff", file: file, line: line)
            XCTAssertEqual(a.light, e.light, "\(where_).light", file: file, line: line)
            XCTAssertEqual(a.dark, e.dark, "\(where_).dark", file: file, line: line)
            XCTAssertEqual(a.change, e.change, "\(where_).change", file: file, line: line)
            XCTAssertEqual(a.tex, e.tex, "\(where_).tex", file: file, line: line)
            XCTAssertEqual(a.raw, e.raw, "\(where_).raw", file: file, line: line)
            XCTAssertEqual(a.display, e.display, "\(where_).display", file: file, line: line)
            XCTAssertEqual(a.png, e.png, "\(where_).png", file: file, line: line)
            XCTAssertEqual(a.width, e.width, "\(where_).width", file: file, line: line)
            XCTAssertEqual(a.height, e.height, "\(where_).height", file: file, line: line)
            XCTAssertEqual(a.row, e.row, "\(where_).row", file: file, line: line)
            XCTAssertEqual(a.column, e.column, "\(where_).column", file: file, line: line)
            XCTAssertEqual(a.header, e.header, "\(where_).header", file: file, line: line)
            XCTAssertEqual(
                a.alignment, e.alignment, "\(where_).alignment", file: file,
                line: line)
            XCTAssertEqual(a.target, e.target, "\(where_).target", file: file, line: line)
            XCTAssertEqual(
                a.reference_kind, e.reference_kind, "\(where_).reference_kind",
                file: file, line: line)
            XCTAssertEqual(a.alert, e.alert, "\(where_).alert", file: file, line: line)
            XCTAssertEqual(a.label, e.label, "\(where_).label", file: file, line: line)
            XCTAssertEqual(a.number, e.number, "\(where_).number", file: file, line: line)
            XCTAssertEqual(a.index, e.index, "\(where_).index", file: file, line: line)
            XCTAssertEqual(a.offset, e.offset, "\(where_).offset", file: file, line: line)
            XCTAssertEqual(a.checked, e.checked, "\(where_).checked", file: file, line: line)
            XCTAssertEqual(a.source, e.source, "\(where_).source", file: file, line: line)
            XCTAssertEqual(a.tag, e.tag, "\(where_).tag", file: file, line: line)
        }
        return actual
    }

    /// Exercises every `Style` variant the renderer can emit.
    private var everyVariantSource: String {
        """
        # Title *em* **strong** ~~strike~~ `code` [link](https://example.com) ![pic](https://img.example/a.png)

        Paragraph with footnote[^label1] and @octocat and #123 and a task list:

        - [ ] open task
        - [x] done task

        ```rust
        fn main() { println!("hi"); }
        ```

        ```diff
        +added line
        -removed line
         context line
        ```

        Math \\(\\frac{1}{2}\\) and block:

        $$x^2 + y^2 = z^2$$

        | head_a | head_b |
        | ------ | :------: |
        | c1 | c2 |

        > [!NOTE]
        > alert body with `code`

        > a quote

        [^label1]: the footnote text with **bold**

        ```mermaid
        graph TD; A-->B;
        ```

        <video src="https://v.example/x.mp4"></video>

        <picture><source media="(prefers-color-scheme: dark)" srcset="https://img.example/dark.png"><img src="https://img.example/light.png"></picture>

        Press <kbd>Ctrl</kbd> and <sub>sub</sub> now.

        ---

        Final paragraph.
        """
    }

    func testFastDecoderMatchesJSONDecoderOnEverySpanKind() throws {
        let allKinds: Set<String> = [
            "heading", "paragraph", "strong", "emphasis", "strikethrough",
            "code", "code_block", "link", "image", "quote", "list_item",
            "table", "table_header", "rule", "reference", "code_metadata",
            "syntax", "diff", "math", "table_cell", "alert",
            "footnote_reference", "footnote_definition", "task", "diagram",
            "video", "dark_image", "html",
        ]
        // GitHub-only kinds appear under `.github`; the standard flavour
        // emits the rest. Coverage is required over the union so the
        // fixture cannot silently stop exercising a variant.
        var seen = Set<String>()
        for flavor in [MarkitFlavor.standard, MarkitFlavor.github] {
            let data = try bridgeBytes(everyVariantSource, flavor: flavor)
            let document = try checkSameDocument(data)
            seen.formUnion(document.spans.map(\.kind))
            if flavor == .github {
                XCTAssertTrue(
                    ["alert", "task", "footnote_reference",
                     "footnote_definition", "diagram", "video", "dark_image",
                     "html"].allSatisfy(seen.contains),
                    "github run lost a github-only kind: \(seen)")
            }
        }
        XCTAssertTrue(
            allKinds.isSubset(of: seen),
            "fixture lost coverage for \(allKinds.subtracting(seen))")
    }

    func testFastDecoderMatchesOnCodeHeavyDocument() throws {
        var source = ""
        for i in 0..<40 {
            source += """
                ## Section \(i) `inline` with **bold** and *italic*

                Text \(i) with [docs](https://example.com/\(i)) and math \\(\\frac{x^\(i)}{2}\\).

                ```rust
                fn section_\(i)() {
                    let v: Vec<u32> = (0..\(i % 17)).collect();
                }
                ```

                | name | value |
                | ---- | ----- |
                | alpha | \(i) |

                > Quote \(i).

                - item \(i) one
                - item \(i) two

                """
        }
        let document = try checkSameDocument(try bridgeBytes(source))
        XCTAssertGreaterThan(document.spans.count, 1000)
    }

    /// `NSNumber(0)` must not read as boolean `false`, and boolean `false`
    /// must not read as integer `0`: the boolean slots (`checked`, `diff`,
    /// `display`, `header`) and the zero-valued integer slots (`row`,
    /// `index`) coexist in task and table spans.
    func testFastDecoderKeepsBooleansAndZeroIntegersApart() throws {
        let source = """
            - [ ] open
            - [x] shut

            | a | b |
            | - | :-: |
            | 0 | 1 |
            """
        let data = try bridgeBytes(source, flavor: .github)
        let document = try checkSameDocument(data)
        let tasks = document.spans.filter { $0.kind == "task" }
        XCTAssertEqual(tasks.count, 2)
        XCTAssertEqual(tasks[0].checked, false)
        XCTAssertEqual(tasks[0].index, 0)
        XCTAssertEqual(tasks[1].checked, true)
        let cells = document.spans.filter { $0.kind == "table_cell" }
        XCTAssertTrue(cells.contains { $0.row == 0 && $0.header == true })
        XCTAssertTrue(cells.contains { $0.row == 1 && $0.header == false })
    }

    func testFastDecoderRejectsMalformedInput() throws {
        XCTAssertThrowsError(try BridgeDecoder.document(from: Data("{}".utf8)))
        XCTAssertThrowsError(
            try BridgeDecoder.document(from: Data("{\"version\":2}".utf8)))
    }
}
