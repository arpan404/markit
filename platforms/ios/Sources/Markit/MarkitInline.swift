#if canImport(SwiftUI) && canImport(UIKit)
import SwiftUI

/// Self-sizing Markdown for a SwiftUI row (LazyVStack, List). It takes the
/// width it is offered and exactly the height its content needs; it never
/// scrolls vertically. Pass the whole text so far while streaming.
///
/// iOS 16+ sizes through `sizeThatFits`. iOS 15 applies the measured height
/// after the first layout at the offered width.
///
/// `flavor: .github` renders PR, issue and comment bodies; see
/// `MarkitInlineView` for `onTask` and `renderDiagram`.
public struct MarkitInline: View {
    public var markdown: String
    public var finalized: Bool
    public var flavor: MarkitFlavor
    public var theme: MarkitTheme
    public var isTextSelectable: Bool
    public var codeCollapse: MarkitCodeCollapse?
    public var onLink: ((URL) -> Void)?
    public var onReference: ((MarkitReference) -> Void)?
    public var resolveImage: (@MainActor (String) async -> UIImage?)?
    public var onTask: ((MarkitTask) -> Void)?
    public var renderDiagram: (@MainActor (_ language: String, _ source: String) async -> UIView?)?
    @State private var height: CGFloat?

    public init(markdown: String, finalized: Bool = true, flavor: MarkitFlavor = .standard, theme: MarkitTheme = .standard,
                isTextSelectable: Bool = true, codeCollapse: MarkitCodeCollapse? = nil,
                onLink: ((URL) -> Void)? = nil, onReference: ((MarkitReference) -> Void)? = nil,
                resolveImage: (@MainActor (String) async -> UIImage?)? = nil,
                onTask: ((MarkitTask) -> Void)? = nil,
                renderDiagram: (@MainActor (_ language: String, _ source: String) async -> UIView?)? = nil) {
        self.markdown = markdown
        self.finalized = finalized
        self.flavor = flavor
        self.theme = theme
        self.isTextSelectable = isTextSelectable
        self.codeCollapse = codeCollapse
        self.onLink = onLink
        self.onReference = onReference
        self.resolveImage = resolveImage
        self.onTask = onTask
        self.renderDiagram = renderDiagram
    }

    public var body: some View {
        let host = MarkitInlineHost(markdown: markdown, finalized: finalized, flavor: flavor, theme: theme,
                                    isTextSelectable: isTextSelectable, codeCollapse: codeCollapse,
                                    onLink: onLink, onReference: onReference, resolveImage: resolveImage,
                                    onTask: onTask, renderDiagram: renderDiagram,
                                    measuredHeight: height, onHeight: { height = $0 })
        if #available(iOS 16.0, *) {
            host
        } else {
            host.frame(height: height ?? 0, alignment: .top)
        }
    }
}

struct MarkitInlineHost: UIViewRepresentable {
    let markdown: String
    let finalized: Bool
    let flavor: MarkitFlavor
    let theme: MarkitTheme
    let isTextSelectable: Bool
    let codeCollapse: MarkitCodeCollapse?
    let onLink: ((URL) -> Void)?
    let onReference: ((MarkitReference) -> Void)?
    let resolveImage: (@MainActor (String) async -> UIImage?)?
    let onTask: ((MarkitTask) -> Void)?
    let renderDiagram: (@MainActor (_ language: String, _ source: String) async -> UIView?)?
    /// Last height the view reported. A change makes SwiftUI measure again.
    let measuredHeight: CGFloat?
    let onHeight: (CGFloat) -> Void

    func makeUIView(context: Context) -> MarkitInlineView {
        let view = MarkitInlineView(theme: theme, flavor: flavor)
        view.setContentHuggingPriority(.defaultLow, for: .horizontal)
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        view.setContentHuggingPriority(.required, for: .vertical)
        return view
    }

    func updateUIView(_ view: MarkitInlineView, context: Context) {
        view.theme = theme
        view.isTextSelectable = isTextSelectable
        view.codeCollapse = codeCollapse
        view.onLink = onLink
        view.onReference = onReference
        view.resolveImage = resolveImage
        view.onTask = onTask
        view.renderDiagram = renderDiagram
        view.flavor = flavor
        view.onContentHeightChange = onHeight
        view.render(markdown, finalized: finalized)
    }

    @available(iOS 16.0, *)
    func sizeThatFits(_ proposal: ProposedViewSize, uiView: MarkitInlineView, context: Context) -> CGSize? {
        let offered = proposal.width ?? 0
        let width = offered > 0 && offered.isFinite ? offered : (uiView.bounds.width > 0 ? uiView.bounds.width : 320)
        return CGSize(width: width, height: uiView.height(for: width))
    }
}
#endif
