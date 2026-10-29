#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/package-notices.sh
# Size-first profile (see Cargo.toml). Math draws with an embedded core cut of
# STIX Two Math; the worker fetches a glyph chunk the first time a formula needs it.
cargo build -p markit-wasm --profile web --locked --features "${MARKIT_FEATURES:-}" --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/web/markit_wasm.wasm --target web --out-dir web/pkg
rm -f web/pkg/STIXTwoMath-Regular.otf web/pkg/math-glyphs-*.bin
cp crates/markit/assets/math-glyphs/math-glyphs-*.bin web/pkg/
