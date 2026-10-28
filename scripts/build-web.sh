#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/package-notices.sh
# Size-first profile (see Cargo.toml). Math draws with an embedded core cut of
# STIX Two Math; the worker fetches the full font the first time a formula needs it.
cargo build -p markit-wasm --profile web --locked --features "${MARKIT_FEATURES:-}" --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/web/markit_wasm.wasm --target web --out-dir web/pkg
latex_rust="$(cargo metadata --format-version 1 --locked | python3 -c 'import json, sys; print(next(p["manifest_path"] for p in json.load(sys.stdin)["packages"] if p["name"] == "latex-rust").rsplit("/", 1)[0])')"
cp "$latex_rust/fonts/stix-two-math/STIXTwoMath-Regular.otf" web/pkg/
