import CMarkit
import Foundation

/// Reads MKNP v2 stream patches (docs/binary-transport.md) into the public
/// `MarkitDocument` model without JSON. An open code fence arrives as an
/// extension of the block the stream already holds, so a long live fence
/// costs Rust time and bridge bytes proportional to each append.
enum NativePatchReader {
    private static let palette: [(String, String)] = [
        ("#24292f", "#c9d1d9"), ("#6e7781", "#8b949e"), ("#0a3069", "#a5d6ff"), ("#0550ae", "#79c0ff"),
        ("#cf222e", "#ff7b72"), ("#8250df", "#d2a8ff"), ("#953800", "#ffa657"), ("#116329", "#7ee787"),
    ]

    private struct Reader {
        let bytes: UnsafeRawBufferPointer
        func word(_ at: Int) throws -> Int {
            guard at >= 0, at <= bytes.count - 4 else { throw MarkitBinaryError.malformed }
            return Int(UInt32(bytes[at]) | UInt32(bytes[at + 1]) << 8 | UInt32(bytes[at + 2]) << 16 | UInt32(bytes[at + 3]) << 24)
        }
    }

    /// Applies one patch to `documents` (the stream's current blocks) and
    /// returns the public patch with the replaced blocks.
    static func apply(_ buffer: MarkitBuffer, to documents: inout [MarkitDocument]) throws -> MarkitPatch {
        defer { markit_buffer_free(buffer) }
        guard buffer.status == 0, let pointer = buffer.data else { throw MarkitError.renderFailed(status: buffer.status) }
        let r = Reader(bytes: UnsafeRawBufferPointer(start: pointer, count: buffer.len))
        guard buffer.len >= 28, Array(r.bytes[0..<4]) == [77, 75, 78, 80] else { throw MarkitBinaryError.malformed }
        guard try r.word(4) == 2 else { throw MarkitBinaryError.unsupportedVersion }
        let replaceFrom = try r.word(8), finalized = try r.word(12) != 0
        let sourceBytes = try r.word(16), parsedBytes = try r.word(20), count = try r.word(24)
        guard replaceFrom <= documents.count else { throw MarkitBinaryError.malformed }
        var at = 28
        var blocks: [MarkitDocument] = []
        blocks.reserveCapacity(count)
        for index in 0..<count {
            let kind = try r.word(at); at += 4
            var header = [Int]()
            if kind == 1 {
                header = try (0..<4).map { try r.word(at + $0 * 4) }; at += 16
            } else if kind != 0 { throw MarkitBinaryError.malformed }
            let length = try r.word(at); at += 4
            guard length <= buffer.len - at else { throw MarkitBinaryError.malformed }
            let part = UnsafeRawBufferPointer(rebasing: r.bytes[at..<(at + length)])
            at += length
            let fresh = try document(part)
            if kind == 0 { blocks.append(fresh); continue }
            let target = replaceFrom + index
            guard target < documents.count else { throw MarkitBinaryError.malformed }
            let old = documents[target]
            let (keepText, head, keepFrom, keepTo) = (header[0], header[1], header[2], header[3])
            let oldUnits = old.text.utf16
            guard keepText <= oldUnits.count, head <= fresh.spans.count,
                  keepFrom <= keepTo, keepTo <= old.spans.count,
                  let kept = String(oldUnits.prefix(keepText)) else { throw MarkitBinaryError.malformed }
            var spans = Array(fresh.spans[..<head])
            spans.reserveCapacity(fresh.spans.count + keepTo - keepFrom)
            spans.append(contentsOf: old.spans[keepFrom..<keepTo])
            spans.append(contentsOf: fresh.spans[head...])
            blocks.append(MarkitDocument(version: fresh.version, text: kept + fresh.text, spans: spans, blocks: fresh.blocks))
        }
        guard at == buffer.len else { throw MarkitBinaryError.malformed }
        documents.removeSubrange(replaceFrom..<documents.count)
        documents.append(contentsOf: blocks)
        return MarkitPatch(replace_from: replaceFrom, blocks: blocks.map { MarkitPatch.Block(document: $0) },
                           finalized: finalized, source_bytes: sourceBytes, parsed_bytes: parsedBytes)
    }

    /// One MKBP v1 document. Same field values as the JSON export, with math
    /// PNG bytes re-encoded as base64 to keep `MarkitSpan.png` unchanged.
    static func document(_ bytes: UnsafeRawBufferPointer) throws -> MarkitDocument {
        let r = Reader(bytes: bytes)
        guard bytes.count >= 32, Array(bytes[0..<4]) == [77, 75, 66, 80] else { throw MarkitBinaryError.malformed }
        guard try r.word(4) == 1 else { throw MarkitBinaryError.unsupportedVersion }
        let version = try r.word(8), textLength = try r.word(12), spanCount = try r.word(16)
        let blockCount = try r.word(20), entries = try r.word(24), blobLength = try r.word(28)
        let spansAt = 32 + textLength, blocksAt = spansAt + spanCount * 40
        let tableAt = blocksAt + blockCount * 12, blobsAt = tableAt + entries * 8
        guard textLength <= bytes.count, spanCount <= bytes.count / 40, blockCount <= bytes.count / 12,
              entries <= bytes.count / 8, blobsAt <= bytes.count, blobLength == bytes.count - blobsAt else {
            throw MarkitBinaryError.malformed
        }
        func blob(_ id: Int) throws -> UnsafeRawBufferPointer {
            guard id < entries else { throw MarkitBinaryError.malformed }
            let offset = try r.word(tableAt + id * 8), length = try r.word(tableAt + id * 8 + 4)
            guard offset <= blobLength, length <= blobLength - offset else { throw MarkitBinaryError.malformed }
            return UnsafeRawBufferPointer(rebasing: bytes[(blobsAt + offset)..<(blobsAt + offset + length)])
        }
        func string(_ id: Int) throws -> String { String(decoding: try blob(id), as: UTF8.self) }
        let text = String(decoding: UnsafeRawBufferPointer(rebasing: bytes[32..<spansAt]), as: UTF8.self)
        var spans: [MarkitSpan] = []
        spans.reserveCapacity(spanCount)
        for i in 0..<spanCount {
            let at = spansAt + i * 40
            let start = try r.word(at), end = try r.word(at + 4), tag = try r.word(at + 8)
            let a = try (0..<7).map { try r.word(at + 12 + $0 * 4) }
            var kind = "", level: Int?, depth: Int?, language: String?, url: String?, file: String?, diff: Bool?
            var light: String?, dark: String?, change: String?, tex: String?, raw: String?, display: Bool?
            var png: String?, width: Int?, height: Int?, row: Int?, column: Int?, header: Bool?
            var alignment: String?, target: String?, referenceKind: String?
            switch tag {
            case 1: kind = "heading"; level = a[0]
            case 2: kind = "paragraph"
            case 3: kind = "strong"
            case 4: kind = "emphasis"
            case 5: kind = "strikethrough"
            case 6: kind = "code"
            case 7: kind = "code_block"; language = try string(a[0])
            case 8: kind = "link"; url = try string(a[0])
            case 9: kind = "image"; url = try string(a[0])
            case 10: kind = "quote"; depth = a[0]
            case 11: kind = "list_item"; depth = a[0]
            case 12: kind = "table"
            case 13: kind = "table_header"
            case 14: kind = "rule"
            case 15: kind = "reference"; referenceKind = try string(a[0]); target = try string(a[1])
            case 16:
                kind = "code_metadata"; diff = a[1] != 0
                if a[0] != Int(UInt32.max) { file = try string(a[0]) }
            case 17:
                guard a[0] < palette.count else { throw MarkitBinaryError.malformed }
                kind = "syntax"; light = palette[a[0]].0; dark = palette[a[0]].1
            case 18: kind = "diff"; change = try string(a[0])
            case 19:
                kind = "math"; tex = try string(a[0]); raw = try string(a[1]); display = a[2] != 0
                png = Data(try blob(a[3])).base64EncodedString(); width = a[4]; height = a[5]
            case 20:
                kind = "table_cell"; row = a[0]; column = a[1]; header = a[2] != 0; alignment = try string(a[3])
            case 21: kind = "syntax"; light = hex(a[0]); dark = hex(a[1])
            case 22: kind = "syntax"; light = try string(a[0]); dark = try string(a[1])
            default: throw MarkitBinaryError.malformed
            }
            spans.append(MarkitSpan(start: start, end: end, kind: kind, level: level, depth: depth, language: language,
                                    url: url, file: file, diff: diff, light: light, dark: dark, change: change, tex: tex,
                                    raw: raw, display: display, png: png, width: width, height: height, row: row,
                                    column: column, header: header, alignment: alignment, target: target,
                                    reference_kind: referenceKind))
        }
        var blocks: [MarkitBlock] = []
        blocks.reserveCapacity(blockCount)
        for i in 0..<blockCount {
            let at = blocksAt + i * 12
            blocks.append(MarkitBlock(start: try r.word(at), end: try r.word(at + 4), kind: try string(try r.word(at + 8))))
        }
        return MarkitDocument(version: version, text: text, spans: spans, blocks: blocks)
    }

    private static func hex(_ rgb: Int) -> String {
        let digits = Array("0123456789abcdef")
        return "#" + String((0..<6).map { digits[(rgb >> (20 - $0 * 4)) & 15] })
    }
}
