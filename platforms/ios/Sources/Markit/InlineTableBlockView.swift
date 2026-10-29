#if canImport(UIKit)
import UIKit

/// A table as a grid of labels that scrolls sideways when wider than the row.
/// Columns size to their content up to the theme's maximum, then wrap.
final class InlineTableBlockView: InlineBlockView {
    private struct Cell { let range: NSRange; let row: Int; let column: Int; let header: Bool; let alignment: NSTextAlignment }
    private let scroller = UIScrollView()
    private let grid = UIView()
    private var labels: [UILabel] = []
    private var cells: [Cell] = []
    private var rules: [UIView] = []
    private var headerBackgrounds: [UIView] = []
    private var columnWidths: [CGFloat] = []
    private var rowHeights: [CGFloat] = []
    private var gridSize = CGSize.zero

    override init(host: any InlineBlockHost, document: MarkitDocument, plan: InlineBlockPlan) {
        super.init(host: host, document: document, plan: plan)
        scroller.showsVerticalScrollIndicator = false
        scroller.alwaysBounceVertical = false
        scroller.scrollsToTop = false
        grid.layer.borderWidth = 1
        grid.clipsToBounds = true
        scroller.addSubview(grid)
        addSubview(scroller)
        accessibilityIdentifier = "markit-table"
        contentDidChange()
    }
    required init?(coder: NSCoder) { fatalError("Created by MarkitInlineView") }

    override func contentDidChange() {
        let range = plan.range
        cells = document.spans.compactMap { span in
            guard span.kind == "table_cell", span.start >= range.location, span.end <= NSMaxRange(range), span.end >= span.start else { return nil }
            return Cell(range: span.range, row: span.row ?? 0, column: max(0, span.column ?? 0), header: span.header ?? false,
                        alignment: span.alignment == "right" ? .right : span.alignment == "center" ? .center : .natural)
        }
        if cells.isEmpty {
            // Rows not yet parsed (or a table this build cannot read): show the text.
            cells = [Cell(range: range, row: 0, column: 0, header: false, alignment: .natural)]
        }
        let styler = self.styler, theme = self.theme
        let rowIndex = Dictionary(uniqueKeysWithValues: Set(cells.map(\.row)).sorted().enumerated().map { ($0.element, $0.offset) })
        let columnCount = (cells.map(\.column).max() ?? 0) + 1
        let pad = theme.tableCellInsets
        let texts = cells.map { aligned(styler.attributed(document, range: $0.range), $0.alignment) }

        var widths = [CGFloat](repeating: 0, count: columnCount)
        for (cell, text) in zip(cells, texts) {
            let natural = text.boundingRect(with: CGSize(width: theme.tableMaxColumnWidth, height: .greatestFiniteMagnitude),
                                            options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil)
            widths[cell.column] = max(widths[cell.column], min(theme.tableMaxColumnWidth, ceil(natural.width)))
        }
        columnWidths = widths.map { max(24, $0) + pad.left + pad.right }
        var heights = [CGFloat](repeating: 0, count: rowIndex.count)
        for (cell, text) in zip(cells, texts) {
            let inner = columnWidths[cell.column] - pad.left - pad.right
            let size = text.boundingRect(with: CGSize(width: inner, height: .greatestFiniteMagnitude),
                                         options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil)
            let row = rowIndex[cell.row] ?? 0
            heights[row] = max(heights[row], ceil(size.height) + pad.top + pad.bottom)
        }
        rowHeights = heights
        gridSize = CGSize(width: columnWidths.reduce(0, +), height: rowHeights.reduce(0, +))

        while labels.count < cells.count {
            let label = UILabel()
            label.numberOfLines = 0
            grid.addSubview(label)
            labels.append(label)
        }
        for (index, label) in labels.enumerated() {
            label.isHidden = index >= cells.count
            guard index < cells.count else { continue }
            label.attributedText = texts[index]
            label.accessibilityTraits = cells[index].header ? .header : .staticText
        }
        cellRows = cells.map { rowIndex[$0.row] ?? 0 }
        headerRows = Set(cells.filter(\.header).map { rowIndex[$0.row] ?? 0 })
        grid.backgroundColor = theme.codeBlockBackground
        grid.layer.cornerRadius = theme.codeCornerRadius
        grid.layer.borderColor = theme.tableBorder.resolvedColor(with: traitCollection).cgColor
        super.contentDidChange()
    }

    private var cellRows: [Int] = []
    private var headerRows: Set<Int> = []

    override func measure(width: CGFloat) -> CGFloat { gridSize.height }

    override func layoutSubviews() {
        super.layoutSubviews()
        scroller.frame = bounds
        scroller.contentSize = CGSize(width: max(bounds.width, gridSize.width), height: gridSize.height)
        scroller.alwaysBounceHorizontal = gridSize.width > bounds.width
        grid.frame = CGRect(origin: .zero, size: gridSize)
        let pad = theme.tableCellInsets
        var rowTops: [CGFloat] = [], y: CGFloat = 0
        for height in rowHeights { rowTops.append(y); y += height }
        var columnLefts: [CGFloat] = [], x: CGFloat = 0
        for width in columnWidths { columnLefts.append(x); x += width }
        for (index, cell) in cells.enumerated() where index < labels.count {
            let row = cellRows[index]
            labels[index].frame = CGRect(x: columnLefts[cell.column] + pad.left, y: rowTops[row] + pad.top,
                                         width: columnWidths[cell.column] - pad.left - pad.right,
                                         height: rowHeights[row] - pad.top - pad.bottom)
        }
        layoutDecorations(rowTops: rowTops)
    }

    private func layoutDecorations(rowTops: [CGFloat]) {
        let theme = self.theme
        let headers = headerRows.sorted()
        while headerBackgrounds.count < headers.count {
            let view = UIView()
            grid.insertSubview(view, at: 0)
            headerBackgrounds.append(view)
        }
        for (index, view) in headerBackgrounds.enumerated() {
            view.isHidden = index >= headers.count
            guard index < headers.count else { continue }
            let row = headers[index]
            view.backgroundColor = theme.tableHeaderBackground
            view.frame = CGRect(x: 0, y: rowTops[row], width: gridSize.width, height: rowHeights[row])
        }
        let lines = max(0, rowHeights.count - 1)
        while rules.count < lines {
            let view = UIView()
            grid.addSubview(view)
            rules.append(view)
        }
        for (index, view) in rules.enumerated() {
            view.isHidden = index >= lines
            guard index < lines else { continue }
            view.backgroundColor = theme.tableBorder
            view.frame = CGRect(x: 0, y: rowTops[index + 1], width: gridSize.width, height: 1)
        }
    }

    private func aligned(_ text: NSAttributedString, _ alignment: NSTextAlignment) -> NSAttributedString {
        guard alignment != .natural else { return text }
        let output = NSMutableAttributedString(attributedString: text)
        output.enumerateAttribute(.paragraphStyle, in: NSRange(location: 0, length: output.length), options: []) { value, range, _ in
            let style = ((value as? NSParagraphStyle)?.mutableCopy() as? NSMutableParagraphStyle) ?? NSMutableParagraphStyle()
            style.alignment = alignment
            output.addAttribute(.paragraphStyle, value: style, range: range)
        }
        return output
    }
}
#endif
