#if canImport(UIKit)
import UIKit

/// Fonts, colours and spacing for rendered Markdown. The default value
/// reproduces the renderer's built-in look. Keep one theme value per
/// appearance in the host (for example a `static let`), so views can tell
/// when it really changed.
///
/// UIFont and UIColor are immutable, so sharing a theme across threads is safe.
public struct MarkitTheme: Equatable, @unchecked Sendable {
    // MARK: Fonts
    public var bodyFont: MarkitFont = .textStyle(.body)
    /// Heading levels 1 to 6. Missing levels use the last entry.
    public var headingFonts: [MarkitFont] = ([30, 25, 21, 19, 17, 16] as [CGFloat]).map {
        MarkitFont.scaled(UIFont.systemFont(ofSize: $0, weight: .semibold))
    }
    /// Inline code and code blocks.
    public var monospaceFont: MarkitFont = .scaled(.monospacedSystemFont(ofSize: 13, weight: .regular))
    /// Table text.
    public var tableFont: MarkitFont = .scaled(.monospacedSystemFont(ofSize: 13, weight: .regular))
    /// GitHub footnote definitions under the document.
    public var footnoteFont: MarkitFont = .textStyle(.footnote)
    /// Code block headers and collapse controls.
    public var captionFont: MarkitFont = .textStyle(.caption1)

    // MARK: Colours
    public var textColor: UIColor = .label
    /// Quotes, code block titles and controls.
    public var secondaryTextColor: UIColor = .secondaryLabel
    /// Nil uses the view's tint colour.
    public var linkColor: UIColor?
    public var inlineCodeBackground: UIColor = .secondarySystemBackground
    public var codeBlockBackground: UIColor = .clear
    public var codeHeaderBackground: UIColor = .secondarySystemBackground
    public var codeBorder: UIColor = .separator
    public var quoteBar: UIColor = .separator
    public var tableBorder: UIColor = .separator
    public var tableHeaderBackground: UIColor = .secondarySystemBackground
    public var ruleColor: UIColor = .separator
    public var diffAddition: UIColor = UIColor.systemGreen.withAlphaComponent(0.12)
    public var diffDeletion: UIColor = UIColor.systemRed.withAlphaComponent(0.12)
    /// GitHub alert accents (`> [!NOTE]` ...): the title and the quote bar.
    public var alertNote: UIColor = .systemBlue
    public var alertTip: UIColor = .systemGreen
    public var alertImportant: UIColor = .systemPurple
    public var alertWarning: UIColor = .systemOrange
    public var alertCaution: UIColor = .systemRed
    /// Replaces the renderer's eight syntax classes, in `defaultSyntaxPalette` order.
    public var syntaxPalette: [MarkitColorPair] = MarkitColorPair.defaultSyntaxPalette
    /// Colour for syntax runs that carry no colour.
    public var syntaxFallback = MarkitColorPair(lightHex: "#24292f", darkHex: "#24292f")

    // MARK: Spacing
    public var lineSpacing: CGFloat = 3
    public var paragraphSpacing: CGFloat = 6
    /// Space between blocks (paragraph groups, code, tables) in inline views.
    public var blockSpacing: CGFloat = 10
    /// Indent per list level.
    public var listIndent: CGFloat = 16
    /// Room for the bullet or number before a list item's text.
    public var listMarkerWidth: CGFloat = 12
    /// Indent per quote level.
    public var quoteIndent: CGFloat = 16
    public var quoteBarWidth: CGFloat = 3
    public var codeInsets = UIEdgeInsets(top: 8, left: 12, bottom: 12, right: 12)
    public var codeCornerRadius: CGFloat = 8
    public var tableCellInsets = UIEdgeInsets(top: 6, left: 10, bottom: 6, right: 10)
    /// Widest a table column grows before its text wraps.
    public var tableMaxColumnWidth: CGFloat = 280
    /// Widest math or other attachments are drawn before they are scaled down.
    public var maxAttachmentWidth: CGFloat = 320
    public var maxImageHeight: CGFloat = 320

    public init() {}

    /// The built-in look.
    public static let standard = MarkitTheme()

    /// Accent of a GitHub alert kind (`note`, `tip`, `important`, `warning`,
    /// `caution`); nil for any other kind.
    public func alertColor(for kind: String) -> UIColor? {
        switch kind {
        case "note": return alertNote
        case "tip": return alertTip
        case "important": return alertImportant
        case "warning": return alertWarning
        case "caution": return alertCaution
        default: return nil
        }
    }

    func headingFont(level: Int) -> MarkitFont {
        guard let last = headingFonts.last else { return bodyFont }
        let index = max(0, level - 1)
        return index < headingFonts.count ? headingFonts[index] : last
    }
}

/// When a long code block shows only its first lines.
public struct MarkitCodeCollapse: Equatable, Sendable {
    /// Blocks with more lines than this start collapsed.
    public var collapseAbove: Int
    /// Lines shown while collapsed.
    public var visibleLines: Int
    public init(collapseAbove: Int = 22, visibleLines: Int = 20) {
        self.collapseAbove = collapseAbove
        self.visibleLines = max(1, min(visibleLines, collapseAbove))
    }
}
#endif
