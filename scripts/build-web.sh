#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/package-notices.sh
cargo build -p markit-wasm --release --locked --features "${MARKIT_FEATURES:-}" --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/markit_wasm.wasm --target web --out-dir web/pkg
