// Run with the source C module and libmarkit_ffi, see docs/binary-transport.md.
import Foundation

@main struct BinaryReaderCheck {
    static func main() throws {
        let source = "# Hello 🦀\n\nInline $x^2$.\n\n```rust\nlet value = 42;\n```\n\n| a | b |\n| --- | --- |\n| x | y |\n"
        let json = try Markit.parse(source)
        for _ in 0..<50 {
            let plan = try MarkitBinaryPlan.parse(source)
            precondition(plan.documentVersion == json.version)
            precondition(plan.text == json.text && plan.spanCount == json.spans.count && plan.blockCount == json.blocks.count)
            var foundPNG = false, foundPalette = false
            for i in 0..<plan.spanCount {
                let span = try plan.span(at: i), expected = json.spans[i]
                precondition(span.start == expected.start && span.end == expected.end)
                if span.tag == 19 {
                    let png = try plan.blob(span.arguments[3])
                    precondition(png == Data(base64Encoded: expected.png!))
                    foundPNG = true
                }
                if span.tag == 17 { precondition(span.arguments[0] < 8); foundPalette = true }
            }
            precondition(foundPNG && foundPalette)
            for i in 0..<plan.blockCount {
                let block = try plan.block(at: i)
                let kind = try plan.string(block.arguments[0])
                precondition(kind == json.blocks[i].kind && block.start == json.blocks[i].start && block.end == json.blocks[i].end)
            }
            do { _ = try plan.span(at: -1); preconditionFailure("accepted negative index") } catch MarkitBinaryError.malformed {}
            do { _ = try plan.blob(UInt32.max); preconditionFailure("accepted invalid blob") } catch MarkitBinaryError.malformed {}
        }
        let code = try MarkitBinaryPlan.highlight("let x = \"🦀\";\r\n", language: "rust")
        precondition(code.text == "let x = \"🦀\";\r\n")
        let empty = try MarkitBinaryPlan.parse("")
        precondition(empty.spanCount == 0)
        print("Swift direct reader: passed 50 owned-buffer round trips, raw PNG parity, UTF-16 ranges, palette, blocks, bounds and code request")
    }
}
