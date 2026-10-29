# Integration details

## Native document format

Schema version 2 contains `text`, semantic `spans`, and top-level `blocks`. Positions are half-open UTF-16 code-unit ranges into rendered text. They match NSRange and Android String/Spannable indexing, including emoji. Table-cell spans also retain row/column indices, header status, and alignment. Math occupies one object-replacement character with PNG bytes, dimensions, and original TeX in its span.

`MarkitDocument.source(...)` restores original TeX when exporting rendered text. Unknown semantic styles can be ignored by a host. Consumers should check `version` before interpreting ranges. Rendering code source directly preserves its exact text, including newlines; Markdown parsing applies CommonMark newline normalization.

The C header specifies buffer ownership and errors. Every non-null result buffer is freed once with `markit_buffer_free`. Streams are opaque handles, owned until `markit_stream_free`, with one caller at a time. Swift serializes stream access. Android uses checked numeric handles backed by a registry, with a maximum of 64 live streams; close streams and views when no longer needed.

## Streaming

`StreamRenderer.append(delta, finalized)` returns `replace_from` and replacement blocks. Retain blocks before that index, remove the previous suffix, then append new blocks. Text before a settled root is not reparsed during ordinary appends. One or two trailing roots stay open until table/list/setext parsing is unambiguous. Display math keeps its region open until its closer arrives. Late reference definitions invalidate earlier output so references resolve correctly.

An unfinished single huge paragraph, list, fence, or formula remains an open root and can require work proportional to that root on each update. Reference-definition updates can require a full parse. Hosts should batch fast token streams to display cadence. The included views and web wrapper retain one queued replacement to avoid unbounded job growth.

Native streams return native documents; the Wasm stream returns HTML only. Public Rust `StreamRenderer::new()` returns both for integrations that need them. `new_native()` and `new_html()` avoid unnecessary output work.

## Host actions

Web `onReference`, UIKit `onReference`, and Android `onReference` receive a reference kind and target. Use the host's existing file, thread, and PR resolution code. Rendering never opens a local file itself. Web links retain normal safe navigation unless intercepted. Native links can be intercepted with `onLink`.

Image resolvers must return bytes the host has already authorized. Web supports PNG/JPEG/WebP/GIF Blob data or PNG Uint8Array data. Native callbacks return UIImage/Bitmap objects. The renderer performs no network requests for images. Abort signals and view disposal release pending web loads and Blob URLs.

### Web workers

Every `MarkitRenderer` created with the same `workerURL` shares one Web Worker and one Wasm instance, started on first use. Each renderer owns a stream inside that worker, and a render sends only the appended text when the source grew. `destroy()` rejects that renderer's pending requests and frees its stream without stopping the shared worker. If the worker fails or `terminateMarkitWorkers()` is called, the next request starts a new worker and renderers resend their source. Pass `worker: 'dedicated'` for the previous behavior: a private worker that `destroy()` terminates.

The worker and Wasm compile take a few tens of milliseconds the first time. Call `preloadMarkit({ workerURL })` (also exported from `@markit/web/react`) when the app starts, so the first message does not wait for them. In the Chromium benchmark, this cut the cold first message from 44 ms to 26 ms.

### React

`@markit/web/react` exports a component and a hook. React is an optional peer dependency; the core package has no runtime dependencies.

```jsx
import { Markit, useMarkit } from '@markit/web/react';
import '@markit/web/style.css';

// Declarative: re-render with longer text while streaming.
<Markit markdown={message.text} finalized={message.done} lazy onReference={openReference} />

// Imperative: stream tokens without putting the text in React state.
function LiveMessage({ source }) {
  const markit = useMarkit();
  useEffect(() => source.subscribe({ token: t => markit.append(t), done: () => markit.finish() }), [source, markit]);
  return <div ref={markit.ref} />;
}
```

The renderer is created when the element attaches and destroyed when it detaches, so StrictMode's double mount creates and frees a stream but never a worker; the latest text is replayed into the new renderer. Callback props are read when used, so new closures do not recreate anything. `<Markit>` is memoized, and each text change is one `render` call; the renderer keeps one job in flight and one queued, so fast token streams coalesce. `lazy` (or an IntersectionObserver `rootMargin` string) shows plain text until the element is near the viewport, then renders it once. Rendered blocks use `content-visibility: auto`. In SwiftUI use `MarkitText` as a scrolling region. In Android Compose embed MarkitView with AndroidView and close it in the owning disposal callback.

## Syntax highlighting

The built-in highlighter is lexical: one forward scan splits the source into contiguous colour runs (comments, strings, numbers, keywords, calls, types, properties, markup tags). Every output reads the same runs: tokens (`highlight`), packed UTF-16 ranges (`highlight_packed`, Wasm `highlightCodePacked`), native documents (`code_document`, `markit_code_binary`, `Markit.highlight`), and fences inside rendered Markdown.

Languages: TypeScript, TSX, JavaScript, JSX, Python, Bash, Rust, Go, Java, Kotlin, Swift, C, C++, C#, JSON, JSON5, YAML, Markdown, SQL, CSS, HTML/XML, diff, plus TOML, INI, Ruby, PHP, Lua, Dockerfile, Makefile, GraphQL, Scala, Dart, Elixir and R. Common file extensions and names are aliases (`kt`, `rb`, `ex`, `mk`, `Makefile`, `Dockerfile`, `cfg`, …). Any other non-empty language uses a generic lexer: `#`, `//`, `/* */` and `--` comments, quoted strings, numbers, a few universal keywords, and calls. In it `#` and `--` start comments only after whitespace, strings end at the line, and an apostrophe after a letter is text, so prose and logs do not turn into one long string. An empty language and `text`, `txt`, `plain` or `plaintext` stay plain.

Explicit highlight calls colour whole sources up to 8 MiB of UTF-8, with no run-count cliff; larger sources return exact plain text with `plain: true`. Packed output costs 12 bytes per run, and lexer runs 8 bytes per run while a result is alive; TypeScript averages about one run per 10 bytes, so a 4 MiB file is about 400k runs (≈4.7 MiB packed). To bound memory on adversarial input, a source stops colouring after 4,194,304 runs and the rest is one plain run. Large results are not memoized: results over 16,384 runs are rebuilt from runs on every call instead of keeping a token or packed copy, and only sources up to 1 MiB enter the 4 MiB per-thread cache.

Fences inside rendered Markdown keep a budget: a fence over 8,192 runs or 1 MiB renders as exact plain text in the HTML, JSON and native outputs (`render_html`, `parse`, `StreamRenderer::new`, `new_html`, `new_native`). Those stream modes rebuild a fence's whole output on each append, so an unbudgeted fence makes streaming much slower (490 KB TypeScript fence appended line by line, `examples/highlight_bench.rs`: JSON 2.9 s → 9.0 s, native MKNP 0.54 s → 7.0 s). The web stream (`MarkdownStream.web()`, used by `MarkitRenderer`) highlights fences with `highlightCodePacked` and so colours them in full; it already resends the fence on every append, and on that fence full colour took the web stream from 0.40 s to 1.9 s in total, about 0.12 ms more per append over 12k appends.

Timings for whole-file highlighting (ace's 61 KB `packages/fake-daemon/src/daemon.ts` repeated to size; median of cold calls on a 4-core Linux container; `cargo run --release -p markit --example highlight_bench -- file.ts` and `node web/bench/highlight.mjs file.ts`):

| Source | Runs | Native packed | Native document JSON | Wasm `highlightCodePacked` in Node | Prism `tokenize` in Node |
| --- | --- | --- | --- | --- | --- |
| 2 KB | 240 | 0.01 ms | 0.05 ms | 0.04 ms | 0.36 ms |
| 61 KB | 5,776 | 0.4 ms | 1.1 ms | 0.8 ms | 11 ms |
| 490 KB | 47,232 | 2.8 ms | 9.6 ms | 6.3 ms | 109 ms |
| 4 MB | 393,784 | 24 ms | 103 ms | 55 ms | 2,215 ms |

There is no line-window API. A line's colours depend on everything before it (an unclosed block comment or string changes all later lines), so colouring lines [a, b) of a file always needs a scan of the prefix, which already yields every run. Lexing is fast enough to do once per file: highlight the whole file once, keep the packed words, and render only the visible lines from them. The web package does this (`code-lines.js` binary-searches the packed runs for each visible row), and native hosts can do the same with `MarkitBinaryPlan`. When a file grows by appending (a tailing log), a call on the longer source rescans only from the last line that can still change, as long as the previous call was on its prefix and that chain started at 1 MiB or less.

## Limits

Native and Wasm streams accept at most 8 MiB of UTF-8 source. Native one-shot wrappers accept at most 16 MiB including code-request JSON. The default highlighter's limits are in [Syntax highlighting](#syntax-highlighting). Highlight and math caches have separate accounted budgets of 4 MiB and 128 entries per rendering thread; allocator overhead and caller-owned results are outside those budgets. Oversized results are not cached.

Math source is limited to 2,048 bytes, with layout dimensions bounded before bitmap allocation. Unsupported formulas display their original source. Web code fences over 400 lines use a 320 px viewport. Native code/table panes use at most a 320 point/dp viewport and recycle visible rows. Extremely wide native code lines have a bounded horizontal layout width; copying retains the full source.
