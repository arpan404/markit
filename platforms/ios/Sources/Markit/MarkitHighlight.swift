#if canImport(UIKit)
import UIKit

extension Markit {
    /// Highlighted source for a file viewer: the exact text of `source` in the
    /// theme's monospace font, coloured with the theme's syntax palette, diff
    /// lines tinted. Pass the displaying view's traits for Dynamic Type and
    /// appearance; nil uses the current content size and dynamic colours.
    public static func highlightedText(source: String, language: String, theme: MarkitTheme = .standard,
                                       traits: UITraitCollection? = nil, fullGrammar: Bool = false) throws -> NSAttributedString {
        let document: MarkitDocument
        do {
            document = try codeDocument(source, language: language, fullGrammar: fullGrammar)
        } catch is MarkitBinaryError {
            // A binary record this build cannot read yet; JSON keeps unknown kinds.
            document = try highlight(source, language: language, fullGrammar: fullGrammar)
        }
        let styler = MarkitStyler(theme: theme, traits: traits)
        let output = NSMutableAttributedString(attributedString: styler.attributed(document, range: NSRange(location: 0, length: (document.text as NSString).length)))
        let whole = NSRange(location: 0, length: output.length)
        // Code text carries no code_block span; set the monospace base under the syntax colours.
        output.enumerateAttribute(.font, in: whole) { value, range, _ in
            guard let font = value as? UIFont, font == styler.body else { return }
            output.addAttribute(.font, value: styler.monospace, range: range)
        }
        return output
    }
}
#endif
