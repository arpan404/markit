#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
cargo test -p markit --features grammars --locked
./scripts/build-web.sh
npm --prefix web ci
npm --prefix web test
