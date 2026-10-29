#if canImport(SwiftUI) && canImport(UIKit)
import SwiftUI

/// A self-scrolling document region. For rows in a transcript use `MarkitInline`.
public struct MarkitText: UIViewRepresentable {
    public var markdown: String
    public var finalized: Bool
    public var theme: MarkitTheme
    public var flavor: MarkitFlavor
    public var onReference: ((MarkitReference) -> Void)?
    public var onTask: ((MarkitTask) -> Void)?
    public init(_ markdown: String, finalized: Bool = true, theme: MarkitTheme = .standard, flavor: MarkitFlavor = .standard,
                onReference: ((MarkitReference) -> Void)? = nil, onTask: ((MarkitTask) -> Void)? = nil) {
        self.markdown = markdown; self.finalized = finalized; self.theme = theme; self.flavor = flavor
        self.onReference = onReference; self.onTask = onTask
    }
    public func makeUIView(context: Context) -> MarkitView { MarkitView() }
    public func updateUIView(_ view: MarkitView, context: Context) {
        view.theme = theme
        view.onReference = onReference
        view.onTask = onTask
        view.flavor = flavor
        view.render(markdown, finalized: finalized)
    }
}
#endif
