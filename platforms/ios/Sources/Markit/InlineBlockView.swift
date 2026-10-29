#if canImport(UIKit)
import UIKit

/// What block views need from the inline view that owns them.
@MainActor protocol InlineBlockHost: AnyObject {
    var theme: MarkitTheme { get }
    var styler: MarkitStyler { get }
    var codeCollapse: MarkitCodeCollapse? { get }
    var isTextSelectable: Bool { get }
    /// A block's height changed outside a render (image loaded, code expanded).
    func blockDidChangeHeight(_ block: InlineBlockView)
    /// Handles a tapped link. True lets the text view open it.
    func open(_ url: URL, attributes: [NSAttributedString.Key: Any]) -> Bool
    func loadImage(_ url: String) async -> UIImage?
}

/// One block in a `MarkitInlineView`. Heights are cached per width, so a
/// settled block is measured once and only changed blocks lay out again.
@MainActor class InlineBlockView: UIView {
    weak var host: (any InlineBlockHost)?
    private(set) var document: MarkitDocument
    private(set) var plan: InlineBlockPlan
    private var measured: (width: CGFloat, height: CGFloat)?

    init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        self.host = host
        self.document = document
        self.plan = plan
        super.init(frame: .zero)
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    /// Whether this view can show `plan` by updating in place.
    func accepts(_ plan: InlineBlockPlan) -> Bool { plan.kind == self.plan.kind }

    func update(document: MarkitDocument, plan: InlineBlockPlan) {
        self.document = document
        self.plan = plan
        contentDidChange()
    }

    /// Theme, traits or options changed.
    func restyle() { contentDidChange() }

    /// Subclasses rebuild their content here and call `super`.
    func contentDidChange() {
        measured = nil
        setNeedsLayout()
    }

    final func height(for width: CGFloat) -> CGFloat {
        if let measured, measured.width == width { return measured.height }
        let height = max(0, ceil(measure(width: width)))
        measured = (width, height)
        return height
    }

    /// Height at `width`; called only when the cache misses.
    func measure(width: CGFloat) -> CGFloat { 0 }

    /// Reports a height change that no render caused.
    func heightDidChange() {
        measured = nil
        setNeedsLayout()
        host?.blockDidChangeHeight(self)
    }

    /// Called once when the view leaves the inline view.
    func discard() {}

    var styler: MarkitStyler { host?.styler ?? MarkitStyler(theme: .standard, traits: traitCollection) }
    var theme: MarkitTheme { host?.theme ?? .standard }
}

/// A TextKit 1 text view, so text can be measured and decorated by range.
@MainActor final class InlineTextStack {
    let storage: NSTextStorage
    let layout: NSLayoutManager
    let container: NSTextContainer
    let view: CopyTextView

    /// Container size for text that never wraps and grows freely downward.
    static let unwrappedSize = CGSize(width: 20_000, height: CGFloat.greatestFiniteMagnitude)

    init(wraps: Bool) {
        let storage = NSTextStorage(), layout = NSLayoutManager()
        let container = NSTextContainer(size: wraps ? CGSize(width: 0, height: CGFloat.greatestFiniteMagnitude) : Self.unwrappedSize)
        container.widthTracksTextView = wraps
        container.heightTracksTextView = false
        container.lineFragmentPadding = 0
        storage.addLayoutManager(layout)
        layout.addTextContainer(container)
        let view = CopyTextView(frame: .zero, textContainer: container)
        view.isEditable = false
        view.isScrollEnabled = false
        view.scrollsToTop = false
        view.backgroundColor = .clear
        view.textContainerInset = .zero
        view.adjustsFontForContentSizeCategory = false
        self.storage = storage
        self.layout = layout
        self.container = container
        self.view = view
    }

    /// Bounding rect of a character range in text view coordinates.
    func rect(for characters: NSRange) -> CGRect? {
        let glyphs = layout.glyphRange(forCharacterRange: characters, actualCharacterRange: nil)
        guard glyphs.length > 0 else { return nil }
        return layout.boundingRect(forGlyphRange: glyphs, in: container)
    }

    /// The link attributes under a point in text view coordinates, if any.
    func link(at point: CGPoint) -> (url: URL, attributes: [NSAttributedString.Key: Any])? {
        guard storage.length > 0 else { return nil }
        let glyph = layout.glyphIndex(for: point, in: container)
        let bounds = layout.boundingRect(forGlyphRange: NSRange(location: glyph, length: 1), in: container)
        guard bounds.contains(point) else { return nil }
        let index = layout.characterIndexForGlyph(at: glyph)
        guard index < storage.length else { return nil }
        let attributes = storage.attributes(at: index, effectiveRange: nil)
        guard let url = attributes[.link] as? URL ?? (attributes[.link] as? String).flatMap(URL.init(string:)) else { return nil }
        return (url, attributes)
    }

    func applyLinkStyle(_ theme: MarkitTheme) {
        if let color = theme.linkColor { view.linkTextAttributes = [.foregroundColor: color] }
    }
}
#endif
