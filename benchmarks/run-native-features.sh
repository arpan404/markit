#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
baseline_ref="${MARKIT_BASELINE:-6bc21b6}"
task_baseline=$(mktemp -d /tmp/markit-baseline.XXXXXX)
git archive "$baseline_ref" | tar -x -C "$task_baseline"
cp benchmarks/native-features.rs "$task_baseline/benchmarks/native-features.rs"
cat >> "$task_baseline/crates/markit/Cargo.toml" <<'MANIFEST'

[[bench]]
name = "features"
path = "../../benchmarks/native-features.rs"
harness = false
MANIFEST
# Keep baseline outputs isolated. Sharing Cargo output directories across
# checkouts can reuse artifacts and invalidate the before/after comparison.
MARKIT_BENCH_REVISION="$baseline_ref" CARGO_TARGET_DIR="$task_baseline/target" \
  cargo bench --manifest-path "$task_baseline/Cargo.toml" -p markit --locked --features grammars --bench features -- "$PWD/benchmarks/results/native-features-before.json"
MARKIT_BENCH_REVISION="$(git rev-parse --short HEAD)+working-tree" \
  cargo bench -p markit --locked --features grammars --bench features -- "$PWD/benchmarks/results/native-features-after.json"
