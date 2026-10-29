# Markit

Shared Rust Markdown engine for iOS, Android, and web. Parses once, renders native text ranges or HTML. No WebView.

Renders CommonMark, tables, task lists, quotes, code fences with copy headers, diffs, `$`/`$$` math, and bare links. Syntax colors come from a built-in lexical tokenizer; fuller Sublime grammars are opt-in. Hosts handle file references, thread links, and image bytes.

An opt-in GitHub flavour renders PR, issue and comment bodies as GitHub does: footnotes, `> [!NOTE]` alerts, an allowlisted HTML subset (`<details>`, `<picture>`, `<kbd>`, ...) whose media the host resolves, task boxes that report their source offset, `@mentions`, `#123`, commit SHAs, emoji shortcodes and mermaid diagram hooks. The default flavour is unchanged: raw HTML stays text.

## Web

```js
import { MarkitRenderer } from './web/index.js';
const renderer = new MarkitRenderer(document.querySelector('#preview'), {
  onReference: ({ kind, target }) => openAppReference(kind, target),
  resolveImage: (url, alt, signal) => authorizedAttachmentBytes(url, signal),
});
await renderer.render('# Hello\n\n**Rendered by Rust.**');
await renderer.render(growingMessage, { finalized: false });
renderer.destroy();

// PR, issue and comment bodies.
const body = new MarkitRenderer(element, {
  flavor: 'github',
  onReference: ({ kind, target }) => openForge(kind, target), // mention, issue, commit too
  onTask: ({ offset, checked }) => save(source.slice(0, offset) + (checked ? 'x' : ' ') + source.slice(offset + 1)),
  resolveImage, resolveMedia: (url, kind, signal) => authorizedVideo(url, signal),
  renderDiagram: (language, source, signal) => drawMermaid(source, signal), // SVG string or element
});
```

Parsing and highlighting run in a worker (one active render, one queued). Serve the demo with `npm --prefix web run serve` after building Wasm.

## iOS

```swift
MarkitText(markdown, finalized: !streaming) // SwiftUI
let body = try Markit.parse(prBody, flavor: .github) // task, alert, diagram, ... spans

let view = MarkitView() // UIKit, main actor
view.onReference = { openReference($0) }
view.resolveImage = { authorizedDecodedImage($0) }
view.render(markdown, finalized: !streaming)
```

## Android

```kotlin
val view = MarkitView(context)
view.onReference = { openReference(it) }
view.resolveImage = { authorizedDecodedBitmap(it) }
view.flavor = MarkitFlavor.GITHUB // PR, issue and comment bodies
view.render(markdown, finalized = !streaming)
view.close()
```

Views patch settled rows in place and recycle code/table cells. Highlight large sources off the UI thread, then `setDocument(Markit.highlight(source, language))`.

## Performance

`cargo bench -p markit --bench throughput`, release profile, Apple Silicon:

| Op | 0.26 KB | 13 KB | 104 KB |
| --- | ---: | ---: | ---: |
| Parse | 0.003 ms | 0.145 ms | 1.268 ms |
| Parse + JSON | 0.005 ms | 0.229 ms | 2.727 ms |
| HTML | 0.003 ms | 0.128 ms | 1.058 ms |

13 KB, time per document (lower is better):

```
HTML           ████████████████ 0.128 ms
Parse          ██████████████████ 0.145 ms
Parse + JSON   ████████████████████████████ 0.229 ms
```

Streaming append after a 104 KB history: ~0.032 ms/update, 68 source bytes reconsidered. Math (6 new formulas): ~0.976 ms uncached, ~0.007 ms cached in HTML.

Against JS renderers (Node v26, same GFM fixture, Markit as release WASM):

| Renderer | 0.26 KB | 12.9 KB | 103 KB |
| --- | ---: | ---: | ---: |
| markit-wasm | 0.005 ms | 0.215 ms | 2.007 ms |
| marked 18.1.2 | 0.016 ms | 0.756 ms | 6.196 ms |
| markdown-it 15.0.3 | 0.025 ms | 1.153 ms | 9.888 ms |
| showdown 2.1.0 | 0.068 ms | 4.427 ms | 250.441 ms |

Markit's HTML includes syntax-colored code; the JS renderers emit plain code blocks. Showdown scales superlinearly on table-heavy input (3.6 → 9.7 → 47 → 250 ms at 50/100/200/400 sections).

## Build and test

```sh
rustup target add wasm32-unknown-unknown aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android x86_64-linux-android
./scripts/build-web.sh
./scripts/build-ios.sh
./scripts/build-android.sh
./scripts/check.sh
```

Details: [integration](docs/integration.md) (including the [GitHub flavour](docs/integration.md#github-flavour)), [binary transport](docs/binary-transport.md), [WebGPU](docs/webgpu.md).

## License

Apache-2.0. See [LICENSE](LICENSE). Third-party notices in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and `licenses/`.
