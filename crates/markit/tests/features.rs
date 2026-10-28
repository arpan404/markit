use markit::{
    fence_info, highlight, parse, render_html, render_math, RenderedBlock, StreamRenderer, Style,
};

#[test]
fn highlighting_preserves_source_and_supports_ace_languages() {
    for language in [
        "ts", "tsx", "js", "jsx", "py", "rust", "go", "java", "kotlin", "swift", "c", "cpp", "cs",
        "json", "json5", "yaml", "sh", "sql", "css", "html", "md",
    ] {
        let source = "// comment 😀\nconst value = \"hello\";\n";
        let result = highlight(source, language);
        assert_eq!(
            result
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<String>(),
            source,
            "{language}"
        );
        assert!(!result.plain, "missing grammar: {language}");
    }
    let code = highlight("const value = 42; // comment", "ts");
    assert!(code.tokens.len() > 2);
    assert!(code.tokens.iter().any(|token| token.light != token.dark));
}

#[test]
fn oversized_and_unknown_code_falls_back_without_losing_text() {
    for (source, language) in [
        ("x".repeat(1_100_000), "rust"),
        ("😀\n".into(), "unknown-markit-language"),
    ] {
        let highlighted = highlight(&source, language);
        assert!(highlighted.plain);
        assert_eq!(highlighted.tokens[0].text, source);
    }
}

#[test]
fn fast_highlighting_handles_multiline_strings_rust_raw_strings_and_unicode() {
    for (source, language) in [
        ("/* a /* nested */ b */\nlet x = r###\"😀\"###;\n", "rust"),
        ("# comment\nvalue = '''multi\nline'''\n", "python"),
        ("<div title=\"😀\">hello</div>", "tsx"),
    ] {
        let code = highlight(source, language);
        assert_eq!(
            code.tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<String>(),
            source
        );
        assert!(!code.plain);
        assert!(code.tokens.len() > 2);
    }
}

#[cfg(feature = "grammars")]
#[test]
fn optional_grammars_preserve_source_and_return_native_ranges() {
    let source = "const view = <div title=\"😀\" />;\n";
    let tokens = markit::highlight_grammar(source, "tsx").unwrap();
    assert_eq!(
        tokens
            .tokens
            .iter()
            .map(|token| token.text.as_str())
            .collect::<String>(),
        source
    );
    assert!(!tokens.plain);
    let document = markit::code_document_grammar(source, "tsx").unwrap();
    assert_eq!(document.text, source);
    assert!(document
        .spans
        .iter()
        .any(|span| matches!(span.style, Style::Syntax { .. })));
}

#[test]
fn fence_metadata_and_diff_lines_survive_the_native_bridge() {
    assert_eq!(
        fence_info("ts title=\"src/hello world.ts\"")
            .file
            .as_deref(),
        Some("src/hello world.ts")
    );
    assert_eq!(fence_info("src/main.rs").language, "rust");
    assert_eq!(fence_info("python:app.py").file.as_deref(), Some("app.py"));
    let document =
        parse("```diff file=src/main.rs\n--- a/main.rs\n+++ b/main.rs\n-old\n+new\n```\n");
    assert!(document.spans.iter().any(|span| matches!(&span.style, Style::CodeMetadata { file, diff: true } if file.as_deref() == Some("src/main.rs"))));
    assert!(document.spans.iter().any(|span| span.style
        == Style::Diff {
            change: "addition".into()
        }));
    assert!(document.spans.iter().any(|span| span.style
        == Style::Diff {
            change: "deletion".into()
        }));
    assert_eq!(document.blocks[0].kind, "code");
}

#[test]
fn native_fences_nested_in_lists_keep_their_own_copy_and_line_viewports() {
    let document =
        parse("- item\n\n  ```rust file=src/a.rs\n  fn main() {}\n  ```\n\n  continued text\n");
    let code = document
        .blocks
        .iter()
        .find(|block| block.kind == "code")
        .unwrap();
    let units: Vec<_> = document.text.encode_utf16().collect();
    assert_eq!(
        String::from_utf16(&units[code.start..code.end]).unwrap(),
        "fn main() {}\n"
    );
    assert!(document
        .blocks
        .iter()
        .any(|block| block.kind == "text" && block.start >= code.end));
}

#[test]
fn math_has_native_pixels_and_web_paths_with_source_copy_metadata() {
    let visual = render_math(r"\frac{x^2}{2}", true).unwrap();
    assert!(visual.svg.contains("<path"));
    assert!(visual.png.starts_with("iVBOR"));
    assert!(visual.width > 0 && visual.height > 0);
    let document = parse(r"Result **\(x^2\)** and $y+1$. $$\frac{1}{2}$$");
    assert_eq!(
        document
            .spans
            .iter()
            .filter(|s| matches!(s.style, Style::Math { .. }))
            .count(),
        3
    );
    let html = render_html(r"Result \(x^2\) and $y+1$.");
    assert_eq!(html.matches("role=\"math\"").count(), 2);
    assert!(html.contains("data-tex="));
}

#[test]
fn math_leaves_code_currency_escaped_dollars_and_invalid_tex_alone() {
    let document = parse("Pay $5 and $10. `$x$`\n\n```text\n\\(z\\)\n```\n\nUnfinished \\(x and invalid \\(\\unknowncommand{\\)\n");
    assert!(!document
        .spans
        .iter()
        .any(|s| matches!(s.style, Style::Math { .. })));
    assert!(document.text.contains("$5 and $10"));
    assert!(document.text.contains("$x$"));
    assert!(document.text.contains("\\(z\\)"));
    assert!(parse(r"Unfinished \(x").text.contains(r"\(x"));
    assert!(parse(r"Use \$x$ and \\(x\\)")
        .spans
        .iter()
        .all(|s| !matches!(s.style, Style::Math { .. })));
}

#[test]
fn adjacent_and_unclosed_math_never_leaks_internal_markers() {
    let document = parse(r"\(x\)\(y\)");
    assert_eq!(
        document
            .spans
            .iter()
            .filter(|span| matches!(span.style, Style::Math { .. }))
            .count(),
        2
    );
    assert!(!document.text.contains("MARKITMATH"));
    let source = r"\(".repeat(8000);
    let document = parse(&source);
    assert_eq!(document.text.trim_end(), source);
    assert!(!render_html(&source).contains("MARKITMATH"));
}

#[test]
fn math_does_not_rewrite_link_destinations_reference_definitions_or_image_alt_text() {
    let source = "[$x$](https://example.com/$a$) ![$b$](photo.png) [r][ref]\n\n[ref]: https://example.com/$v$\n";
    let html = render_html(source);
    assert!(html.contains("href=\"https://example.com/$a$\""));
    assert!(html.contains("href=\"https://example.com/$v$\""));
    assert!(html.contains("alt=\"$b$\""));
    assert!(!html.contains("MARKITMATH"));
    assert_eq!(
        parse(source)
            .spans
            .iter()
            .filter(|s| matches!(s.style, Style::Math { .. }))
            .count(),
        1
    );
}

#[test]
fn bare_links_are_linked_but_never_nested_in_explicit_links_or_code() {
    let html = render_html("See https://example.com, [https://label.example](https://target.example) and `https://code.example`.");
    assert_eq!(html.matches("<a ").count(), 2);
    let document = parse("https://example.com `https://code.example`");
    assert_eq!(
        document
            .spans
            .iter()
            .filter(|s| matches!(s.style, Style::Link { .. }))
            .count(),
        1
    );
}

#[test]
fn streaming_final_output_matches_whole_document_at_every_character_boundary() {
    for source in [
        "# Heading 😀\n\nParagraph **bold**.\n\n- one\n- two\n\nTail\n",
        "Setext\n===\n\n| A | B |\n| --- | --- |\n| x | y |\n\n```rust\nfn main() {}\n\n```\n",
        "[late][ref]\n\nAnother paragraph.\n\n[ref]: https://example.com\n  \"Title\"\n",
        "> quote\n> - one\n>\n> another\n\nDone\n",
        "Math \\(x^2\\).\n\n$$\\frac{1}{2}$$\n\nEnd.\n",
        "Before\n\n\\[\n\nx^2 + y^2\n\n\\]\n\nAfter\n",
        "Before\n\n$$\n\nx^2\n\n$$\n\nAfter\n",
    ] {
        let mut stream = StreamRenderer::new();
        let mut blocks: Vec<std::sync::Arc<RenderedBlock>> = Vec::new();
        for c in source.chars() {
            let patch = stream.append(&c.to_string(), false);
            blocks.truncate(patch.replace_from);
            blocks.extend(patch.blocks);
        }
        let patch = stream.append("", true);
        blocks.truncate(patch.replace_from);
        blocks.extend(patch.blocks);
        assert_eq!(
            blocks
                .iter()
                .map(|block| block.html.as_str())
                .collect::<String>(),
            render_html(source),
            "{source}"
        );
    }
}

#[test]
fn streaming_reuses_settled_blocks_and_reset_discards_old_source() {
    let mut stream = StreamRenderer::new();
    stream.append(
        &format!("{}\n\nOpen tail", "Settled paragraph.\n\n".repeat(100)),
        false,
    );
    let update = stream.append(" continues", false);
    assert!(update.parsed_bytes < 100);
    assert!(update.replace_from >= 100);
    stream.reset();
    let update = stream.append("New", true);
    assert_eq!(update.replace_from, 0);
    assert_eq!(stream.source(), "New");
}

#[test]
fn compact_highlighting_matches_owned_tokens_and_native_utf16_spans() {
    for (source, language) in [
        ("const x = \"😀\";\n/* café */ fn()", "ts"),
        ("let x = r###\"😀\"###; 'a: loop {}", "rs"),
        ("SELECT name FROM records WHERE x IS NOT NULL", "sql"),
        ("a = '\"hello' # café\n", "python"),
        ("<div title=\"😀\">hello</div>", "tsx"),
        ("😀", "unknown-😀"),
        ("", "unknown"),
        ("", "rust"),
    ] {
        let code = highlight(source, language);
        let packed = markit::highlight_packed(source, language);
        assert_eq!(packed[0], 1);
        assert_eq!(packed[1] != 0, code.plain);
        let language_end = 3 + packed[2] as usize;
        let language_units: Vec<_> = packed[3..language_end].iter().map(|&n| n as u16).collect();
        assert_eq!(String::from_utf16(&language_units).unwrap(), code.language);
        let mut offset = 0;
        // `as_chunks` is stable on current CI toolchains and unstable on 1.83.
        #[allow(unknown_lints)]
        #[allow(clippy::chunks_exact_to_as_chunks)]
        let runs: Vec<_> = packed[language_end..].chunks_exact(3).collect();
        assert_eq!(runs.len(), code.tokens.len());
        for (run, token) in runs.iter().zip(&code.tokens) {
            offset += token.text.encode_utf16().count();
            assert_eq!(run[0] as usize, offset);
            assert_eq!(format!("#{:06x}", run[1]), token.light);
            assert_eq!(format!("#{:06x}", run[2]), token.dark);
        }
        let native = markit::code_document(source, language);
        assert_eq!(native.text, source);
        assert_eq!(native.blocks[0].end, source.encode_utf16().count());
        let syntax: Vec<_> = native
            .spans
            .iter()
            .filter(|span| matches!(span.style, Style::Syntax { .. }))
            .collect();
        assert_eq!(syntax.len(), if code.plain { 0 } else { code.tokens.len() });
        let mut start = 0;
        for (span, token) in syntax.iter().zip(&code.tokens) {
            assert_eq!(span.start, start);
            start += token.text.encode_utf16().count();
            assert_eq!(span.end, start);
            assert_eq!(
                span.style,
                Style::Syntax {
                    light: token.light.clone().into(),
                    dark: token.dark.clone().into()
                }
            );
        }
    }
}

#[cfg(feature = "grammars")]
#[test]
fn shared_grammar_parse_keeps_both_theme_colors_identical() {
    use syntect::{easy::HighlightLines, util::LinesWithEndings};
    let syntaxes = two_face::syntax::extra_newlines();
    let themes = two_face::theme::extra();
    for (source, language) in [
        (
            "/* multi\nline 😀 */\nconst view = <div title=\"café\" />;\n",
            "tsx",
        ),
        (
            "/* nested /* inner */ outer */\nfn f<'a>(x: &'a str) { let s = r###\"😀\"###; }\n",
            "rust",
        ),
        (
            "x = '''multi\nline 😀'''\ndef f(x): return x + 42\n",
            "python",
        ),
    ] {
        let syntax = syntaxes.find_syntax_by_token(language).unwrap();
        let actual = markit::highlight_grammar(source, language).unwrap();
        assert!(!actual.plain);
        for (theme, dark) in [
            (two_face::theme::EmbeddedThemeName::InspiredGithub, false),
            (two_face::theme::EmbeddedThemeName::Dracula, true),
        ] {
            let mut reference = HighlightLines::new(syntax, themes.get(theme));
            let mut expected = Vec::new();
            for line in LinesWithEndings::from(source) {
                for (style, text) in reference.highlight_line(line, &syntaxes).unwrap() {
                    let c = style.foreground;
                    expected.extend(std::iter::repeat_n(
                        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b),
                        text.len(),
                    ));
                }
            }
            let colors: Vec<_> = actual
                .tokens
                .iter()
                .flat_map(|token| {
                    std::iter::repeat_n(
                        if dark {
                            token.dark.clone()
                        } else {
                            token.light.clone()
                        },
                        token.text.len(),
                    )
                })
                .collect();
            assert_eq!(colors, expected, "{language}, dark={dark}");
        }
    }
}
