import XCTest
@testable import Markit

/// M2 frame-rate harness: render+decode time distributions at three document
/// sizes through the real C bridge, plus (on UIKit platforms) the per-row
/// text-conversion and MarkitView cell cost that dominates scroll frames.
///
/// What runs where:
/// - Mac host (`swift test`): parse-only tests. UIKit sections compile out
///   because UIKit is unavailable, so a host run never touches MarkitView.
/// - iOS simulator/device (Xcode): full path, including a scroll-hitch proxy
///   that times every `cellForRowAt` through the real `MarkitView` data
///   source and counts rows over a 16.7 ms frame budget.
///
/// The tests assert correctness only; timing is printed, never asserted, so
/// this harness cannot fail CI on a slow machine. Read the `MARKIT_PERF`
/// lines in the test log.
///
/// Host run (from the repository root):
///   ./scripts/package-notices.sh
///   cargo build -p markit-ffi --release --locked
///   MARKIT_SOURCE_BRIDGE=1 swift test --package-path platforms/ios \
///     -Xlinker "$PWD/target/release/libmarkit_ffi.a" --filter FrameHarnessTests
///
/// Device run: build the XCFramework with ./scripts/build-ios.sh, add this
/// package to an iOS test host in Xcode, and run the MarkitTests target on a
/// device. No simulator was driven from this Mac (no devices available).
final class FrameHarnessTests: XCTestCase {
    static var sink = 0

    /// GFM section mirroring benchmarks/fixtures.mjs so mobile numbers are
    /// comparable with the existing Mac-host native comparison.
    static let section = """
# Renderer benchmark

A paragraph with **strong text**, *emphasis*, `inline code`, and a [documentation link](https://example.com/docs). Unicode stays intact: café 🦀.

- First item with **bold**
- Second item with *emphasis*

> A quoted paragraph with a [link](https://example.com).

| Platform | Output |
| :--- | ---: |
| iOS | Native text |
| Android | Native text |

```rust
fn greet(name: &str) -> String {
    format!("Hello {name} 🦀")
}
```

Math \\(x^2\\) inline.

---

"""

    /// Whole-section repeats reaching at least `target` UTF-8 bytes. Repeats
    /// (not truncation) keep every fence and delimiter balanced.
    static func fixture(targetBytes target: Int) -> String {
        let sectionBytes = section.utf8.count
        let count = max(1, (target + sectionBytes - 1) / sectionBytes)
        return String(repeating: section, count: count)
    }

    struct Stats {
        let medianMs: Double
        let p95Ms: Double
        let maxMs: Double
        let iterations: Int
    }

    static func percentile(_ values: [Double], _ p: Double) -> Double {
        let sorted = values.sorted()
        return sorted[min(sorted.count - 1, Int(ceil(Double(sorted.count) * p)) - 1)]
    }

    /// Times `operation` after `warmup` untimed runs. Returns stats; folds a
    /// checksum into `sink` so the optimizer cannot drop the parse.
    static func timeIt(warmup: Int, iterations: Int, _ operation: () throws -> Int) rethrows -> Stats {
        for _ in 0..<warmup { sink ^= try operation() }
        var samples: [Double] = []
        for _ in 0..<iterations {
            let start = DispatchTime.now().uptimeNanoseconds
            sink ^= try operation()
            samples.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000)
        }
        return Stats(medianMs: percentile(samples, 0.5), p95Ms: percentile(samples, 0.95), maxMs: samples.max() ?? 0, iterations: iterations)
    }

    func checkRenderDecode(fixture name: String, targetBytes: Int, warmup: Int, iterations: Int) throws {
        let source = Self.fixture(targetBytes: targetBytes)
        var checked = false
        let stats = try Self.timeIt(warmup: warmup, iterations: iterations) {
            let document = try Markit.parse(source)
            if !checked {
                checked = true
                XCTAssertEqual(document.version, 2)
                XCTAssertFalse(document.blocks.isEmpty)
                XCTAssertTrue(document.spans.contains { $0.kind == "strong" })
                XCTAssertTrue(document.spans.contains { $0.kind == "link" })
            }
            return document.text.utf16.count + document.spans.count + document.blocks.count
        }
        print(String(format: "MARKIT_PERF phase=render_decode fixture=%@ bytes=%d iterations=%d median_ms=%.3f p95_ms=%.3f max_ms=%.3f",
            name, source.utf8.count, stats.iterations, stats.medianMs, stats.p95Ms, stats.maxMs))
    }

    func testRenderDecodeSmall() throws {
        try checkRenderDecode(fixture: "small", targetBytes: 4_096, warmup: 5, iterations: 25)
    }

    func testRenderDecodeMedium() throws {
        try checkRenderDecode(fixture: "medium", targetBytes: 32_768, warmup: 3, iterations: 9)
    }

    func testRenderDecodeLarge() throws {
        try checkRenderDecode(fixture: "large", targetBytes: 131_072, warmup: 2, iterations: 5)
    }

#if canImport(UIKit)
    /// Full view path on UIKit platforms only: NSAttributedString conversion
    /// (what `cellForRowAt` builds per row) plus `MarkitView.setDocument` and
    /// a per-row `cellForRowAt` sweep as the scroll-frame proxy. The MainActor
    /// hop is required because `MarkitView` is main-actor isolated.
    @MainActor func testViewCellScrollCost() throws {
        let traits = UITraitCollection()
        for (name, target) in [("small", 4_096), ("medium", 32_768), ("large", 131_072)] {
            let source = Self.fixture(targetBytes: target)
            let document = try Markit.parse(source)
            let fullRange = NSRange(location: 0, length: (document.text as NSString).length)

            let conversion = Self.timeIt(warmup: 2, iterations: 5) {
                attributed(document, range: fullRange, traits: traits).length
            }
            print(String(format: "MARKIT_PERF phase=attributed_convert fixture=%@ bytes=%d iterations=%d median_ms=%.3f p95_ms=%.3f max_ms=%.3f",
                name, source.utf8.count, conversion.iterations, conversion.medianMs, conversion.p95Ms, conversion.maxMs))

            // The real MarkitView path: synchronous setDocument, then one
            // cell per row exactly as a scroll would request them.
            let view = MarkitView(frame: CGRect(x: 0, y: 0, width: 390, height: 844))
            let displayStart = DispatchTime.now().uptimeNanoseconds
            view.setDocument(document)
            let displayMs = Double(DispatchTime.now().uptimeNanoseconds - displayStart) / 1_000_000

            let table = UITableView(frame: view.bounds)
            let rows = view.tableView(table, numberOfRowsInSection: 0)
            XCTAssertGreaterThan(rows, 0)
            var samples: [Double] = []
            for row in 0..<rows {
                let start = DispatchTime.now().uptimeNanoseconds
                let cell = view.tableView(table, cellForRowAt: IndexPath(row: row, section: 0))
                Self.sink ^= cell.contentView.subviews.count
                samples.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1_000_000)
            }
            let hitches = samples.filter { $0 > 16.7 }.count
            print(String(format: "MARKIT_PERF phase=view_cells fixture=%@ bytes=%d rows=%d display_ms=%.3f cell_median_ms=%.3f cell_p95_ms=%.3f cell_max_ms=%.3f hitches_over_16ms=%d",
                name, source.utf8.count, rows, displayMs,
                Self.percentile(samples, 0.5), Self.percentile(samples, 0.95), samples.max() ?? 0, hitches))
        }
        print("MARKIT_PERF sink=\(Self.sink)")
    }
#endif
}
