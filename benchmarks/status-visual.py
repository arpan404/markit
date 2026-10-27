from pathlib import Path
import json, html
root=Path(__file__).resolve().parent.parent
b=json.loads((root/'benchmarks/results/browser.json').read_text())
n=json.loads((root/'benchmarks/results/native-host.json').read_text())
m=json.loads((root/'benchmarks/results/math.json').read_text())
e=json.loads((root/'benchmarks/results/environment.json').read_text())
css='''body{background:transparent;color:var(--foreground);margin:0} .status-flow,.bench-grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,290px),1fr));gap:18px} .status-flow{grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:12px} .stage{padding:10px 0;border-top:1px solid var(--border)} .stage p{margin:7px 0 0} .small{color:var(--muted-foreground)} .measure{display:grid;grid-template-columns:minmax(100px,1fr) minmax(45px,1.2fr) 82px;gap:10px;align-items:center;margin:10px 0} .track{height:8px;background:var(--surface-2);border:0} .fill{height:100%;background:var(--muted-foreground)} .fill.markit{background:var(--accent)} .number{text-align:right;font-variant-numeric:tabular-nums} .table-wrap{max-width:100%;overflow-x:auto} .table-wrap table{min-width:300px} .bench-grid section{min-width:0} .legend{display:flex;gap:14px;flex-wrap:wrap} .legend span::before{content:"";display:inline-block;width:9px;height:9px;background:var(--muted-foreground);margin-right:6px} .legend .markit::before{background:var(--accent)} section+section{margin-top:0} hr{margin:18px 0} .callout{margin-top:12px} h3{margin:10px 0} li{margin:6px 0} .snapshot{margin:0 0 14px} @media(max-width:440px){.measure{grid-template-columns:minmax(88px,1fr) minmax(32px,1fr) 74px;gap:6px}}'''
def doc(body): return '<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>'+css+'</style></head><body>'+body+'</body></html>'
def table(headers, rows):
 return '<div class="table-wrap"><table><thead><tr>'+''.join('<th>'+html.escape(str(h))+'</th>' for h in headers)+'</tr></thead><tbody>'+''.join('<tr>'+''.join('<td>'+str(cell)+'</td>' for cell in row)+'</tr>' for row in rows)+'</tbody></table></div>'
def bars(rows, unit='ms'):
 maxv=max(value for _,value,_ in rows)
 return ''.join(f'<div class="measure"><span>{html.escape(label)}</span><div class="track"><div class="fill {"markit" if markit else ""}" style="width:{value/maxv*100:.4f}%"></div></div><span class="number">{value:.3f} {unit}</span></div>' for label,value,markit in rows)
status='''<p class="snapshot"><span class="badge success">Renderer implemented</span> <span class="badge warning">Production validation incomplete</span></p>
<div class="status-flow">
<div class="stage"><span class="badge success">Done</span><p>Shared Rust engine</p></div>
<div class="stage"><span class="badge success">Done</span><p>iOS · Android · Web implementations</p></div>
<div class="stage"><span class="badge warning">Partial</span><p>Runtime and performance verification</p></div>
<div class="stage"><span class="badge">Pending</span><p>Ace application integration</p></div>
<div class="stage"><span class="badge">Pending</span><p>CI, review, merge and release</p></div>
</div>
<h3>Platform coverage</h3>'''
status+=table(['Platform','Implemented and checked','Still unverified'],[
 ['Web','Wasm worker, Markdown, highlighting, math, diffs, copy, streaming and virtualized code.<br><span class="badge success">8 browser tests passed</span>','Full Ace transcript performance; highlighting optimization'],
 ['iOS','UIKit/SwiftUI adapters and XCFramework build.<br><span class="badge success">3 Swift/C bridge tests passed</span>','Device UI, scrolling, selection, accessibility, memory and frame times'],
 ['Android','Native views, JNI, AAR, demo APK and instrumentation APK build.<br><span class="badge warning">3 device tests not run</span>','Instrumentation execution and native UI/performance on a device'],
])
status+='''<p class="small">Shared core: 23 Rust tests passed. These are the last recorded local checks; SDK builds are not device runtime tests.</p>
<hr><h3>Next work, in order</h3><ol>
<li><strong>Close measured performance gaps.</strong> Profile Wasm lexical/JSON overhead, keep full grammars opt-in, and evaluate skipping native PNG generation on the web math path.</li>
<li><strong>Validate native runtime behavior.</strong> Run the native tests and frame/memory checks on shared devices. Ace device controls are currently disabled.</li>
<li><strong>Connect the host application.</strong> Wire Ace file/thread/PR actions, authorized attachments, and transcript rendering to the new adapters.</li>
<li><strong>Complete delivery.</strong> Get CI results, review and merge PR #1, then package a versioned release if publishing is wanted.</li>
</ol><p class="small">PR #1 is open, ready for review and currently mergeable. GitHub reports no CI checks. Platform binaries are buildable locally; packages are not published.</p>'''
(root/'benchmarks/artifacts/project-status.html').write_text(doc(status))
body=f'<p class="snapshot">Measured on {html.escape(e["cpu"])} · macOS {html.escape(e["os"])} · Chromium {html.escape(b["browser_version"])}</p><p class="legend"><span class="markit">Markit</span><span>Existing baseline</span></p><div class="bench-grid">'
last=b['html'][-2:]
body+='<section><h3>Markdown → HTML · 131,400 bytes</h3>'+bars([('Markit Wasm',last[0]['median_ms'],True),('Marked 18',last[1]['median_ms'],False)])+f'<p><span class="badge success">{last[1]["median_ms"]/last[0]["median_ms"]:.1f}× faster on this workload</span></p><p class="small">Matched block text, headings, table rows and tasks. No code/math; no layout or painting.</p></section>'
lex=[r for r in b['highlighting'] if r['bytes']==16500 and r['category']=='lexical']
body+='<section><h3>TypeScript lexical highlighting · 16,500 bytes</h3>'+bars([('Markit Wasm + JSON',lex[0]['median_ms'],True),('Ace JS lexical',lex[1]['median_ms'],False)])+f'<p><span class="badge warning">Markit takes {lex[0]["median_ms"]/lex[1]["median_ms"]:.1f}× as long here</span></p><p class="small">Uncached source. Markit includes bridge/JSON and light/dark colors; Ace emits simpler token kinds.</p></section>'
last=n[-3:]
body+='<section><h3>Native preparation · 131,264 bytes</h3>'+bars([('Markit Rust/C/Swift',last[0]['median_ms'],True),('Ace Swift/Foundation',last[1]['median_ms'],False),('Foundation full',last[2]['median_ms'],False)])+f'<p><span class="badge success">{last[1]["median_ms"]/last[0]["median_ms"]:.1f}× faster than Ace preparation</span></p><p class="small">Actual Swift model code running on the Mac. Different presentation models; no phone UI or frame-rate claim.</p></section>'
grammar=[r for r in b['highlighting'] if r['bytes']==16500 and r['category']=='full_grammar']
body+='<section><h3>Full TypeScript grammars · 16,500 bytes</h3>'+table(['Path','Median'],[[html.escape(r['name']),f'{r["median_ms"]:.3f} ms'] for r in grammar])+'''<p><span class="badge warning">Performance work required</span></p><p class="small">Prism and Sublime grammars differ, but the current optional Wasm grammar path is far too costly for interactive use at these timings.</p></section></div><hr>'''
body+='<h3>All Markdown input sizes</h3>'+table(['Bytes','Markit Wasm','Marked'],[[f'{a["bytes"]:,}',f'{a["median_ms"]:.3f} ms',f'{c["median_ms"]:.3f} ms'] for a,c in zip(b['html'][::2],b['html'][1::2])])
body+='<h3>All native host input sizes</h3>'+table(['Bytes','Markit bridge','Ace Swift/Foundation','Foundation full'],[[f'{a["bytes"]:,}',f'{a["median_ms"]:.3f} ms',f'{c["median_ms"]:.3f} ms',f'{d["median_ms"]:.3f} ms'] for a,c,d in zip(n[::3],n[1::3],n[2::3])])
body+='<h3>Math preparation · different outputs</h3>'+table(['Formula','Markit new formula','KaTeX new formula','Markit repeated'],[[group,f'{rows[0]["median_ms"]:.4f} ms',f'{rows[1]["median_ms"]:.4f} ms',f'{rows[2]["median_ms"]:.4f} ms'] for group,rows in [(name,[r for r in m['math'] if r['formula']==name]) for name in ['simple','sum_fraction']]])
body+='<p class="small">Markit creates SVG and a native bitmap; KaTeX emits HTML + MathML and leaves browser layout for later. These are different workloads. Cached Markit timings must not be compared as uncached wins.</p>'
body+='<h3>Streaming append after 131,411 bytes</h3>'+table(['Output path','Median per append'],[[html.escape(r['name']),f'{r["median_ms"]*1000:.1f} µs'] for r in b['streaming']])
body+='<p class="small">History setup excluded. HTML patches and token patches do different work; no direct speedup claim.</p><hr><p class="small">Nine steady-state batches; values are medians of per-call batch averages. No Android on-device timing: the existing client uses plain Text and device controls are disabled. Cold-start, raw batch samples and methodology are preserved in the benchmark report.</p>'
(root/'benchmarks/artifacts/performance-comparison.html').write_text(doc(body))
