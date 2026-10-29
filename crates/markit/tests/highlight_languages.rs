//! Lexical colours for unknown languages and the lexical-only languages
//! file viewers open, checked by the colour each construct receives.

const COMMENT: &str = "#6e7781";
const STRING: &str = "#0a3069";
const NUMBER: &str = "#0550ae";
const PROPERTY: &str = "#0550ae";
const KEYWORD: &str = "#cf222e";
const FUNCTION: &str = "#8250df";
const TYPE: &str = "#953800";
const PLAIN: &str = "#24292f";

/// Light colour of the token that covers the first occurrence of `needle`.
fn colour(code: &str, language: &str, needle: &str) -> String {
    let highlighted = markit::highlight(code, language);
    assert_eq!(
        highlighted
            .tokens
            .iter()
            .map(|t| t.text.as_str())
            .collect::<String>(),
        code,
        "{language}: tokens must rebuild the source"
    );
    let at = code
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} not in source"));
    let mut start = 0;
    for token in &highlighted.tokens {
        let end = start + token.text.len();
        if start <= at && at + needle.len() <= end {
            return token.light.clone();
        }
        start = end;
    }
    panic!("{language}: {needle:?} spans several tokens");
}

fn check(language: &str, code: &str, expected: &[(&str, &str)]) {
    assert!(!markit::highlight(code, language).plain, "{language}");
    for (needle, tone) in expected {
        assert_eq!(
            colour(code, language, needle),
            *tone,
            "{language}: {needle:?} in {code:?}"
        );
    }
}

#[test]
fn unknown_languages_colour_comments_strings_numbers_and_common_keywords() {
    let code = "# hash note\n// slash note\n/* block\nnote */\n-- dash note\nlet total = \"sum\" + 42\nif ready then run(total) end\n";
    check(
        "made-up-language",
        code,
        &[
            ("hash note", COMMENT),
            ("slash note", COMMENT),
            ("block\nnote", COMMENT),
            ("dash note", COMMENT),
            ("\"sum\"", STRING),
            ("42", NUMBER),
            ("let", KEYWORD),
            ("then", KEYWORD),
            ("run", FUNCTION),
        ],
    );
}

#[test]
fn unknown_languages_do_not_turn_prose_punctuation_into_strings_or_comments() {
    let code = "don't stop, a#b and x-- y\nit's \"open quote\nnext line\n";
    check(
        "notes",
        code,
        &[
            ("stop", PLAIN),
            ("a#b", PLAIN),
            ("x-- y", PLAIN),
            ("\"open quote", STRING),
            ("next line", PLAIN),
        ],
    );
}

#[test]
fn explicitly_plain_languages_and_missing_languages_stay_plain() {
    for language in ["", "text", "TXT", "plaintext"] {
        let code = "# not a comment \"nor a string\" 42\n";
        let highlighted = markit::highlight(code, language);
        assert!(highlighted.plain, "{language:?}");
        assert_eq!(highlighted.tokens.len(), 1);
    }
}

/// Language, source, and the colour expected for each needle.
type Case = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

#[test]
fn file_viewer_languages_colour_their_comments_strings_and_keywords() {
    let cases: &[Case] = &[
        (
            "toml",
            "[package]\nname = \"markit\" # crate\nlicense = '''Apache\n2.0'''\npublish = false\n",
            &[
                ("name", PROPERTY),
                ("\"markit\"", STRING),
                ("crate", COMMENT),
                ("Apache\n2.0", STRING),
                ("false", KEYWORD),
            ],
        ),
        (
            "ini",
            "; settings\n[server]\nport = 8080\nname = it's here # note\n",
            &[
                ("settings", COMMENT),
                ("port", PROPERTY),
                ("8080", NUMBER),
                ("here", PLAIN),
                ("note", COMMENT),
            ],
        ),
        (
            "rb",
            "# greet\ndef greet(name)\n  puts \"hi #{name}\"\nend\n",
            &[
                ("# greet", COMMENT),
                ("def", KEYWORD),
                ("\"hi #{name}\"", STRING),
                ("end", KEYWORD),
            ],
        ),
        (
            "php",
            "<?php\n# legacy\nfunction add($a) { return $this->b + 1; } // c\n",
            &[
                ("legacy", COMMENT),
                ("function", KEYWORD),
                ("add", FUNCTION),
                ("$this", KEYWORD),
                ("// c", COMMENT),
            ],
        ),
        (
            "lua",
            "-- note\nlocal s = [[long\nstring]] --[[ block\ncomment ]]\nreturn s\n",
            &[
                ("note", COMMENT),
                ("local", KEYWORD),
                ("long\nstring", STRING),
                ("block\ncomment", COMMENT),
                ("return", KEYWORD),
            ],
        ),
        (
            "Dockerfile",
            "# build\nFROM rust:1 AS build\nRUN echo don't add user\n",
            &[
                ("# build", COMMENT),
                ("FROM", KEYWORD),
                ("AS", KEYWORD),
                ("RUN", KEYWORD),
                ("add user", PLAIN),
            ],
        ),
        (
            "Makefile",
            "CC = clang # compiler\nifeq ($(CC),gcc)\nendif\n",
            &[
                ("CC", PROPERTY),
                ("compiler", COMMENT),
                ("ifeq", KEYWORD),
                ("endif", KEYWORD),
            ],
        ),
        (
            "graphql",
            "# q\nquery User($id: ID!) { user(id: $id) { name } }\n\"\"\"doc\nblock\"\"\"\n",
            &[
                ("# q", COMMENT),
                ("query", KEYWORD),
                ("User", FUNCTION),
                ("doc\nblock", STRING),
            ],
        ),
        (
            "scala",
            "/* a /* nested */ b */\nobject Main { def run(): Unit = println(\"\"\"hi\"\"\") }\n",
            &[
                ("b */", COMMENT),
                ("object", KEYWORD),
                ("def", KEYWORD),
                ("Unit", TYPE),
                ("\"\"\"hi\"\"\"", STRING),
            ],
        ),
        (
            "dart",
            "// c\nfinal greeting = '''multi\nline''';\nvoid main() => print(greeting);\n",
            &[
                ("// c", COMMENT),
                ("final", KEYWORD),
                ("multi\nline", STRING),
                ("void", KEYWORD),
                ("main", FUNCTION),
            ],
        ),
        (
            "exs",
            "# c\ndefmodule Hi do\n  def go, do: :ok\nend\n",
            &[
                ("# c", COMMENT),
                ("defmodule", KEYWORD),
                ("Hi", TYPE),
                ("end", KEYWORD),
            ],
        ),
        (
            "r",
            "# c\nsquare <- function(x) x^2\nif (TRUE) print(NULL)\n",
            &[
                ("# c", COMMENT),
                ("function", KEYWORD),
                ("TRUE", KEYWORD),
                ("print", FUNCTION),
            ],
        ),
    ];
    for (language, code, expected) in cases {
        check(language, code, expected);
    }
}
