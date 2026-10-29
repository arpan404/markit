use markit::{encode_document_binary, render_document_binary};
use serde_json::Value;
#[path = "fixtures/mkbp.rs"]
mod mkbp;
use mkbp::decode;

#[test]
fn six_language_fixtures_plus_math_and_tables_equal_every_json_field() {
    for (lang, code) in [
        ("ts", "const x: string = '🦀'; // hi\nconsole.log(x);"),
        (
            "rust",
            "fn main() { let x = r#\"hi\"#; println!(\"{}\", x); }",
        ),
        ("python", "def hello(x):\n    # note\n    return f'hi {x}'"),
        ("sql", "SELECT name, 42 FROM users WHERE id = 1; -- note"),
        ("html", "<div class=\"hi\">🦀 &amp; text</div>"),
        ("unknown", "plain 🦀 text\0 with no syntax"),
    ] {
        let source = format!("# Heading 🦀\n\n```{lang} file=src/demo\n{code}\n```\n\n**strong** *em* ~~strike~~ `inline` [link](https://example.test) ![alt](pic.png)\n\n> quote\n\n- item\n\n---\n\n[file](file:///tmp/a)\n\n```diff\n+add\n-remove\n```\n");
        let expected: Value = serde_json::from_str(&markit::render_document_json(&source)).unwrap();
        assert_eq!(
            decode(&render_document_binary(&source).unwrap()),
            expected,
            "{lang}"
        );
    }
    for source in [
        "Inline $x^2$ and $x^2$ 🦀.\n\n$$\\frac{a}{b}$$\n",
        "| left | right | empty |\n| :--- | ---: | :---: |\n| **🦀** | `x` | |\n| a | b | c |\n",
        "",
    ] {
        let expected: Value = serde_json::from_str(&markit::render_document_json(source)).unwrap();
        assert_eq!(decode(&render_document_binary(source).unwrap()), expected);
    }
}

#[test]
fn direct_code_preserves_unicode_and_rgb_and_noncanonical_colors() {
    let mut doc = markit::code_document("let 🦀 = 1;\r\n", "rust");
    for (light, dark) in [("#abcdef", "#012345"), ("#ABCDEF", "custom")] {
        doc.spans.push(markit::Span {
            start: 0,
            end: 1,
            style: markit::Style::Syntax {
                light: light.into(),
                dark: dark.into(),
            },
        });
    }
    assert_eq!(
        decode(&encode_document_binary(&doc).unwrap()),
        serde_json::to_value(&doc).unwrap()
    );
}

#[cfg(feature = "grammars")]
#[test]
fn full_grammar_colors_equal_json() {
    let doc = markit::code_document_grammar("fn main() { println!(\"🦀\"); }", "rust").unwrap();
    assert_eq!(
        decode(&encode_document_binary(&doc).unwrap()),
        serde_json::to_value(&doc).unwrap()
    );
}
