import UIKit
import Markit

/// Opt-in app harness. Durations cover synchronous bridge and viewport work,
/// not GPU presentation or physical-device frame timing.
@MainActor enum MobileBenchmark {
    static func run() async {
        do {
            let url = Bundle.main.url(forResource: "fixtures", withExtension: "json", subdirectory: "mobile")!
            let fixtures = try JSONDecoder().decode([String: String].self, from: Data(contentsOf: url))
            let window = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.flatMap(\.windows).first { $0.isKeyWindow }!
            var cases: [String: Any] = [:]
            for name in fixtures.keys.sorted() {
                let source = fixtures[name]!
                var parse: [Double] = [], layout: [Double] = [], scroll: [Double] = []
                for _ in 0..<10 {
                    let start = CACurrentMediaTime()
                    let document = try Markit.parse(source)
                    parse.append((CACurrentMediaTime() - start) * 1000)
                    let view = MarkitView(frame: window.bounds.inset(by: window.safeAreaInsets))
                    window.addSubview(view)
                    let before = CACurrentMediaTime()
                    view.setDocument(document); view.layoutIfNeeded()
                    layout.append((CACurrentMediaTime() - before) * 1000)
                    let table = view.subviews.compactMap { $0 as? UITableView }.first!
                    let scrolled = CACurrentMediaTime()
                    table.setContentOffset(CGPoint(x: 0, y: max(0, table.contentSize.height - table.bounds.height)), animated: false)
                    table.layoutIfNeeded()
                    scroll.append((CACurrentMediaTime() - scrolled) * 1000)
                    view.removeFromSuperview()
                    await Task.yield()
                }
                cases[name] = ["sourceBytes": source.utf8.count, "bridgeMs": parse, "viewportLayoutMs": layout, "scrollToEndMs": scroll]
            }
            let result: [String: Any] = ["platform": "iOS Simulator", "os": UIDevice.current.systemVersion, "viewportWidth": window.bounds.width, "viewportHeight": window.bounds.height, "cases": cases]
            let data = try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("benchmark.json"))
            print("MARKIT_BENCH_DONE")
        } catch { print("MARKIT_BENCH_ERROR: \(error)") }
    }
}
