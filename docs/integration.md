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

## Limits

Native and Wasm streams accept at most 8 MiB of UTF-8 source. Native one-shot wrappers accept at most 16 MiB including code-request JSON. The default highlighter returns exact plain text for source over 1 MiB or token output over 8,192 runs. Highlight and math caches have separate accounted budgets of 4 MiB and 128 entries per rendering thread; allocator overhead and caller-owned results are outside those budgets. Oversized results are not cached.

Math source is limited to 2,048 bytes, with layout dimensions bounded before bitmap allocation. Unsupported formulas display their original source. Web code fences over 400 lines use a 320 px viewport. Native code/table panes use at most a 320 point/dp viewport and recycle visible rows. Extremely wide native code lines have a bounded horizontal layout width; copying retains the full source.
