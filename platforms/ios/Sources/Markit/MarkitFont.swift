#if canImport(UIKit)
import UIKit

/// A Dynamic Type text style, mirrored here so themes stay plain values.
public enum MarkitTextStyle: Sendable, Equatable {
    case largeTitle, title1, title2, title3, headline, subheadline, body, callout, footnote, caption1, caption2

    var uiKit: UIFont.TextStyle {
        switch self {
        case .largeTitle: return .largeTitle
        case .title1: return .title1
        case .title2: return .title2
        case .title3: return .title3
        case .headline: return .headline
        case .subheadline: return .subheadline
        case .body: return .body
        case .callout: return .callout
        case .footnote: return .footnote
        case .caption1: return .caption1
        case .caption2: return .caption2
        }
    }
}

/// A font that resolves against a trait collection, so it follows Dynamic Type.
/// UIFont is immutable, which makes the stored value safe to share.
public struct MarkitFont: Equatable, @unchecked Sendable {
    enum Source: Equatable {
        case textStyle(MarkitTextStyle)
        case scaled(UIFont, MarkitTextStyle, maximumPointSize: CGFloat?)
        case fixed(UIFont)
    }
    let source: Source

    /// The system's preferred font for a text style.
    public static func textStyle(_ style: MarkitTextStyle) -> MarkitFont { MarkitFont(source: .textStyle(style)) }
    /// `font` at the default content size, scaled like `style` for other sizes.
    public static func scaled(_ font: UIFont, relativeTo style: MarkitTextStyle = .body, maximumPointSize: CGFloat? = nil) -> MarkitFont {
        MarkitFont(source: .scaled(font, style, maximumPointSize: maximumPointSize))
    }
    /// A font that never scales.
    public static func fixed(_ font: UIFont) -> MarkitFont { MarkitFont(source: .fixed(font)) }
    /// The system font at `size` points (default content size), scaled like `style`.
    public static func system(size: CGFloat, weight: UIFont.Weight = .regular, design: UIFontDescriptor.SystemDesign = .default,
                              relativeTo style: MarkitTextStyle = .body) -> MarkitFont {
        let base = UIFont.systemFont(ofSize: size, weight: weight)
        let font = base.fontDescriptor.withDesign(design).map { UIFont(descriptor: $0, size: size) } ?? base
        return .scaled(font, relativeTo: style)
    }

    /// The concrete font. Nil traits use the app's current content size category.
    public func font(compatibleWith traits: UITraitCollection?) -> UIFont {
        switch source {
        case .textStyle(let style):
            if let traits { return UIFont.preferredFont(forTextStyle: style.uiKit, compatibleWith: traits) }
            return UIFont.preferredFont(forTextStyle: style.uiKit)
        case .scaled(let font, let style, let maximum):
            let metrics = UIFontMetrics(forTextStyle: style.uiKit)
            if let maximum { return metrics.scaledFont(for: font, maximumPointSize: maximum, compatibleWith: traits) }
            return metrics.scaledFont(for: font, compatibleWith: traits)
        case .fixed(let font):
            return font
        }
    }
}

/// A light/dark colour pair, as the syntax palette uses.
public struct MarkitColorPair: Equatable, @unchecked Sendable {
    public var light: UIColor
    public var dark: UIColor
    public init(light: UIColor, dark: UIColor) { self.light = light; self.dark = dark }
    public init(lightHex: String, darkHex: String) {
        self.init(light: UIColor(markitHex: lightHex), dark: UIColor(markitHex: darkHex))
    }

    /// The colour for one appearance, or a dynamic colour when the appearance is unknown.
    public func color(for style: UIUserInterfaceStyle?) -> UIColor {
        switch style {
        case .some(.dark): return dark
        case .some: return light
        case .none:
            let light = self.light, dark = self.dark
            return UIColor { $0.userInterfaceStyle == .dark ? dark : light }
        }
    }

    /// The renderer's eight syntax classes, in palette order (text, comment, string,
    /// constant, keyword, function/type, variable/attribute, tag/insertion).
    static let defaultSyntaxHex: [(light: String, dark: String)] = [
        ("#24292f", "#c9d1d9"), ("#6e7781", "#8b949e"), ("#0a3069", "#a5d6ff"), ("#0550ae", "#79c0ff"),
        ("#cf222e", "#ff7b72"), ("#8250df", "#d2a8ff"), ("#953800", "#ffa657"), ("#116329", "#7ee787"),
    ]
    /// Palette index for a span's light colour, so a theme can replace a class.
    static let defaultSyntaxIndex: [String: Int] = Dictionary(
        uniqueKeysWithValues: defaultSyntaxHex.enumerated().map { ($0.element.light, $0.offset) })
    public static var defaultSyntaxPalette: [MarkitColorPair] {
        defaultSyntaxHex.map { MarkitColorPair(lightHex: $0.light, darkHex: $0.dark) }
    }
}
#endif
