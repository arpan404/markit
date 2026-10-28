# Highlighting and math optimization

Measured on Apple M3 Pro, macOS 27.2, Chromium 156.0.8078.4. The original browser results are retained in `browser.json` and `math.json`; the new runs are in `browser-optimized.json` and `math-optimized.json`. Host Rust comparisons use isolated Cargo outputs for baseline commit `6bc21b6` and these performance changes. Raw samples are in [benchmarks/results](../benchmarks/results/).

## Web lexical highlighting

The worker now transfers a Uint32Array of UTF-16 offsets and RGB colors, then reconstructs the same public token objects in JavaScript. The original JSON API is preserved. Compiler-generated keyword matches replace repeated keyword-list scans; adjacent colors merge as source ranges before text allocation. Full source keys remain in the cache to verify hash collisions. The newest cache entry is compared before hashing to keep repeated-file hits cheap.

All rows below force cache misses with unique suffixes. Times include Wasm conversion and either JSON decoding or reconstruction of complete token objects. They exclude worker messaging and DOM work. These are TypeScript fixtures, not every language.

| Base input bytes / path | Before ms | After ms | Before / after |
| --- | ---: | ---: | ---: |
| 4,200, worker range path | 0.4015 | 0.0461 | 8.72x |
| 4,200, original JSON API | 0.4015 | 0.1469 | 2.73x |
| 16,500, worker range path | 1.0724 | 0.1740 | 6.16x |
| 16,500, original JSON API | 1.0724 | 0.5708 | 1.88x |

| Base input bytes | Markit ranges + tokens ms | Ace lexical ms | Prism colored lines ms |
| --- | ---: | ---: | ---: |
| 4,200 | 0.0461 | 0.0481 | 0.3839 |
| 16,500 | 0.1740 | 0.1904 | 1.4681 |

The range path is comparable to Ace's simpler token-kind output on these fixtures. Prism and Markit use different grammars and metadata. No identical-color equivalence is claimed against those external libraries. Tests compare the range path with Markit's original token API, including colors, Unicode, aliases, SQL case, plain fallback, and token limits.

## Optional full grammars

Each line is parsed once and its scope operations are applied to both themes. Native builds use Oniguruma instead of the portable backtracking backend; Wasm retains the pure Rust backend. Theme colors are checked against independent per-theme parsing on multiline Rust, Python, and TSX fixtures.

| Wasm base input bytes | Before ms | After ms | Before / after |
| --- | ---: | ---: | ---: |
| 4,200 | 77.2500 | 35.7200 | 2.16x |
| 16,500 | 279.9250 | 139.8550 | 2.00x |

Full grammars remain too expensive for low-latency web editing. They stay opt-in. The default worker and Markdown fences use the fast lexical scanner. Neither grammar coverage nor source text is dropped to meet a timing target.

## Host Rust preparation

These rows exclude Swift decoding, JNI, and mobile layout. Each miss case cycles through 256 preallocated inputs, more than the 128-entry cache cap, across nine timed batches. Cache hits are separate. Native math documents request only PNG; HTML requests only SVG. The public combined math API still returns both formats.

| Operation / input bytes | Before ms | After ms | Before / after |
| --- | ---: | ---: | ---: |
| lexical tokens, miss, 4,222 | 0.1259 | 0.0420 | 3.00x |
| native lexical spans, miss, 4,222 | 0.1554 | 0.0323 | 4.81x |
| lexical tokens, hit, 4,200 | 0.0002 | 0.0001 | 1.47x |
| full grammar tokens, miss, 4,222 | 45.8745 | 6.3117 | 7.27x |
| lexical tokens, miss, 16,522 | 0.4648 | 0.1614 | 2.88x |
| native lexical spans, miss, 16,522 | 0.5955 | 0.1270 | 4.69x |
| lexical tokens, hit, 16,500 | 0.0006 | 0.0004 | 1.52x |
| full grammar tokens, miss, 16,522 | 182.6968 | 24.5618 | 7.44x |
| combined SVG/PNG math simple, miss, 7 | 0.0948 | 0.0929 | 1.02x |
| native document math simple, miss, 11 | 0.0950 | 0.0836 | 1.14x |
| HTML math simple, miss, 11 | 0.0967 | 0.0219 | 4.42x |
| combined SVG/PNG math sum_fraction, miss, 45 | 0.3431 | 0.3417 | 1.00x |
| native document math sum_fraction, miss, 49 | 0.3471 | 0.3102 | 1.12x |
| HTML math sum_fraction, miss, 49 | 0.3408 | 0.0611 | 5.57x |

## Web math

The old HTML path created PNG bytes that the browser did not consume. It now produces SVG only. Native output retains the same PNG bytes and dimensions. Separate cache modes prevent an SVG-only result from satisfying a native pixel request.

| Formula, uncached | Before ms | After ms | Before / after |
| --- | ---: | ---: | ---: |
| simple | 0.2224 | 0.0470 | 4.73x |
| sum_fraction | 0.7614 | 0.1205 | 6.32x |

The before/after HTML SVG output is equivalent; unnecessary bitmap work was removed. KaTeX still emits a different HTML/MathML format and remains faster at string generation on these fixtures. No equivalent-output speedup over KaTeX is claimed.

## Ace streaming workloads

Measured on a Linux x86_64 cloud VM, rustc 1.83.0, `cargo run --release -p markit --example ace_bench`. Before is `fe9f9fe` (`perf/packed-color-scan-opt`). After is this branch. Each median is one process, 5–7 samples. Absolute times moved by roughly 20% between later runs on the same VM; the ratios below use the first before run and the last after run. Output is unchanged: `examples/hash_outputs.rs` stays `144f38f180411ce5`, and the stream golden hash in `tests/stream_parity.rs` is unchanged.

The harness streams a 3.5 KB assistant message in 133 chunks (prose, a Rust fence, a diff, math, a task list, and a table) and, separately, appends 400 Rust lines to an open fence. Counts are allocator calls and bytes requested, including the harness.

| Workload | Before | After | Before / after |
| --- | ---: | ---: | ---: |
| Chat stream, HTML | 2.892 ms, 67,919 allocs | 0.661 ms, 2,866 allocs | 4.4x, 24x fewer allocs |
| Chat stream, native document | 2.831 ms, 64,583 allocs | 0.966 ms, 3,760 allocs | 2.9x, 17x fewer allocs |
| Chat stream, HTML and native | 5.711 ms, 131,655 allocs | 1.162 ms, 4,165 allocs | 4.9x, 32x fewer allocs |
| 400-line open Rust fence, HTML | 98.559 ms | 5.957 ms | 16.5x |
| 400-line open Rust fence, native build | 76.865 ms | 18.475 ms | 4.2x |
| 400-line open Rust fence, both | 171.087 ms | 22.193 ms | 7.7x |
| One-shot HTML, 14,482-byte Rust fence | 0.222 ms, 5,228 allocs | 0.080 ms, 30 allocs | 2.8x, 174x fewer allocs |
| One-shot HTML, 5,558-byte diff | 0.095 ms, 1,707 allocs | 0.034 ms, 29 allocs | 2.8x |

An open fence used to rebuild every line of HTML and every native span on each append. HTML now keeps the closed lines and rewrites the open line. Native documents reuse stable syntax and diff spans and merge them in parser order instead of sorting. Palette colors are borrowed, so cloning a span does not copy the color strings. Unchanged held blocks are reused when their source and references are unchanged. `linkify` is skipped when a text node has no `:`, `@`, or `www.`.

The same 400-line native stream, including serialization of every patch:

| Encoding | Median | Last patch |
| --- | ---: | ---: |
| JSON `markit_stream_append` | 107.208 ms | 405,456 bytes |
| `encode_native_patch` / `markit_stream_append_binary` (MKNP v1) | 35.321 ms | 221,450 bytes |

MKNP is a versioned wrapper of the existing MKBP documents. UIKit and JNI still call the JSON export, so this speedup is not on the mobile call path yet. Building the native document is 18 ms of the JSON total; encoding the JSON is the rest.

Web Rust append of the 400-line fence went from 0.263 ms to 0.474 ms. Each append now records UTF-16 offsets for a top-level fence body. The worker patch (`appendPacked`, magic `MKWP`) sends those offsets and packed highlight words instead of the fence text. Nested fences whose lines are rewritten (quote markers, indentation) still embed the body. The DOM contract is unchanged: the page slices the Markdown it already holds. This VM did not run Playwright, so that path is covered by the Rust patch tests and the existing browser suite when CI runs it.

## Validation and limits

- Rust workspace tests pass with default and optional features; formatting and Clippy pass with warnings denied.
- Nine browser flows and three Swift tests through the real C bridge pass. The browser demo was opened in Ace and had no console errors.
- Optional-grammar iOS device/simulator XCFramework and Android arm64/x86_64 libraries build. Android AAR, demo APK, and instrumentation test APK assemble. The iOS C/Rust deployment target matches Swift's iOS 15 minimum.
- CI checks both native feature sets and compiles the optional Wasm build. GitHub CI status is separate from these local results.
- No mobile UI or instrumentation tests were executed. Device controls were disabled; no shared simulator was created, erased, or driven. iPhone and Android frame times, memory, and battery remain unmeasured.
- Native JSON/Swift decoding remains a separate cost. These native lexical gains do not imply the same ratio for whole Markdown documents or UIKit/Android drawing.
- Open streaming roots, late references, and cold grammar compilation can remain expensive. These results do not establish that every feature on every input is fast.

The streaming table above was measured here, not on the M3. This environment did not run Swift, the iOS simulator, Android instrumentation, or Playwright. `cargo test -p markit --lib --tests`, the same tests with `--features grammars`, and `cargo test -p markit-ffi --lib` passed. `cargo clippy -p markit -p markit-ffi --all-targets -- -D warnings` passed. `cargo build -p markit-wasm --release --target wasm32-unknown-unknown` produced a 2,840,665-byte `markit_wasm.wasm`. A before size was not captured. The crate graph is the same, including the PNG rasterizer, so this pass is not a Wasm size reduction. wasm-bindgen and Playwright were not run.

A 400-line native fence is still superlinear: every append materializes the full span list, and the JSON export walks it again. MKNP removes the JSON walk and not the span copy. Prose that never settles (one giant paragraph, an open list, a late reference definition) still reparses the tail. Full grammars stay opt-in. KaTeX is still faster at math string generation on the earlier Mac fixtures. iPhone and Android frame times are still unmeasured.

## Reproduce

## Highlighting, math and native transport pass

Measured on the shared Linux x86_64 VM (8 vCPU), rustc 1.91.1, release profile, on 2026-10-28. Before is `7df24be`; after is this pass. Each before/after pair ran back to back in one script on an otherwise idle box: `cargo run --release -p markit --features grammars --example perf_pass`, `--example ace_bench`, and `cargo bench -p markit --bench throughput --features grammars`. Medians of 5–15 samples from one process; expect roughly 5–10% run-to-run noise on this VM. Output is unchanged: `hash_outputs` prints `144f38f180411ce5`, the stream golden is `0x62934f069ba14d79`, and the new `tests/golden.rs` math (`1c14432ffdfbb735`) and full-grammar (`86113db1668408bd`) hashes were recorded at `7df24be`.

| Case | Before | After | Change |
| --- | ---: | ---: | ---: |
| Native fence, 2,000 lines by line, JSON patches | 633.2 ms | 324.8 ms | 1.9x |
| Native fence, 2,000 lines, MKNP v1 | 389.9 ms | 93.2 ms | 4.2x |
| Native fence, 2,000 lines, MKNP v2 (new) | — | 47.2 ms | 8.3x vs v1 before |
| Same, per append over the last 50 lines | 0.243 ms (v1) | 0.009 ms (v2) | 27x |
| Same, open-fence patch size | 154,850 B (v1) | 279 B (v2) | |
| Native fence, 400 lines, MKNP v1 → v2 (`ace_bench`) | 31.5 ms | 14.3 ms | 2.2x |
| `ace_bench` stream_fence_400 native / html | 16.7 / 4.6 ms | 13.1 / 3.6 ms | 1.3x |
| Lexical packed hit, Rust 200 lines | 0.0120 ms | 0.0009 ms | 13x |
| Lexical packed miss, Rust 200 lines | 0.0602 ms | 0.0562 ms | 1.07x |
| `ace_bench` highlight_grow_120 sum | 0.570 ms | 0.380 ms | 1.5x |
| Full grammar, Rust 200 lines, cold | 17.7 ms | 8.5 ms | 2.1x |
| Full grammar, TSX 200 lines, cold | 24.0 ms | 17.4 ms | 1.4x |
| Full grammar, Rust 200 lines, grown line by line | 60.8 ms | 10.2 ms | 6.0x |
| Full grammar, TSX 200 lines, grown line by line | 48.0 ms | 19.3 ms | 2.5x |
| Full grammar, exact repeat | 0.034 ms | 0.001 ms | |
| Full grammar, first use (TS) | 7.8 ms | 8.0 ms | unchanged |
| Math, 6 new formulas, SVG + PNG API | 3.82 ms | 2.79 ms | 1.4x |
| Math, 6 new formulas, native PNG only | 2.36 ms | 2.28 ms | 1.04x |
| Math, 6 new formulas, HTML SVG only | 0.62 ms | 0.58 ms | 1.07x |
| Math, streamed (native / both) | 2.84 / 3.06 ms | 2.61 / 2.74 ms | 1.1x |
| Throughput bench, math SVG + PNG | 1,887 µs | 1,625 µs | 1.16x |
| Throughput bench, grammar cold / grow 200 | 19.4 / 61.8 ms | 7.6 / 10.4 ms | 2.6x / 6.0x |
| Wasm (`wasm-bindgen` output) | 2,692,403 B | 2,160,196 B | −19.8% |
| Wasm, gzip −9 | 1,329,446 B | 1,122,984 B | −15.5% |

The Markdown throughput rows (parse, JSON, HTML at 1/50/400 sections) and the 104 KB streaming append moved within run-to-run noise.

What changed:

- **Lexical.** Growing fences check the append hot slot before the hashed cache. Past the 8,192-run limit, the plain fallback stays sticky for later appends unless the fence's later text could change earlier tones, so a 2,000-line fence no longer rescans and rehashes the whole fence on every line. Cache keys use a word-at-a-time hash and are still confirmed by full comparison. Packed words are cached per result, and UTF-16 offsets use an ASCII fast path.
- **Full grammars.** Theme styles depend only on the scope stack, so both themes resolve once per distinct stack, not through two `HighlightIterator`s per line. Highlighters are built once per process. Line checkpoints share snapshots. A pure append reuses the previous token vector in place when the caller has dropped it, and is kept out of the byte-budgeted cache. First use still pays the syntax-set load.
- **Math.** Native PNGs use a port of latex-rust's tiny-skia raster with a per-thread glyph outline cache. The PNG bytes are identical. Wasm no longer links tiny-skia or the PNG encoder. Most of the remaining PNG time is anti-aliased fill and deflate; the encoder settings are fixed by the golden PNG bytes.
- **Native transport.** MKNP v2 ([binary transport](binary-transport.md)) sends an open fence as an extension of the block the host already holds. `markit_stream_append_patch` and `Markit.nativeAppendPatch` expose it, and Swift and Kotlin `MarkitStream` now use it in place of JSON. The non-lazy stream path only reuses the fence span list when the fence body actually grew; otherwise it compares blocks as before.

Verification: `cargo test --workspace` passes with default features and with `--features markit/grammars`. `cargo clippy --workspace --all-targets -D warnings` passes for both, and `cargo fmt` is clean. `tests/native_patch.rs` replays MKNP v2 patches against the eager JSON stream for each append. The Kotlin `NativePatchReader` and `MarkitStream` were compiled with kotlinc 2.0.21 and run on a desktop JVM through the real JNI functions from `android.rs`, built as a host cdylib with an `android.util.Base64` stub. In that run, 22,628 appends produced the same documents as the JSON bridge. The Swift `NativePatchReader` and `MarkitStream` were compiled with Swift 6.0.3 on Linux against the C header and a host `libmarkit_ffi.a`. There, 2,473 appends matched `markit_stream_append` JSON decoding. Neither has been run on an iOS or Android device or in their UI layers, and frame times are unmeasured.

Remaining gaps:

- Swift and Kotlin still copy the kept text and span range per extension, which is linear in the fence on the host side.
- `lex()` still does a `starts_with` comparison over the previous fence per append.
- The web fence path still copies fence source.
- Full-grammar regex matching (Oniguruma natively) and first-use syntax loading are unchanged.
- Math PNG fill and deflate dominate new-formula cost.
- A Wasm `renderDocument` or native-mode stream keeps math as source text, because PNG is native-only.

Run `cargo run --release -p markit --example ace_bench` for the streaming table. `cargo run --release -p markit --example hash_outputs` prints the output fingerprint. Run `./benchmarks/run-native-features.sh` for the isolated host comparison. Build web packages with `./scripts/build-web.sh`, then build optional grammar Wasm into `benchmarks/artifacts/grammars-web` as in `benchmarks/run.sh`. Run `BENCH_OUTPUT=browser-optimized.json node benchmarks/run-browser.mjs` and `BENCH_FUNCTION=runMathBench BENCH_OUTPUT=math-optimized.json node benchmarks/run-browser.mjs` sequentially on an idle machine. Finally run `python3 benchmarks/write-optimization-report.py`. See [methodology](../benchmarks/README.md) for fixture and batch details.
