from pathlib import Path
import json

root = Path(__file__).resolve().parent.parent
results = root / 'benchmarks/results'
load = lambda name: json.loads((results / name).read_text())
before = load('browser.json')
after = load('browser-optimized.json')
math_before = load('math.json')
math_after = load('math-optimized.json')
native_before = load('native-features-before.json')
native_after = load('native-features-after.json')
def find(rows, name, size=None, formula=None):
    return next(r for r in rows if r['name'] == name and (size is None or r['bytes'] == size) and (formula is None or r['formula'] == formula))
def table_row(label, old, new):
    a, b = old['median_ms'], new['median_ms']
    return f'| {label} | {a:.4f} | {b:.4f} | {a / b:.2f}x |'
lines = ['# Highlighting and math optimization', '',
'Measured on Apple M3 Pro, macOS 27.2, Chromium ' + after['browser_version'] + '. The original browser results are retained in `browser.json` and `math.json`; the new runs are in `browser-optimized.json` and `math-optimized.json`. Host Rust comparisons use isolated Cargo outputs for baseline commit `6bc21b6` and these performance changes. Raw samples are in [benchmarks/results](../benchmarks/results/).', '',
'## Web lexical highlighting', '',
'The worker now transfers a Uint32Array of UTF-16 offsets and RGB colors, then reconstructs the same public token objects in JavaScript. The original JSON API is preserved. Compiler-generated keyword matches replace repeated keyword-list scans; adjacent colors merge as source ranges before text allocation. Full source keys remain in the cache to verify hash collisions. The newest cache entry is compared before hashing to keep repeated-file hits cheap.', '',
'All rows below force cache misses with unique suffixes. Times include Wasm conversion and either JSON decoding or reconstruction of complete token objects. They exclude worker messaging and DOM work. These are TypeScript fixtures, not every language.', '',
'| Base input bytes / path | Before ms | After ms | Before / after |', '| --- | ---: | ---: | ---: |']
for size in [4200,16500]:
    old = find(before['highlighting'], 'Markit Wasm lexical + JSON decode', size)
    for name,label in [('Markit Wasm lexical ranges + tokens','worker range path'),('Markit Wasm lexical + JSON decode','original JSON API')]:
        lines.append(table_row(f'{size:,}, {label}',old,find(after['highlighting'],name,size)))
lines += ['', '| Base input bytes | Markit ranges + tokens ms | Ace lexical ms | Prism colored lines ms |', '| --- | ---: | ---: | ---: |']
for size in [4200,16500]:
    vals=[find(after['highlighting'], n, size)['median_ms'] for n in ['Markit Wasm lexical ranges + tokens','Ace JS lexical','Ace Prism colored lines']]
    lines.append(f'| {size:,} | {vals[0]:.4f} | {vals[1]:.4f} | {vals[2]:.4f} |')
lines += ['', 'The range path is comparable to Ace\'s simpler token-kind output on these fixtures. Prism and Markit use different grammars and metadata. No identical-color equivalence is claimed against those external libraries. Tests compare the range path with Markit\'s original token API, including colors, Unicode, aliases, SQL case, plain fallback, and token limits.', '',
'## Optional full grammars', '',
'Each line is parsed once and its scope operations are applied to both themes. Native builds use Oniguruma instead of the portable backtracking backend; Wasm retains the pure Rust backend. Theme colors are checked against independent per-theme parsing on multiline Rust, Python, and TSX fixtures.', '',
'| Wasm base input bytes | Before ms | After ms | Before / after |', '| --- | ---: | ---: | ---: |']
for size in [4200,16500]:
    lines.append(table_row(f'{size:,}',find(before['highlighting'],'Markit Wasm full grammar + JSON decode',size),find(after['highlighting'],'Markit Wasm full grammar + JSON decode',size)))
lines += ['', 'Full grammars remain too expensive for low-latency web editing. They stay opt-in. The default worker and Markdown fences use the fast lexical scanner. Neither grammar coverage nor source text is dropped to meet a timing target.', '',
'## Host Rust preparation', '',
'These rows exclude Swift decoding, JNI, and mobile layout. Each miss case cycles through 256 preallocated inputs, more than the 128-entry cache cap, across nine timed batches. Cache hits are separate. Native math documents request only PNG; HTML requests only SVG. The public combined math API still returns both formats.', '',
'| Operation / input bytes | Before ms | After ms | Before / after |', '| --- | ---: | ---: | ---: |']
for old,new in zip(native_before['results'],native_after['results']):
    assert (old['name'],old['bytes']) == (new['name'],new['bytes'])
    label = old['name'].replace('native PNG math','combined SVG/PNG math')
    lines.append(table_row(f"{label}, {old['bytes']:,}",old,new))
lines += ['', '## Web math', '',
'The old HTML path created PNG bytes that the browser did not consume. It now produces SVG only. Native output retains the same PNG bytes and dimensions. Separate cache modes prevent an SVG-only result from satisfying a native pixel request.', '',
'| Formula, uncached | Before ms | After ms | Before / after |', '| --- | ---: | ---: | ---: |']
for formula in ['simple','sum_fraction']:
    lines.append(table_row(formula,find(math_before['math'],'Markit Wasm SVG + PNG, uncached',formula=formula),find(math_after['math'],'Markit Wasm SVG, uncached',formula=formula)))
lines += ['', 'The before/after HTML SVG output is equivalent; unnecessary bitmap work was removed. KaTeX still emits a different HTML/MathML format and remains faster at string generation on these fixtures. No equivalent-output speedup over KaTeX is claimed.', '',
'## Validation and limits', '',
'- Rust workspace tests pass with default and optional features; formatting and Clippy pass with warnings denied.',
'- Nine browser flows and three Swift tests through the real C bridge pass. The browser demo was opened in Ace and had no console errors.',
'- Optional-grammar iOS device/simulator XCFramework and Android arm64/x86_64 libraries build. Android AAR, demo APK, and instrumentation test APK assemble. The iOS C/Rust deployment target matches Swift\'s iOS 15 minimum.',
'- CI checks both native feature sets and compiles the optional Wasm build. GitHub CI status is separate from these local results.',
'- No mobile UI or instrumentation tests were executed. Device controls were disabled; no shared simulator was created, erased, or driven. iPhone and Android frame times, memory, and battery remain unmeasured.',
'- Native JSON/Swift decoding remains a separate cost. These native lexical gains do not imply the same ratio for whole Markdown documents or UIKit/Android drawing.',
'- Open streaming roots, late references, and cold grammar compilation can remain expensive. These results do not establish that every feature on every input is fast.', '',
'## Reproduce', '',
'Run `./benchmarks/run-native-features.sh` for the isolated host comparison. Build web packages with `./scripts/build-web.sh`, then build optional grammar Wasm into `benchmarks/artifacts/grammars-web` as in `benchmarks/run.sh`. Run `BENCH_OUTPUT=browser-optimized.json node benchmarks/run-browser.mjs` and `BENCH_FUNCTION=runMathBench BENCH_OUTPUT=math-optimized.json node benchmarks/run-browser.mjs` sequentially on an idle machine. Finally run `python3 benchmarks/write-optimization-report.py`. See [methodology](../benchmarks/README.md) for fixture and batch details.', '']
(root / 'docs/performance-optimization.md').write_text('\n'.join(lines))
