#if canImport(UIKit)
import UIKit

/// A closed GitHub-flavour diagram fence (```` ```mermaid ````). It shows as
/// its code block until the host's `renderDiagram` returns a view, which then
/// takes the block's place at the height it asks for. A nil drawing, or no
/// hook, keeps the code block. Streaming updates with the same source keep
/// the drawing.
final class InlineDiagramBlockView: InlineBlockView {
    private var fallback: InlineCodeBlockView?
    private var drawn: UIView?
    private var load: Task<Void, Never>?
    /// The diagram last asked for, so an unchanged one is never redrawn.
    private var requested: (language: String, source: String)?

    override init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        super.init(host: host, document: document, plan: plan)
        let fallback = InlineCodeBlockView(host: self, document: document, plan: plan)
        addSubview(fallback)
        self.fallback = fallback
        contentDidChange()
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    override func discard() {
        load?.cancel()
        fallback?.discard()
    }

    override func contentDidChange() {
        fallback?.update(document: document, plan: plan)
        requestDrawing()
        super.contentDidChange()
    }

    /// Whether the host's drawing is showing (for tests and accessibility).
    var isDrawn: Bool { drawn != nil }

    private func requestDrawing() {
        guard case .diagram(let language, let source) = plan.kind, host?.drawsDiagrams == true else {
            // Without a hook the code block is the diagram.
            load?.cancel(); load = nil; requested = nil
            show(nil)
            return
        }
        if let requested, requested.language == language, requested.source == source { return }
        requested = (language, source)
        load?.cancel()
        show(nil)
        load = Task { [weak self] in
            guard let host = self?.host else { return }
            let view = await host.drawDiagram(language: language, source: source)
            guard let self, !Task.isCancelled, let requested = self.requested,
                  requested.language == language, requested.source == source else { return }
            self.show(view)
        }
    }

    private func show(_ view: UIView?) {
        guard view !== drawn else { return }
        drawn?.removeFromSuperview()
        drawn = view
        if let view {
            if view.accessibilityLabel == nil && !view.isAccessibilityElement && view.accessibilityElements == nil {
                view.isAccessibilityElement = true
                view.accessibilityLabel = "Diagram"
            }
            addSubview(view)
        }
        fallback?.isHidden = view != nil
        let shown: UIView? = view ?? fallback
        accessibilityElements = shown.map { [$0] }
        heightDidChange()
    }

    override func measure(width: CGFloat) -> CGFloat {
        guard let drawn else { return fallback?.height(for: width) ?? 0 }
        return Self.fittedHeight(of: drawn, width: width)
    }

    /// Height the host's view asks for at `width`: its intrinsic size scaled
    /// down to fit, its Auto Layout height, or what `sizeThatFits` reports.
    static func fittedHeight(of view: UIView, width: CGFloat) -> CGFloat {
        let intrinsic = view.intrinsicContentSize
        if intrinsic.width > 0, intrinsic.height > 0 {
            return intrinsic.width > width ? intrinsic.height * width / intrinsic.width : intrinsic.height
        }
        if !view.translatesAutoresizingMaskIntoConstraints {
            let size = view.systemLayoutSizeFitting(CGSize(width: width, height: 0),
                                                    withHorizontalFittingPriority: .required,
                                                    verticalFittingPriority: .fittingSizeLevel)
            if size.height > 0 { return size.height }
        }
        return max(0, view.sizeThatFits(CGSize(width: width, height: CGFloat.greatestFiniteMagnitude)).height)
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        fallback?.frame = bounds
        drawn?.frame = bounds
    }
}

/// The fallback code block asks its diagram block, which answers for the inline view.
extension InlineDiagramBlockView: InlineBlockHost {
    var codeCollapse: MarkitCodeCollapse? { host?.codeCollapse }
    var isTextSelectable: Bool { host?.isTextSelectable ?? true }
    var drawsDiagrams: Bool { false }

    func blockDidChangeHeight(_ block: InlineBlockView) {
        if drawn == nil { heightDidChange() }
    }

    func open(_ url: URL, attributes: [NSAttributedString.Key: Any]) -> Bool {
        host?.open(url, attributes: attributes) ?? false
    }

    func loadImage(_ url: String) async -> UIImage? { await host?.loadImage(url) }

    func drawDiagram(language: String, source: String) async -> UIView? { nil }
}
#endif
