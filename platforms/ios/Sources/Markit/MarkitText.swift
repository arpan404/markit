#if canImport(SwiftUI) && canImport(UIKit)
import SwiftUI

public struct MarkitText: UIViewRepresentable {
    public var markdown: String
    public var finalized: Bool
    public var onReference: ((MarkitReference) -> Void)?
    public init(_ markdown: String, finalized: Bool = true, onReference: ((MarkitReference) -> Void)? = nil) {
        self.markdown = markdown; self.finalized = finalized; self.onReference = onReference
    }
    public func makeUIView(context: Context) -> MarkitView { MarkitView() }
    public func updateUIView(_ view: MarkitView, context: Context) { view.onReference = onReference; view.render(markdown, finalized: finalized) }
}
#endif
