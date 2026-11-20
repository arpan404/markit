#if canImport(UIKit)
import UIKit

/// Prose: paragraphs, headings, lists and quotes of one text root (or one
/// piece of a long root), with math attachments, links and quote bars.
final class InlineTextBlockView: InlineBlockView, UITextViewDelegate {
    private let text: InlineTextStack
    /// Quote bars; a GitHub alert's bar takes its accent instead of `quoteBar`.
    private var quotes: [(range: NSRange, depth: Int, color: UIColor?)] = []
    private var bars: [UIView] = []
    private lazy var tap = UITapGestureRecognizer(target: self, action: #selector(tapped(_:)))
    /// What the block shows, styled.
    private var content = NSAttributedString()
    /// The text view still holds older text (or none). It takes `content` once the block has a
    /// width, so TextKit lays the text out once, at that width, and never at width zero.
    private var stale = false

    override init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        text = InlineTextStack(wraps: true)
        super.init(host: host, document: document, plan: plan)
        text.view.delegate = self
        addSubview(text.view)
        contentDidChange()
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    override func contentDidChange() {
        let content = styler.attributed(document, range: plan.range)
        self.content = content
        stale = true
        if bounds.width > 0 { show() }
        text.applyLinkStyle(theme)
        let selectable = host?.isTextSelectable ?? true
        // Selection also enables UITextView's own link taps; without it, a tap recognizer handles links.
        text.view.isSelectable = selectable
        if selectable { text.view.removeGestureRecognizer(tap) } else if tap.view == nil { text.view.addGestureRecognizer(tap) }
        let alerts = document.spans.filter { $0.kind == "alert" }
        let palette = theme
        quotes = document.spans.compactMap { span in
            guard span.kind == "quote" else { return nil }
            let clipped = NSIntersectionRange(span.range, plan.range)
            guard clipped.length > 0 else { return nil }
            let alert = alerts.first { $0.start == span.start && $0.end == span.end }?.alert
            return (range: NSRange(location: clipped.location - plan.range.location, length: clipped.length), depth: span.depth ?? 1,
                    color: alert.flatMap { palette.alertColor(for: $0) })
        }
        text.view.accessibilityValue = spokenText(content)
        super.contentDidChange()
    }

    /// Reads the text view's own layout when it already shows this text at `width` (a block
    /// updated in place while streaming); otherwise lays the text out once, with no view.
    override func measure(width: CGFloat) -> CGFloat {
        if !stale, text.container.size.width == width {
            text.layout.ensureLayout(for: text.container)
            return text.layout.usedRect(for: text.container).height
        }
        return TextMeasure.main.height(of: content, width: width)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        text.view.frame = bounds
        if stale, bounds.width > 0 { show() }
        layoutQuoteBars()
    }

    private func show() {
        text.view.attributedText = content
        stale = false
    }

    private func layoutQuoteBars() {
        if !quotes.isEmpty { text.layout.ensureLayout(for: text.container) }
        var used = 0
        for quote in quotes {
            guard let rect = text.rect(for: quote.range) else { continue }
            if used == bars.count {
                let bar = UIView()
                bar.isAccessibilityElement = false
                insertSubview(bar, belowSubview: text.view)
                bars.append(bar)
            }
            let bar = bars[used]
            bar.backgroundColor = quote.color ?? theme.quoteBar
            bar.isHidden = false
            bar.frame = CGRect(x: CGFloat(quote.depth - 1) * theme.quoteIndent + 2, y: rect.minY,
                               width: theme.quoteBarWidth, height: rect.height)
            used += 1
        }
        for bar in bars[used...] { bar.isHidden = true }
    }

    /// Text with attachments read as their formula source, for VoiceOver.
    private func spokenText(_ content: NSAttributedString) -> String? {
        var hasAttachment = false
        content.enumerateAttribute(.attachment, in: NSRange(location: 0, length: content.length), options: []) { value, _, stop in
            if value != nil { hasAttachment = true; stop.pointee = true }
        }
        guard hasAttachment else { return nil }
        var spoken = ""
        content.enumerateAttributes(in: NSRange(location: 0, length: content.length), options: []) { attributes, range, _ in
            if attributes[.attachment] != nil {
                spoken += attributes[.markitRaw] as? String ?? ""
            } else {
                spoken += (content.string as NSString).substring(with: range)
            }
        }
        return spoken
    }

    /// Bounds, in this view, of a range of the document this block shows; nil when outside it.
    func rect(forDocumentRange range: NSRange) -> CGRect? {
        let clipped = NSIntersectionRange(range, plan.range)
        guard clipped.length > 0 else { return nil }
        if stale { show() }
        text.layout.ensureLayout(for: text.container)
        return text.rect(for: NSRange(location: clipped.location - plan.range.location, length: clipped.length))
            .map { text.view.convert($0, to: self) }
    }

    @objc private func tapped(_ recognizer: UITapGestureRecognizer) {
        guard let link = text.link(at: recognizer.location(in: text.view)) else { return }
        _ = host?.open(link.url, attributes: link.attributes)
    }

    func textView(_ textView: UITextView, shouldInteractWith url: URL, in characterRange: NSRange,
                  interaction: UITextItemInteraction) -> Bool {
        let attributes = textView.attributedText.attributes(at: characterRange.location, effectiveRange: nil)
        // Previews and menus are for real links, not references or task boxes.
        guard interaction == .invokeDefaultAction else { return attributes[.markitReference] == nil && attributes[.markitTaskOffset] == nil }
        return host?.open(url, attributes: attributes) ?? false
    }
}
#endif
