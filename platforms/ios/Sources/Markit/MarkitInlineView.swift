#if canImport(UIKit)
import UIKit

/// Markdown that sizes itself to its content and never scrolls vertically,
/// for use as a row in a transcript (UITableView/UICollectionView cells,
/// SwiftUI LazyVStack/List through `MarkitInline`).
///
/// Streaming: call `render(_:finalized:)` with the whole text so far. Rust
/// reparses only the open trailing roots, and only views for replaced roots
/// are updated; settled blocks keep their views and cached heights. Code
/// blocks and tables scroll sideways. Resolve links, references and images in
/// the host; this view never fetches URLs or creates a web view.
///
/// The GitHub flavour (`flavor: .github`, for PR, issue and comment bodies)
/// adds alerts with an accented title and bar, footnotes (superscript
/// references and the notes below the document), task boxes the host can
/// tick (`onTask`), `@mention`, `#123` and commit references (`onReference`)
/// and diagrams the host can draw (`renderDiagram`).
@MainActor public final class MarkitInlineView: UIView {
    public var onLink: ((URL) -> Void)?
    /// App references. In the GitHub flavour also `mention`, `issue` and
    /// `commit`, and `footnote` (target: the footnote's label) for a tapped
    /// footnote reference. Without this hook a footnote reference scrolls the
    /// enclosing scroll view to its note and announces it.
    public var onReference: ((MarkitReference) -> Void)?
    /// GitHub flavour: a task box was tapped. Boxes are tappable only while
    /// this is set; the host rewrites `source[offset]` and renders again.
    public var onTask: ((MarkitTask) -> Void)? {
        didSet { if (oldValue == nil) != (onTask == nil) { restyleAll() } }
    }
    /// GitHub flavour: draws a closed diagram fence (`mermaid`). The returned
    /// view replaces the code block at the height it asks for (intrinsic size,
    /// Auto Layout or `sizeThatFits` at the block's width); nil keeps the code.
    /// Called once per diagram source; never with a fence still streaming.
    public var renderDiagram: (@MainActor (_ language: String, _ source: String) async -> UIView?)? {
        didSet { if (oldValue == nil) != (renderDiagram == nil) { restyleAll() } }
    }
    /// The dialect of the Markdown. Changing it renders the current text again.
    public var flavor: MarkitFlavor {
        didSet {
            guard oldValue != flavor else { return }
            worker = RenderWorker(flavor: flavor)
            guard let source = previous else { return }
            previous = nil
            render(source, finalized: finalized)
        }
    }
    /// Returns an image the host has already authorized for an image URL.
    public var resolveImage: (@MainActor (String) async -> UIImage?)?
    public var onError: ((Error) -> Void)?
    /// Called (asynchronously, after layout) when the height at the current
    /// width changes, for hosts that must re-measure the row.
    public var onContentHeightChange: ((CGFloat) -> Void)?
    public var theme: MarkitTheme {
        didSet { if oldValue != theme { restyleAll() } }
    }
    /// Nil never collapses code blocks.
    public var codeCollapse: MarkitCodeCollapse? {
        didSet { if oldValue != codeCollapse { restyleAll() } }
    }
    /// When false, text cannot be selected; links still respond to taps.
    public var isTextSelectable = true {
        didSet { if oldValue != isTextSelectable { restyleAll() } }
    }
    /// The first render of a source up to this many UTF-8 bytes happens
    /// synchronously, so a row appears at its final height in its first layout.
    public var synchronousRenderLimit = 64 * 1024

    private var worker: RenderWorker
    private var generation = 0
    private var previous: String?
    private var finalized = true
    private var dirty = Int.max
    private(set) var blocks: [InlineBlockView] = []
    private var cachedStyler: MarkitStyler?
    private var lastWidth: CGFloat = 0
    private var reportedHeight: CGFloat = -1
    private var reportScheduled = false

    public init(theme: MarkitTheme = .standard, flavor: MarkitFlavor = .standard) {
        self.theme = theme
        self.flavor = flavor
        worker = RenderWorker(flavor: flavor)
        super.init(frame: .zero)
        accessibilityIdentifier = "markit-inline"
    }
    public required init?(coder: NSCoder) { fatalError("Use init(theme:)") }

    // MARK: Content

    /// Shows `source`. While streaming pass the whole text so far with
    /// `finalized: false`; text that only grows is parsed incrementally.
    public func render(_ source: String, finalized: Bool = true) {
        guard source != previous || self.finalized != finalized else { return }
        let first = previous == nil
        if first { dirty = 0 }
        previous = source; self.finalized = finalized; generation += 1
        let revision = generation
        if first, source.utf8.count <= synchronousRenderLimit, let result = worker.renderNow(source, finalized: finalized) {
            receive(result, revision: revision)
            return
        }
        worker.submit(source, finalized: finalized) { [weak self] result in
            guard let self else { return }
            markitOnMain { self.receive(result, revision: revision) }
        }
    }

    /// Shows an already parsed document.
    public func setDocument(_ document: MarkitDocument) {
        generation += 1; previous = nil; dirty = Int.max
        apply([document], changedFrom: 0)
    }

    /// Removes all content, for reuse in another row.
    public func reset() {
        generation += 1; previous = nil; dirty = Int.max
        apply([], changedFrom: 0)
    }

    private func receive(_ result: Result<RenderWorker.Output, Error>, revision: Int) {
        guard previous != nil else { return }
        switch result {
        case .success(let output):
            dirty = min(dirty, output.changedFrom)
            if generation == revision { apply(output.documents, changedFrom: dirty); dirty = Int.max }
        case .failure(let error):
            if generation == revision { onError?(error) }
        }
    }

    /// Replaces views for documents from `changedFrom` on. Earlier views are
    /// untouched; replaced views of the same kind are updated in place.
    func apply(_ documents: [MarkitDocument], changedFrom: Int) {
        let start = max(0, min(changedFrom, documents.count))
        let plans = InlinePlanner.plan(documents, from: start)
        let keep = blocks.firstIndex { $0.plan.document >= start } ?? blocks.count
        let stale = Array(blocks[keep...])
        blocks.removeSubrange(keep...)
        var reused = Set<ObjectIdentifier>()
        for (offset, plan) in plans.enumerated() {
            let document = documents[plan.document]
            if offset < stale.count, stale[offset].accepts(plan) {
                stale[offset].update(document: document, plan: plan)
                reused.insert(ObjectIdentifier(stale[offset]))
                blocks.append(stale[offset])
            } else {
                let view = makeBlock(document: document, plan: plan)
                addSubview(view)
                blocks.append(view)
            }
        }
        for view in stale where !reused.contains(ObjectIdentifier(view)) {
            view.discard()
            view.removeFromSuperview()
        }
        // VoiceOver reads blocks in document order, whatever the subview order.
        accessibilityElements = blocks
        contentHeightDidChange()
    }

    private func makeBlock(document: MarkitDocument, plan: InlineBlockPlan) -> InlineBlockView {
        switch plan.kind {
        case .text: return InlineTextBlockView(host: self, document: document, plan: plan)
        case .code: return InlineCodeBlockView(host: self, document: document, plan: plan)
        case .table: return InlineTableBlockView(host: self, document: document, plan: plan)
        case .rule: return InlineRuleBlockView(host: self, document: document, plan: plan)
        case .image: return InlineImageBlockView(host: self, document: document, plan: plan)
        case .diagram: return InlineDiagramBlockView(host: self, document: document, plan: plan)
        }
    }

    private func restyleAll() {
        cachedStyler = nil
        for block in blocks { block.restyle() }
        contentHeightDidChange()
    }

    // MARK: Sizing and layout

    /// Height of the content laid out at `width`. Settled blocks answer from cache.
    public func height(for width: CGFloat) -> CGFloat {
        var y: CGFloat = 0
        layOut(width: width) { _, frame in y = frame.maxY }
        return ceil(y)
    }

    public override func sizeThatFits(_ size: CGSize) -> CGSize {
        let width = size.width > 0 && size.width.isFinite ? size.width : (bounds.width > 0 ? bounds.width : 320)
        return CGSize(width: width, height: height(for: width))
    }

    public override var intrinsicContentSize: CGSize {
        guard bounds.width > 0 else { return CGSize(width: UIView.noIntrinsicMetric, height: UIView.noIntrinsicMetric) }
        return CGSize(width: UIView.noIntrinsicMetric, height: height(for: bounds.width))
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        layOut(width: bounds.width) { block, frame in
            block.frame = frame
            block.isHidden = frame.height == 0
        }
        if bounds.width != lastWidth {
            lastWidth = bounds.width
            invalidateIntrinsicContentSize()
            scheduleHeightReport()
        }
    }

    /// Stacks blocks top to bottom. Empty blocks take no space or spacing.
    private func layOut(width: CGFloat, place: (InlineBlockView, CGRect) -> Void) {
        var y: CGFloat = 0, placedAny = false
        for block in blocks {
            let inset = CGFloat(block.plan.depth) * theme.listIndent
            let blockWidth = max(1, width - inset)
            let height = block.height(for: blockWidth)
            if height > 0, placedAny { y += block.plan.continuation ? theme.paragraphSpacing : theme.blockSpacing }
            place(block, CGRect(x: inset, y: y, width: blockWidth, height: height))
            if height > 0 { y += height; placedAny = true }
        }
    }

    private func contentHeightDidChange() {
        invalidateIntrinsicContentSize()
        setNeedsLayout()
        scheduleHeightReport()
    }

    private func scheduleHeightReport() {
        guard onContentHeightChange != nil, !reportScheduled else { return }
        reportScheduled = true
        Task { [weak self] in self?.reportHeight() }
    }

    private func reportHeight() {
        reportScheduled = false
        let width = bounds.width > 0 ? bounds.width : lastWidth
        guard width > 0 else { return }
        let height = self.height(for: width)
        guard height != reportedHeight else { return }
        reportedHeight = height
        onContentHeightChange?(height)
    }

    public override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) {
        super.traitCollectionDidChange(previousTraitCollection)
        // Restyle only when the traits the current styling was built with no longer match.
        guard let used = cachedStyler?.traits else { return }
        guard used.preferredContentSizeCategory != traitCollection.preferredContentSizeCategory
            || used.userInterfaceStyle != traitCollection.userInterfaceStyle else { return }
        restyleAll()
    }
}

extension MarkitInlineView: InlineBlockHost {
    var styler: MarkitStyler {
        if let cachedStyler { return cachedStyler }
        var styler = MarkitStyler(theme: theme, traits: traitCollection)
        styler.tasksEnabled = onTask != nil
        cachedStyler = styler
        return styler
    }

    func blockDidChangeHeight(_ block: InlineBlockView) { contentHeightDidChange() }

    func open(_ url: URL, attributes: [NSAttributedString.Key: Any]) -> Bool {
        if let task = MarkitTask(toggling: attributes) {
            onTask?(task)
            return false
        }
        if let target = attributes[.markitReference] as? String {
            let reference = MarkitReference(kind: attributes[.markitReferenceKind] as? String ?? "file", target: target)
            if let onReference { onReference(reference) } else if reference.kind == "footnote" { showFootnote(label: target) }
            return false
        }
        if let onLink { onLink(url); return false }
        return ["https", "http", "mailto", "tel"].contains(url.scheme?.lowercased() ?? "")
    }

    func loadImage(_ url: String) async -> UIImage? {
        guard let resolveImage else { return nil }
        return await resolveImage(url)
    }

    var drawsDiagrams: Bool { renderDiagram != nil }

    func drawDiagram(language: String, source: String) async -> UIView? {
        guard let renderDiagram else { return nil }
        return await renderDiagram(language, source)
    }
}

extension MarkitInlineView {
    /// Brings a footnote's note into view in the enclosing scroll view and reads it out.
    func showFootnote(label: String) {
        for block in blocks {
            guard let note = block.document.spans.first(where: { $0.kind == "footnote_definition" && $0.label == label }) else { continue }
            guard NSIntersectionRange(note.range, block.plan.range).length > 0 else { continue }
            let local = (block as? InlineTextBlockView)?.rect(forDocumentRange: note.range) ?? block.bounds
            var ancestor = superview
            while let view = ancestor, !(view is UIScrollView) { ancestor = view.superview }
            if let scroller = ancestor as? UIScrollView {
                scroller.scrollRectToVisible(block.convert(local, to: scroller).insetBy(dx: 0, dy: -24), animated: true)
            }
            let text = (block.document.text as NSString).substring(with: NSIntersectionRange(note.range, NSRange(location: 0, length: (block.document.text as NSString).length)))
            UIAccessibility.post(notification: .announcement, argument: text.trimmingCharacters(in: .whitespacesAndNewlines))
            return
        }
    }
}
#endif
