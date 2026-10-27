from pathlib import Path
import json
root=Path(__file__).resolve().parent.parent
browser=json.loads((root/'benchmarks/results/browser.json').read_text())
native=json.loads((root/'benchmarks/results/native-host.json').read_text())
math=json.loads((root/'benchmarks/results/math.json').read_text())
env=json.loads((root/'benchmarks/results/environment.json').read_text())
lines=['# Renderer comparison benchmark','','See [the optimization report](performance-optimization.md) for the later before/after pass and compact range transport.','',f"Measured on {env['cpu']}, macOS {env['os']}, Chromium {browser['browser_version']}. Marked {env['marked']}, Prism {env['prism']}, KaTeX {env['katex']}. The native model code is from Ace commit `{env['ace_commit'][:12]}`; the Markit engine is from `{env['markit_commit'][:12]}`.",'',
'The tables describe the recorded workloads, including bridge conversion. Default lexical and optional full-grammar paths have different coloring fidelity. Compare uncached cases separately from cache hits. These results do not establish iPhone/Android frame rates or complete Ace application performance.','',
'## Markdown to HTML','',
'The inputs contain headings, paragraphs, inline styles, links, lists, tables, and tasks, with no code/math/raw HTML. Normalized block text, headings, table rows, and task counts match before timing. Markit includes Wasm string conversion; Marked uses its standard parser. No DOM layout or painting is timed.','',
'| Actual input bytes | Markit Wasm | Marked | Marked time / Markit time |','| --- | ---: | ---: | ---: |']
for i in range(0,len(browser['html']),2):
 a,b=browser['html'][i:i+2]; lines.append(f"| {a['bytes']:,} | {a['median_ms']:.3f} ms | {b['median_ms']:.3f} ms | {b['median_ms']/a['median_ms']:.2f}x |")
lines+=['','Marked does not sanitize its default HTML output. These are controlled, trusted fixtures, not an equivalent secure application pipeline. See [Marked documentation](https://marked.js.org/).','',
'## Syntax highlighting','',
'Uncached inputs have unique suffixes so Markit cannot win by returning a cached source. Markit timings include Wasm conversion, Rust tokenization, JSON serialization, and JS decoding. Ace lexical output is simpler token-kind data; Prism uses Ace\'s actual flat colored-line wrapper. Different coloring fidelity and metadata mean these are workload costs, not identical-output algorithms.','',
'| Input bytes | Path | Median | Cache |','| --- | --- | ---: | --- |']
for r in browser['highlighting']:
 lines.append(f"| {r['bytes']:,} | {r['name']} | {r['median_ms']:.3f} ms | {r['cache']} |")
lines+=['','The full grammar and lexical paths do different work. See the optimization report for measurements of the compact range API alongside the original JSON API. A cached Markit result is shown separately and must not be treated as an uncached win.','',
'## Native preparation on the host Mac','',
'The same common-syntax input is passed to the actual Ace Swift block parser plus Foundation inline parser, Foundation\'s full AttributedString parser, and Markit\'s Rust/C/JSON/Swift bridge. The outputs are different presentation models. Swift uses `-O`; Rust uses release optimization. These are macOS executions of native code, not device or UIKit layout benchmarks.','',
'| Input bytes | Markit native bridge | Ace Swift + Foundation | Foundation full Markdown |','| --- | ---: | ---: | ---: |']
for i in range(0,len(native),3):
 a,b,c=native[i:i+3]; lines.append(f"| {a['bytes']:,} | {a['median_ms']:.3f} ms | {b['median_ms']:.3f} ms | {c['median_ms']:.3f} ms |")
lines+=['','Markit preparation is about 1.9x faster than Ace\'s current Swift/Foundation preparation on these inputs. The JSON bridge still consumes substantial time at larger sizes. UIKit/SwiftUI drawing, selection, images, accessibility, frame hitches, energy, and memory are unmeasured.','',
'## Math preparation','',
'The original math measurements include SVG and PNG creation. Later builds produce SVG for HTML and PNG for native documents, with separate cache entries for each output mode. KaTeX emits HTML plus MathML whose layout/painting happens later in the browser. These operations do different work, so no equivalent-output speedup ratio is claimed. The KaTeX settings prohibit trusted HTML/URL commands.','',
'| Formula | Path | Median |','| --- | --- | ---: |']
for r in math['math']: lines.append(f"| {r['formula']} | {r['name']} | {r['median_ms']:.4f} ms |")
lines+=['','The optimization report measures the later vector-only HTML path. Cached formulas are fast, but that does not remove the cost of new/edited formulas.','',
'## Streaming append','',
'One append after a 131,411-byte history; setup is outside the timed interval. Markit creates HTML patches and decodes JSON, while Ace creates token patches. The outputs differ, so there is no direct speedup claim.','',
'| Path | Median per append |','| --- | ---: |']
for r in browser['streaming']: lines.append(f"| {r['name']} | {r['median_ms']*1000:.1f} µs |")
lines+=['','## First-use observations','',
'These are single observations on localhost, with module loading/initialization and one first operation included. They are not statistically sampled startup/network benchmarks.','',
'| Path | Observed time |','| --- | ---: |']
for r in browser['cold']+math['cold']: lines.append(f"| {r['name']} | {r['ms']:.1f} ms |")
lines+=['','## Remaining measurements and work','',
'- Run native iOS UI and Android instrumentation tests on the registered shared devices. Device controls were disabled during this work.','- Measure mobile frame times, scrolling, memory, and full UI rendering. Mac host results cannot stand in for those.','- There is no equivalent current Android Markdown parser baseline: Ace\'s Android transcript uses plain Text. Do not present plain-text display as a Markdown performance win.','- Compare compact range transport separately from the original JSON API.','- Keep full grammars opt-in while addressing their steady-state cost.','- Track native bitmap generation independently of web SVG output.','- Integrate the renderer into Ace, then compare the full transcript pipeline, not just the library.','',
'Nine batches are retained for each steady-state case. The main statistic is the median of per-call batch averages. The stored p95 is a percentile of batch averages, not production tail latency. See [methodology and reproduction](../benchmarks/README.md) and [raw results](../benchmarks/results/).', '']
(root/'docs/benchmark-comparison.md').write_text('\n'.join(lines))
