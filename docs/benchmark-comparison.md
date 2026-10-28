# Renderer comparison benchmark

These are the original baseline measurements. See [the optimization report](performance-optimization.md) for the later speed improvements, the Linux streaming measurements, and remaining gaps.

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

Wasm builds no longer link the PNG rasterizer, so new formulas on the web produce SVG only; the table above predates that change and has not been re-run in a browser. Cached formulas are fast, but that does not remove the cost of new/edited formulas.

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

## Streaming fences after the Linux pass

The Mac streaming row above is one append after a finished history. It does not show an assistant message that is still inside a large fence. On the Linux VM, appending 400 Rust lines to an open fence cost 98.6 ms of HTML and 76.9 ms of native document construction before this pass, and 6.0 ms / 18.5 ms after, with identical patch bytes. Serializing those native patches as JSON cost 107 ms; the MKNP binary wrapper cost 35 ms and 221 KB instead of 405 KB. UIKit and JNI then still took the JSON export. A later Linux pass moved them to MKNP v2 fence extensions: a 2,000-line fence streamed by line went from 390 ms with MKNP v1 to 47 ms, with 279-byte open-fence patches. Swift and Kotlin readers were checked against the JSON bridge on Linux hosts, not devices. Details and the chat-message table are in [the optimization report](performance-optimization.md).

## React transcript in Chromium vs Ace web

This compares `@markit/web/react` with the Markdown stack in Ace's web chat, Ace commit `2502a66`, both mounted through production React 19 in real Chromium. Ace's side is its own `Prose`/`Markdown` components, its shared `markdown.worker` block stream, its cold main-thread path, its regex highlighter and lazy KaTeX. Only app components that need Ace's client or router are stubbed. Scripts, fixtures and raw samples are in [`web/bench`](../web/bench/README.md).

Measured on an 8-vCPU Linux VM (Intel Xeon), with Playwright's headless Chromium 156.0.8078.4, rustc 1.99.1 and Node 24 (gzip and brotli sizes come from Node's zlib). The tables show medians of 7 runs (1x) and 5 runs (`--throttle 4`); every run uses a fresh browser context. Times run until the DOM has been quiet for 300 ms and are counted to the last mutation. "Main thread" is CDP `TaskDuration` and includes style, layout and paint.

| Scenario | Markit 1x | Ace 1x | Markit 4x CPU | Ace 4x CPU |
| --- | ---: | ---: | ---: | ---: |
| First message, cold: mount to settled | **12.5 ms** | 26 ms | **31 ms** | 77 ms |
| First message, cold: navigation to settled | **69 ms** | 116 ms | **199 ms** | 387 ms |
| First message, cold, explicit `preloadMarkit()`: mount to settled | 12.3 ms | — | 30 ms | — |
| First message, warm | **3.9 ms** | 10.3 ms | **11.5 ms** | 24 ms |
| 50 messages (33 KB): settled | **37 ms** | 71 ms | **72 ms** | 283 ms |
| 50 messages: main thread | **38 ms** | 102 ms | **133 ms** | 406 ms |
| 200 messages (135 KB): settled | **61 ms** | 224 ms | **206 ms** | 845 ms |
| 200 messages: main thread | **67 ms** | 223 ms | **243 ms** | 935 ms |
| 200 messages, `lazy`: settled / DOM nodes | 43 ms / 1,159 | — | 190 ms / 929 | — |
| Stream a 6.6 KB answer, 277 tokens, 1 per frame: main thread | **191 ms** | 240 ms | **598 ms** | 766 ms |
| Stream: JS script time | **42 ms** | 88 ms | **125 ms** | 283 ms |
| Stream: worst frame / last token to final DOM | 17.3 / 1.1 ms | 17.8 / 3.7 ms | 20.6 / 3.4 ms | 28.2 / 12.7 ms |

There were no long tasks over 50 ms, and so no total blocking time, for either renderer in any scenario, even at 4x. Ace paces open-block updates (10–20 a second) and builds finished messages in small tasks. Markit renders in its worker and paces an open message to at most 30 updates a second (every 32–100 ms, four times the last round trip; the first token and the final render are not delayed). It patches the DOM in short tasks.

Memory and transfer were the same in every run:

| After mounting 200 messages | Markit | Ace |
| --- | ---: | ---: |
| Main-thread JS heap after GC | **3.5 MB** | 13.6 MB |
| DOM nodes | **10,555** | 14,335 |
| Workers | 1 | 1 |
| Worker JS heap / ArrayBuffer backing | **1.2 MB / 0.5 MB** | 3.7 MB / 0.6 MB |
| Wasm linear memory | 2.8 MB | — |
| Transfer, everything the page fetched, gzip -9 | **497.4 KB**, 5 files | 519.8 KB, 13 files |
| Same, brotli 11 | **412.3 KB** | 450.9 KB |

Markit's transfer is made of five files:

- the app chunk with React (74.6 KB);
- the worker (3.3 KB);
- the CSS (1.2 KB);
- a shared 0.4 KB chunk;
- the Wasm module (418.0 KB, rustc 1.99.1), about 153 KB of which is the embedded core math font.

Some formulas need a glyph outside the core font, for example `\mathfrak`, `\mathscr`, lowercase `\mathbb`, rare symbols and accented text. The first such formula makes the worker fetch the full STIX Two Math once (644 KB gzip) and re-render. That fetch is not in these numbers, and no fixture triggers it.

What changed since the previous table (cold 44 ms, 50 messages 92 ms, streaming at 4x 781 ms, transfer 1.21 MB):

- **Size.** Several changes cut the download:
  - a size-optimized Wasm profile (`opt-level = "z"`, fat LTO, strip);
  - a core cut of the math font that keeps every glyph's metrics and bounding box, with the full font fetched on demand;
  - a vendored latex-rust whose ttf-parser drops the Apple, variable-font and glyph-name tables;
  - one HTML event pipeline;
  - a lazy WebGPU chunk.

  The harness now bundles Markit's JS the way a host bundler would. It also links Ace's duplicated CSS once, which removes two duplicate 3.5 KB sheets from Ace's transfer.
- **Cold start.** Importing `index.js` starts the shared worker after the current task. The idle worker then lays out one formula, so font parsing and cold Wasm code stay off the first message's path. latex-rust's `ssty` setup now looks up only covered glyphs. Ace also warms KaTeX at idle, but starts its worker on first use.
- **50 messages.** This is mostly the same start-up work: the worker is ready when the transcript mounts.
- **Streaming.** Pacing an open message halves DOM, style and layout work. At 1x, style recalculations per stream went from about 249 to 124.

Ace is now behind in every row measured here.

Caveats:

- Ace's Tailwind stylesheet is not built, so Ace's layout and style costs are probably understated.
- The cold rows include Markit's start-on-import: the worker starts when the page loads, before the message mounts. A page that imports Markit only when the first message arrives pays worker start, Wasm compile and the first formula (30–45 ms at 1x in single runs on this VM) on that message.
- Pacing is a behaviour choice: an open message updates at most every 32 ms (Ace: 50–100 ms). `streamInterval: 0` restores per-update rendering at the old cost.
- Ace's thread and file references, images and links are stubbed, so Ace does less work per message than in the app.
- Chromium's CPU throttle slows the page's main thread. It may not slow workers in the same way, which would favour the side that does more work off the main thread: Markit for rendering, and Ace's worker for parsing.
- Wasm linear memory never shrinks. JS heap figures do not include it, so it is listed separately.
- This is one Linux VM. Safari, Firefox, mobile devices, scrolling, and the integrated Ace app are unmeasured.

## Remaining measurements and work

- Run native iOS UI and Android instrumentation tests on the registered shared devices. Device controls were disabled during this work.
- Measure mobile frame times, scrolling, memory, and full UI rendering. Mac host results cannot stand in for those.
- There is no equivalent current Android Markdown parser baseline: Ace's Android transcript uses plain Text. Do not present plain-text display as a Markdown performance win.
- Profile Wasm lexical scanning, cache lookup, serialization, and decoding separately; reduce overhead without silently changing feature coverage.
- Keep full grammars opt-in while addressing their steady-state cost.
- Integrate the renderer into Ace, then compare the full transcript pipeline, not just the library. The React harness in `web/bench` is a step towards this, but it stubs Ace components and does not build Tailwind.
- Build Ace with its own Vite and Tailwind setup and repeat the size and style/layout comparison; this table uses an esbuild bundle of Ace's components.
- Measure how often real transcripts need the full math font (a one-time 644 KB fetch).
- Run the React benchmark in Safari and Firefox, and on mid-range Android Chrome.

Nine batches are retained for each steady-state case. The main statistic is the median of per-call batch averages. The stored p95 is a percentile of batch averages, not production tail latency. See [methodology and reproduction](../benchmarks/README.md) and [raw results](../benchmarks/results/).
