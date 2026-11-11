#!/usr/bin/env bash
# Builds markit's prebuilt artifacts into dist/v<version>/ and, with --publish, publishes them as
# GitHub release v<version>, so apps embedding markit download them instead of building Rust.
#
#   scripts/release.sh            # build and check only
#   scripts/release.sh --publish  # then create the release (needs gh, signed in)
#
# Artifacts:
#   markit-web-<version>.tgz   the @markit/web npm package (renderer, worker, Wasm, types, licences)
#   markit-ios-<version>.zip   the Swift package (platforms/ios) with MarkitCore.xcframework built
#   SHA256SUMS                 checksums of both
#
# The version is Cargo.toml's, and web/package.json must match. The tree must be clean, so a
# release is exactly a commit. Toolchains are named, not taken from PATH: WEB_TOOLCHAIN (default
# 1.95.0) needs the wasm32 target and wasm-bindgen-cli at Cargo.lock's version; IOS_TOOLCHAIN
# (default 1.91.1) needs the iOS targets, and stays older because Xcode's linker can't read
# objects from a newer LLVM.
set -euo pipefail
cd "$(dirname "$0")/.."

publish=false
[ "${1:-}" = "--publish" ] && publish=true

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
web_version=$(node -p 'require("./web/package.json").version')
if [ -z "$version" ] || [ "$version" != "$web_version" ]; then
  echo "Cargo.toml ($version) and web/package.json ($web_version) must name the same version" >&2
  exit 1
fi
if ! git diff --quiet HEAD || [ -n "$(git ls-files --others --exclude-standard)" ]; then
  echo "Commit or remove local changes first: a release is exactly one commit" >&2
  exit 1
fi

toolchain_path() {
  local cargo
  cargo=$(rustup which --toolchain "$1" cargo) || {
    echo "Rust $1 is not installed (rustup toolchain install $1)" >&2
    exit 1
  }
  dirname "$cargo"
}
web_bin=$(toolchain_path "${WEB_TOOLCHAIN:-1.95.0}")
ios_bin=$(toolchain_path "${IOS_TOOLCHAIN:-1.91.1}")

out="dist/v$version"
rm -rf "$out"
mkdir -p "$out"

echo "== web (Rust $("$web_bin/rustc" --version))"
PATH="$web_bin:$PATH" ./scripts/build-web.sh
(cd web && npm pack --silent --pack-destination "../$out" >/dev/null)
web_archive="$out/markit-web-$version.tgz"
[ -f "$web_archive" ] || { echo "npm pack did not write $web_archive" >&2; exit 1; }
# Listed once and searched afterwards: under pipefail, `grep -q` closing a pipe early would fail it.
web_files=$(tar -tzf "$web_archive")
for required in package/pkg/markit_wasm_bg.wasm package/pkg/markit_wasm.js package/worker.js \
  package/licenses/THIRD_PARTY_NOTICES.md package/react.js package/index.d.ts; do
  grep -qx "$required" <<<"$web_files" || {
    echo "$web_archive is missing $required" >&2
    exit 1
  }
done

if [ "$(uname -s)" = "Darwin" ]; then
  echo "== iOS (Rust $("$ios_bin/rustc" --version))"
  PATH="$ios_bin:$PATH" ./scripts/build-ios.sh
  ios_archive="$out/markit-ios-$version.zip"
  # The Swift package as SwiftPM reads it, with the core and licence notices it builds.
  (cd platforms && zip -qry "../$ios_archive" ios -x 'ios/.build/*' 'ios/.swiftpm/*')
  ios_files=$(unzip -l "$ios_archive")
  grep -q "ios/Artifacts/MarkitCore.xcframework/Info.plist" <<<"$ios_files" || {
    echo "$ios_archive is missing MarkitCore.xcframework" >&2
    exit 1
  }
else
  echo "Skipping iOS: it builds on macOS only" >&2
fi

(cd "$out" && shasum -a 256 markit-* > SHA256SUMS)
cat "$out/SHA256SUMS"

if $publish; then
  tag="v$version"
  if gh release view "$tag" >/dev/null 2>&1; then
    echo "Release $tag already exists; bump the version first" >&2
    exit 1
  fi
  gh release create "$tag" "$out"/markit-* "$out/SHA256SUMS" \
    --target "$(git rev-parse HEAD)" \
    --title "markit $version" \
    --notes "Prebuilt markit $version from $(git rev-parse --short HEAD): the @markit/web package (renderer, worker, Wasm, types and licences) and the iOS Swift package with MarkitCore.xcframework. Checksums are in SHA256SUMS."
fi
