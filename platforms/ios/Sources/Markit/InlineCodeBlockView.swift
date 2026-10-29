#if canImport(UIKit)
import UIKit

/// A fenced code block: a header with its file or language and Copy, then
/// unwrapped code that scrolls sideways, diff lines tinted across the block,
/// and an optional "Show all" control for long blocks.
final class InlineCodeBlockView: InlineBlockView {
    private let header = UIView()
    private let title = UILabel()
    private let copyButton = UIButton(type: .system)
    private let divider = UIView()
    private let scroller = UIScrollView()
    private let code: InlineTextStack
    private let footerDivider = UIView()
    private let toggle = UIButton(type: .system)
    private var bands: [UIView] = []
    private var diffLines: [(range: NSRange, color: UIColor)] = []
    private var textSize = CGSize.zero
    private var lineCount = 0
    private var collapsible = false
    private var expanded = false
    private var copiedReset: Task<Void, Never>?

    override init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        code = InlineTextStack(wraps: false)
        super.init(host: host, document: document, plan: plan)
        clipsToBounds = true
        layer.borderWidth = 1
        title.lineBreakMode = .byTruncatingHead
        title.adjustsFontForContentSizeCategory = false
        copyButton.setTitle("Copy", for: .normal)
        copyButton.accessibilityLabel = "Copy code"
        copyButton.addTarget(self, action: #selector(copyCode), for: .touchUpInside)
        toggle.contentHorizontalAlignment = .leading
        toggle.addTarget(self, action: #selector(toggleExpanded), for: .touchUpInside)
        scroller.showsVerticalScrollIndicator = false
        scroller.alwaysBounceVertical = false
        scroller.scrollsToTop = false
        scroller.addSubview(code.view)
        header.addSubview(title)
        header.addSubview(copyButton)
        for view in [header, divider, scroller, footerDivider, toggle] { addSubview(view) }
        contentDidChange()
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    override func discard() { copiedReset?.cancel() }

    override func contentDidChange() {
        let theme = self.theme
        let text = document.text as NSString
        let range = NSIntersectionRange(plan.range, NSRange(location: 0, length: text.length))
        lineCount = range.length == 0 ? 0 : countLines(in: range, text: text)
        let rule = host?.codeCollapse
        collapsible = rule.map { lineCount > $0.collapseAbove } ?? false
        let shown = collapsible && !expanded ? firstLines(of: range, count: rule?.visibleLines ?? lineCount, text: text) : range

        var styler = self.styler
        styler.diffBackgrounds = false
        code.view.attributedText = styler.attributed(document, range: shown)
        code.view.isSelectable = host?.isTextSelectable ?? true
        code.view.accessibilityLabel = titleText
        // A non-scrolling UITextView resizes its container to its own frame, which
        // was fitted to the previous text; measure unbounded so new lines aren't cut.
        code.container.size = InlineTextStack.unwrappedSize
        code.layout.ensureLayout(for: code.container)
        let used = code.layout.usedRect(for: code.container)
        textSize = CGSize(width: ceil(used.width), height: ceil(used.height))
        diffLines = document.spans.compactMap { span in
            guard span.kind == "diff", let color = styler.diffColor(span) else { return nil }
            let clipped = NSIntersectionRange(span.range, shown)
            guard clipped.length > 0 else { return nil }
            return (range: NSRange(location: clipped.location - shown.location, length: clipped.length), color: color)
        }

        let caption = theme.captionFont.font(compatibleWith: traitCollection)
        title.text = titleText
        title.font = caption
        title.textColor = theme.secondaryTextColor
        copyButton.titleLabel?.font = caption
        copyButton.tintColor = theme.secondaryTextColor
        toggle.titleLabel?.font = caption
        toggle.tintColor = theme.secondaryTextColor
        toggle.setTitle(expanded ? "Collapse" : "Show all \(lineCount) lines", for: .normal)
        toggle.accessibilityValue = expanded ? "Expanded" : "Collapsed"
        toggle.isHidden = !collapsible
        footerDivider.isHidden = !collapsible
        header.backgroundColor = theme.codeHeaderBackground
        backgroundColor = theme.codeBlockBackground
        divider.backgroundColor = theme.codeBorder
        footerDivider.backgroundColor = theme.codeBorder
        layer.cornerRadius = theme.codeCornerRadius
        layer.borderColor = theme.codeBorder.resolvedColor(with: traitCollection).cgColor
        super.contentDidChange()
    }

    private var titleText: String {
        let start = plan.range.location
        if let file = document.spans.first(where: { $0.kind == "code_metadata" && $0.start == start })?.file, !file.isEmpty { return file }
        if let language = document.spans.first(where: { $0.kind == "code_block" && $0.start == start })?.language, !language.isEmpty { return language }
        return "text"
    }

    private var headerHeight: CGFloat { max(32, ceil(theme.captionFont.font(compatibleWith: traitCollection).lineHeight) + 14) }
    private var codeHeight: CGFloat { textSize.height + theme.codeInsets.top + theme.codeInsets.bottom }

    override func measure(width: CGFloat) -> CGFloat {
        headerHeight + 1 + codeHeight + (collapsible ? 1 + headerHeight : 0)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let width = bounds.width, insets = theme.codeInsets
        let headerHeight = self.headerHeight
        header.frame = CGRect(x: 0, y: 0, width: width, height: headerHeight)
        let button = copyButton.sizeThatFits(CGSize(width: width, height: headerHeight))
        copyButton.frame = CGRect(x: width - button.width - insets.right, y: 0, width: button.width, height: headerHeight)
        title.frame = CGRect(x: insets.left, y: 0, width: max(0, copyButton.frame.minX - insets.left - 8), height: headerHeight)
        divider.frame = CGRect(x: 0, y: headerHeight, width: width, height: 1)
        scroller.frame = CGRect(x: 0, y: headerHeight + 1, width: width, height: codeHeight)
        let contentWidth = max(width, textSize.width + insets.left + insets.right)
        scroller.contentSize = CGSize(width: contentWidth, height: codeHeight)
        scroller.alwaysBounceHorizontal = contentWidth > width
        code.view.frame = CGRect(x: insets.left, y: insets.top, width: textSize.width, height: textSize.height)
        layoutBands(width: contentWidth)
        footerDivider.frame = CGRect(x: 0, y: scroller.frame.maxY, width: width, height: 1)
        toggle.frame = CGRect(x: insets.left, y: scroller.frame.maxY + 1, width: width - insets.left - insets.right, height: headerHeight)
    }

    private func layoutBands(width: CGFloat) {
        var used = 0
        for line in diffLines {
            guard let rect = code.rect(for: line.range) else { continue }
            if used == bands.count {
                let band = UIView()
                band.isAccessibilityElement = false
                scroller.insertSubview(band, belowSubview: code.view)
                bands.append(band)
            }
            bands[used].backgroundColor = line.color
            bands[used].isHidden = false
            bands[used].frame = CGRect(x: 0, y: theme.codeInsets.top + rect.minY, width: width, height: rect.height)
            used += 1
        }
        for band in bands[used...] { band.isHidden = true }
    }

    @objc private func copyCode() {
        UIPasteboard.general.string = document.source(in: plan.range)
        copyButton.setTitle("Copied", for: .normal)
        copyButton.accessibilityLabel = "Copied"
        UIAccessibility.post(notification: .announcement, argument: "Copied")
        copiedReset?.cancel()
        copiedReset = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            guard let self, !Task.isCancelled else { return }
            self.copyButton.setTitle("Copy", for: .normal)
            self.copyButton.accessibilityLabel = "Copy code"
        }
    }

    @objc private func toggleExpanded() {
        expanded.toggle()
        contentDidChange()
        heightDidChange()
    }

    private func countLines(in range: NSRange, text: NSString) -> Int {
        var count = 1, at = range.location
        let end = NSMaxRange(range)
        while at < end {
            let found = text.range(of: "\n", options: .literal, range: NSRange(location: at, length: end - at))
            guard found.location != NSNotFound else { break }
            count += 1
            at = NSMaxRange(found)
        }
        return count
    }

    private func firstLines(of range: NSRange, count lines: Int, text: NSString) -> NSRange {
        var remaining = lines, at = range.location
        let end = NSMaxRange(range)
        while remaining > 0, at < end {
            let found = text.range(of: "\n", options: .literal, range: NSRange(location: at, length: end - at))
            guard found.location != NSNotFound else { return range }
            remaining -= 1
            if remaining == 0 { return NSRange(location: range.location, length: found.location - range.location) }
            at = NSMaxRange(found)
        }
        return range
    }
}
#endif
