#if canImport(SwiftUI) && canImport(UIKit)
import SwiftUI

/// A self-scrolling document region. For rows in a transcript use `MarkitInline`.
public struct MarkitText: UIViewRepresentable {
    public var markdown: String
    public var finalized: Bool
    public var theme: MarkitTheme
    public var onReference: ((MarkitReference) -> Void)?
    public init(_ markdown: String, finalized: Bool = true, theme: MarkitTheme = .standard,
                onReference: ((MarkitReference) -> Void)? = nil) {
        self.markdown = markdown; self.finalized = finalized; self.theme = theme; self.onReference = onReference
    }
    public func makeUIView(context: Context) -> MarkitView { MarkitView() }
    public func updateUIView(_ view: MarkitView, context: Context) {
        view.theme = theme
        view.onReference = onReference
        view.render(markdown, finalized: finalized)
    }
}
#endif
