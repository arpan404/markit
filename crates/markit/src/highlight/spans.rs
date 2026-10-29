//! Native document views of a highlighted source: owned spans and the
//! borrowed span walk the bridges serialize from.
use super::{lex, tone::utf16_len};

pub(crate) fn code_document(code: &str, language: &str) -> crate::Document {
    let lexed = lex(code, language);
    let mut spans = Vec::with_capacity(lexed.runs.len() + 1);
    let mut offset = 0;
    spans.push(crate::Span {
        start: 0,
        end: 0,
        style: crate::Style::CodeBlock {
            language: lexed.language.clone(),
        },
    });
    for (from, to, tone) in lexed.utf16_ends(code) {
        if !lexed.plain && to > from {
            let (light, dark) = tone.colors();
            spans.push(crate::Span {
                start: from,
                end: to,
                style: crate::Style::Syntax {
                    light: light.into(),
                    dark: dark.into(),
                },
            });
        }
        offset = to;
    }
    spans[0].end = offset;
    crate::Document {
        version: 2,
        text: code.into(),
        spans,
        blocks: vec![crate::Block {
            start: 0,
            end: offset,
            kind: "code".into(),
        }],
    }
}

/// Borrowed view of one [`code_document`](crate::code_document) span, without
/// the per-run owned [`Style::Syntax`](crate::Style::Syntax) strings. The
/// first emission is always the code-block span; syntax spans follow in
/// document order and are skipped when the source falls back to plain text.
/// Colors are `&'static str` palette entries, so streaming them (into JSON or
/// native ranges) never allocates per run. Spans are only valid for the
/// duration of the `emit` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowedCodeSpan<'a> {
    CodeBlock {
        start: usize,
        end: usize,
        language: &'a str,
    },
    Syntax {
        start: usize,
        end: usize,
        light: &'static str,
        dark: &'static str,
    },
}

/// Emit every [`code_document`](crate::code_document) span without building
/// owned tokens or [`Style::Syntax`](crate::Style::Syntax) strings. The two
/// [`String`] clones per run dominate native misses; the shared [`lex`] cache
/// entry already holds compact `(end, Tone)` runs, so this walks them exactly
/// like [`code_document`](crate::code_document) and borrows the rest.
pub fn for_each_code_span(code: &str, language: &str, mut emit: impl FnMut(BorrowedCodeSpan<'_>)) {
    let lexed = lex(code, language);
    let total = utf16_len(code);
    emit(BorrowedCodeSpan::CodeBlock {
        start: 0,
        end: total,
        language: &lexed.language,
    });
    if lexed.plain {
        return;
    }
    for (start, end, tone) in lexed.utf16_ends(code) {
        if end > start {
            let (light, dark) = tone.colors();
            emit(BorrowedCodeSpan::Syntax {
                start,
                end,
                light,
                dark,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::code_document_json;
    use super::*;

    #[test]
    fn borrowed_code_spans_match_owned_document() {
        for (source, language) in [
            (
                "export function greet(name: string): string {\n  // c 🦀\n  return `Hi ${name}!`;\n}\n",
                "ts",
            ),
            (
                "fn main() {\n    let x: u32 = 0xff; // hi\n    println!(\"r#{raw}#\");\n}\n",
                "rust",
            ),
            (
                "def f(x):\n    # comment\n    return \"\"\"triple 🦀\"\"\" + 1.5\n",
                "python",
            ),
            ("SELECT a FROM t WHERE x = 1 AND y = 'Z';\n", "sql"),
            ("<div class=\"a\"><!-- c --><p>hi</p></div>\n", "html"),
            ("just plain text here\n", "unknown-markit-language"),
            ("", "rust"),
            ("", "unknown-markit-language"),
        ] {
            let owned = code_document(source, language);
            assert_eq!(
                code_document_json(source, language),
                serde_json::to_string(&owned).expect("document serialization is infallible"),
                "{language}"
            );
            let mut rebuilt = Vec::with_capacity(owned.spans.len());
            for_each_code_span(source, language, |span| {
                rebuilt.push(match span {
                    BorrowedCodeSpan::CodeBlock {
                        start,
                        end,
                        language,
                    } => crate::Span {
                        start,
                        end,
                        style: crate::Style::CodeBlock {
                            language: language.into(),
                        },
                    },
                    BorrowedCodeSpan::Syntax {
                        start,
                        end,
                        light,
                        dark,
                    } => crate::Span {
                        start,
                        end,
                        style: crate::Style::Syntax {
                            light: light.into(),
                            dark: dark.into(),
                        },
                    },
                });
            });
            assert_eq!(rebuilt, owned.spans, "{language}");
        }
    }
}
