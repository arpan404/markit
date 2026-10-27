use markit::{parse, render_document_json, render_html, render_html_into, Document, Style};

#[test]
fn native_styles_use_utf16_ranges_after_markdown_is_removed() {
    let document = parse("# Hello 🦀\n\nA **bold 😀** and *italic* with `code`.\n");
    assert_eq!(document.text, "Hello 🦀\nA bold 😀 and italic with code.\n");
    let utf16: Vec<u16> = document.text.encode_utf16().collect();
    let strong = document
        .spans
        .iter()
        .find(|span| span.style == Style::Strong)
        .unwrap();
    assert_eq!(
        String::from_utf16(&utf16[strong.start..strong.end]).unwrap(),
        "bold 😀"
    );
    assert!(document
        .spans
        .iter()
        .any(|span| span.style == Style::Heading { level: 1 }));
    assert!(document
        .spans
        .iter()
        .all(|span| span.start < span.end && span.end <= utf16.len()));
    let decoded: Document = serde_json::from_str(&render_document_json("**😀**")).unwrap();
    assert_eq!(decoded, parse("**😀**"));
}

#[test]
fn tables_tasks_lists_and_code_keep_their_native_semantics() {
    let document = parse("3. third\n4. fourth\n\n- [x] done\n- [ ] open\n\n| Name | Value |\n| --- | --- |\n| A | B |\n\n```rust\nlet x = 1;\n```\n");
    assert!(document.text.contains("3. third\n4. fourth\n"));
    assert!(document.text.contains("☑ done"));
    assert!(document.text.contains("☐ open"));
    assert!(document.text.contains("Name\tValue\nA\tB\n"));
    assert!(document.spans.iter().any(|span| span.style
        == Style::CodeBlock {
            language: "rust".into()
        }));
    assert!(document
        .spans
        .iter()
        .any(|span| span.style == Style::TableHeader));
}

#[test]
fn reference_links_and_nested_inline_styles_resolve() {
    let document = parse("**bold *and italic*** [docs][ref]\n\n[ref]: https://example.com/docs\n");
    assert_eq!(document.text, "bold and italic docs\n");
    assert!(document.spans.iter().any(|s| s.style == Style::Strong));
    assert!(document.spans.iter().any(|s| s.style == Style::Emphasis));
    assert!(document.spans.iter().any(|s| s.style
        == Style::Link {
            url: "https://example.com/docs".into()
        }));
}

#[test]
fn raw_html_is_visible_text_and_unsafe_destinations_are_inert() {
    let source = "<script>alert('x')</script>\n\n[bad](javascript:alert%281%29) ![bad](data:image/svg+xml,x) [ok](https://example.com)\n";
    let html = render_html(source);
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("javascript:"));
    assert!(!html.contains("data:image"));
    assert!(html.contains("href=\"https://example.com\""));
    let document = parse(source);
    assert!(document.text.contains("<script>"));
    assert!(!document.spans.iter().any(|s| matches!(&s.style, Style::Link { url } | Style::Image { url } if url.starts_with("javascript:") || url.starts_with("data:"))));
}

#[test]
fn url_policy_rejects_browser_normalization_bypasses() {
    for unsafe_url in [
        "JaVaScRiPt:alert(1)",
        "java\tscript:x",
        "//evil.example",
        "\\evil.example",
        "vbscript:x",
        "file:///etc/passwd",
        "data:text/html,x",
    ] {
        assert!(!markit::is_safe_url(unsafe_url), "{unsafe_url:?}");
    }
    for safe_url in [
        "https://example.com",
        "HTTP://example.com",
        "mailto:a@example.com",
        "tel:+123",
        "/docs",
        "../image.png",
        "#section",
        "docs?q=a:b",
    ] {
        assert!(markit::is_safe_url(safe_url), "{safe_url:?}");
    }
}

#[test]
fn html_buffer_reuse_clears_previous_output_and_escapes_code() {
    let mut buffer = String::from("stale text");
    render_html_into("```html\n<script>&\n```", &mut buffer);
    assert!(buffer.contains("class=\"language-html\""));
    assert!(buffer.contains("&lt;"));
    assert!(buffer.contains("&amp;"));
    assert!(!buffer.contains("<script>"));
    assert!(!buffer.contains("stale text"));
    render_html_into("**next**", &mut buffer);
    assert_eq!(buffer, "<p><strong>next</strong></p>\n");
}

#[test]
fn incomplete_fences_and_unicode_are_safe_for_stream_snapshots() {
    let source = "# 😀\n\n- one\n  - two\n\n```rust\nfn main() {\n\n🦀";
    for end in (0..=source.len()).filter(|&end| source.is_char_boundary(end)) {
        let document = parse(&source[..end]);
        let text: Vec<_> = document.text.encode_utf16().collect();
        for span in &document.spans {
            assert!(span.start < span.end && span.end <= text.len());
            assert!(String::from_utf16(&text[span.start..span.end]).is_ok());
        }
        let _ = render_html(&source[..end]);
    }
    let document = parse(source);
    assert!(document.text.contains("fn main() {\n\n🦀"));
}
