#if canImport(UIKit)
import UIKit

/// A bounded viewport for code lines or table rows. Only visible native cells
/// are created, even when a source file contains thousands of lines.
final class MarkitRowsPane: UIView, UITableViewDataSource {
    private let table = UITableView(frame: .zero, style: .plain)
    private let document: MarkitDocument
    private let sourceRange: NSRange
    private var lines: [NSRange] = []
    private var lineSpans: [[MarkitSpan]]?
    private var cells: [[MarkitSpan]] = []
    private let isTable: Bool
    private let contentWidth: CGFloat

    init(document: MarkitDocument, range: NSRange, kind: String, availableWidth: CGFloat) {
        self.document = document; sourceRange = range; isTable = kind == "table"
        let text = document.text as NSString
        if isTable {
            let spans = document.spans.filter { $0.kind == "table_cell" && $0.start >= range.location && $0.end <= NSMaxRange(range) }
            let groups = Dictionary(grouping: spans, by: { $0.row ?? 0 })
            cells = groups.keys.sorted().map { groups[$0]!.sorted { ($0.column ?? 0) < ($1.column ?? 0) } }
            contentWidth = max(availableWidth, CGFloat(cells.first?.count ?? 1) * 160)
        } else {
            var start = range.location
            while start < NSMaxRange(range) {
                let remainder = NSRange(location: start, length: NSMaxRange(range) - start)
                let newline = text.range(of: "\n", range: remainder)
                let end = newline.location == NSNotFound ? NSMaxRange(range) : newline.location
                lines.append(NSRange(location: start, length: end - start))
                start = newline.location == NSNotFound ? NSMaxRange(range) : end + 1
            }
            // Small or unhighlighted blocks are cheaper to scan directly.
            if lines.count > 50 && document.spans.count > lines.count {
                lineSpans = spansByLine(document.spans, lines: lines)
            }
            let widest = lines.map(\.length).max() ?? 0
            contentWidth = max(availableWidth, min(12000, CGFloat(widest + 8) * 8))
        }
        super.init(frame: .zero)
        let header = UIStackView(); header.axis = .horizontal; header.alignment = .center
        header.isLayoutMarginsRelativeArrangement = true; header.directionalLayoutMargins = .init(top: 6, leading: 12, bottom: 6, trailing: 12)
        header.backgroundColor = .secondarySystemBackground
        let label = UILabel(); label.font = .preferredFont(forTextStyle: .caption1); label.textColor = .secondaryLabel
        let metadata = document.spans.first { $0.kind == "code_metadata" && $0.start == range.location }
        label.text = isTable ? "Table" : metadata?.file ?? document.spans.first(where: { $0.kind == "code_block" && $0.start == range.location })?.language ?? "Code"
        label.lineBreakMode = .byTruncatingHead
        let button = UIButton(type: .system); button.setTitle("Copy", for: .normal); button.accessibilityLabel = isTable ? "Copy table" : "Copy code"
        button.addAction(UIAction { _ in UIPasteboard.general.string = document.source(in: range); button.setTitle("Copied", for: .normal) }, for: .touchUpInside)
        header.addArrangedSubview(label); header.addArrangedSubview(button)
        let scroller = UIScrollView(); scroller.alwaysBounceHorizontal = contentWidth > availableWidth
        table.dataSource = self; table.separatorStyle = isTable ? .singleLine : .none
        table.rowHeight = isTable ? 48 : ceil(UIFontMetrics.default.scaledFont(for: .monospacedSystemFont(ofSize: 13, weight: .regular)).lineHeight + 4)
        table.backgroundColor = .clear; table.contentInset = .zero
        table.accessibilityIdentifier = isTable ? "markit-table-rows" : "markit-code-lines"
        scroller.addSubview(table)
        let stack = UIStackView(arrangedSubviews: [header, scroller]); stack.axis = .vertical
        addSubview(stack)
        for view in [stack, table] { view.translatesAutoresizingMaskIntoConstraints = false }
        let count = isTable ? cells.count : lines.count
        let height = min(320, max(isTable ? 48 : 22, CGFloat(count) * table.rowHeight))
        table.isScrollEnabled = CGFloat(count) * table.rowHeight > height
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: topAnchor), stack.bottomAnchor.constraint(equalTo: bottomAnchor), stack.leadingAnchor.constraint(equalTo: leadingAnchor), stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroller.heightAnchor.constraint(equalToConstant: height),
            table.topAnchor.constraint(equalTo: scroller.contentLayoutGuide.topAnchor), table.bottomAnchor.constraint(equalTo: scroller.contentLayoutGuide.bottomAnchor), table.leadingAnchor.constraint(equalTo: scroller.contentLayoutGuide.leadingAnchor), table.trailingAnchor.constraint(equalTo: scroller.contentLayoutGuide.trailingAnchor),
            table.widthAnchor.constraint(equalToConstant: contentWidth), table.heightAnchor.constraint(equalTo: scroller.frameLayoutGuide.heightAnchor)
        ])
        layer.cornerRadius = 8; layer.borderWidth = 1; layer.borderColor = UIColor.separator.cgColor; clipsToBounds = true
    }
    required init?(coder: NSCoder) { fatalError("Use init(document:range:kind:availableWidth:)") }
    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { isTable ? cells.count : lines.count }
    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "line") ?? UITableViewCell(style: .default, reuseIdentifier: "line")
        cell.selectionStyle = .none; cell.backgroundColor = .clear
        cell.contentView.subviews.forEach { $0.removeFromSuperview() }
        let stack = UIStackView(); stack.axis = .horizontal; stack.alignment = .center
        if isTable {
            for span in cells[indexPath.row] {
                let label = UILabel(); label.attributedText = attributed(document, range: span.range, traits: traitCollection)
                label.textAlignment = span.alignment == "right" ? .right : span.alignment == "center" ? .center : .left
                label.numberOfLines = 2; label.widthAnchor.constraint(equalToConstant: contentWidth / CGFloat(max(1, cells[indexPath.row].count)) - 16).isActive = true
                stack.addArrangedSubview(label)
            }
            stack.spacing = 16
        } else {
            let number = UILabel(); number.text = String(indexPath.row + 1); number.textColor = .tertiaryLabel; number.textAlignment = .right
            number.font = .monospacedDigitSystemFont(ofSize: 11, weight: .regular); number.widthAnchor.constraint(equalToConstant: 40).isActive = true
            let label = UILabel(); label.attributedText = attributed(document, range: lines[indexPath.row], traits: traitCollection, candidates: lineSpans?[indexPath.row])
            stack.addArrangedSubview(number); stack.addArrangedSubview(label); stack.spacing = 12
        }
        cell.contentView.addSubview(stack); stack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo: cell.contentView.leadingAnchor, constant: 8), stack.trailingAnchor.constraint(equalTo: cell.contentView.trailingAnchor, constant: -8), stack.topAnchor.constraint(equalTo: cell.contentView.topAnchor), stack.bottomAnchor.constraint(equalTo: cell.contentView.bottomAnchor)])
        return cell
    }
}
/// Bucket intersecting spans once, preserving their original application order.
/// Code rows then style only their own runs instead of scanning the whole file.
func spansByLine(_ spans: [MarkitSpan], lines: [NSRange]) -> [[MarkitSpan]] {
    var result = Array(repeating: [MarkitSpan](), count: lines.count)
    for span in spans {
        var low = 0, high = lines.count
        while low < high {
            let mid = (low + high) / 2
            if NSMaxRange(lines[mid]) <= span.start { low = mid + 1 } else { high = mid }
        }
        while low < lines.count && lines[low].location < span.end {
            if NSIntersectionRange(lines[low], span.range).length > 0 { result[low].append(span) }
            low += 1
        }
    }
    return result
}
#endif
