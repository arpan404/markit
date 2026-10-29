#if canImport(UIKit)
import UIKit

extension NSAttributedString.Key {
    /// Original source (for example TeX with delimiters) of an attachment run.
    public static var markitRaw: NSAttributedString.Key { NSAttributedString.Key("dev.markit.raw") }
    /// Target of an app reference run (file, thread, PR...).
    public static var markitReference: NSAttributedString.Key { NSAttributedString.Key("dev.markit.reference") }
    /// Kind of an app reference run.
    public static var markitReferenceKind: NSAttributedString.Key { NSAttributedString.Key("dev.markit.reference.kind") }
    /// GitHub flavour: the alert kind (`note`, `warning`, ...) over an alert.
    public static var markitAlert: NSAttributedString.Key { NSAttributedString.Key("dev.markit.alert") }
    /// GitHub flavour, on a task box glyph that the host can tick: its
    /// source offset (NSNumber); `markitTaskIndex` and `markitTaskChecked` go with it.
    static var markitTaskOffset: NSAttributedString.Key { NSAttributedString.Key("dev.markit.task.offset") }
    static var markitTaskIndex: NSAttributedString.Key { NSAttributedString.Key("dev.markit.task.index") }
    static var markitTaskChecked: NSAttributedString.Key { NSAttributedString.Key("dev.markit.task.checked") }
}

extension MarkitTask {
    /// The tapped task that `attributes` (a task glyph's) asks for, ticking or clearing it.
    init?(toggling attributes: [NSAttributedString.Key: Any]) {
        guard let offset = attributes[.markitTaskOffset] as? NSNumber else { return nil }
        let index = (attributes[.markitTaskIndex] as? NSNumber)?.intValue ?? 0
        let checked = (attributes[.markitTaskChecked] as? NSNumber)?.boolValue ?? false
        self.init(index: index, offset: offset.intValue, checked: !checked)
    }
}
extension UIColor {
    convenience init(markitHex value: String) {
        let rgb = UInt32(value.dropFirst(), radix: 16) ?? 0
        self.init(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: 1)
    }
}

/// Turns document ranges into attributed text with one theme and trait
/// collection. Unknown span kinds keep their text with body styling, so new
/// renderer features never fail to display.
struct MarkitStyler {
    let theme: MarkitTheme
    /// Nil resolves fonts for the current content size and uses dynamic colours.
    let traits: UITraitCollection?
    /// Diff lines tint the glyph background. Views that draw full-width bands turn this off.
    var diffBackgrounds = true
    /// GitHub task glyphs become tappable links (the host set `onTask`).
    var tasksEnabled = false
    let body: UIFont
    let monospace: UIFont
    let table: UIFont
    private let palette: [UIColor]

    init(theme: MarkitTheme, traits: UITraitCollection?) {
        self.theme = theme
        self.traits = traits
        body = theme.bodyFont.font(compatibleWith: traits)
        monospace = theme.monospaceFont.font(compatibleWith: traits)
        table = theme.tableFont.font(compatibleWith: traits)
        palette = theme.syntaxPalette.map { $0.color(for: traits?.userInterfaceStyle) }
    }

    func attributed(_ document: MarkitDocument, range: NSRange) -> NSAttributedString {
        let source = document.text as NSString
        let safeRange = NSIntersectionRange(range, NSRange(location: 0, length: source.length))
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineSpacing = theme.lineSpacing
        paragraph.paragraphSpacing = theme.paragraphSpacing
        let output = NSMutableAttributedString(string: source.substring(with: safeRange),
                                               attributes: [.font: body, .foregroundColor: theme.textColor, .paragraphStyle: paragraph])
        let spans = document.spans.filter { $0.end >= $0.start && NSIntersectionRange($0.range, safeRange).length > 0 }
        for span in spans {
            let clipped = NSIntersectionRange(span.range, safeRange)
            apply(span, to: output, at: NSRange(location: clipped.location - safeRange.location, length: clipped.length))
        }
        // An alert's title line takes its accent over the quote's colour; only where the alert starts.
        for span in spans where span.kind == "alert" && span.start >= safeRange.location {
            let clipped = NSIntersectionRange(span.range, safeRange)
            alertTitle(span, in: output, at: NSRange(location: clipped.location - safeRange.location, length: clipped.length))
        }
        // Superscripts shrink whatever font their run ended up with.
        for span in spans where span.kind == "footnote_reference" || (span.kind == "html" && (span.tag == "sup" || span.tag == "sub")) {
            let clipped = NSIntersectionRange(span.range, safeRange)
            shift(NSRange(location: clipped.location - safeRange.location, length: clipped.length), in: output, up: span.tag != "sub")
        }
        // Font traits compose after base fonts, regardless of equal span boundaries.
        for span in spans where span.kind == "strong" || span.kind == "emphasis" || span.kind == "table_header" {
            let clipped = NSIntersectionRange(span.range, safeRange)
            let local = NSRange(location: clipped.location - safeRange.location, length: clipped.length)
            let trait: UIFontDescriptor.SymbolicTraits = span.kind == "emphasis" ? .traitItalic : .traitBold
            let fallback = body
            output.enumerateAttribute(.font, in: local) { value, subrange, _ in
                let font = value as? UIFont ?? fallback
                if let descriptor = font.fontDescriptor.withSymbolicTraits(font.fontDescriptor.symbolicTraits.union(trait)) {
                    output.addAttribute(.font, value: UIFont(descriptor: descriptor, size: font.pointSize), range: subrange)
                }
            }
        }
        return output
    }

    private func apply(_ span: MarkitSpan, to output: NSMutableAttributedString, at local: NSRange) {
        switch span.kind {
        case "heading":
            let level = span.level ?? 1
            output.addAttribute(.font, value: theme.headingFont(level: level).font(compatibleWith: traits), range: local)
            output.addAttribute(.accessibilityTextHeadingLevel, value: NSNumber(value: level), range: local)
        case "code", "code_block":
            output.addAttribute(.font, value: monospace, range: local)
            if span.kind == "code" { output.addAttribute(.backgroundColor, value: theme.inlineCodeBackground, range: local) }
        case "table":
            output.addAttribute(.font, value: table, range: local)
        case "syntax":
            output.addAttribute(.foregroundColor, value: syntaxColor(span), range: local)
        case "strikethrough":
            output.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: local)
        case "link":
            if let url = span.url.flatMap(URL.init(string:)) {
                output.addAttribute(.link, value: url, range: local)
                if let color = theme.linkColor { output.addAttribute(.foregroundColor, value: color, range: local) }
            }
        case "reference":
            let reference = MarkitReference(kind: span.reference_kind ?? "file", target: span.target ?? "")
            output.addAttributes([.link: reference.url, .markitReference: reference.target, .markitReferenceKind: reference.kind], range: local)
            if let color = theme.linkColor { output.addAttribute(.foregroundColor, value: color, range: local) }
        case "quote", "list_item":
            indent(span, in: output, at: local)
        case "diff":
            guard diffBackgrounds, let color = diffColor(span) else { return }
            output.addAttribute(.backgroundColor, value: color, range: local)
        case "math":
            attachMath(span, to: output, at: local)
        case "alert":
            if let kind = span.alert { output.addAttribute(.markitAlert, value: kind, range: local) }
        case "footnote_reference":
            // A reference of kind `footnote`: hosts get it through `onReference`.
            let reference = MarkitReference(kind: "footnote", target: span.label ?? String(span.number ?? 0))
            output.addAttributes([.link: reference.url, .markitReference: reference.target, .markitReferenceKind: reference.kind], range: local)
            if let color = theme.linkColor { output.addAttribute(.foregroundColor, value: color, range: local) }
        case "footnote_definition":
            output.addAttributes([.font: theme.footnoteFont.font(compatibleWith: traits), .foregroundColor: theme.secondaryTextColor], range: local)
        case "task":
            task(span, in: output, at: local)
        case "html":
            if span.tag == "u" || span.tag == "ins" { output.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: local) }
        default:
            // Unknown and structural kinds (paragraph, rule, table_cell, image...) keep body text.
            break
        }
    }

    /// The title line (the alert's first line) takes the alert's accent.
    private func alertTitle(_ span: MarkitSpan, in output: NSMutableAttributedString, at local: NSRange) {
        guard let color = span.alert.flatMap({ theme.alertColor(for: $0) }) else { return }
        let text = output.string as NSString
        let newline = text.range(of: "\n", options: .literal, range: local)
        let title = newline.location == NSNotFound ? local : NSRange(location: local.location, length: newline.location - local.location)
        output.addAttribute(.foregroundColor, value: color, range: title)
    }

    /// The box glyph (the span's first character). Tappable only when the host ticks boxes.
    private func task(_ span: MarkitSpan, in output: NSMutableAttributedString, at local: NSRange) {
        guard local.length > 0 else { return }
        let glyph = (output.string as NSString).rangeOfComposedCharacterSequence(at: local.location)
        guard tasksEnabled, let offset = span.offset,
              let url = URL(string: "markit-task://toggle/\(span.index ?? 0)") else {
            output.addAttribute(.foregroundColor, value: theme.secondaryTextColor, range: glyph)
            return
        }
        output.addAttributes([
            .link: url,
            .markitTaskOffset: NSNumber(value: offset),
            .markitTaskIndex: NSNumber(value: span.index ?? 0),
            .markitTaskChecked: NSNumber(value: span.checked ?? false),
        ], range: glyph)
    }

    /// Raises (or lowers) a run as a smaller superscript (or subscript).
    private func shift(_ local: NSRange, in output: NSMutableAttributedString, up: Bool) {
        let fallback = body
        output.enumerateAttribute(.font, in: local) { value, subrange, _ in
            let font = value as? UIFont ?? fallback
            output.addAttributes([
                .font: font.withSize(font.pointSize * 0.75),
                .baselineOffset: font.pointSize * (up ? 0.35 : -0.15),
            ], range: subrange)
        }
    }

    private func indent(_ span: MarkitSpan, in output: NSMutableAttributedString, at local: NSRange) {
        let quote = span.kind == "quote"
        let indent = CGFloat(span.depth ?? 1) * (quote ? theme.quoteIndent : theme.listIndent)
        let firstLine = quote ? indent : indent - theme.listMarkerWidth
        output.enumerateAttribute(.paragraphStyle, in: local) { value, subrange, _ in
            let style = ((value as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle) ?? NSMutableParagraphStyle()
            style.headIndent = max(style.headIndent, indent)
            style.firstLineHeadIndent = max(style.firstLineHeadIndent, firstLine)
            output.addAttribute(.paragraphStyle, value: style, range: subrange)
        }
        if quote { output.addAttribute(.foregroundColor, value: theme.secondaryTextColor, range: local) }
    }

    func diffColor(_ span: MarkitSpan) -> UIColor? {
        switch span.change {
        case "addition": return theme.diffAddition
        case "deletion": return theme.diffDeletion
        default: return nil
        }
    }

    private func syntaxColor(_ span: MarkitSpan) -> UIColor {
        if let light = span.light, let index = MarkitColorPair.defaultSyntaxIndex[light], index < palette.count,
           span.dark == nil || span.dark == MarkitColorPair.defaultSyntaxHex[index].dark {
            return palette[index]
        }
        let light = span.light.map { UIColor(markitHex: $0) } ?? theme.syntaxFallback.light
        let dark = span.dark.map { UIColor(markitHex: $0) } ?? theme.syntaxFallback.dark
        return MarkitColorPair(light: light, dark: dark).color(for: traits?.userInterfaceStyle)
    }

    private func attachMath(_ span: MarkitSpan, to output: NSMutableAttributedString, at local: NSRange) {
        // The source travels with the run even when no image is drawn, for copy and plain-text hosts.
        output.addAttribute(.markitRaw, value: span.raw ?? span.tex ?? "", range: local)
        guard local.length == 1, let data = span.png.flatMap({ Data(base64Encoded: $0) }), let image = UIImage(data: data) else { return }
        let ink = traits.map { theme.textColor.resolvedColor(with: $0) } ?? theme.textColor
        let attachment = NSTextAttachment()
        attachment.image = image.withTintColor(ink, renderingMode: .alwaysOriginal)
        let scale = body.pointSize / 16
        let width = CGFloat(span.width ?? 0) / 2 * scale, height = CGFloat(span.height ?? 0) / 2 * scale
        let fit = min(1, theme.maxAttachmentWidth / max(1, width))
        attachment.bounds = CGRect(x: 0, y: -4, width: width * fit, height: height * fit)
        output.addAttribute(.attachment, value: attachment, range: local)
    }
}

/// Copies attachment runs as their original source.
final class CopyTextView: UITextView {
    override func copy(_ sender: Any?) {
        var text = ""
        attributedText.enumerateAttribute(.markitRaw, in: selectedRange) { value, range, _ in
            text += (value as? String) ?? (attributedText.string as NSString).substring(with: range)
        }
        UIPasteboard.general.string = text
    }
}
#endif
