# Third-party notices

Markit uses pulldown-cmark for CommonMark parsing, linkify for bare URLs, and LaTeX-Rust for mathematical layout and rendering.

The GitHub flavour's emoji shortcode table (`crates/markit/src/github/emoji_table.rs`) is generated from GitHub's gemoji list (MIT, Copyright (c) 2019 GitHub, Inc.) and Unicode emoji data (Unicode License v3) as published by the emojis crate; those license files are in `licenses/dependencies/emojis-0.9.1`. Rust dependency versions and checksums are recorded in Cargo.lock. Their upstream copyright and license notices remain applicable.

The default math renderer embeds STIX Two Math 2.13 under the SIL Open Font License 1.1. LaTeX-Rust's modified dvips color table uses the LaTeX Project Public License. The original notice, font license, modified table with its original header, and Rust-source licenses are included in `licenses/`. See `LaTeX-Rust-NOTICE.txt` for the upstream graphics bundle source and modification details.

The optional `grammars` build uses syntect and two-face. `licenses/optional-grammar-notices.md` preserves the bundled syntax/theme notices exported by two-face. Native grammar builds also use Oniguruma through onig/onig_sys; their license notices are in `licenses/Oniguruma-COPYING.txt`, `licenses/onig-LICENSE.md`, and `licenses/onig-sys-LICENSE.md`. WebAssembly uses the pure Rust regex backend. Those grammars and regex dependencies are absent from default builds.

Distribute these notices and the `licenses/` directory alongside packaged binaries. The build scripts copy them into the web package, Android assets, and the Swift package's resource bundle.
