import Foundation

/// One view in an inline (non-scrolling) rendering.
struct InlineBlockPlan: Equatable {
    enum Kind: Equatable {
        case text, code, table, rule
        case image(url: String)
    }
    /// Index of the stream document (top-level root) this view shows.
    let document: Int
    /// UTF-16 range in that document's text. For images, the alt-text span.
    let range: NSRange
    let kind: Kind
    /// List/quote nesting depth around a code block or table.
    let depth: Int
    /// A later piece of the same text root: spaced like a paragraph, not a block.
    let continuation: Bool
}

/// Maps stream documents to inline views. Pure, so streaming reuse can be
/// planned and tested without UIKit. Work is proportional to the documents
/// planned, which during streaming are only the replaced trailing roots.
enum InlinePlanner {
    /// Text roots longer than this are split at line ends, so a streamed delta
    /// re-lays out only the last piece of a long list or multi-paragraph root.
    static let pieceLimit = 3000
    static let pieceMinimum = 1500

    static func plan(_ documents: [MarkitDocument], from start: Int) -> [InlineBlockPlan] {
        guard start < documents.count else { return [] }
        return (max(0, start)..<documents.count).flatMap { plan(documents[$0], index: $0) }
    }

    static func plan(_ document: MarkitDocument, index: Int) -> [InlineBlockPlan] {
        let text = document.text as NSString
        let whole = NSRange(location: 0, length: text.length)
        let images = document.spans.filter { $0.kind == "image" && $0.url != nil && $0.end >= $0.start }
        var result: [InlineBlockPlan] = []
        for block in document.blocks where block.end >= block.start {
            let clipped = NSIntersectionRange(block.range, whole)
            let range = trimmingNewlines(clipped, in: text)
            switch block.kind {
            case "code":
                result.append(InlineBlockPlan(document: index, range: range, kind: .code, depth: depth(of: range, in: document), continuation: false))
            case "table":
                guard range.length > 0 else { continue }
                result.append(InlineBlockPlan(document: index, range: range, kind: .table, depth: depth(of: range, in: document), continuation: false))
            case "rule":
                result.append(InlineBlockPlan(document: index, range: range, kind: .rule, depth: 0, continuation: false))
            default:
                // "text" and kinds this build does not know render as text.
                for (offset, piece) in pieces(of: range, in: text).enumerated() {
                    result.append(InlineBlockPlan(document: index, range: piece, kind: .text, depth: 0, continuation: offset > 0))
                    for image in images where image.start >= piece.location && image.start < NSMaxRange(piece) {
                        result.append(InlineBlockPlan(document: index, range: image.range, kind: .image(url: image.url ?? ""), depth: 0, continuation: true))
                    }
                }
            }
        }
        return result
    }

    /// Splits a long text range at line ends inside [minimum, limit] from each
    /// piece start, or at the next line end after that. Never splits a line.
    static func pieces(of range: NSRange, in text: NSString) -> [NSRange] {
        guard range.length > 0 else { return [] }
        var result: [NSRange] = []
        var start = range.location
        let end = NSMaxRange(range)
        while start < end {
            var stop = end
            if end - start > pieceLimit {
                let window = NSRange(location: start + pieceMinimum, length: pieceLimit - pieceMinimum)
                var newline = text.range(of: "\n", options: .backwards, range: window)
                if newline.location == NSNotFound {
                    newline = text.range(of: "\n", options: [], range: NSRange(location: start + pieceLimit, length: end - start - pieceLimit))
                }
                if newline.location != NSNotFound { stop = newline.location }
            }
            let piece = trimmingNewlines(NSRange(location: start, length: stop - start), in: text)
            if piece.length > 0 { result.append(piece) }
            start = stop == end ? end : stop + 1
        }
        return result
    }

    /// Drops trailing line ends, which would otherwise add an empty line.
    static func trimmingNewlines(_ range: NSRange, in text: NSString) -> NSRange {
        var length = range.length
        while length > 0, text.character(at: range.location + length - 1) == 10 { length -= 1 }
        return NSRange(location: range.location, length: length)
    }

    /// Deepest list item or quote fully containing `range`.
    static func depth(of range: NSRange, in document: MarkitDocument) -> Int {
        document.spans
            .filter { ($0.kind == "list_item" || $0.kind == "quote") && $0.start <= range.location && $0.end >= NSMaxRange(range) }
            .map { $0.depth ?? 1 }
            .max() ?? 0
    }
}
