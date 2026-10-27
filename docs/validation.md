# Validation

Local verification on MacBook Pro Rijan, 2026-10-27:

| Check | Result |
| --- | --- |
| Rust formatting | Pass |
| Clippy, workspace/all targets/all features, warnings denied | Pass |
| Rust workspace tests with optional grammars | 28 passed |
| Swift tests through the linked Rust C library | 3 passed |
| Playwright Chromium browser flows | 11 passed |
| iOS arm64 device and simulator libraries / XCFramework | Build passes |
| iOS SwiftUI/UIKit example, arm64 simulator SDK | Build passes |
| Android arm64-v8a and x86_64 JNI shared libraries | Build passes |
| Android AAR and example APK | Build passes |
| Android instrumentation test APK | Build passes; 3 tests not executed |
| Kotlin JNI declarations against exported Rust symbols | Names/signatures checked with javap and llvm-nm |
| Android ELF load-segment alignment | 16 KiB alignment checked with llvm-readelf |
| Hardware WebGPU flow, ordinary browser with no backend override | 1 passed locally; optional hardware suite |
| Web package contents | npm pack dry run |
| Desktop/390 px browser layout | Inspected; responsive overflow check passes |
| Ace browser console | No errors observed |

Optional-grammar iOS and Android cores compile. Default and optional core Rust tests pass. The optional native backend uses Oniguruma, with the iOS deployment target set to the Swift package minimum of 15.0 and the Android C compiler set to the same NDK/API as Rust. CI now checks both native feature sets.

The browser tests compare compact-worker tokens with the original token API, including Unicode, aliases, SQL case, plain fallback and the token cap. They also cover Wasm output, syntax colors, math, tables/tasks, reference activation, escaped HTML, blocked automatic image requests, streamed DOM reuse, late reference definitions, replacement/coalesced renders, 5,000-line code virtualization, complete source copying, host-supplied image bytes, and TeX copying.

Native device/simulator UI tests and Android instrumentation execution have not run. Ace's device controls are disabled in this session. SDK compilation and host bridge tests do not prove native scrolling, layout, accessibility, or performance on a device. No shared simulator was created, erased, or driven.

Android builds use both `max-page-size=16384` and `common-page-size=16384`, following [Android's NDK guidance](https://developer.android.com/guide/practices/page-sizes). ELF alignment is a packaging check, not a substitute for execution on a device with 16 KiB pages.

## Reproduce

Run from the repository root:

```sh
# Core and browser. Install the matching wasm-bindgen CLI first.
cd web && npm ci && npx playwright install chromium && cd ..
./scripts/check.sh

# Swift bridge execution on the host.
./scripts/package-notices.sh
cargo build -p markit-ffi --release --locked
MARKIT_SOURCE_BRIDGE=1 swift test --package-path platforms/ios -Xlinker "$PWD/target/release/libmarkit_ffi.a"

# iOS package and example build. Requires Xcode and xcodegen.
./scripts/build-ios.sh
xcodegen generate --spec examples/ios/project.yml
xcodebuild -project examples/ios/MarkitDemo.xcodeproj -scheme MarkitDemo -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' ARCHS=arm64 ONLY_ACTIVE_ARCH=YES CODE_SIGNING_ALLOWED=NO build

# Android packaging. Requires JDK 17, SDK 36 and an installed NDK.
./scripts/build-android.sh
platforms/android/gradlew -p platforms/android :markit:assembleDebug :markit:assembleDebugAndroidTest :demo:assembleDebug
```

To execute Android tests, first coordinate use of an existing shared device with the other active threads, then run `:markit:connectedDebugAndroidTest` with its ADB serial in `ANDROID_SERIAL`. Do not create a project-specific emulator. iOS simulator work must use the registered shared UDIDs without parallel test clones.

GitHub CI repeats core/browser tests and native packaging checks. Its status must be checked on the PR; local results above do not imply CI has passed.
