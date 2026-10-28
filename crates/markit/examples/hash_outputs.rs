fn mix(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash = (*hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
}
fn main() {
    let mut hash = 0xcbf29ce484222325u64;
    let fixtures = [
        ("const x = \"😀\";\n/* café */ fn()", "ts"),
        ("let x = r###\"😀\"###; 'a: loop {}", "rs"),
        ("SELECT Name FROM records WHERE x IS NOT NULL", "sql"),
        ("a = '\"hello' # café\n", "python"),
        ("<div title=\"😀\">hello</div>", "tsx"),
        ("😀 plain", "unknown-😀"),
        ("--- a\n+++ b\n-old\n+new\n", "diff"),
        ("fn main() {\n    let s = \"hi\";\n}\n", "rust"),
    ];
    for (source, language) in fixtures {
        mix(
            &mut hash,
            markit::highlight_json(source, language).as_bytes(),
        );
        let packed = markit::highlight_packed(source, language);
        for word in packed {
            mix(&mut hash, &word.to_le_bytes());
        }
        mix(
            &mut hash,
            serde_json::to_string(&markit::code_document(source, language))
                .unwrap()
                .as_bytes(),
        );
    }
    let markdown = [
        "# Hello 🦀\n\nA **bold** and *em* with `code` and https://example.com.\n\n- [x] one\n- [ ] two\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n```rust\nfn main() {\n    let x = 1;\n}\n```\n\n```diff\n--- a\n+++ b\n-old\n+new\n```\n\nMath $x^2$ and $$\\frac{1}{2}$$.\n\nSee `src/lib.rs`.\n",
        "```rust\nlet x = 1;\n```\n",
        "Para\n\n```python\nx = 1\n```\n\nAfter\n",
    ];
    for source in markdown {
        mix(&mut hash, markit::render_html(source).as_bytes());
        mix(&mut hash, markit::render_document_json(source).as_bytes());
        mix(&mut hash, &markit::render_document_binary(source).unwrap());
    }
    let mut stream = markit::StreamRenderer::new();
    let chat = "Hello **world**\n\n```rust\nfn a() {}\nfn b() {}\n```\n\nTail\n";
    for chunk in chat.as_bytes().chunks(3) {
        let patch = stream.append(std::str::from_utf8(chunk).unwrap(), false);
        mix(&mut hash, &serde_json::to_vec(&patch).unwrap());
    }
    println!("{hash:016x}");
}
