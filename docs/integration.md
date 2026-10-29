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

The renderer is created when the element attaches and destroyed when it detaches, so StrictMode's double mount creates and frees a stream but never a worker; the latest text is replayed into the new renderer. Callback props are read when used, so new closures do not recreate anything. `<Markit>` is memoized, and each text change is one `render` call; the renderer keeps one job in flight and one queued, so fast token streams coalesce. `lazy` (or an IntersectionObserver `rootMargin` string) shows plain text until the element is near the viewport, then renders it once. Rendered blocks use `content-visibility: auto`. In SwiftUI use `MarkitText` as a scrolling region, or `MarkitInline` for self-sizing rows (see [iOS](#ios)). In Android Compose embed MarkitView with AndroidView and close it in the owning disposal callback.

## iOS

The Swift package builds in the Swift 6 language mode on Swift 6 toolchains (`Package@swift-6.0.swift`) and in Swift 5 mode on 5.9/5.10. UI types are `@MainActor`; `MarkitDocument`, `MarkitTheme` and `MarkitStream` are `Sendable`.

### Inline rows

`MarkitInlineView` (UIKit) and `MarkitInline` (SwiftUI) size to their content at the offered width and never scroll vertically, so an answer can be one row of a transcript (LazyVStack, List, table or collection view cells). `MarkitView`/`MarkitText` remain the self-scrolling, virtualized option for one long document.

```swift
// SwiftUI: one row per assistant message.
MarkitInline(markdown: message.text, finalized: message.done, theme: AppTheme.markdown,
             isTextSelectable: !message.hasMenu,
             codeCollapse: MarkitCodeCollapse(collapseAbove: 22, visibleLines: 20),
             onLink: { url in openURL(url) },
             onReference: { reference in open(reference.kind, reference.target) },
             resolveImage: { url in await attachments.image(for: url) })

// UIKit: size the cell with height(for:) or sizeThatFits.
let view = MarkitInlineView(theme: AppTheme.markdown)
view.onContentHeightChange = { [weak self] _ in self?.relayoutRow() }
view.render(text, finalized: done)
```

- The first render of a source up to `synchronousRenderLimit` (64 KiB) is parsed on the main thread, so a row appears at its final height in its first layout pass. Later renders (streaming deltas) parse on a background queue with one job in flight and one queued.
- Each stream root maps to one or more block views. A delta replaces only the roots Rust reopened; views for settled roots, and their cached heights, are kept. Replaced views of the same kind update in place. Text roots longer than 3,000 UTF-16 units are split at line ends, so a long list re-lays out only its last piece.
- Code blocks have a header (file or language, Copy), unwrapped text that scrolls sideways, full-width diff tints and an optional "Show all N lines" collapse. Copy takes the whole block. Tables are label grids that scroll sideways. Display and inline math are image attachments read by VoiceOver as their TeX source, copied as source.
- Links go to `onLink` (or open http/https/mailto/tel when it is nil), references to `onReference`. Images appear only when `resolveImage` returns one; nothing is fetched.
- Dynamic Type and appearance changes restyle every block. Unknown block or span kinds render as plain text.
- On iOS 16+ `MarkitInline` sizes through `sizeThatFits`; on iOS 15 it applies the height the view reports after its first layout.

### Themes

`MarkitTheme` holds body, heading (levels 1–6), monospace, table and caption fonts (`MarkitFont`: a text style, a font scaled with `UIFontMetrics`, or a fixed font), text/secondary/link colours, code background, header and border, quote bar, table border and header, diff tints, the eight-class syntax palette (`MarkitColorPair` light/dark), indents and spacing. `MarkitTheme.standard` is the built-in look. Keep one theme value per appearance (for example a `static let`); views restyle only when the theme compares unequal.

### Attributed strings

For hosts that build their own views:

```swift
let text: NSAttributedString = document.attributedString(theme: theme, traits: view.traitCollection)
let block = document.attributedString(forBlockAt: 0, theme: theme, traits: traits)
let swiftUI: AttributedString = document.attributedText(theme: theme)        // for Text
let source: NSAttributedString = try Markit.highlightedText(source: file, language: "swift",
                                                            theme: theme, traits: traits)
```

References are links with `.markitReference`/`.markitReferenceKind` attributes; `MarkitReference(url:)` decodes the link. Math runs carry their source under `.markitRaw` (in `AttributedString` they read as source text, since `Text` cannot draw attachments). `highlightedText` uses the binary transport, so large source files avoid JSON decoding.

## Limits

Native and Wasm streams accept at most 8 MiB of UTF-8 source. Native one-shot wrappers accept at most 16 MiB including code-request JSON. The default highlighter returns exact plain text for source over 1 MiB or token output over 8,192 runs. Highlight and math caches have separate accounted budgets of 4 MiB and 128 entries per rendering thread; allocator overhead and caller-owned results are outside those budgets. Oversized results are not cached.

Math source is limited to 2,048 bytes, with layout dimensions bounded before bitmap allocation. Unsupported formulas display their original source. Web code fences over 400 lines use a 320 px viewport. Native code/table panes use at most a 320 point/dp viewport and recycle visible rows. Extremely wide native code lines have a bounded horizontal layout width; copying retains the full source.
