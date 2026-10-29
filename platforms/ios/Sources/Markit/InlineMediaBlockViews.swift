#if canImport(UIKit)
import UIKit

/// A thematic break as a hairline.
final class InlineRuleBlockView: InlineBlockView {
    private let line = UIView()

    override init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        super.init(host: host, document: document, plan: plan)
        isAccessibilityElement = false
        addSubview(line)
        contentDidChange()
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    override func contentDidChange() {
        line.backgroundColor = theme.ruleColor
        super.contentDidChange()
    }
    override func measure(width: CGFloat) -> CGFloat { 9 }
    override func layoutSubviews() {
        super.layoutSubviews()
        line.frame = CGRect(x: 0, y: 4, width: bounds.width, height: 1)
    }
}

/// An image the host resolved. Nothing is fetched here: until `resolveImage`
/// returns an image, the block takes no space (its alt text is in the prose).
final class InlineImageBlockView: InlineBlockView {
    private let imageView = UIImageView()
    private var load: Task<Void, Never>?
    private var requested: String?

    override init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        super.init(host: host, document: document, plan: plan)
        imageView.contentMode = .scaleAspectFit
        imageView.clipsToBounds = true
        imageView.isAccessibilityElement = true
        imageView.accessibilityTraits = .image
        addSubview(imageView)
        contentDidChange()
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    override func discard() { load?.cancel() }

    override func contentDidChange() {
        let text = document.text as NSString
        let alt = NSIntersectionRange(plan.range, NSRange(location: 0, length: text.length))
        imageView.accessibilityLabel = text.substring(with: alt)
        if case .image(let url) = plan.kind, url != requested {
            requested = url
            load?.cancel()
            imageView.image = nil
            load = Task { [weak self] in
                guard let host = self?.host else { return }
                let image = await host.loadImage(url)
                guard let self, !Task.isCancelled, self.requested == url else { return }
                self.imageView.image = image
                if image != nil { self.heightDidChange() }
            }
        }
        super.contentDidChange()
    }

    private func fitted(width: CGFloat) -> CGSize {
        guard let image = imageView.image, image.size.width > 0, image.size.height > 0 else { return .zero }
        var size = image.size
        if size.width > width { size = CGSize(width: width, height: size.height * width / size.width) }
        if size.height > theme.maxImageHeight { size = CGSize(width: size.width * theme.maxImageHeight / size.height, height: theme.maxImageHeight) }
        return CGSize(width: floor(size.width), height: ceil(size.height))
    }

    override func measure(width: CGFloat) -> CGFloat { fitted(width: width).height }

    override func layoutSubviews() {
        super.layoutSubviews()
        imageView.frame = CGRect(origin: .zero, size: fitted(width: bounds.width))
    }
}
#endif
