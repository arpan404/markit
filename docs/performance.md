# Engine measurements

The original measurements below are retained for reference. [The optimization report](performance-optimization.md) records the latest before/after results for highlighting and math.

Measured locally on MacBook Pro Rijan on 2026-10-27 with `cargo bench -p markit --bench throughput`, Rust 1.91.1, and the release profile in this repository. Each whole-document case warms up ten times, then runs for at least 300 ms. The fixture repeats prose, headings, links, lists, tasks, tables, and fenced Rust code. Highlight caches are warm; the fixture contains no math.

| Operation | 261 bytes | 13,050 bytes | 104,400 bytes |
| --- | ---: | ---: | ---: |
| Native parse and highlighting | 4.2 µs | 211.1 µs | 2,059.3 µs |
| Native parse and JSON | 7.4 µs | 301.2 µs | 2,797.1 µs |
| HTML, including code controls | 4.4 µs | 191.1 µs | 2,210.1 µs |
| HTML, reused output buffer | 4.9 µs | 196.9 µs | 1,701.6 µs |

An append after a 104,411-byte history averaged 13.5 µs across 100 runs. It reconsidered 68 source bytes. History setup was outside the timed region; the measurement includes Rust patch construction but excludes patch JSON serialization, language bridges, and UI work.

The default highlighter is a linear tokenizer. Full Sublime grammars are optional because their first-use regex compilation caused multi-second startup in the browser during development. The default demo's observed startup round trip was about 49-121 ms on localhost, including worker/Wasm startup and mathematical rendering. Those observations are not a controlled browser benchmark.

These are single-machine observations, not platform guarantees. They exclude UIKit/Android layout and drawing, native JSON decoding, network delivery, and end-to-end frame timing. New comparisons against Ace's Marked/Prism/KaTeX and Swift/Foundation paths are in [the comparison report](benchmark-comparison.md). They reveal slower highlighting paths as well as faster Markdown preparation. Buffer reuse varied between cases; its primary contract is allocation reuse.

Settled streaming roots avoid repeated parsing during ordinary appends. An open fence now rewrites only its open line in HTML and extends stable native spans; the full span list is still copied into each patch, and the JSON stream export still walks that list. An open giant paragraph or list, and a late reference definition, still reparse the tail. See [the optimization report](performance-optimization.md) for the Linux before/after table. Late reference definitions can invalidate earlier blocks. Native code and table cells recycle within bounded viewports, while the web wrapper removes offscreen lines from large code-fence DOMs. Native frame-time validation is still pending device access.

Caches have separately accounted 4 MiB budgets for fast highlighting, optional grammar highlighting, and math, with at most 128 entries each per rendering thread. Allocator overhead and results retained by callers are outside those accounting budgets. Oversized/unknown code remains exact plain source; unsupported math remains TeX.
