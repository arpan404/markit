# latex-rust 2.1.1, vendored for Markit

Copied from the crates.io package `latex-rust` 2.1.1
(<https://github.com/jscarr64/LaTeX-Rust>, MIT OR Apache-2.0; see
`LICENSE-MIT`, `LICENSE-APACHE`, and `NOTICE`). Only `src/`, `data/`,
`fonts/`, the licenses, and the README and changelog are kept; tests, golden
files, and benches are not. The root `Cargo.toml` uses it through
`[patch.crates-io]`.

Markit changes, both with identical output:

1. `Cargo.toml`: `ttf-parser` is built without its default features, with only
   `std` and `opentype-layout`. latex-rust reads CFF outlines, `MATH`, and the
   `ssty` feature of `GSUB`; it never uses the Apple layout, variable font, or
   glyph name tables. This removes about 12.5 KB (gzip) from the web Wasm.
2. `src/font/mod.rs`: `MathFont::from_bytes` looks up `ssty` alternates only
   for glyphs that an `ssty` coverage table lists (`ssty_covered_glyph_ids`)
   instead of for all 6,760 glyph ids. Any other glyph has no alternate, so the
   table is the same. In the browser this cut the first formula's cost by
   several milliseconds.

Drop this copy once upstream has equivalent changes.
