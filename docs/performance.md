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

Settled streaming roots avoid repeated parsing during ordinary appends. An open fence rewrites only its open line in HTML and extends stable native spans. Swift and Kotlin streams now receive MKNP v2 patches that carry only the appended fence text and spans, so the Rust side of a long live fence stays proportional to each append; the JSON stream export still walks the full span list, and hosts still copy their kept text and spans. Math PNGs are native-only; Wasm renders SVG. An open giant paragraph or list, and a late reference definition, still reparse the tail. See [the optimization report](performance-optimization.md) for the Linux before/after table. Late reference definitions can invalidate earlier blocks. Native code and table cells recycle within bounded viewports, while the web wrapper removes offscreen lines from large code-fence DOMs. Native frame-time validation is still pending device access.

Caches have separately accounted 4 MiB budgets for fast highlighting, optional grammar highlighting, and math, with at most 128 entries each per rendering thread. Allocator overhead and results retained by callers are outside those accounting budgets. Oversized/unknown code remains exact plain source; unsupported math remains TeX.

## Web download and start-up

The web Wasm builds with `--profile web` (`opt-level = "z"`, fat LTO, `strip`, `panic = "abort"`); `wasm-opt -Oz` made gzip larger, so it is not run. Against `opt-level = "s"`/`"3"`, a 277-token stream in Node differed by under 2 ms. The module embeds a core cut of STIX Two Math (`crates/markit/assets/math-core.otf`, 154 KB gzip, built by `scripts/math-font-web.py` from the glyphs `examples/math_font_subset.rs` lays out). It keeps every glyph's advance, bounding box, MATH data and `ssty` alternates, so every layout is identical; only uncommon outlines are missing. The other outlines ship as 17 chunks grouped by alphabet or symbol family (`pkg/math-glyphs-N.bin`, 4.5–55 KB gzip each, 316 KB in all). Before drawing a glyph from a chunk that is not loaded, the Wasm reports the chunk; the worker fetches it once, merges it into the face (`math_glyphs::merge` rewrites only the CFF CharStrings INDEX, which the script places last), and re-renders the document. A `\mathfrak` formula used to fetch the full font (644 KB gzip); now it fetches 25.6 KB. `tests/math_core_font.rs` checks metrics, outlines, layouts and that merging the chunks restores every reachable outline, and `tests/golden.rs` pins both web math paths. latex-rust is vendored (`third_party/latex-rust`, see its `MARKIT.md`) to build ttf-parser without unused table parsers and to set up `ssty` lookups from coverage tables. In Chromium the page transfers 497.4 KB gzip (412.3 KB brotli), down from 1.21 MB.

Importing `index.js` starts the default shared worker after the current task (`globalThis.markitPreload = false` opts out), and the idle worker lays out one formula. A message mounted after that settles in about 12.5 ms at 1x. Streaming renders of an open message are paced at 32–100 ms. See [the browser comparison](benchmark-comparison.md#react-transcript-in-chromium-vs-ace-web).

## Follow-up validation

On 2026-10-28, rebuilding stale web artifacts restored all 31 Playwright tests on macOS. The saved Linux comparison for `perf/optimize-more` reports streaming main-thread medians of 181.1 ms at 1x and 489.0 ms at 4x, compared with 191.5 and 597.7 ms on `perf/beat-ace`. The runs were separate; these are observations rather than a controlled speedup estimate. Glyph chunking increases the core transfer by about 2.9 KB gzip and reduces the first Fraktur outline fetch from 644 KB to 25.6 KB. A text-node append experiment did not improve the profile and was removed. See [the comparison report](benchmark-comparison.md#follow-up-validation-on-macos-2026-10-28) for validation and fresh macOS results.

## iOS code viewport

Long highlighted code panes bucket spans by line once rather than scanning the entire document for each visible line. Small or unhighlighted panes retain the direct scan. On iOS 27 Simulator, the release 1,000-line fixture's median synchronous viewport layout fell from 18.13 to 12.75 ms. This is a host simulator result, not device FPS. Indexing adds span-reference storage; peak memory is unmeasured. Android release app measurements are also retained. See [native app viewport results](benchmark-comparison.md#native-app-viewport-benchmarks-2026-10-28).

## Reusable native code cells

Both native renderers now retain line-number and code-text views as cells are rebound. UIKit retains the stack and constraints too. Android indexes spans by line for long highlighted blocks and avoids table-cell work in code panes. In release simulator/emulator runs, eight highlighted-code scroll round trips fell from 113.23 to 38.80 ms on iOS and 22.86 to 14.81 ms on Android. These are synchronous binding/layout costs, not device frame times. See [the native reuse comparison](benchmark-comparison.md#native-code-cell-reuse-follow-up) for raw samples, tests and limits.

## Real transcript follow-up

A frozen sample of 5,549 unique saved Ace assistant messages changed the focus from giant code blocks to short messages and table/list-heavy answers. iOS first-document loading now uses a data reload rather than a batch of row insertions. The real list-heavy fixture's initial viewport fell from 39.45 to 14.44 ms. Android reuses table column views and indexes spans for large tables; eight scroll round trips through an extracted real table fell from 14.37 to 6.49 ms. Typical short messages were essentially unchanged. These are warm simulator/emulator UI timings, not physical-device or full Ace app measurements. See [the real transcript report](benchmark-comparison.md#real-ace-sqlite-transcript-workload), including the iOS first-load versus later-scroll tradeoff. No transcript text is committed.

## Block cell reuse and per-row spans

Native views split each document's spans by block once when a patch arrives, so binding a prose row, its list or quote inset, its images and its code or table pane reads only the spans that intersect that block instead of every span in the document. Prose rows now reuse their text view, stack and constraints, and rows that keep their position across a patch are reconfigured in place (`reconfigureRows` on UIKit, `notifyItemRangeChanged` on Android), so a streaming message updates its last block's views instead of deleting and recreating them. Code and table panes are built once per row and reattached when they scroll back into view; Android's panes share one pool of line and table-row views. iOS table cells now use per-cell span buckets like Android. Both platforms cache decoded math images and parsed syntax colors, and UIKit caches its Dynamic Type fonts. On the web, every glyph chunk a render needs is merged into the math font in one pass, so the font is copied once per render rather than once per chunk. These changes have not yet been measured on the native benchmark harness.
