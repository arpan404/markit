# Renderer comparison benchmark

These are the original baseline measurements. See [the optimization report](performance-optimization.md) for the later speed improvements and remaining gaps.

Measured on Apple M3 Pro, macOS 27.2, Chromium 156.0.8078.4. Marked 18.0.14, Prism 1.30.0, KaTeX 0.19.0. The native model code is from Ace commit `b92d9d261848`; the Markit engine is from `0405eedf9202`.

Markit is faster on the measured Markdown-to-HTML and native host preparation workloads. Wasm lexical highlighting is slower than Ace's lightweight JS tokenizer. Optional full grammars are much slower than Prism and need performance work before interactive use. These results do not establish iPhone/Android frame rates or complete Ace application performance.

## Markdown to HTML

The inputs contain headings, paragraphs, inline styles, links, lists, tables, and tasks, with no code/math/raw HTML. Normalized block text, headings, table rows, and task counts match before timing. Markit includes Wasm string conversion; Marked uses its standard parser. No DOM layout or painting is timed.

| Actual input bytes | Markit Wasm | Marked | Marked time / Markit time |
| --- | ---: | ---: | ---: |
| 4,380 | 0.076 ms | 0.300 ms | 3.93x |
| 32,850 | 0.525 ms | 2.165 ms | 4.12x |
| 131,400 | 2.121 ms | 8.770 ms | 4.14x |

Marked does not sanitize its default HTML output. These are controlled, trusted fixtures, not an equivalent secure application pipeline. See [Marked documentation](https://marked.js.org/).

## Syntax highlighting

Uncached inputs have unique suffixes so Markit cannot win by returning a cached source. Markit timings include Wasm conversion, Rust tokenization, JSON serialization, and JS decoding. Ace lexical output is simpler token-kind data; Prism uses Ace's actual flat colored-line wrapper. Different coloring fidelity and metadata mean these are workload costs, not identical-output algorithms.

| Input bytes | Path | Median | Cache |
| --- | --- | ---: | --- |
| 4,200 | Markit Wasm lexical + JSON decode | 0.402 ms | miss |
| 4,200 | Ace JS lexical | 0.051 ms | none |
| 4,200 | Ace Prism colored lines | 0.410 ms | source miss; grammar warm |
| 4,200 | Markit Wasm repeated file + JSON decode | 0.098 ms | hit |
| 16,500 | Markit Wasm lexical + JSON decode | 1.072 ms | miss |
| 16,500 | Ace JS lexical | 0.202 ms | none |
| 16,500 | Ace Prism colored lines | 1.543 ms | source miss; grammar warm |
| 16,500 | Markit Wasm repeated file + JSON decode | 0.378 ms | hit |
| 4,200 | Markit Wasm full grammar + JSON decode | 77.250 ms | miss; grammar warm |
| 16,500 | Markit Wasm full grammar + JSON decode | 279.925 ms | miss; grammar warm |

Default uncached Wasm lexical highlighting costs about 5-8x as much as Ace's simple lexical tokenizer on these fixtures. Optional full grammars cost about 180-190x as much as Prism's colored-line path. Both findings are open performance gaps. A cached Markit result is shown separately and must not be treated as an uncached win.

## Native preparation on the host Mac

The same common-syntax input is passed to the actual Ace Swift block parser plus Foundation inline parser, Foundation's full AttributedString parser, and Markit's Rust/C/JSON/Swift bridge. The outputs are different presentation models. Swift uses `-O`; Rust uses release optimization. These are macOS executions of native code, not device or UIKit layout benchmarks.

| Input bytes | Markit native bridge | Ace Swift + Foundation | Foundation full Markdown |
| --- | ---: | ---: | ---: |
| 4,102 | 0.715 ms | 1.382 ms | 2.015 ms |
| 32,816 | 5.581 ms | 10.463 ms | 17.807 ms |
| 131,264 | 22.099 ms | 41.044 ms | 73.772 ms |

Markit preparation is about 1.9x faster than Ace's current Swift/Foundation preparation on these inputs. The JSON bridge still consumes substantial time at larger sizes. UIKit/SwiftUI drawing, selection, images, accessibility, frame hitches, energy, and memory are unmeasured.

## Math preparation

Markit currently creates vector layout, SVG paths, and native PNG data even for its web path. KaTeX emits HTML plus MathML whose layout/painting happens later in the browser. These operations do different work, so no equivalent-output speedup ratio is claimed. The KaTeX settings prohibit trusted HTML/URL commands.

| Formula | Path | Median |
| --- | --- | ---: |
| simple | Markit Wasm SVG + PNG, uncached | 0.2224 ms |
| simple | KaTeX HTML + MathML, uncached | 0.0137 ms |
| simple | Markit Wasm math, repeated formula | 0.0019 ms |
| sum_fraction | Markit Wasm SVG + PNG, uncached | 0.7614 ms |
| sum_fraction | KaTeX HTML + MathML, uncached | 0.0416 ms |
| sum_fraction | Markit Wasm math, repeated formula | 0.0039 ms |

Candidate work includes a web-only vector math path that avoids unused PNG generation. This has not yet been implemented or measured. Cached formulas are fast, but that does not remove the cost of new/edited formulas.

## Streaming append

One append after a 131,411-byte history; setup is outside the timed interval. Markit creates HTML patches and decodes JSON, while Ace creates token patches. The outputs differ, so there is no direct speedup claim.

| Path | Median per append |
| --- | ---: |
| Markit Wasm HTML patch + JSON decode | 19.6 µs |
| Ace BlockStream tokens | 24.6 µs |

## First-use observations

These are single observations on localhost, with module loading/initialization and one first operation included. They are not statistically sampled startup/network benchmarks.

| Path | Observed time |
| --- | ---: |
| Markit Wasm import + init + first HTML | 25.9 ms |
| Marked import + first HTML | 19.5 ms |
| Ace lexical import + first highlight | 3.2 ms |
| Ace Prism import + TS grammar + first highlight | 9.8 ms |
| Markit full grammar import + init + first TS highlight | 313.3 ms |
| Markit module + init + first math | 27.4 ms |
| KaTeX module + first math | 12.9 ms |

## Remaining measurements and work

- Run native iOS UI and Android instrumentation tests on the registered shared devices. Device controls were disabled during this work.
- Measure mobile frame times, scrolling, memory, and full UI rendering. Mac host results cannot stand in for those.
- There is no equivalent current Android Markdown parser baseline: Ace's Android transcript uses plain Text. Do not present plain-text display as a Markdown performance win.
- Profile Wasm lexical scanning, cache lookup, serialization, and decoding separately; reduce overhead without silently changing feature coverage.
- Keep full grammars opt-in while addressing their steady-state cost.
- Evaluate a web-only math path that skips native PNG work.
- Integrate the renderer into Ace, then compare the full transcript pipeline, not just the library.

Nine batches are retained for each steady-state case. The main statistic is the median of per-call batch averages. The stored p95 is a percentile of batch averages, not production tail latency. See [methodology and reproduction](../benchmarks/README.md) and [raw results](../benchmarks/results/).
