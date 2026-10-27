# Markit

A shared Rust Markdown renderer with UIKit/SwiftUI views for iOS, native Android views, and a WebAssembly worker for the web. Native clients use text, image, table, and list views. They do not run HTML in a WebView.

Includes CommonMark, tables, task lists, nested quotes/lists, syntax highlighting, code headers and copy controls, line numbers, diff fences, inline/display math, bare links, and host callbacks for app references and attachment images.

## Build

Install Rust, Node, and `wasm-bindgen-cli` matching the `wasm-bindgen` version in Cargo.lock. Native builds also need Xcode or Android SDK/NDK and JDK 17. Build scripts package third-party notices with the binaries.

```sh
rustup target add wasm32-unknown-unknown aarch64-apple-ios aarch64-apple-ios-sim aarch64-linux-android x86_64-linux-android
./scripts/build-web.sh
./scripts/build-ios.sh
./scripts/build-android.sh
```

The iOS script produces `platforms/ios/Artifacts/MarkitCore.xcframework` for arm64 devices and Apple Silicon simulators. Then add the local Swift package at `platforms/ios` to your app. Generate the example project with `xcodegen generate --spec examples/ios/project.yml`.

The Android script uses `ANDROID_HOME` and `ANDROID_NDK_HOME` when available, with an installed-NDK fallback. It packages arm64-v8a and x86_64 libraries into `platforms/android/markit/src/main/jniLibs`. Build the AAR and example APK with:

```sh
platforms/android/gradlew -p platforms/android :markit:assembleDebug :demo:assembleDebug
```

Generated binaries are local build artifacts, not checked into Git. The packages are not published to a registry.

## Web

```js
import { MarkitRenderer } from './web/index.js';
// Also load web/style.css.
const renderer = new MarkitRenderer(document.querySelector('#preview'), {
  onReference: ({ kind, target }) => openAppReference(kind, target),
  resolveImage: (url, alt, signal) => authorizedAttachmentBytes(url, signal),
});
await renderer.render('# Hello\n\n**Rendered by Rust.**');
await renderer.render(growingMessage, { finalized: false });
const code = await renderer.highlight(sourceFile, 'typescript');
// Call when the owning view unmounts.
renderer.destroy();
```

Serve the demo with `npm --prefix web run serve` after building Wasm, then open `http://127.0.0.1:4173`. The JavaScript wrapper has no production npm dependencies. Its worker loads the generated Wasm module. There is no JavaScript Markdown-parser fallback.

## iOS

```swift
import Markit

// SwiftUI, with its own scrolling viewport.
MarkitText(markdown, finalized: !streaming)

// UIKit, on the main actor.
let view = MarkitView()
view.onReference = { reference in openReference(reference) }
view.resolveImage = { url in authorizedDecodedImage(url) }
view.render(markdown, finalized: !streaming)

// These synchronous functions belong off the UI thread for large inputs.
let document = try Markit.parse(markdown)
let highlighted = try Markit.highlight(sourceFile, language: "swift")
```

## Android

```kotlin
import dev.markit.MarkitView

val view = MarkitView(context)
view.onReference = { reference -> openReference(reference) }
view.resolveImage = { url -> authorizedDecodedBitmap(url) }
view.render(markdown, finalized = !streaming) // UI thread
// Release the native stream when the screen is destroyed.
view.close()
```

Both native views render changed suffixes without rebuilding settled rows. Code lines and table rows use bounded native viewports with recycled cells. Large prose blocks split into smaller text rows. Complete source files can be displayed with `setDocument(Markit.highlight(source, language))` after highlighting off the UI thread.

## Highlighting, math, and app hooks

The default highlighter is a linear Rust tokenizer with light/dark colors and aliases for Ace's language families. It handles comments, quoted/raw/multiline strings, keywords, numbers, functions, types, properties, and markup. It is lexical coloring, not a language compiler or a full Prism replacement.

For fuller Sublime grammars, build with `MARKIT_FEATURES=grammars ./scripts/build-web.sh`, or the corresponding native script. Use web `highlight(source, language, { grammar: true })`, Swift `highlight(..., fullGrammar: true)`, or Kotlin `highlight(..., fullGrammar = true)`. Optional grammars are excluded from default binaries and can have substantial first-use cost. Markdown fences use the fast path.

Math supports `$...$`, `$$...$$`, `\(...\)`, and `\[...\]`. Rust lays out formulas with embedded STIX Two Math, emitting SVG paths for web and PNG attachments for native text. Copying restores the original TeX delimiters. Unsupported, invalid, oversized, or excessively deep formulas remain visible as source.

Reference hooks recognize file paths in inline code, explicit `file://` links, `ace://thread/...` / `thread://...` links, and GitHub pull-request links. Hosts own file opening, thread navigation, PR presentation, and attachment authorization. Remote images are not fetched automatically. Web image resolvers return Blob or Uint8Array data; native resolvers return decoded UIImage/Bitmap values.

See [feature coverage](docs/ace-feature-parity.md) and [integration details](docs/integration.md). Ace itself has not been modified by this repository.

## Performance and validation

The engine uses a streaming parser, half-open UTF-16 native ranges, bounded highlight/math caches, and append patches for settled blocks. Native input crosses C/JNI once per update. Web parsing and highlighting run in a worker with one active render and one queued replacement.

```sh
./scripts/check.sh
cargo bench -p markit --bench throughput
```

`check.sh` requires Playwright Chromium, installed with `cd web && npx playwright install chromium`. Swift bridge tests, Android instrumentation commands, build checks, and measured limits are documented in [validation](docs/validation.md) and [performance](docs/performance.md). Native simulator UI tests have not run in this session because Ace device controls are disabled. See [measured comparisons with Ace's existing libraries](docs/benchmark-comparison.md) for the initial baseline, and [the optimization report](docs/performance-optimization.md) for the faster range transport, native grammar backend, math paths, and remaining gaps.

For optional standard WebGPU code drawing, use `{ codeRendering: 'webgpu' }`. See [WebGPU measurements and efficiency limits](docs/webgpu.md). Automatic selection retains the text renderer until target-device measurements justify a GPU default.
