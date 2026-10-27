use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeToken {
    pub text: String,
    pub light: String,
    pub dark: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HighlightedCode {
    pub language: String,
    pub plain: bool,
    pub tokens: Vec<CodeToken>,
}
thread_local! {
    static CACHE: RefCell<crate::cache::Cache<(String, String), Arc<LexedCode>>> = RefCell::new(crate::cache::Cache::new(4 * 1024 * 1024));
}
// Byte offsets share the caller's source. Materialize owned tokens only for
// callers of the original token API; native spans and Wasm ranges skip them.
struct LexedCode {
    language: String,
    plain: bool,
    runs: Vec<(usize, Tone)>,
    tokens: OnceLock<Arc<HighlightedCode>>,
}

pub fn language_alias(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "ts" => "typescript",
        "js" | "mjs" | "cjs" => "javascript",
        "py" => "python",
        "rs" => "rust",
        "sh" | "zsh" | "shell" | "shellscript" | "console" => "bash",
        "c++" => "cpp",
        "cs" => "csharp",
        "yml" => "yaml",
        "md" => "markdown",
        "xml" | "svg" | "markup" => "html",
        "jsonc" => "json",
        "patch" => "diff",
        other => return other.to_owned(),
    }
    .into()
}
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Plain,
    Comment,
    String,
    Number,
    Keyword,
    Function,
    Type,
    Property,
    Tag,
    Punctuation,
}
impl Tone {
    fn colors(self) -> (&'static str, &'static str) {
        match self {
            Self::Plain | Self::Punctuation => ("#24292f", "#c9d1d9"),
            Self::Comment => ("#6e7781", "#8b949e"),
            Self::String => ("#0a3069", "#a5d6ff"),
            Self::Number | Self::Property => ("#0550ae", "#79c0ff"),
            Self::Keyword => ("#cf222e", "#ff7b72"),
            Self::Function => ("#8250df", "#d2a8ff"),
            Self::Type => ("#953800", "#ffa657"),
            Self::Tag => ("#116329", "#7ee787"),
        }
    }
}
fn word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}
fn keyword(word: &str, language: &str) -> bool {
    // SQL keywords are case insensitive; other languages preserve identifier case.
    let sql_word;
    let word = if language == "sql" {
        sql_word = word.to_ascii_lowercase();
        sql_word.as_str()
    } else {
        word
    };
    match language {
        "python" => matches!(
            word,
            "and"
                | "as"
                | "assert"
                | "async"
                | "await"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "del"
                | "elif"
                | "else"
                | "except"
                | "False"
                | "finally"
                | "for"
                | "from"
                | "global"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "None"
                | "nonlocal"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "True"
                | "try"
                | "while"
                | "with"
                | "yield"
                | "self"
        ),
        "bash" => matches!(
            word,
            "if" | "then"
                | "else"
                | "elif"
                | "fi"
                | "for"
                | "in"
                | "do"
                | "done"
                | "case"
                | "esac"
                | "while"
                | "until"
                | "function"
                | "export"
                | "local"
                | "return"
                | "echo"
                | "cd"
                | "source"
                | "readonly"
                | "unset"
                | "declare"
                | "select"
                | "break"
                | "continue"
        ),
        "sql" => matches!(
            word,
            "select"
                | "from"
                | "where"
                | "join"
                | "left"
                | "right"
                | "inner"
                | "outer"
                | "on"
                | "as"
                | "insert"
                | "into"
                | "update"
                | "delete"
                | "create"
                | "table"
                | "index"
                | "drop"
                | "alter"
                | "values"
                | "set"
                | "null"
                | "not"
                | "and"
                | "or"
                | "order"
                | "by"
                | "group"
                | "having"
                | "limit"
                | "offset"
                | "union"
                | "all"
                | "distinct"
                | "case"
                | "when"
                | "then"
                | "else"
                | "end"
                | "primary"
                | "key"
                | "references"
                | "default"
                | "true"
                | "false"
        ),
        "json" | "json5" => matches!(word, "true" | "false" | "null" | "Infinity" | "NaN"),
        "yaml" => matches!(
            word,
            "true" | "false" | "null" | "yes" | "no" | "on" | "off"
        ),
        "rust" => matches!(
            word,
            "as" | "async"
                | "await"
                | "break"
                | "const"
                | "continue"
                | "crate"
                | "dyn"
                | "else"
                | "enum"
                | "extern"
                | "false"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "ref"
                | "return"
                | "self"
                | "Self"
                | "static"
                | "struct"
                | "super"
                | "trait"
                | "true"
                | "type"
                | "unsafe"
                | "use"
                | "where"
                | "while"
                | "abstract"
                | "become"
                | "box"
                | "do"
                | "final"
                | "macro"
                | "override"
                | "priv"
                | "typeof"
                | "unsized"
                | "virtual"
                | "yield"
                | "try"
                | "union"
        ),
        "swift" => matches!(
            word,
            "actor"
                | "any"
                | "as"
                | "associatedtype"
                | "async"
                | "await"
                | "borrowing"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "consuming"
                | "continue"
                | "convenience"
                | "default"
                | "defer"
                | "deinit"
                | "didSet"
                | "do"
                | "dynamic"
                | "else"
                | "enum"
                | "extension"
                | "fallthrough"
                | "false"
                | "fileprivate"
                | "final"
                | "for"
                | "func"
                | "get"
                | "guard"
                | "if"
                | "import"
                | "in"
                | "indirect"
                | "init"
                | "inout"
                | "internal"
                | "is"
                | "isolated"
                | "lazy"
                | "let"
                | "mutating"
                | "nil"
                | "nonisolated"
                | "nonmutating"
                | "open"
                | "operator"
                | "optional"
                | "override"
                | "package"
                | "private"
                | "protocol"
                | "public"
                | "repeat"
                | "required"
                | "rethrows"
                | "return"
                | "self"
                | "Self"
                | "set"
                | "some"
                | "static"
                | "struct"
                | "subscript"
                | "super"
                | "switch"
                | "throw"
                | "throws"
                | "true"
                | "try"
                | "typealias"
                | "unowned"
                | "var"
                | "weak"
                | "where"
                | "while"
                | "willSet"
        ),
        _ => matches!(
            word,
            "abstract"
                | "as"
                | "async"
                | "await"
                | "boolean"
                | "break"
                | "case"
                | "catch"
                | "char"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "defer"
                | "delete"
                | "do"
                | "double"
                | "else"
                | "enum"
                | "export"
                | "extends"
                | "false"
                | "final"
                | "finally"
                | "float"
                | "for"
                | "from"
                | "func"
                | "function"
                | "go"
                | "if"
                | "implements"
                | "import"
                | "in"
                | "instanceof"
                | "int"
                | "interface"
                | "internal"
                | "is"
                | "let"
                | "long"
                | "map"
                | "match"
                | "namespace"
                | "native"
                | "new"
                | "null"
                | "object"
                | "of"
                | "out"
                | "override"
                | "package"
                | "private"
                | "protected"
                | "public"
                | "readonly"
                | "record"
                | "ref"
                | "return"
                | "satisfies"
                | "sealed"
                | "self"
                | "short"
                | "static"
                | "string"
                | "struct"
                | "super"
                | "switch"
                | "synchronized"
                | "this"
                | "throw"
                | "throws"
                | "trait"
                | "true"
                | "try"
                | "type"
                | "typeof"
                | "undefined"
                | "unsigned"
                | "use"
                | "using"
                | "val"
                | "var"
                | "virtual"
                | "void"
                | "volatile"
                | "when"
                | "where"
                | "while"
                | "with"
                | "yield"
        ),
    }
}
fn scan(code: &str, language: &str) -> LexedCode {
    let supported = [
        "typescript",
        "tsx",
        "javascript",
        "jsx",
        "python",
        "bash",
        "rust",
        "go",
        "java",
        "kotlin",
        "swift",
        "c",
        "cpp",
        "csharp",
        "json",
        "json5",
        "yaml",
        "markdown",
        "sql",
        "css",
        "html",
        "diff",
        "clike",
    ]
    .contains(&language);
    let plain = || LexedCode {
        language: language.into(),
        plain: true,
        runs: vec![(code.len(), Tone::Plain)],
        tokens: OnceLock::new(),
    };
    if !supported || code.len() > 1024 * 1024 {
        return plain();
    }
    let bytes = code.as_bytes();
    let mut at = 0;
    let mut runs = Vec::new();
    let mut in_tag = false;
    let hash_comment = matches!(language, "python" | "bash" | "yaml");
    let slash_comment = !matches!(language, "python" | "bash" | "yaml" | "html" | "markdown");
    let markup = matches!(language, "html" | "jsx" | "tsx");
    let mut pending_start = 0;
    let mut pending_tone = Tone::Plain;
    while at < bytes.len() {
        let start = at;
        let byte = bytes[at];
        let mut tone = Tone::Plain;
        if (hash_comment && byte == b'#')
            || (slash_comment && code[at..].starts_with("//"))
            || (language == "sql" && code[at..].starts_with("--"))
        {
            at += code[at..].find('\n').unwrap_or(bytes.len() - at);
            tone = Tone::Comment;
        } else if slash_comment && code[at..].starts_with("/*") {
            at += 2;
            let mut depth = 1;
            while at < bytes.len() && depth > 0 {
                if language == "rust" && code[at..].starts_with("/*") {
                    depth += 1;
                    at += 2;
                } else if code[at..].starts_with("*/") {
                    depth -= 1;
                    at += 2;
                } else {
                    at += code[at..].chars().next().unwrap().len_utf8();
                }
            }
            tone = Tone::Comment;
        } else if markup && code[at..].starts_with("<!--") {
            at = code[at + 4..]
                .find("-->")
                .map_or(bytes.len(), |end| at + 4 + end + 3);
            tone = Tone::Comment;
        } else if language == "rust" && byte == b'r' && code[at + 1..].starts_with(['#', '"']) {
            let hashes = bytes[at + 1..].iter().take_while(|&&b| b == b'#').count();
            let quote = at + 1 + hashes;
            if bytes.get(quote) == Some(&b'"') {
                let end_marker = format!("\"{}", "#".repeat(hashes));
                at = code[quote + 1..]
                    .find(&end_marker)
                    .map_or(bytes.len(), |end| quote + 1 + end + end_marker.len());
                tone = Tone::String;
            } else {
                at += 1;
            }
        } else if matches!(byte, b'"' | b'\'' | b'`') {
            // Rust lifetimes are identifiers rather than unterminated strings.
            let lifetime = language == "rust"
                && byte == b'\''
                && bytes.get(at + 1).is_some_and(|b| b.is_ascii_alphabetic())
                && bytes.get(at + 2) != Some(&b'\'');
            if lifetime {
                at += 1;
                while at < bytes.len() && word_byte(bytes[at]) {
                    at += 1;
                }
            } else {
                let triple = ["python", "kotlin", "swift"].contains(&language)
                    && bytes
                        .get(at..at + 3)
                        .is_some_and(|part| part == [byte, byte, byte]);
                let marker = if triple { 3 } else { 1 };
                at += marker;
                while at < bytes.len() {
                    if bytes[at] == b'\\' {
                        at += 1;
                        if at < bytes.len() {
                            at += code[at..].chars().next().unwrap().len_utf8();
                        }
                    } else if bytes[at] == byte
                        && (!triple
                            || bytes
                                .get(at..at + 3)
                                .is_some_and(|part| part == [byte, byte, byte]))
                    {
                        at += marker;
                        break;
                    } else {
                        at += code[at..].chars().next().unwrap().len_utf8();
                    }
                }
                tone = Tone::String;
            }
        } else if byte.is_ascii_digit() && (at == 0 || !word_byte(bytes[at - 1])) {
            at += 1;
            while at < bytes.len()
                && (bytes[at].is_ascii_alphanumeric() || matches!(bytes[at], b'.' | b'_'))
            {
                at += 1;
            }
            tone = Tone::Number;
        } else if byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') {
            at += 1;
            while at < bytes.len() && word_byte(bytes[at]) {
                at += 1;
            }
            let word = &code[start..at];
            let after = code[at..].trim_start();
            tone = if keyword(word, language) {
                Tone::Keyword
            } else if in_tag && markup {
                if after.starts_with('=') {
                    Tone::Property
                } else {
                    Tone::Tag
                }
            } else if after.starts_with('(') {
                Tone::Function
            } else if after.starts_with(':') {
                Tone::Property
            } else if word.as_bytes()[0].is_ascii_uppercase() {
                Tone::Type
            } else {
                Tone::Plain
            };
        } else {
            if markup {
                if byte == b'<' {
                    in_tag = true;
                } else if byte == b'>' {
                    in_tag = false;
                }
            }
            at += code[at..].chars().next().unwrap().len_utf8();
            if byte.is_ascii_punctuation() {
                tone = Tone::Punctuation;
            }
        }
        // Plain and punctuation have the same palette and merge into one run.
        let tone = if tone == Tone::Punctuation {
            Tone::Plain
        } else {
            tone
        };
        if tone != pending_tone {
            if start > pending_start {
                runs.push((start, pending_tone));
            }
            pending_start = start;
            pending_tone = tone;
        }
        if runs.len() > 8192 {
            return plain();
        }
    }
    if at > pending_start {
        runs.push((at, pending_tone));
    }
    if runs.len() > 8192 {
        return plain();
    }
    LexedCode {
        language: language.into(),
        plain: false,
        runs,
        tokens: OnceLock::new(),
    }
}
fn lex(code: &str, language: &str) -> Arc<LexedCode> {
    let language = language_alias(language);
    if let Some(cached) = CACHE.with(|cache| {
        cache
            .borrow_mut()
            .get_by(&(language.as_str(), code), |key| {
                key.0 == language && key.1 == code
            })
    }) {
        return cached;
    }
    let result = Arc::new(scan(code, &language));
    // Reserve the budget for lazy token materialization as well as compact runs.
    let weight = language.len() + code.len() * 2 + result.runs.len() * 128;
    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert((language, code.to_owned()), result.clone(), weight)
    });
    result
}

pub fn highlight(code: &str, language: &str) -> Arc<HighlightedCode> {
    let lexed = lex(code, language);
    lexed
        .tokens
        .get_or_init(|| {
            let mut start = 0;
            let tokens = lexed
                .runs
                .iter()
                .map(|&(end, tone)| {
                    let (light, dark) = tone.colors();
                    let token = CodeToken {
                        text: code[start..end].into(),
                        light: light.into(),
                        dark: dark.into(),
                    };
                    start = end;
                    token
                })
                .collect();
            Arc::new(HighlightedCode {
                language: lexed.language.clone(),
                plain: lexed.plain,
                tokens,
            })
        })
        .clone()
}

pub(crate) fn for_each_run(code: &str, language: &str, mut emit: impl FnMut(&str, &str, &str)) {
    let lexed = lex(code, language);
    let mut start = 0;
    for &(end, tone) in &lexed.runs {
        let (light, dark) = tone.colors();
        emit(&code[start..end], light, dark);
        start = end;
    }
}

pub(crate) fn append_syntax_spans(
    code: &str,
    language: &str,
    base: usize,
    spans: &mut Vec<crate::Span>,
) {
    let lexed = lex(code, language);
    if lexed.plain {
        return;
    }
    let mut start = 0;
    let mut offset = base;
    for &(end, tone) in &lexed.runs {
        let next = offset + code[start..end].encode_utf16().count();
        if next > offset {
            let (light, dark) = tone.colors();
            spans.push(crate::Span {
                start: offset,
                end: next,
                style: crate::Style::Syntax {
                    light: light.into(),
                    dark: dark.into(),
                },
            });
        }
        start = end;
        offset = next;
    }
}

pub(crate) fn code_document(code: &str, language: &str) -> crate::Document {
    let lexed = lex(code, language);
    let mut spans = Vec::with_capacity(lexed.runs.len() + 1);
    let mut start = 0;
    let mut offset = 0;
    spans.push(crate::Span {
        start: 0,
        end: 0,
        style: crate::Style::CodeBlock {
            language: lexed.language.clone(),
        },
    });
    for &(end, tone) in &lexed.runs {
        let next = offset + code[start..end].encode_utf16().count();
        if !lexed.plain && next > offset {
            let (light, dark) = tone.colors();
            spans.push(crate::Span {
                start: offset,
                end: next,
                style: crate::Style::Syntax {
                    light: light.into(),
                    dark: dark.into(),
                },
            });
        }
        start = end;
        offset = next;
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

/// Compact lexical result for Wasm. Words are version 1, plain flag, language
/// UTF-16 length, language UTF-16 units, then triples of end UTF-16 offset and
/// light/dark 24-bit RGB. Runs are contiguous and start at offset zero.
pub fn highlight_packed(code: &str, language: &str) -> Vec<u32> {
    let lexed = lex(code, language);
    let language_units = lexed.language.encode_utf16().count();
    let mut words = Vec::with_capacity(3 + language_units + lexed.runs.len() * 3);
    words.extend([1, u32::from(lexed.plain), language_units as u32]);
    words.extend(lexed.language.encode_utf16().map(u32::from));
    let mut start = 0;
    let mut offset = 0;
    for &(end, tone) in &lexed.runs {
        offset += code[start..end].encode_utf16().count() as u32;
        let (light, dark) = tone.colors();
        words.extend([
            offset,
            u32::from_str_radix(&light[1..], 16).unwrap(),
            u32::from_str_radix(&dark[1..], 16).unwrap(),
        ]);
        start = end;
    }
    words
}
