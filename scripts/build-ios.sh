#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/package-notices.sh
# Match the Swift package minimum for Rust and optional C grammar dependencies.
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"
for target in aarch64-apple-ios aarch64-apple-ios-sim; do
  cargo build -p markit-ffi --release --locked --features "${MARKIT_FEATURES:-}" --target "$target"
done
artifact=platforms/ios/Artifacts/MarkitCore.xcframework
mkdir -p platforms/ios/Artifacts
if [ -d "$artifact" ]; then rm -rf "$artifact"; fi
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libmarkit_ffi.a -headers platforms/ios/Sources/CMarkit/include \
  -library target/aarch64-apple-ios-sim/release/libmarkit_ffi.a -headers platforms/ios/Sources/CMarkit/include \
  -output "$artifact"
