#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Baselines execute from the existing Ace checkout; its source is not modified.
export ACE_ROOT="${ACE_ROOT:-$(cd ../ace && pwd)}"
node benchmarks/prepare.mjs
./scripts/build-web.sh
cargo build -p markit-wasm --release --locked --features grammars --target wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/markit_wasm.wasm --target web --out-dir benchmarks/artifacts/grammars-web
node benchmarks/run-browser.mjs
BENCH_FUNCTION=runMathBench BENCH_OUTPUT=math.json node benchmarks/run-browser.mjs
cargo build -p markit-ffi --release --locked
swiftc -O -I platforms/ios/Sources/CMarkit/include benchmarks/compare-native.swift platforms/ios/Sources/Markit/Markit.swift "$ACE_ROOT/apps/ios/Sources/Models/Markdown.swift" target/release/libmarkit_ffi.a -o benchmarks/artifacts/native-comparison
benchmarks/artifacts/native-comparison benchmarks/artifacts/native-fixtures.json benchmarks/results/native-host.json
python3 benchmarks/write-report.py
