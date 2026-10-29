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

Results are applied at most once per animation frame (a macrotask when the page is hidden or has no `requestAnimationFrame`). Every patch finished since the last frame is folded into one DOM update; `render()` resolves after that update, with `superseded: true` for a source a newer one replaced in the same frame. The open last block keeps its element: its markup is morphed in place (text nodes change, unchanged nodes stay) and code fences append only lines from the first changed one. Settled blocks are never rebuilt. In a Chromium run streaming a 41 KB answer in 20-character deltas at 200 deltas/s (`MARKIT_BENCH=1 npx playwright test stream-bench`), DOM updates fell from 2,070 to 623 (one per frame), mutation records from 4,132 to 1,945, removed nodes from 2,044 to 197, and main-thread task time from 1,477 ms to 920 ms; settled blocks remounted 19 times before and never after.

`worker: 'inline'` runs the same Wasm and the same request handling (`render-core.js`) on the calling thread. It is chosen automatically where `Worker` does not exist, as in Node or jsdom unit tests. Pass the module as `wasm` (bytes or a `WebAssembly.Module`), e.g. `readFileSync(fileURLToPath(import.meta.resolve('@markit/web/pkg/markit_wasm_bg.wasm')))` in a test setup file; without it the engine fetches `markit_wasm_bg.wasm` beside its glue. Requests stay asynchronous and run in order. One inline engine serves the page or process.

### Code fences

`code.lineNumbersFrom` (default 1) shows the line-number gutter once a fence has that many lines; it appears on the same element as a streamed fence crosses the threshold. `code.collapseAfter` folds a fence more than two lines longer behind a "Show all N lines" / "Collapse" button (`aria-expanded`, `aria-controls`); the reader's choice survives streaming. Copy writes the full source and announces "Copied to clipboard" through a `role="status"` region. Diff and patch fences show `+added −removed` in the header, tint rows as `<ins>`/`<del>`, and number old and new lines from `@@ -a,b +c,d @@` hunk headers. A row's kind depends only on the rows before it (hunk counts decide whether `---` is a removed line or a file header), so streamed rows never change kind. Colors come from `--mk-added`, `--mk-removed`, `--mk-added-bg`, `--mk-removed-bg` and the existing `--mk-*` tokens.

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

## GitHub flavour

The default flavour renders agent prose: raw HTML is shown as text and nothing below applies. Choose the GitHub flavour for PR, issue and comment bodies:

| Surface | Selection |
| --- | --- |
| Rust | `render_html_with(source, Flavor::Github)`, `parse_with`, `render_document_json_with`, `render_document_binary_with`, `StreamRenderer::new_web().with_flavor(Flavor::Github)` (also `set_flavor`, which resets a stream whose flavour changes) |
| Wasm | `renderHtmlFlavor(source, 'github')`, `renderDocumentFlavor`, `MarkdownStream.webFlavor('github')` |
| Web | `new MarkitRenderer(element, { flavor: 'github', onTask, resolveMedia, renderDiagram })` |
| C | `markit_render_flavor(source, len, format, MARKIT_FLAVOR_GITHUB)`, `markit_render_binary_flavor`, `markit_stream_new_flavor` |
| Swift | `Markit.parse(source, flavor: .github)`, `MarkitBinaryPlan.parse(source, flavor: .github)`, `MarkitStream(flavor: .github)` |
| Kotlin | `Markit.parse(source, MarkitFlavor.GITHUB)`, `MarkitStream(MarkitFlavor.GITHUB)`, `MarkitView.flavor` |

What it adds, in HTML and in native spans:

- **Alerts.** `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]` and `[!CAUTION]` become `<div class="mk-alert mk-alert-note" data-markit-alert="note">` with a `<p class="mk-alert-title">Note</p>` first. Natively the quote keeps its `quote` span, the title is the first line (with a `strong` span) and an `alert` span (`alert: "note"`) covers it all.
- **Footnotes.** `[^label]` references to a defined note become `<sup class="mk-footnote-ref"><a data-markit-footnote="1">1</a></sup>`, numbered in order of first reference. Definitions are drawn after the last block in `<section class="mk-footnotes" data-markit-footnotes>`, each `<li data-markit-footnote-def="1">` ending with a `data-markit-footnote-back="1"` link. Unreferenced definitions are not drawn; undefined references stay text. Markit emits no `id`s, so several documents on one page never collide; the web renderer scrolls within its own element. Natively a reference is the text `[1]` with a `footnote_reference` span (`label`, `number`), and the notes follow a rule as a numbered list with `footnote_definition` spans.
- **Raw HTML.** An allowlist matching ace's forge renderer: `a`, `abbr`, `b`, `bdo`, `blockquote`, `br`, `caption`, `center`, `cite`, `code`, `dd`, `del`, `details`, `dfn`, `div`, `dl`, `dt`, `em`, `figcaption`, `figure`, `h1`-`h6`, `hr`, `i`, `img`, `ins`, `kbd`, `li`, `mark`, `ol`, `p`, `picture`, `pre`, `q`, `rp`, `rt`, `ruby`, `s`, `samp`, `small`, `source`, `span`, `strike`, `strong`, `sub`, `summary`, `sup`, `table`, `tbody`, `td`, `tfoot`, `th`, `thead`, `time`, `tr`, `tt`, `u`, `ul`, `var`, `video`, `wbr`. Kept attributes: `a` `href` (the link policy, judged after decoding character references) and `title`; `abbr` `title`; `bdo` `dir`; `details` `open`; `img` `alt`, `title`, `width`, `height`; `ol` `start`, `type`; `source` `media`, `type`; `td`/`th` `colspan`, `rowspan`; `time` `datetime`; `align` on `div`, `p`, `h1`-`h6`, `td`, `th`, `img`. Every tag is rebuilt from that list; any other tag (`script`, `style`, `iframe`, `form`, `svg`, `math`, ...) is shown as its source text, and `on*` handlers, `style`, `class`, `id` and `target` never survive. Comments are dropped. Images, picture sources and videos never load: `<img data-markit-src>`, `<source data-markit-srcset>` (the first http(s) or root-relative candidate) and `<video data-markit-src controls preload="none">` (its content is dropped) wait for the host. An element opened at the top level (`<details>`) holds the Markdown blocks after it until it closes, and an inline tag closes with its paragraph, so output is always well nested; a stream keeps such blocks in one rendered block. Natively formatting tags map onto existing kinds (`b` is `strong`, `kbd` is `code`, `a` is `link`) and an `html` span names the element (`kbd`, `sub`, `sup`, `mark`, `ins`, `u`, `details`, `summary`, ...); `img` is an `image` span, a `<picture>` dark-mode source adds a `dark_image` span over it, and `<video>` is the text `[video]` with a `video` span.
- **Task boxes.** Each box carries `data-markit-task="index"` and `data-markit-task-offset="offset"`, natively a `task` span (`index`, `offset`, `checked`). `offset` is the UTF-16 index of the character inside the box's `[ ]` in the Markdown source, the same mark ace's `taskMarks` places: the host flips `source[offset]` between `' '` and `'x'` (Rust `markit::with_task(source, offset, checked)` refuses a stale offset). Boxes in code, comments and footnotes are never placed. Web boxes are enabled only when `onTask({ index, offset, checked })` is set.
- **Forge references.** Outside code and links: `@user` and `@org/team` (`mention`, target `user` or `org/team`), `#123` and `owner/repo#123` (`issue`, target as written), and commit SHAs of 7 to 40 lowercase hex characters that mix digits and letters (or are a full SHA), plus `owner/repo@sha` (`commit`, target the full SHA or `owner/repo@sha`, shown shortened to 7 characters in `<code>`). The guards follow ace: no match inside a word, path, e-mail address, URL or entity. They are `<a role="link" class="mk-mention" data-markit-reference data-reference-kind>` delivered to `onReference({ kind, target })`, natively `reference` spans. Emoji shortcodes from GitHub's gemoji list (`:tada:`) become the emoji; unknown ones stay text.
- **Diagrams.** A closed ```` ```mermaid ```` fence is wrapped in `<div class="mk-diagram" data-markit-diagram="mermaid" data-markit-diagram-source="...">` with the highlighted code block inside as the fallback; natively a `diagram` span (`language`, `source`) covers the code block. An unclosed fence stays code, so a stream never asks for a half-written diagram. The web renderer calls `renderDiagram(language, source, signal)`: an element is inserted, a string is treated as SVG and shown as an `<img>` from a `data:` URL (so it cannot run script), and on `null` or an error the code block stays.

Streaming stays equal to a one-shot render after every append. A document that defines a footnote is reparsed from the start on each append (a later definition can number or link older text); blocks without footnote references are still reused, and other documents settle as usual.

## Limits

Native and Wasm streams accept at most 8 MiB of UTF-8 source. Native one-shot wrappers accept at most 16 MiB including code-request JSON. The default highlighter's limits are in [Syntax highlighting](#syntax-highlighting). Highlight and math caches have separate accounted budgets of 4 MiB and 128 entries per rendering thread; allocator overhead and caller-owned results are outside those budgets. Oversized results are not cached.

Math source is limited to 2,048 bytes, with layout dimensions bounded before bitmap allocation. Unsupported formulas display their original source. Web code fences over 400 lines use a 320 px viewport. Native code/table panes use at most a 320 point/dp viewport and recycle visible rows. Extremely wide native code lines have a bounded horizontal layout width; copying retains the full source.
