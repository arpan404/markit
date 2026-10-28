#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/package-notices.sh
# Size-first profile; see `[profile.web]` in Cargo.toml.
cargo build -p markit-wasm --profile web --locked --features "${MARKIT_FEATURES:-}" --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/web/markit_wasm.wasm --target web --out-dir web/pkg
