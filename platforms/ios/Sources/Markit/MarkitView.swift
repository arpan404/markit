#if canImport(UIKit)
import UIKit

/// Native virtualized Markdown. Resolve app references and authorized attachment
/// images in the host. This view never fetches URLs or creates a WebView.
@MainActor public final class MarkitView: UIView, UITableViewDataSource, UITextViewDelegate {
    public var onLink: ((URL) -> Void)?
    public var onReference: ((MarkitReference) -> Void)?
    public var resolveImage: ((String) -> UIImage?)?
    public var onError: ((Error) -> Void)?
    private let table = UITableView(frame: .zero, style: .plain)
    private let worker = RenderWorker()
    private var generation = 0
    private var previous: String?
    private var finalized = true
    private var dirty = Int.max
    /// A virtualized block. `spans` holds only the document spans that
    /// intersect `range`, bucketed once per document, so binding a row never
    /// scans the whole document. Code and table panes are built once per row.
    private struct Row {
        let docIndex: Int; let document: MarkitDocument; let range: NSRange; let kind: String
        let spans: [MarkitSpan]
        let pane = PaneCache()
        var isPane: Bool { kind == "code" || kind == "table" }
        /// List or quote nesting around a code or table block.
        var inset: CGFloat {
            guard isPane else { return 0 }
            let depth = spans.filter { ($0.kind == "list_item" || $0.kind == "quote") && $0.start <= range.location && $0.end >= NSMaxRange(range) }.map { $0.depth ?? 1 }.max() ?? 0
            return CGFloat(depth) * 16
        }
    }
    private final class PaneCache { var pane: MarkitRowsPane?; var width: CGFloat = -1 }
    private var rows: [Row] = []

    public override init(frame: CGRect) {
        super.init(frame: frame)
        table.dataSource = self; table.separatorStyle = .none; table.estimatedRowHeight = 100
        table.register(TextBlockCell.self, forCellReuseIdentifier: TextBlockCell.identifier)
        table.register(PaneBlockCell.self, forCellReuseIdentifier: PaneBlockCell.identifier)
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
            DispatchQueue.main.async {
                guard let self, self.previous != nil else { return }
                switch result {
                case .success(let output):
                    self.dirty = min(self.dirty, output.changedFrom)
                    if self.generation == revision { self.display(output.documents, changedFrom: self.dirty); self.dirty = Int.max }
                case .failure(let error): if self.generation == revision { self.onError?(error) }
                }
            }
        }
    }
    public func setDocument(_ document: MarkitDocument) { generation += 1; previous = nil; dirty = 0; display([document], changedFrom: 0) }
    private func display(_ documents: [MarkitDocument], changedFrom: Int) {
        let first = rows.firstIndex { $0.docIndex >= changedFrom } ?? rows.count
        let oldCount = rows.count
        var appended: [Row] = []
        for (index, document) in documents.enumerated() where index >= changedFrom {
            appended.append(contentsOf: Self.rows(for: document, at: index))
        }
        if oldCount == 0 {
            rows.append(contentsOf: appended)
            table.reloadData()
            return
        }
        table.performBatchUpdates {
            // UIKit must observe the old count before the transaction begins.
            // In particular, the first update may precede the first layout.
            let old = rows[first...].map { TextBlockCell.reuses($0.kind) }
            rows.replaceSubrange(first..<rows.count, with: appended)
            let newCount = rows.count, shared = min(oldCount, newCount)
            // Rows that keep their position and cell type are reconfigured in
            // place. A streaming block then updates its existing cell instead
            // of being deleted and rebuilt on every append.
            var reconfigure: [IndexPath] = [], reload: [IndexPath] = []
            for row in first..<shared {
                let path = IndexPath(row: row, section: 0)
                if old[row - first] == TextBlockCell.reuses(rows[row].kind) { reconfigure.append(path) } else { reload.append(path) }
            }
            if oldCount > newCount { table.deleteRows(at: (newCount..<oldCount).map { IndexPath(row: $0, section: 0) }, with: .none) }
            if newCount > oldCount { table.insertRows(at: (oldCount..<newCount).map { IndexPath(row: $0, section: 0) }, with: .none) }
            if !reload.isEmpty { table.reloadRows(at: reload, with: .none) }
            if !reconfigure.isEmpty { table.reconfigureRows(at: reconfigure) }
        }
    }
    /// Splits a document's blocks into rows and buckets its spans by row once.
    private static func rows(for document: MarkitDocument, at index: Int) -> [Row] {
        var ranges: [(NSRange, String)] = []
        let text = document.text as NSString
        for block in document.blocks {
            guard block.kind == "text" && block.end - block.start > 3000 else { ranges.append((block.range, block.kind)); continue }
            var start = block.start
            while start < block.end {
                var end = min(block.end, start + 2500)
                if end < block.end {
                    let search = NSRange(location: start + 1500, length: end - start - 1500)
                    let boundary = text.rangeOfCharacter(from: .whitespacesAndNewlines, options: .backwards, range: search)
                    if boundary.location != NSNotFound { end = NSMaxRange(boundary) }
                    if (0xD800...0xDBFF).contains(Int(text.character(at: end - 1))) { end -= 1 }
                }
                ranges.append((NSRange(location: start, length: end - start), "text")); start = end
            }
        }
        let bounds = ranges.map { $0.0 }
        let sorted = zip(bounds, bounds.dropFirst()).allSatisfy { NSMaxRange($0) <= $1.location }
        let buckets = sorted ? spansByLine(document.spans, lines: bounds) : nil
        return ranges.enumerated().map { offset, entry in
            Row(docIndex: index, document: document, range: entry.0, kind: entry.1,
                spans: buckets?[offset] ?? document.spans.filter { NSIntersectionRange($0.range, entry.0).length > 0 })
        }
    }
    @objc private func refreshAppearance() { invalidatePanes(); table.reloadData() }
    public override func traitCollectionDidChange(_ previousTraitCollection: UITraitCollection?) { super.traitCollectionDidChange(previousTraitCollection); invalidatePanes(); table.reloadData() }
    private func invalidatePanes() { for row in rows { row.pane.pane = nil } }
    public func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { rows.count }
    public func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let row = rows[indexPath.row]
        if row.isPane {
            let cell = tableView.dequeueReusableCell(withIdentifier: PaneBlockCell.identifier, for: indexPath) as! PaneBlockCell
            let inset = row.inset
            let width = max(200, tableView.bounds.width - 24 - inset)
            if row.pane.pane == nil || row.pane.width != width {
                row.pane.pane = MarkitRowsPane(document: row.document, range: row.range, kind: row.kind, availableWidth: width, spans: row.spans)
                row.pane.width = width
            }
            cell.show(row.pane.pane!, inset: inset)
            return cell
        }
        let cell = tableView.dequeueReusableCell(withIdentifier: TextBlockCell.identifier, for: indexPath) as! TextBlockCell
        cell.body.delegate = self
        cell.body.attributedText = attributed(row.document, range: row.range, traits: traitCollection, candidates: row.spans)
        cell.removeImages()
        for span in row.spans where span.kind == "image" {
            if let url = span.url, let image = resolveImage?(url) {
                let imageView = UIImageView(image: image); imageView.contentMode = .scaleAspectFit; imageView.clipsToBounds = true
                imageView.accessibilityLabel = (row.document.text as NSString).substring(with: span.range); imageView.isAccessibilityElement = true
                imageView.heightAnchor.constraint(equalToConstant: min(320, image.size.height)).isActive = true
                cell.stack.addArrangedSubview(imageView)
            }
        }
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
/// A prose block. The stack, text view and constraints live as long as the cell.
private final class TextBlockCell: UITableViewCell {
    static let identifier = "markit-text-block"
    /// Whether rows of `kind` use this cell type rather than `PaneBlockCell`.
    static func reuses(_ kind: String) -> Bool { kind != "code" && kind != "table" }
    let stack = UIStackView()
    let body = CopyTextView()
    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)
        selectionStyle = .none; backgroundColor = .clear
        body.isEditable = false; body.isScrollEnabled = false
        body.backgroundColor = .clear; body.textContainerInset = .zero; body.textContainer.lineFragmentPadding = 0
        body.adjustsFontForContentSizeCategory = true
        stack.axis = .vertical; stack.spacing = 10
        stack.addArrangedSubview(body)
        contentView.addSubview(stack); stack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 12),
            stack.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -12),
            stack.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 8),
            stack.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -8)
        ])
    }
    required init?(coder: NSCoder) { fatalError("Use init(style:reuseIdentifier:)") }
    func removeImages() { for view in stack.arrangedSubviews where view !== body { view.removeFromSuperview() } }
}
/// Hosts a row's cached code or table pane. A pane moves between cells as
/// they are reused, so its subviews are not rebuilt when it scrolls back in.
private final class PaneBlockCell: UITableViewCell {
    static let identifier = "markit-pane-block"
    private weak var pane: UIView?
    private var leading: NSLayoutConstraint?
    override init(style: UITableViewCell.CellStyle, reuseIdentifier: String?) {
        super.init(style: style, reuseIdentifier: reuseIdentifier)
        selectionStyle = .none; backgroundColor = .clear
    }
    required init?(coder: NSCoder) { fatalError("Use init(style:reuseIdentifier:)") }
    func show(_ next: UIView, inset: CGFloat) {
        if pane === next && next.superview === contentView {
            leading?.constant = 12 + inset
            return
        }
        if let pane, pane.superview === contentView { pane.removeFromSuperview() }
        pane = next
        next.removeFromSuperview()
        contentView.addSubview(next); next.translatesAutoresizingMaskIntoConstraints = false
        let leading = next.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 12 + inset)
        self.leading = leading
        NSLayoutConstraint.activate([
            leading,
            next.trailingAnchor.constraint(equalTo: contentView.trailingAnchor, constant: -12),
            next.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 8),
            next.bottomAnchor.constraint(equalTo: contentView.bottomAnchor, constant: -8)
        ])
    }
}
#endif
