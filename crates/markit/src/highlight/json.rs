//! JSON writers that stream compact runs straight into serde, byte-identical
//! to serializing the owned token and document structs.
use super::{lex, tone::utf16_len, LexedCode};

/// Serialize `{"version","text","spans","blocks"}` directly from compact
/// runs, without materializing a [`Document`](crate::Document). The native
/// `code_document` bridge needs JSON text, not span structs; building them
/// first costs two owned [`String`]s per run plus the document, block-kind
/// and text clones, then a second pass to re-serialize. This emits
/// byte-identical JSON: field order matches the derived `Serialize` impls
/// (`Span` fields, then the flattened internally-tagged `Style`), and
/// `&str`/`&'static str` serialize exactly like the owned [`String`]s they
/// replace.
pub fn code_document_json(code: &str, language: &str) -> String {
    serde_json::to_string(&CodeDocumentJson {
        code,
        lexed: &lex(code, language),
    })
    .expect("code document serialization is infallible")
}

struct CodeDocumentJson<'a> {
    code: &'a str,
    lexed: &'a LexedCode,
}

impl serde::Serialize for CodeDocumentJson<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = serializer.serialize_struct("Document", 4)?;
        out.serialize_field("version", &2u32)?;
        out.serialize_field("text", &self.code)?;
        out.serialize_field("spans", &DocumentSpans { doc: self })?;
        out.serialize_field("blocks", &DocumentBlocks { doc: self })?;
        out.end()
    }
}

struct DocumentSpans<'a> {
    doc: &'a CodeDocumentJson<'a>,
}

impl serde::Serialize for DocumentSpans<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let runs = self.doc.lexed.runs.len();
        let mut seq = serializer.serialize_seq(Some(runs + 1))?;
        let total = utf16_len(self.doc.code);
        // Same shape and field order as `Span` with a flattened `CodeBlock`.
        seq.serialize_element(&BorrowedCodeBlockSpan {
            start: 0,
            end: total,
            kind: "code_block",
            language: &self.doc.lexed.language,
        })?;
        if !self.doc.lexed.plain {
            for (start, end, tone) in self.doc.lexed.utf16_ends(self.doc.code) {
                if end > start {
                    let (light, dark) = tone.colors();
                    // Same shape and field order as `Span` with flattened `Syntax`.
                    seq.serialize_element(&BorrowedSyntaxSpan {
                        start,
                        end,
                        kind: "syntax",
                        light,
                        dark,
                    })?;
                }
            }
        }
        seq.end()
    }
}

#[derive(serde::Serialize)]
struct BorrowedCodeBlockSpan<'a> {
    start: usize,
    end: usize,
    kind: &'static str,
    language: &'a str,
}

#[derive(serde::Serialize)]
struct BorrowedSyntaxSpan {
    start: usize,
    end: usize,
    kind: &'static str,
    light: &'static str,
    dark: &'static str,
}

struct DocumentBlocks<'a> {
    doc: &'a CodeDocumentJson<'a>,
}

impl serde::Serialize for DocumentBlocks<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(1))?;
        seq.serialize_element(&BorrowedBlock {
            start: 0,
            end: utf16_len(self.doc.code),
            kind: "code",
        })?;
        seq.end()
    }
}

#[derive(serde::Serialize)]
struct BorrowedBlock {
    start: usize,
    end: usize,
    kind: &'static str,
}

/// Serialize `{"language","plain","tokens":[...]}` directly from compact
/// runs, without materializing owned [`HighlightedCode`] tokens. The Wasm
/// `highlightCode` bridge needs JSON text, not token structs; building them
/// first costs three owned `String`s per token (duplicating the whole source
/// inside the shared cache entry) plus a second pass to re-serialize. This
/// emits byte-identical JSON: field order matches the derived `Serialize`
/// impls, and `&str`/`&'static str` serialize exactly like the owned
/// `String`s they replace.
pub fn highlight_json(code: &str, language: &str) -> String {
    serde_json::to_string(&RunsJson {
        code,
        lexed: &lex(code, language),
    })
    .expect("highlight serialization is infallible")
}

struct RunsJson<'a> {
    code: &'a str,
    lexed: &'a LexedCode,
}

impl serde::Serialize for RunsJson<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = serializer.serialize_struct("HighlightedCode", 3)?;
        out.serialize_field("language", &self.lexed.language)?;
        out.serialize_field("plain", &self.lexed.plain)?;
        out.serialize_field("tokens", &RunsTokens { json: self })?;
        out.end()
    }
}

struct RunsTokens<'a> {
    json: &'a RunsJson<'a>,
}

impl serde::Serialize for RunsTokens<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.json.lexed.runs.len()))?;
        let mut start = 0;
        for run in &self.json.lexed.runs {
            let (end, tone) = (run.end(), run.tone);
            let (light, dark) = tone.colors();
            // Same shape and field order as `CodeToken`.
            seq.serialize_element(&BorrowedToken {
                text: &self.json.code[start..end],
                light,
                dark,
            })?;
            start = end;
        }
        seq.end()
    }
}

#[derive(serde::Serialize)]
struct BorrowedToken<'a> {
    text: &'a str,
    light: &'static str,
    dark: &'static str,
}
