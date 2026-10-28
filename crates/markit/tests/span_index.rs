use markit::{code_document, parse, SpanIndex};

#[test]
fn indexed_ranges_preserve_scan_order_for_nested_unicode_and_empty_cells() {
    let fixtures = [
        "# 😀 **bold**\n\n> quote *inner*\n>\n> ```rust\n> let café = 2;\n> ```\n\n|a|b|\n|-|-|\n||😀|\n",
        "", "text\n\n---\n\nlast",
    ];
    for source in fixtures {
        let document = parse(source);
        let before = serde_json::to_string(&document).unwrap();
        let index = SpanIndex::new(&document);
        let end = document.text.encode_utf16().count();
        for from in 0..=end {
            for to in from..=end {
                let expected: Vec<_> = document
                    .spans
                    .iter()
                    .filter(|s| from < to && s.start < s.end && s.start < to && s.end > from)
                    .collect();
                assert_eq!(index.query(from..to), expected);
            }
        }
        for (i, block) in document.blocks.iter().enumerate() {
            assert_eq!(index.block(i), index.query(block.start..block.end));
        }
        assert_eq!(before, serde_json::to_string(&document).unwrap());
    }
}

#[test]
fn indexed_lines_include_enclosing_code_style_and_utf16_offsets() {
    let doc = code_document("let 😀 = 1;\n/* across\nlines */\n", "rust");
    let index = SpanIndex::new(&doc);
    let mut start = 0;
    for (i, line) in doc.text.split_inclusive('\n').enumerate() {
        let end = start + line.encode_utf16().count();
        let expected: Vec<_> = doc
            .spans
            .iter()
            .filter(|s| s.start < end && s.end > start)
            .collect();
        assert_eq!(index.line(i), expected);
        start = end;
    }
    assert!(index.line(3).is_empty());
    assert!(index.line(100).is_empty());
}
