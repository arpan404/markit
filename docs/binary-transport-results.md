# Binary transport result

Verdict: KEEP. Base commit: 6e578a9 on perf/packed-color-scan-opt.
The requested worktree command failed because that branch was already checked
out in the main markit checkout. Work proceeded in /tmp/sol-bin detached at the
same commit. No commits or pushes were made.

## Release measurements

Apple Silicon macOS, release profile with thin LTO and one codegen unit. Composed
Markdown includes TS code, prose, a table and math. The benchmark performs
encode-only work on a shared pre-parsed Document, and separately measures full
rendering from source. All caches are warmed. Three process runs each took
three rounds of 31 samples after four warmup passes, rotating/interleaving JSON
encode, binary encode, JSON render and binary render. Samples batch 30, 4, or 1
calls at small, medium and large sizes respectively. Results below are medians
of the nine round medians. Raw CSVs preserve every round.

| Source bytes | JSON bytes | Binary bytes | JSON encode ms | Binary encode ms | JSON render ms | Binary render ms |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 4,158 | 69,840 | 28,243 | 0.048092 | 0.016147 | 0.145258 | 0.112437 |
| 65,538 | 1,125,260 | 432,173 | 0.757677 | 0.241802 | 2.345323 | 1.819271 |
| 524,304 | 9,164,074 | 3,451,224 | 6.278250 | 2.251167 | 21.138666 | 17.161334 |

Size reduction is 59.56%, 61.59%, and 62.34%. Median per-round paired encode
speedups are 2.95x, 3.15x, and 2.77x. The largest deviation of any paired encode
speedup from its size's median is 2.55%, 1.53%, and 4.62%. End-to-end speedups
are 1.29x, 1.29x, and 1.23x, with at most 1.65% relative variation.

The machine was busy with other work: one-minute load averages observed during
this task were 34.65 and 17.67. Absolute times in the second process run drifted
substantially, e.g. large binary encode medians of 3.07–3.25 ms rather than
2.21–2.33 ms in the first. They are not repeatable to ±5% across all nine rounds.
The third process run returned to 2.19–2.25 ms for large binary encode; all its
round medians are within about 5% of the first process run's medians. The paired
ratios stay within the requested ±5% band across all runs, including the slow
second run. This supports KEEP without hiding the noisy absolute results.

## Correctness and scope

- `cargo test --workspace`: green, 31 tests plus doc tests.
- `cargo test -p markit --features grammars --locked`: green.
- Field-for-field independent binary-to-JSON equality on TS, Rust, Python, SQL,
  HTML and unknown fences, plus composed math and table documents and empty input.
- Direct code tests include Unicode, CRLF, non-palette RGB, unusual color strings
  and full grammar parity. FFI tests check both exports and invalid arguments.
- Standalone compiled Swift check passes 50 owned-buffer round trips, raw PNG
  parity, UTF-16 ranges, palette IDs, block records, bounds and direct code requests.
- `cargo fmt --all -- --check` and `git diff --check`: green.
- Strict Clippy fails on an unchanged baseline lint at
  crates/markit/tests/features.rs:283 (`chunks_exact_to_as_chunks`). The line is
  present at HEAD. All-target/all-feature Clippy passes with only that lint allowed.

All JSON APIs and consumers remain intact. One shared Rust encoder provides
numeric style tags, fixed-width records, interned strings/blobs, lexical palette
IDs, grammar RGB pairs and raw PNG bytes. Two additive C exports use existing
buffer ownership/free and status conventions. Swift's reader is opt-in and
owns the original Rust allocation; existing UIKit rendering remains on JSON.
This benchmark makes no claim about native UI rendering or cold math/grammar
initialization. The internal Rust Document still stores math PNGs as base64;
the binary encoder decodes these before emitting raw deduplicated PNGs.

The saved diff includes implementation, tests, standalone Swift check, release
benchmark, byte spec and this report. The isolated worktree is removed after
saving artifacts. Applying the diff elsewhere is left to the owner.
