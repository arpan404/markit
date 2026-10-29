#if canImport(UIKit)
import XCTest
@testable import Markit

final class ThemeTests: XCTestCase {
    private let large = UITraitCollection(preferredContentSizeCategory: .large)

    private func range(of needle: String, in text: NSAttributedString) throws -> NSRange {
        let found = (text.string as NSString).range(of: needle)
        XCTAssertNotEqual(found.location, NSNotFound, "missing \(needle)")
        return found
    }

    func testThemeFontsAndColoursReachHeadingsBodyAndInlineCode() throws {
        var theme = MarkitTheme()
        theme.headingFonts = [.fixed(.systemFont(ofSize: 40, weight: .bold))]
        theme.bodyFont = .fixed(.systemFont(ofSize: 11))
        theme.monospaceFont = .fixed(.monospacedSystemFont(ofSize: 9, weight: .regular))
        theme.textColor = .systemPurple
        theme.inlineCodeBackground = .systemYellow
        let text = try Markit.parse("# Title\n\nBody `code`").attributedString(theme: theme, traits: large)

        let title = try range(of: "Title", in: text), body = try range(of: "Body", in: text), code = try range(of: "code", in: text)
        XCTAssertEqual((text.attribute(.font, at: title.location, effectiveRange: nil) as? UIFont)?.pointSize, 40)
        XCTAssertEqual((text.attribute(.font, at: body.location, effectiveRange: nil) as? UIFont)?.pointSize, 11)
        XCTAssertEqual(text.attribute(.foregroundColor, at: body.location, effectiveRange: nil) as? UIColor, .systemPurple)
        XCTAssertEqual((text.attribute(.font, at: code.location, effectiveRange: nil) as? UIFont)?.pointSize, 9)
        XCTAssertEqual(text.attribute(.backgroundColor, at: code.location, effectiveRange: nil) as? UIColor, .systemYellow)
    }

    func testDefaultThemeKeepsTheBuiltInLook() throws {
        let text = try Markit.parse("# Title\n\nBody `code`").attributedString(traits: large)
        let title = try range(of: "Title", in: text), body = try range(of: "Body", in: text), code = try range(of: "code", in: text)
        let heading = try XCTUnwrap(text.attribute(.font, at: title.location, effectiveRange: nil) as? UIFont)
        XCTAssertEqual(heading.pointSize, 30)
        XCTAssertEqual(text.attribute(.font, at: body.location, effectiveRange: nil) as? UIFont,
                       UIFont.preferredFont(forTextStyle: .body, compatibleWith: large))
        XCTAssertEqual((text.attribute(.font, at: code.location, effectiveRange: nil) as? UIFont)?.pointSize, 13)
        XCTAssertEqual(text.attribute(.backgroundColor, at: code.location, effectiveRange: nil) as? UIColor, .secondarySystemBackground)
        let paragraph = try XCTUnwrap(text.attribute(.paragraphStyle, at: body.location, effectiveRange: nil) as? NSParagraphStyle)
        XCTAssertEqual(paragraph.lineSpacing, 3)
    }

    func testLargerContentSizeEnlargesBodyText() throws {
        let document = try Markit.parse("Body")
        let normal = document.attributedString(traits: large)
        let bigger = document.attributedString(traits: UITraitCollection(preferredContentSizeCategory: .accessibilityExtraLarge))
        let size = { (text: NSAttributedString) in (text.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)?.pointSize ?? 0 }
        XCTAssertGreaterThan(size(bigger), size(normal))
    }

    func testSyntaxPaletteOverrideRecoloursHighlightedCode() throws {
        var theme = MarkitTheme()
        theme.syntaxPalette = Array(repeating: MarkitColorPair(light: .systemPink, dark: .systemPink), count: 8)
        let light = UITraitCollection(traitsFrom: [large, UITraitCollection(userInterfaceStyle: .light)])
        let code = try Markit.highlightedText(source: "let x = \"s\"; // note\n", language: "rust", theme: theme, traits: light)
        var colours: [UIColor] = []
        var fonts: [UIFont] = []
        code.enumerateAttributes(in: NSRange(location: 0, length: code.length), options: []) { attributes, _, _ in
            if let colour = attributes[.foregroundColor] as? UIColor { colours.append(colour) }
            if let font = attributes[.font] as? UIFont { fonts.append(font) }
        }
        XCTAssertEqual(code.string, "let x = \"s\"; // note\n")
        XCTAssertTrue(colours.contains(.systemPink))
        let defaults = MarkitColorPair.defaultSyntaxPalette.map(\.light)
        XCTAssertFalse(colours.contains { colour in defaults.contains(colour) })
        XCTAssertTrue(fonts.allSatisfy { $0.fontDescriptor.symbolicTraits.contains(.traitMonoSpace) })
    }

    func testUnknownSpanKindsRenderTheirText() throws {
        let document = try makeDocument(#"""
        {"version":2,"text":"Note: see [^1] @octo","spans":[{"start":0,"end":20,"kind":"alert","alert":"note"},
         {"start":10,"end":14,"kind":"marginalia"},{"start":15,"end":20,"kind":"mention"}],
         "blocks":[{"start":0,"end":20,"kind":"alert"}]}
        """#)
        let text = document.attributedString(traits: large)
        XCTAssertEqual(text.string, document.text)
        XCTAssertEqual(text.attribute(.font, at: 12, effectiveRange: nil) as? UIFont, UIFont.preferredFont(forTextStyle: .body, compatibleWith: large))
    }

    func testSwiftUITextReadsMathAsSourceAndKeepsReferences() throws {
        let document = try Markit.parse(#"Math \(x^2\) here"#)
        let text = String(document.attributedText(traits: large).characters)
        XCTAssertTrue(text.contains(#"\(x^2\)"#), text)
        XCTAssertFalse(text.contains("\u{FFFC}"))
    }
}
#endif
