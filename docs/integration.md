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

Native and Wasm streams accept at most 8 MiB of UTF-8 source. Native one-shot wrappers accept at most 16 MiB including code-request JSON. The default highlighter returns exact plain text for source over 1 MiB or token output over 8,192 runs. Highlight and math caches have separate accounted budgets of 4 MiB and 128 entries per rendering thread; allocator overhead and caller-owned results are outside those budgets. Oversized results are not cached.

Math source is limited to 2,048 bytes, with layout dimensions bounded before bitmap allocation. Unsupported formulas display their original source. Web code fences over 400 lines use a 320 px viewport. Native code/table panes use at most a 320 point/dp viewport and recycle visible rows. Extremely wide native code lines have a bounded horizontal layout width; copying retains the full source.
