import Foundation

/// Fast decoder for the JSON documents the Rust bridge emits.
///
/// `Markit.decode` routes `MarkitDocument` here instead of `JSONDecoder`.
/// The renderer emits dense span arrays, and `JSONDecoder` spends most of its
/// time in per-value boxing and `Decodable` plumbing. This decoder keeps the
/// parsed property lists as `NSDictionary`/`NSArray` (so per-span dictionaries
/// are never copied into Swift), looks keys up with pre-bridged `NSString`
/// constants, and reads only the fields each span `kind` can carry.
///
/// Ground truth stays `MarkitDocument`'s `Decodable` conformance:
/// `BridgeDecodeTests` decodes identical bridge bytes both ways and asserts
/// field-by-field equality over fixtures covering every span kind.
///
/// The pre-bridged key constants are immortal `CFString`s shared across
/// threads; `nonisolated(unsafe)` records that they are never mutated.
enum BridgeDecoder {
    private static nonisolated(unsafe) let kSpans: NSString = "spans"
    private static nonisolated(unsafe) let kBlocks: NSString = "blocks"
    private static nonisolated(unsafe) let kVersion: NSString = "version"
    private static nonisolated(unsafe) let kText: NSString = "text"
    private static nonisolated(unsafe) let kStart: NSString = "start"
    private static nonisolated(unsafe) let kEnd: NSString = "end"
    private static nonisolated(unsafe) let kKind: NSString = "kind"
    private static nonisolated(unsafe) let kLevel: NSString = "level"
    private static nonisolated(unsafe) let kDepth: NSString = "depth"
    private static nonisolated(unsafe) let kLanguage: NSString = "language"
    private static nonisolated(unsafe) let kUrl: NSString = "url"
    private static nonisolated(unsafe) let kFile: NSString = "file"
    private static nonisolated(unsafe) let kDiff: NSString = "diff"
    private static nonisolated(unsafe) let kLight: NSString = "light"
    private static nonisolated(unsafe) let kDark: NSString = "dark"
    private static nonisolated(unsafe) let kChange: NSString = "change"
    private static nonisolated(unsafe) let kTex: NSString = "tex"
    private static nonisolated(unsafe) let kRaw: NSString = "raw"
    private static nonisolated(unsafe) let kDisplay: NSString = "display"
    private static nonisolated(unsafe) let kPng: NSString = "png"
    private static nonisolated(unsafe) let kWidth: NSString = "width"
    private static nonisolated(unsafe) let kHeight: NSString = "height"
    private static nonisolated(unsafe) let kRow: NSString = "row"
    private static nonisolated(unsafe) let kColumn: NSString = "column"
    private static nonisolated(unsafe) let kHeader: NSString = "header"
    private static nonisolated(unsafe) let kAlignment: NSString = "alignment"
    private static nonisolated(unsafe) let kTarget: NSString = "target"
    private static nonisolated(unsafe) let kReferenceKind: NSString = "reference_kind"
    private static nonisolated(unsafe) let kAlert: NSString = "alert"
    private static nonisolated(unsafe) let kLabel: NSString = "label"
    private static nonisolated(unsafe) let kNumber: NSString = "number"
    private static nonisolated(unsafe) let kIndex: NSString = "index"
    private static nonisolated(unsafe) let kOffset: NSString = "offset"
    private static nonisolated(unsafe) let kChecked: NSString = "checked"
    private static nonisolated(unsafe) let kSource: NSString = "source"
    private static nonisolated(unsafe) let kTag: NSString = "tag"

    /// JSON booleans arrive as `CFBoolean`; JSON numbers as `NSNumber`.
    /// A plain `as? Bool` cast conflates the two (`NSNumber(0)` reads as
    /// `false`), so booleans are accepted only from `CFBoolean` values and
    /// integers only from non-boolean numbers.
    @inline(__always) private static func isBoolean(_ value: AnyObject) -> Bool {
        CFGetTypeID(value) == CFBooleanGetTypeID()
    }
    @inline(__always) private static func bool(
        _ dict: NSDictionary, _ key: NSString
    ) -> Bool? {
        guard let value = dict.object(forKey: key) as? AnyObject,
            isBoolean(value)
        else { return nil }
        return CFBooleanGetValue((value as! CFBoolean))
    }
    @inline(__always) private static func int(
        _ dict: NSDictionary, _ key: NSString
    ) -> Int? {
        guard let value = dict.object(forKey: key) as? AnyObject,
            !isBoolean(value), let number = value as? NSNumber
        else { return nil }
        return number.intValue
    }
    @inline(__always) private static func string(
        _ dict: NSDictionary, _ key: NSString
    ) -> String? {
        dict.object(forKey: key) as? String
    }

    private static func corrupted(_ key: String) -> Error {
        DecodingError.dataCorrupted(
            DecodingError.Context(
                codingPath: [], debugDescription: "bridge document missing \(key)"))
    }

    static func document(from data: Data) throws -> MarkitDocument {
        guard let top = try JSONSerialization.jsonObject(with: data, options: [])
            as? NSDictionary
        else { throw corrupted("document") }
        return try document(from: top)
    }

    static func document(from top: NSDictionary) throws -> MarkitDocument {
        guard let version = (top.object(forKey: kVersion) as? NSNumber)
            .map({ $0.intValue })
        else { throw corrupted("version") }
        guard let text = top.object(forKey: kText) as? String else {
            throw corrupted("text")
        }
        guard let rawSpans = top.object(forKey: kSpans) as? NSArray else {
            throw corrupted("spans")
        }
        var spans = [MarkitSpan]()
        spans.reserveCapacity(rawSpans.count)
        for case let dict as NSDictionary in rawSpans {
            spans.append(try span(from: dict))
        }
        var blocks = [MarkitBlock]()
        if let rawBlocks = top.object(forKey: kBlocks) as? NSArray {
            blocks.reserveCapacity(rawBlocks.count)
            for case let dict as NSDictionary in rawBlocks {
                guard let start = int(dict, kStart), let end = int(dict, kEnd),
                    let kind = string(dict, kKind)
                else { throw corrupted("block") }
                blocks.append(MarkitBlock(start: start, end: end, kind: kind))
            }
        }
        return MarkitDocument(
            version: version, text: text, spans: spans, blocks: blocks)
    }

    private static func span(from dict: NSDictionary) throws -> MarkitSpan {
        guard let start = int(dict, kStart), let end = int(dict, kEnd),
            let kind = string(dict, kKind)
        else { throw corrupted("span start/end/kind") }
        // Only the fields the renderer's `Style` variant for `kind` can
        // emit are read; anything else stays nil, matching the decoder.
        var level: Int?, depth: Int?
        var language, url, file, light, dark, change, tex, raw, png: String?
        var alignment, target, reference_kind, alert, label: String?
        var source, tag: String?
        var width, height, row, column, number, index, offset: Int?
        var diff, display, header, checked: Bool?
        switch kind {
        case "heading":
            level = int(dict, kLevel)
        case "code_block":
            language = string(dict, kLanguage)
        case "link", "image", "video", "dark_image":
            url = string(dict, kUrl)
        case "quote", "list_item":
            depth = int(dict, kDepth)
        case "reference":
            reference_kind = string(dict, kReferenceKind)
            target = string(dict, kTarget)
        case "code_metadata":
            file = string(dict, kFile)
            diff = bool(dict, kDiff)
        case "syntax":
            light = string(dict, kLight)
            dark = string(dict, kDark)
        case "diff":
            change = string(dict, kChange)
        case "math":
            tex = string(dict, kTex)
            raw = string(dict, kRaw)
            display = bool(dict, kDisplay)
            png = string(dict, kPng)
            width = int(dict, kWidth)
            height = int(dict, kHeight)
        case "table_cell":
            row = int(dict, kRow)
            column = int(dict, kColumn)
            header = bool(dict, kHeader)
            alignment = string(dict, kAlignment)
        case "alert":
            alert = string(dict, kAlert)
        case "footnote_reference", "footnote_definition":
            label = string(dict, kLabel)
            number = int(dict, kNumber)
        case "task":
            index = int(dict, kIndex)
            offset = int(dict, kOffset)
            checked = bool(dict, kChecked)
        case "diagram":
            language = string(dict, kLanguage)
            source = string(dict, kSource)
        case "html":
            tag = string(dict, kTag)
        case "paragraph", "strong", "emphasis", "strikethrough", "code",
            "table", "table_header", "rule":
            break
        default:
            // An unknown kind keeps decoding correct by reading everything.
            level = int(dict, kLevel); depth = int(dict, kDepth)
            language = string(dict, kLanguage); url = string(dict, kUrl)
            file = string(dict, kFile); diff = bool(dict, kDiff)
            light = string(dict, kLight); dark = string(dict, kDark)
            change = string(dict, kChange); tex = string(dict, kTex)
            raw = string(dict, kRaw); display = bool(dict, kDisplay)
            png = string(dict, kPng); width = int(dict, kWidth)
            height = int(dict, kHeight); row = int(dict, kRow)
            column = int(dict, kColumn); header = bool(dict, kHeader)
            alignment = string(dict, kAlignment)
            target = string(dict, kTarget)
            reference_kind = string(dict, kReferenceKind)
            alert = string(dict, kAlert); label = string(dict, kLabel)
            number = int(dict, kNumber); index = int(dict, kIndex)
            offset = int(dict, kOffset); checked = bool(dict, kChecked)
            source = string(dict, kSource); tag = string(dict, kTag)
        }
        return MarkitSpan(
            start: start, end: end, kind: kind, level: level, depth: depth,
            language: language, url: url, file: file, diff: diff,
            light: light, dark: dark, change: change, tex: tex, raw: raw,
            display: display, png: png, width: width, height: height,
            row: row, column: column, header: header, alignment: alignment,
            target: target, reference_kind: reference_kind, alert: alert,
            label: label, number: number, index: index, offset: offset,
            checked: checked, source: source, tag: tag)
    }
}
