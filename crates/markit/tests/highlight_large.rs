//! Explicit highlight calls colour whole files; Markdown fences keep their
//! smaller plain-text budget.

use markit::Style;

const COMMENT: u32 = 0x6e7781;
const KEYWORD: u32 = 0xcf222e;

/// About 480 KB of TypeScript with roughly 100k lexical runs, ending in a
/// comment so the very end of the file has a distinctive colour.
fn large_typescript() -> String {
    let mut code = String::new();
    for i in 0..6000 {
        code.push_str(&format!(
            "export function item{i}(value: number): string {{ return `${{value}}-{i}` + \"é\"; }}\n"
        ));
    }
    code.push_str("// end of file 🦀\n");
    code
}

/// (UTF-16 end, light colour) of each packed run.
fn packed_runs(words: &[u32]) -> Vec<(u32, u32)> {
    let first = 3 + words[2] as usize;
    words[first..]
        .chunks_exact(3)
        .map(|run| (run[0], run[1]))
        .collect()
}

#[test]
fn explicit_highlight_colours_a_file_far_past_eight_thousand_runs() {
    let code = large_typescript();
    let words = markit::highlight_packed(&code, "ts");
    assert_eq!(words[1], 0, "not plain");
    let runs = packed_runs(&words);
    assert!(runs.len() > 50_000, "{} runs", runs.len());
    let utf16 = code.encode_utf16().count() as u32;
    assert_eq!(runs.last().unwrap().0, utf16);
    // The last line is still a comment and the first word still a keyword.
    let comment_start = code[..code.rfind("// end").unwrap()].encode_utf16().count() as u32;
    let last_comment = runs.iter().find(|run| run.0 > comment_start).unwrap();
    assert_eq!(last_comment.1, COMMENT);
    assert_eq!(runs[0], (6, KEYWORD));

    let tokens = markit::highlight(&code, "ts");
    assert!(!tokens.plain);
    assert_eq!(tokens.tokens.len(), runs.len());
    assert_eq!(
        tokens
            .tokens
            .iter()
            .map(|t| t.text.as_str())
            .collect::<String>(),
        code
    );
    let document = markit::code_document(&code, "ts");
    let syntax = document
        .spans
        .iter()
        .filter(|span| matches!(span.style, Style::Syntax { .. }))
        .count();
    assert_eq!(syntax, runs.len());
    let json: serde_json::Value =
        serde_json::from_str(&markit::highlight_json(&code, "ts")).unwrap();
    assert_eq!(json["tokens"].as_array().unwrap().len(), runs.len());
}

#[test]
fn markdown_fences_over_the_run_budget_render_plain_while_explicit_calls_stay_coloured() {
    let code = large_typescript();
    let markdown = format!("Intro\n\n```ts\n{code}```\n");
    // Explicit, then fence, then explicit again: the shared lexer cache must
    // not leak one view's fallback into the other.
    assert_eq!(markit::highlight_packed(&code, "ts")[1], 0);
    let document = markit::parse(&markdown);
    assert!(!document
        .spans
        .iter()
        .any(|span| matches!(span.style, Style::Syntax { .. })));
    assert!(!markit::render_html(&markdown).contains("--mk-light:#cf222e"));
    assert_eq!(markit::highlight_packed(&code, "ts")[1], 0);
    // A small fence in the same document is still coloured.
    let small = markit::parse("```ts\nconst x = 1;\n```\n");
    assert!(small
        .spans
        .iter()
        .any(|span| matches!(span.style, Style::Syntax { .. })));
}

#[test]
fn growing_a_large_explicit_source_matches_a_cold_highlight() {
    let code = large_typescript();
    let lines: Vec<usize> = code.match_indices('\n').map(|(at, _)| at + 1).collect();
    // Grow by 500-line steps, as a viewer tailing a log or a streamed file.
    for &end in lines.iter().step_by(500).chain(lines.last()) {
        let prefix = &code[..end];
        let grown = markit::highlight_packed(prefix, "typescript");
        let prefix_owned = prefix.to_owned();
        let cold =
            std::thread::spawn(move || markit::highlight_packed(&prefix_owned, "typescript"))
                .join()
                .unwrap();
        assert_eq!(grown, cold, "at {end}");
    }
}
