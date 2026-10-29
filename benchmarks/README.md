# Comparisons against the existing Ace libraries

These benchmarks measure Markit's current engine against the actual libraries and model code present in the adjacent Ace checkout. They do not modify Ace. Raw results are checked into `results/`; downloaded/bundled code, compiled Swift executables, and Wasm artifacts are ignored.

## Reproduce

On the benchmark Mac, install Node, Bun, Rust, matching wasm-bindgen CLI, and Playwright Chromium from `web/`. The native comparison requires Swift/Foundation and the host Rust library.

```sh
ACE_ROOT=/path/to/ace ./benchmarks/run.sh
```

Run on an idle machine. The script executes browser and native measurements sequentially. Browser measurements use headless Chromium, not Node execution of browser libraries. A local server sets COOP/COEP headers for higher-resolution performance timing. No device, simulator, or Ace process is driven.

## What each measurement includes

| Group | Markit | Existing baseline | Qualification |
| --- | --- | --- | --- |
| Markdown HTML | Rust through Wasm, input/output conversion, HTML creation | Marked 18.0.14 HTML creation | Same trusted fixture; normalized block text, headings, table rows, and task counts checked before timing |
| Lexical highlighting | Rust tokenizer, Wasm crossings, JSON serialization and JS decoding; the range path instead transfers typed UTF-16 offsets and RGB values, then reconstructs identical tokens in JS | Ace's current JS lexical highlighter | Different token metadata; Markit includes light/dark colors, Ace emits token kinds. Both preserve exact source |
| Full grammars | Optional Rust/Sublime grammar engine plus Wasm and JSON | Prism 1.30.0 through Ace's actual colored-line wrapper | Different grammars; both paths return colored source. Grammar loading is outside steady-state timing |
| Native preparation | Rust parse, C bridge, serialization, Swift JSON decoding | Actual Ace Swift block parser with Foundation inline parsing | Same common-syntax input, different presentation data. Host macOS execution, not iPhone layout |
| Foundation reference | Same Markit native path | Foundation full AttributedString Markdown | Reference parser, not Ace's exact composition; output structures differ |
| Math | Markdown parsing, vector math layout, SVG path output; native PNG work is excluded from the HTML path | KaTeX 0.19.0 HTML plus MathML string creation | Different outputs and amount of work; browser layout/painting excluded, so no equivalent-output speedup claim |
| Streaming | Wasm HTML patches plus JSON decoding | Ace's actual BlockStream token patches | Same append/history; different patch contents, no speedup ratio |

The Markdown fixtures deliberately exclude code/math/raw HTML so those extra Markit features do not distort the HTML comparison. Marked's default HTML output is not sanitized; these inputs are trusted. This is not a comparison of complete secure application pipelines.

Code tokenization uses unique source suffixes to force Markit's content cache to miss. Reported code sizes identify the base source; each uncached invocation adds a short comment suffix. Repeated-file cache hits are reported separately. Full-grammar timing is never presented as equivalent to the simpler lexical path. Source preservation is checked before measurement.

Synchronous browser cases use a 50 ms warmup and nine batches lasting at least 70 ms. The async Prism path loads its grammar and completes a source-preservation call before its nine measured batches. Native cases warm up for 20 invocations and drain an autorelease pool per operation for every baseline. Results report the median of per-call batch averages. `p95_batch_ms` is the 95th percentile of those nine batch averages, not a production request-latency percentile. Streaming excludes history setup and averages 100 fresh-stream appends per batch. Results retain all batch values.

Ace browser sources are bundled with Bun without minification for this driver. Cold module/first-use measurements are single observations on localhost, not statistically sampled download/startup benchmarks. Native results are host Mac measurements. There is no Android on-device benchmark: the current Ace Android transcript uses plain Text, and device controls were disabled. Do not infer Android/iPhone FPS, memory, battery, or startup from these results.

## Current findings

See [the comparison report](../docs/benchmark-comparison.md). The measurements expose remaining work in Wasm lexical highlighting and especially optional full grammars. They do not establish that Markit wins every workload.

## Host feature before/after measurements

Run `./benchmarks/run-native-features.sh` to compare shared Rust preparation with commit `6bc21b6`. Set `MARKIT_BASELINE` to choose another compatible commit. The baseline is extracted into a temporary directory with its own Cargo output directory. Do not share build outputs across the two checkouts. The script leaves the temporary baseline available for inspection.

Each case runs nine batches of at least 40 ms after 16 warmup calls. Uncached cases cycle through 256 preallocated source strings, more than the 128-entry cache cap; cache-hit cases are separate. Results include the source byte count and all samples. These are host Rust measurements, without Swift/JNI decoding or mobile UI layout. `cargo run -p markit --release --features grammars --example profile_highlight -- [file]` separately times regex parsing and the two theme passes. Without a file it uses the embedded TypeScript fixture.

## Native app benchmarks

The opt-in release demo harness measures the real UIKit and RecyclerView bindings. It does not run Ace or establish device FPS. Use an otherwise idle host and run iOS and Android sequentially. Both apps load `benchmarks/mobile/fixtures.json`. The first observation remains in each array; report the median of entries 1–9. Bridge parsing shares caches, so those nine entries are warm process measurements.

Build iOS with a Rust toolchain containing `aarch64-apple-ios` and `aarch64-apple-ios-sim`, then generate the example with XcodeGen. For example, replace `IOS_DEVICE` with a simulator UUID:

```sh
RUSTC="$(rustup which rustc)" ./scripts/build-ios.sh
xcodegen generate --spec examples/ios/project.yml
xcodebuild -project examples/ios/MarkitDemo.xcodeproj -scheme MarkitDemo \
  -configuration Release -destination "id=$IOS_DEVICE" ARCHS=arm64 \
  ONLY_ACTIVE_ARCH=YES -derivedDataPath /tmp/markit-ios-derived \
  CODE_SIGNING_ALLOWED=NO build
xcrun simctl install "$IOS_DEVICE" /tmp/markit-ios-derived/Build/Products/Release-iphonesimulator/MarkitDemo.app
python3 benchmarks/run-mobile.py ios --device "$IOS_DEVICE" --out benchmarks/results/ios-simulator-after.json
```

To run the UIKit regression and attributed-output parity tests, use the same `xcodebuild` invocation with `ENABLE_TESTABILITY=YES test` in place of `build`.

For Android, install Rust Android targets and NDK r27 or newer. The demo's release build uses the local debug signing key for installation; Java/Kotlin and Rust remain release builds, without a debugger attached. Set `ANDROID_HOME` and Java 17 as required locally:

```sh
RUSTC="$(rustup which rustc)" ./scripts/build-android.sh
(cd platforms/android && ./gradlew :demo:assembleRelease)
adb -s "$ANDROID_DEVICE" install -r examples/android/build/outputs/apk/release/demo-release.apk
python3 benchmarks/run-mobile.py android --device "$ANDROID_DEVICE" --out benchmarks/results/android-emulator.json
(cd platforms/android && ./gradlew :markit:connectedDebugAndroidTest)
```

The runner launches the installed app with benchmark arguments and collects its output. The iOS JSON is in the app's Documents directory; Android emits one JSON logcat record per fixture. The app must finish within 60 seconds. App installation, startup and benchmark output I/O are outside the measured intervals. No personal app data is cleared.

Viewport samples synchronously construct/layout a new renderer from a decoded document. Scroll samples move the outer document list to its end and request layout; they exclude presentation and nested code-pane scrolling. The code5000 fixture exceeds the highlighting cap and exercises exact plain-text fallback. No physical-device, frame-time, memory, battery, or equivalent Ace native UI comparison is claimed. See the report for device identifiers, raw samples and known limitations.
