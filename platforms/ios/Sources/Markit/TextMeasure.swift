#if canImport(UIKit)
import UIKit

/// How tall attributed text is at a width, from a TextKit 1 layout with no view: the same
/// typesetting a block's text view does, with no line fragment padding or insets.
///
/// A non-scrolling text view asked the same (`sizeThatFits`) lays its text out at the asked
/// width, then again at its own frame's when it restores its container, and a text view that
/// has no frame yet lays out at width zero. This lays the text out once.
///
/// Not thread-safe: use one instance per thread.
final class TextMeasure {
    private let storage = NSTextStorage()
    private let layout = NSLayoutManager()
    private let container = NSTextContainer(size: CGSize(width: 0, height: CGFloat.greatestFiniteMagnitude))

    init() {
        container.lineFragmentPadding = 0
        layout.addTextContainer(container)
        storage.addLayoutManager(layout)
    }

    func height(of text: NSAttributedString, width: CGFloat) -> CGFloat {
        // The width goes first, so the text is never laid out at the previous one.
        container.size = CGSize(width: width, height: CGFloat.greatestFiniteMagnitude)
        storage.setAttributedString(text)
        layout.ensureLayout(for: container)
        let height = layout.usedRect(for: container).height
        // Let go of the text and its attachments until the next measure.
        storage.setAttributedString(NSAttributedString())
        return height
    }

    /// The main thread's, which block views measure with.
    @MainActor static let main = TextMeasure()
}
#endif
