import Foundation

struct Fixture: Decodable { let name: String; let source: String }
struct Measurement: Encodable {
    let name: String
    let fixture: String
    let bytes: Int
    let median_ms: Double
    let p95_batch_ms: Double
    let samples_ms: [Double]
    let iterations: Int
}
@main struct NativeComparison {
    static var sink = 0
    static func percentile(_ values: [Double], _ p: Double) -> Double { let sorted=values.sorted(); return sorted[min(sorted.count-1,Int(ceil(Double(sorted.count)*p))-1)] }
    static func measure(_ name: String, fixture: Fixture, operation: () throws -> Int) rethrows -> Measurement {
        for _ in 0..<20 { sink ^= try autoreleasepool { try operation() } }
        var samples: [Double] = []; var total=0
        for _ in 0..<9 {
            let start=DispatchTime.now().uptimeNanoseconds; var count=0
            repeat { sink ^= try autoreleasepool { try operation() }; count += 1 } while DispatchTime.now().uptimeNanoseconds-start < 70_000_000
            samples.append(Double(DispatchTime.now().uptimeNanoseconds-start)/1e6/Double(count)); total += count
        }
        return Measurement(name:name,fixture:fixture.name,bytes:fixture.source.utf8.count,median_ms:percentile(samples,0.5),p95_batch_ms:percentile(samples,0.95),samples_ms:samples,iterations:total)
    }
    static func acePreparation(_ source: String) -> Int {
        var count=0
        for block in Markdown.blocks(source) {
            switch block {
            case .paragraph(let text), .quote(let text), .heading(_,let text):
                let styled=Markdown.inline(text); count += styled.characters.count + styled.runs.count
            case .list(_,_,let items):
                for text in items { let styled=Markdown.inline(text); count += styled.characters.count + styled.runs.count }
            case .table(let headers,let rows):
                for text in headers + rows.flatMap({$0}) { let styled=Markdown.inline(text); count += styled.characters.count + styled.runs.count }
            case .code(_,let text): count += text.utf8.count
            case .rule: count += 1
            }
        }
        return count
    }
    static func main() throws {
        let fixtures=try JSONDecoder().decode([Fixture].self,from:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[1])))
        let full=AttributedString.MarkdownParsingOptions(interpretedSyntax:.full,failurePolicy:.returnPartiallyParsedIfPossible)
        var results:[Measurement]=[]
        for fixture in fixtures {
            results.append(try measure("Markit Rust + C bridge + Swift JSON",fixture:fixture) {
                let doc=try Markit.parse(fixture.source); return doc.text.utf16.count + doc.spans.count + doc.blocks.count
            })
            results.append(measure("Ace Swift blocks + Foundation inline",fixture:fixture) { acePreparation(fixture.source) })
            results.append(try measure("Foundation full Markdown",fixture:fixture) {
                let text=try AttributedString(markdown:fixture.source,options:full); return text.characters.count + text.runs.count
            })
        }
        let encoder=JSONEncoder(); encoder.outputFormatting=[.prettyPrinted,.sortedKeys]
        let data=try encoder.encode(results)
        try data.write(to:URL(fileURLWithPath:CommandLine.arguments[2]))
        print(String(decoding:data,as:UTF8.self))
        if sink == Int.min { print(sink) }
    }
}
