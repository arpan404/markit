#if canImport(SwiftUI) && canImport(UIKit)
import SwiftUI

/// Self-sizing Markdown for a SwiftUI row (LazyVStack, List). It takes the
/// width it is offered and exactly the height its content needs; it never
/// scrolls vertically. Pass the whole text so far while streaming.
///
/// iOS 16+ sizes through `sizeThatFits`. iOS 15 applies the measured height
/// after the first layout at the offered width.
public struct MarkitInline: View {
    public var markdown: String
    public var finalized: Bool
    public var theme: MarkitTheme
    public var isTextSelectable: Bool
    public var codeCollapse: MarkitCodeCollapse?
    public var onLink: ((URL) -> Void)?
    public var onReference: ((MarkitReference) -> Void)?
    public var resolveImage: (@MainActor (String) async -> UIImage?)?
    @State private var height: CGFloat?

    public init(markdown: String, finalized: Bool = true, theme: MarkitTheme = .standard,
                isTextSelectable: Bool = true, codeCollapse: MarkitCodeCollapse? = nil,
                onLink: ((URL) -> Void)? = nil, onReference: ((MarkitReference) -> Void)? = nil,
                resolveImage: (@MainActor (String) async -> UIImage?)? = nil) {
        self.markdown = markdown
        self.finalized = finalized
        self.theme = theme
        self.isTextSelectable = isTextSelectable
        self.codeCollapse = codeCollapse
        self.onLink = onLink
        self.onReference = onReference
        self.resolveImage = resolveImage
    }

    public var body: some View {
        let host = MarkitInlineHost(markdown: markdown, finalized: finalized, theme: theme,
                                    isTextSelectable: isTextSelectable, codeCollapse: codeCollapse,
                                    onLink: onLink, onReference: onReference, resolveImage: resolveImage,
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
    let theme: MarkitTheme
    let isTextSelectable: Bool
    let codeCollapse: MarkitCodeCollapse?
    let onLink: ((URL) -> Void)?
    let onReference: ((MarkitReference) -> Void)?
    let resolveImage: (@MainActor (String) async -> UIImage?)?
    /// Last height the view reported. A change makes SwiftUI measure again.
    let measuredHeight: CGFloat?
    let onHeight: (CGFloat) -> Void

    func makeUIView(context: Context) -> MarkitInlineView {
        let view = MarkitInlineView(theme: theme)
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
