#if canImport(UIKit)
import CoreText
import UIKit
#if canImport(SwiftUI)
import SwiftUI
#endif

extension MarkitDocument {
    /// Themed attributed text for `range` (the whole document when nil).
    /// Pass the displaying view's traits; nil uses the current content size
    /// and dynamic colours. Math runs are image attachments carrying their
    /// source under `.markitRaw`; references are links with `.markitReference`.
    public func attributedString(in range: NSRange? = nil, theme: MarkitTheme = .standard,
                                 traits: UITraitCollection? = nil) -> NSAttributedString {
        let whole = NSRange(location: 0, length: (text as NSString).length)
        return MarkitStyler(theme: theme, traits: traits).attributed(self, range: range ?? whole)
    }

    /// Themed attributed text for one entry of `blocks`, or nil when out of range.
    public func attributedString(forBlockAt index: Int, theme: MarkitTheme = .standard,
                                 traits: UITraitCollection? = nil) -> NSAttributedString? {
        guard blocks.indices.contains(index) else { return nil }
        return attributedString(in: blocks[index].range, theme: theme, traits: traits)
    }
}

#if canImport(SwiftUI)
extension MarkitDocument {
    /// Themed text for SwiftUI `Text`, carrying both SwiftUI and UIKit
    /// attributes. `Text` cannot draw attachments, so math reads as its source;
    /// references are links that `MarkitReference(url:)` decodes.
    public func attributedText(in range: NSRange? = nil, theme: MarkitTheme = .standard,
                               traits: UITraitCollection? = nil) -> AttributedString {
        MarkitSwiftUIText.convert(attributedString(in: range, theme: theme, traits: traits))
    }
}

enum MarkitSwiftUIText {
    static func convert(_ source: NSAttributedString) -> AttributedString {
        var result = AttributedString()
        let string = source.string as NSString
        source.enumerateAttributes(in: NSRange(location: 0, length: source.length), options: []) { attributes, range, _ in
            var text = string.substring(with: range)
            if let raw = attributes[.markitRaw] as? String { text = raw } else if attributes[.attachment] != nil { text = "" }
            guard !text.isEmpty else { return }
            var container = AttributeContainer()
            if let font = attributes[.font] as? UIFont {
                container[AttributeScopes.UIKitAttributes.FontAttribute.self] = font
                container[AttributeScopes.SwiftUIAttributes.FontAttribute.self] = Font(font as CTFont)
            }
            if let color = attributes[.foregroundColor] as? UIColor {
                container[AttributeScopes.UIKitAttributes.ForegroundColorAttribute.self] = color
                container[AttributeScopes.SwiftUIAttributes.ForegroundColorAttribute.self] = Color(color)
            }
            if let color = attributes[.backgroundColor] as? UIColor {
                container[AttributeScopes.UIKitAttributes.BackgroundColorAttribute.self] = color
                container[AttributeScopes.SwiftUIAttributes.BackgroundColorAttribute.self] = Color(color)
            }
            if attributes[.strikethroughStyle] != nil {
                container[AttributeScopes.UIKitAttributes.StrikethroughStyleAttribute.self] = .single
                container[AttributeScopes.SwiftUIAttributes.StrikethroughStyleAttribute.self] = .single
            }
            if let style = attributes[.paragraphStyle] as? NSParagraphStyle {
                container[AttributeScopes.UIKitAttributes.ParagraphStyleAttribute.self] = style
            }
            if let url = attributes[.link] as? URL {
                container[AttributeScopes.FoundationAttributes.LinkAttribute.self] = url
            }
            result.append(AttributedString(text, attributes: container))
        }
        return result
    }
}
#endif
#endif
