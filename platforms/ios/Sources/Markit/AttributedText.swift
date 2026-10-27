#if canImport(UIKit)
import UIKit

extension NSAttributedString.Key {
    static let markitRaw = NSAttributedString.Key("dev.markit.raw")
    static let markitReference = NSAttributedString.Key("dev.markit.reference")
    static let markitReferenceKind = NSAttributedString.Key("dev.markit.reference.kind")
}
extension UIColor {
    convenience init(markitHex value: String) {
        let rgb = UInt32(value.dropFirst(), radix: 16) ?? 0
        self.init(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255, blue: CGFloat(rgb & 255) / 255, alpha: 1)
    }
}

func attributed(_ document: MarkitDocument, range: NSRange, traits: UITraitCollection) -> NSAttributedString {
    let source = document.text as NSString
    let safeRange = NSIntersectionRange(range, NSRange(location: 0, length: source.length))
    let paragraph = NSMutableParagraphStyle(); paragraph.lineSpacing = 3; paragraph.paragraphSpacing = 6
    let body = UIFont.preferredFont(forTextStyle: .body, compatibleWith: traits)
    let output = NSMutableAttributedString(string: source.substring(with: safeRange), attributes: [.font: body, .foregroundColor: UIColor.label, .paragraphStyle: paragraph])
    let spans = document.spans.filter { NSIntersectionRange($0.range, safeRange).length > 0 }
    for span in spans {
        let clipped = NSIntersectionRange(span.range, safeRange)
        let local = NSRange(location: clipped.location - safeRange.location, length: clipped.length)
        switch span.kind {
        case "heading":
            let size: CGFloat = [30, 25, 21, 19, 17, 16][min(5, max(0, (span.level ?? 1) - 1))]
            output.addAttribute(.font, value: UIFontMetrics.default.scaledFont(for: .systemFont(ofSize: size, weight: .semibold), compatibleWith: traits), range: local)
        case "code", "code_block", "table":
            output.addAttribute(.font, value: UIFontMetrics.default.scaledFont(for: .monospacedSystemFont(ofSize: 13, weight: .regular), compatibleWith: traits), range: local)
            if span.kind == "code" { output.addAttribute(.backgroundColor, value: UIColor.secondarySystemBackground, range: local) }
        case "syntax":
            output.addAttribute(.foregroundColor, value: UIColor(markitHex: (traits.userInterfaceStyle == .dark ? span.dark : span.light) ?? "#24292f"), range: local)
        case "strikethrough": output.addAttribute(.strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: local)
        case "link": if let url = span.url.flatMap(URL.init(string:)) { output.addAttribute(.link, value: url, range: local) }
        case "reference":
            output.addAttributes([.link: URL(string: "markit-reference://open")!, .markitReference: span.target ?? "", .markitReferenceKind: span.reference_kind ?? "file"], range: local)
        case "quote", "list_item":
            output.enumerateAttribute(.paragraphStyle, in: local) { value, subrange, _ in
                let style = ((value as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle) ?? NSMutableParagraphStyle()
                let indent = CGFloat(span.depth ?? 1) * 16
                style.headIndent = max(style.headIndent, indent)
                style.firstLineHeadIndent = max(style.firstLineHeadIndent, span.kind == "quote" ? indent : indent - 12)
                output.addAttribute(.paragraphStyle, value: style, range: subrange)
            }
            if span.kind == "quote" { output.addAttribute(.foregroundColor, value: UIColor.secondaryLabel, range: local) }
        case "diff":
            let color: UIColor? = span.change == "addition" ? .systemGreen : span.change == "deletion" ? .systemRed : nil
            if let color { output.addAttribute(.backgroundColor, value: color.withAlphaComponent(0.12), range: local) }
        case "math":
            if let data = span.png.flatMap({ Data(base64Encoded: $0) }), let image = UIImage(data: data), local.length == 1 {
                let attachment = NSTextAttachment()
                attachment.image = image.withTintColor(.label.resolvedColor(with: traits), renderingMode: .alwaysOriginal)
                let scale = body.pointSize / 16
                let width = CGFloat(span.width ?? 0) / 2 * scale, height = CGFloat(span.height ?? 0) / 2 * scale
                let fit = min(1, 320 / max(1, width))
                attachment.bounds = CGRect(x: 0, y: -4, width: width * fit, height: height * fit)
                output.addAttributes([.attachment: attachment, .markitRaw: span.raw ?? span.tex ?? ""], range: local)
            }
        default: break
        }
    }
    // Font traits compose after base fonts, regardless of equal span boundaries.
    for span in spans where span.kind == "strong" || span.kind == "emphasis" || span.kind == "table_header" {
        let clipped = NSIntersectionRange(span.range, safeRange)
        let local = NSRange(location: clipped.location - safeRange.location, length: clipped.length)
        output.enumerateAttribute(.font, in: local) { value, subrange, _ in
            let font = value as? UIFont ?? body
            let trait: UIFontDescriptor.SymbolicTraits = span.kind == "emphasis" ? .traitItalic : .traitBold
            if let descriptor = font.fontDescriptor.withSymbolicTraits(font.fontDescriptor.symbolicTraits.union(trait)) {
                output.addAttribute(.font, value: UIFont(descriptor: descriptor, size: font.pointSize), range: subrange)
            }
        }
    }
    return output
}

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
