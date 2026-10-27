#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/package-notices.sh
sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ndk="${ANDROID_NDK_HOME:-$sdk/ndk/27.1.12297006}"
if [ ! -d "$ndk" ]; then
  for candidate in "$sdk"/ndk/*; do
    if [ -d "$candidate/toolchains/llvm" ]; then ndk="$candidate"; fi
  done
fi
case "$(uname -s)" in Darwin) host=darwin-x86_64;; Linux) host=linux-x86_64;; *) echo 'Set up Android NDK on macOS or Linux' >&2; exit 1;; esac
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk/toolchains/llvm/prebuilt/$host/bin/aarch64-linux-android23-clang"
export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$ndk/toolchains/llvm/prebuilt/$host/bin/x86_64-linux-android23-clang"
# Optional native grammar regexes compile C with the same NDK/API as Rust.
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export CC_x86_64_linux_android="$CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER"
export AR_aarch64_linux_android="$ndk/toolchains/llvm/prebuilt/$host/bin/llvm-ar"
export AR_x86_64_linux_android="$ndk/toolchains/llvm/prebuilt/$host/bin/llvm-ar"
# NDK r27 needs an explicit link alignment for modern 16 KiB-page devices.
for target in aarch64-linux-android x86_64-linux-android; do
  cargo rustc -p markit-ffi --release --locked --features "${MARKIT_FEATURES:-}" --target "$target" -- -C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-z,common-page-size=16384
done
mkdir -p platforms/android/markit/src/main/jniLibs/{arm64-v8a,x86_64}
cp target/aarch64-linux-android/release/libmarkit_ffi.so platforms/android/markit/src/main/jniLibs/arm64-v8a/
cp target/x86_64-linux-android/release/libmarkit_ffi.so platforms/android/markit/src/main/jniLibs/x86_64/
