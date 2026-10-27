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

## Validation and limits

- Rust workspace tests pass with default and optional features; formatting and Clippy pass with warnings denied.
- Nine browser flows and three Swift tests through the real C bridge pass. The browser demo was opened in Ace and had no console errors.
- Optional-grammar iOS device/simulator XCFramework and Android arm64/x86_64 libraries build. Android AAR, demo APK, and instrumentation test APK assemble. The iOS C/Rust deployment target matches Swift's iOS 15 minimum.
- CI checks both native feature sets and compiles the optional Wasm build. GitHub CI status is separate from these local results.
- No mobile UI or instrumentation tests were executed. Device controls were disabled; no shared simulator was created, erased, or driven. iPhone and Android frame times, memory, and battery remain unmeasured.
- Native JSON/Swift decoding remains a separate cost. These native lexical gains do not imply the same ratio for whole Markdown documents or UIKit/Android drawing.
- Open streaming roots, late references, and cold grammar compilation can remain expensive. These results do not establish that every feature on every input is fast.

## Reproduce

Run `./benchmarks/run-native-features.sh` for the isolated host comparison. Build web packages with `./scripts/build-web.sh`, then build optional grammar Wasm into `benchmarks/artifacts/grammars-web` as in `benchmarks/run.sh`. Run `BENCH_OUTPUT=browser-optimized.json node benchmarks/run-browser.mjs` and `BENCH_FUNCTION=runMathBench BENCH_OUTPUT=math-optimized.json node benchmarks/run-browser.mjs` sequentially on an idle machine. Finally run `python3 benchmarks/write-optimization-report.py`. See [methodology](../benchmarks/README.md) for fixture and batch details.
