#if canImport(UIKit)
import UIKit

/// Native virtualized Markdown. Resolve app references and authorized attachment
/// images in the host. This view never fetches URLs or creates a WebView.
@MainActor public final class MarkitView: UIView, UITableViewDataSource, UITextViewDelegate {
    public var onLink: ((URL) -> Void)?
    public var onReference: ((MarkitReference) -> Void)?
    public var resolveImage: ((String) -> UIImage?)?
    public var onError: ((Error) -> Void)?
    /// Fonts, colours and spacing. Keep one value per appearance in the host.
    public var theme: MarkitTheme = .standard {
        didSet { if oldValue != theme { table.reloadData() } }
    }
    private let table = UITableView(frame: .zero, style: .plain)
    private let worker = RenderWorker()
    private var generation = 0
    private var previous: String?
    private var finalized = true
    private var dirty = Int.max
    private struct Row { let docIndex: Int; let document: MarkitDocument; let range: NSRange; let kind: String }
    private var rows: [Row] = []

    public override init(frame: CGRect) {
        super.init(frame: frame)
        table.dataSource = self; table.separatorStyle = .none; table.estimatedRowHeight = 100
        table.rowHeight = UITableView.automaticDimension; table.backgroundColor = .clear
        table.keyboardDismissMode = .interactive; table.accessibilityIdentifier = "markit-blocks"
        addSubview(table); table.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([table.topAnchor.constraint(equalTo: topAnchor), table.bottomAnchor.constraint(equalTo: bottomAnchor), table.leadingAnchor.constraint(equalTo: leadingAnchor), table.trailingAnchor.constraint(equalTo: trailingAnchor)])
        NotificationCenter.default.addObserver(self, selector: #selector(refreshAppearance), name: UIContentSizeCategory.didChangeNotification, object: nil)
    }
    public required init?(coder: NSCoder) { fatalError("Use init(frame:)") }
    deinit { NotificationCenter.default.removeObserver(self) }

    public func render(_ source: String, finalized: Bool = true) {
        guard source != previous || self.finalized != finalized else { return }
        if previous == nil { dirty = 0 }
        previous = source; self.finalized = finalized; generation += 1
        let revision = generation
        worker.submit(source, finalized: finalized) { [weak self] result in
            guard let self else { return }
            markitOnMain { self.receive(result, revision: revision) }
        }
    }
    private func receive(_ result: Result<RenderWorker.Output, Error>, revision: Int) {
        guard previous != nil else { return }
        switch result {
        case .success(let output):
            dirty = min(dirty, output.changedFrom)
            if generation == revision { display(output.documents, changedFrom: dirty); dirty = Int.max }
        case .failure(let error): if generation == revision { onError?(error) }
        }
    }
    public func setDocument(_ document: MarkitDocument) { generation += 1; previous = nil; dirty = 0; display([document], changedFrom: 0) }
    private func display(_ documents: [MarkitDocument], changedFrom: Int) {
        let first = rows.firstIndex { $0.docIndex >= changedFrom } ?? rows.count
        let oldCount = rows.count
        rows.removeSubrange(first..<rows.count)
        let appended = documents.enumerated().filter { $0.offset >= changedFrom }.flatMap { index, document in document.blocks.flatMap { block -> [Row] in
            guard block.kind == "text" && block.end - block.start > 3000 else { return [Row(docIndex: index, document: document, range: block.range, kind: block.kind)] }
            let text = document.text as NSString
            var pieces: [Row] = [], start = block.start
            while start < block.end {
                var end = min(block.end, start + 2500)
                if end < block.end {
                    let search = NSRange(location: start + 1500, length: end - start - 1500)
                    let boundary = text.rangeOfCharacter(from: .whitespacesAndNewlines, options: .backwards, range: search)
                    if boundary.location != NSNotFound { end = NSMaxRange(boundary) }
                    if (0xD800...0xDBFF).contains(Int(text.character(at: end - 1))) { end -= 1 }
                }
                pieces.append(Row(docIndex: index, document: document, range: NSRange(location: start, length: end - start), kind: "text")); start = end
            }
            return pieces
        } }
        rows.append(contentsOf: appended)
        table.performBatchUpdates {
            if oldCount > first { table.deleteRows(at: (first..<oldCount).map { IndexPath(row: $0, section: 0) }, with: .none) }
            if rows.count > first { table.insertRows(at: (first..<rows.count).map { IndexPath(row: $0, section: 0) }, with: .none) }
        }
    }
    @objc private func refreshAppearance() { table.reloadData() }
    public override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) { super.traitCollectionDidChange(previousTraitCollection); table.reloadData() }
    public func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { rows.count }
    public func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "block") ?? UITableViewCell(style: .default, reuseIdentifier: "block")
        cell.selectionStyle = .none; cell.backgroundColor = .clear
        cell.contentView.subviews.forEach { $0.removeFromSuperview() }
        let row = rows[indexPath.row]
        let inset = row.kind == "code" || row.kind == "table" ? CGFloat(row.document.spans.filter { ($0.kind == "list_item" || $0.kind == "quote") && $0.start <= row.range.location && $0.end >= NSMaxRange(row.range) }.map { $0.depth ?? 1 }.max() ?? 0) * 16 : 0
        let stack = UIStackView(); stack.axis = .vertical; stack.spacing = 10
        if row.kind == "code" || row.kind == "table" {
            stack.addArrangedSubview(MarkitRowsPane(document: row.document, range: row.range, kind: row.kind, availableWidth: max(200, tableView.bounds.width - 24 - inset), theme: theme))
        } else {
            let text = CopyTextView(); text.isEditable = false; text.isScrollEnabled = false; text.delegate = self
            text.backgroundColor = .clear; text.textContainerInset = .zero; text.textContainer.lineFragmentPadding = 0
            text.adjustsFontForContentSizeCategory = true
            text.attributedText = MarkitStyler(theme: theme, traits: traitCollection).attributed(row.document, range: row.range)
            stack.addArrangedSubview(text)
            for span in row.document.spans where span.kind == "image" && NSIntersectionRange(span.range, row.range).length > 0 {
                if let url = span.url, let image = resolveImage?(url) {
                    let imageView = UIImageView(image: image); imageView.contentMode = .scaleAspectFit; imageView.clipsToBounds = true
                    imageView.accessibilityLabel = (row.document.text as NSString).substring(with: span.range); imageView.isAccessibilityElement = true
                    imageView.heightAnchor.constraint(equalToConstant: min(320, image.size.height)).isActive = true
                    stack.addArrangedSubview(imageView)
                }
            }
        }
        cell.contentView.addSubview(stack); stack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo: cell.contentView.leadingAnchor, constant: 12 + inset), stack.trailingAnchor.constraint(equalTo: cell.contentView.trailingAnchor, constant: -12), stack.topAnchor.constraint(equalTo: cell.contentView.topAnchor, constant: 8), stack.bottomAnchor.constraint(equalTo: cell.contentView.bottomAnchor, constant: -8)])
        return cell
    }
    public func textView(_ textView: UITextView, shouldInteractWith URL: URL, in characterRange: NSRange, interaction: UITextItemInteraction) -> Bool {
        if let target = textView.attributedText.attribute(.markitReference, at: characterRange.location, effectiveRange: nil) as? String {
            let kind = textView.attributedText.attribute(.markitReferenceKind, at: characterRange.location, effectiveRange: nil) as? String ?? "file"
            onReference?(MarkitReference(kind: kind, target: target)); return false
        }
        if let onLink { onLink(URL); return false }
        return ["https", "http", "mailto", "tel"].contains(URL.scheme?.lowercased() ?? "")
    }
}
#endif
