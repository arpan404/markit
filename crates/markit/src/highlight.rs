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
    static APPEND_SCAN: RefCell<Option<(String, Arc<LexedCode>)>> = const { RefCell::new(None) };
    static CACHE: RefCell<crate::cache::Cache<(String, String), Arc<LexedCode>>> = RefCell::new(crate::cache::Cache::new(4 * 1024 * 1024));
}
// Byte offsets share the caller's source. Materialize owned tokens only for
// callers of the original token API; native spans and Wasm ranges skip them.
#[derive(Clone, Copy, Default)]
struct Checkpoint {
    at: usize,
    in_tag: bool,
    runs: usize,
    pending_start: usize,
    pending_tone: Option<Tone>,
}

struct LexedCode {
    checkpoint: Checkpoint,
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
// Wire palette order is stable for binary transport version 1.
pub(crate) const PALETTE: [(&str, &str, u32, u32); 8] = [
    ("#24292f", "#c9d1d9", 0x24292f, 0xc9d1d9),
    ("#6e7781", "#8b949e", 0x6e7781, 0x8b949e),
    ("#0a3069", "#a5d6ff", 0x0a3069, 0xa5d6ff),
    ("#0550ae", "#79c0ff", 0x0550ae, 0x79c0ff),
    ("#cf222e", "#ff7b72", 0xcf222e, 0xff7b72),
    ("#8250df", "#d2a8ff", 0x8250df, 0xd2a8ff),
    ("#953800", "#ffa657", 0x953800, 0xffa657),
    ("#116329", "#7ee787", 0x116329, 0x7ee787),
];
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
    fn palette_id(self) -> usize {
        match self {
            Self::Plain | Self::Punctuation => 0,
            Self::Comment => 1,
            Self::String => 2,
            Self::Number | Self::Property => 3,
            Self::Keyword => 4,
            Self::Function => 5,
            Self::Type => 6,
            Self::Tag => 7,
        }
    }
    fn colors(self) -> (&'static str, &'static str) {
        let p = PALETTE[self.palette_id()];
        (p.0, p.1)
    }
    fn rgb(self) -> (u32, u32) {
        let p = PALETTE[self.palette_id()];
        (p.2, p.3)
    }
}
fn word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}
/// UTF-16 length with a SIMD-friendly ASCII fast path. Code is usually ASCII;
/// `encode_utf16().count()` fully decodes UTF-8, which costs a second pass
/// over every run. Identical result on all inputs.
fn utf16_len(text: &str) -> usize {
    let bytes = text.as_bytes();
    if bytes.is_ascii() {
        bytes.len()
    } else {
        text.encode_utf16().count()
    }
}
/// Length of the UTF-8 sequence starting at a known character boundary.
/// The scanner only advances by whole characters, so `bytes[at]` is always a
/// leading byte and this matches `str::chars().next().len_utf8()`.
fn step_len(byte: u8) -> usize {
    if byte < 0x80 {
        1
    } else if (byte >> 5) == 0b110 {
        2
    } else if (byte >> 4) == 0b1110 {
        3
    } else {
        4
    }
}
fn keyword(word: &str, language: &str) -> bool {
    // Length dispatch first: most identifiers miss on length alone without
    // comparing string bytes. SQL stays case insensitive without allocating
    // a lowercased copy per identifier (`eq_ignore_ascii_case` folds ASCII,
    // which matches `to_ascii_lowercase` on scanner identifiers exactly).
    match language {
        "python" => match word.len() {
            2 => matches!(word, "as" | "if" | "in" | "is" | "or"),
            3 => matches!(word, "and" | "def" | "del" | "for" | "not" | "try"),
            4 => matches!(
                word,
                "None" | "True" | "elif" | "else" | "from" | "pass" | "self" | "with"
            ),
            5 => matches!(
                word,
                "False" | "async" | "await" | "break" | "class" | "raise" | "while" | "yield"
            ),
            6 => matches!(
                word,
                "assert" | "except" | "global" | "import" | "lambda" | "return"
            ),
            7 => matches!(word, "finally"),
            8 => matches!(word, "continue" | "nonlocal"),
            _ => false,
        },
        "bash" => match word.len() {
            2 => matches!(word, "cd" | "do" | "fi" | "if" | "in"),
            3 => matches!(word, "for"),
            4 => matches!(
                word,
                "case" | "done" | "echo" | "elif" | "else" | "esac" | "then"
            ),
            5 => matches!(word, "break" | "local" | "unset" | "until" | "while"),
            6 => matches!(word, "export" | "return" | "select" | "source"),
            7 => matches!(word, "declare"),
            8 => matches!(word, "continue" | "function" | "readonly"),
            _ => false,
        },
        "sql" => match word.len() {
            2 => {
                word.eq_ignore_ascii_case("as")
                    || word.eq_ignore_ascii_case("by")
                    || word.eq_ignore_ascii_case("on")
                    || word.eq_ignore_ascii_case("or")
            }
            3 => {
                word.eq_ignore_ascii_case("all")
                    || word.eq_ignore_ascii_case("and")
                    || word.eq_ignore_ascii_case("end")
                    || word.eq_ignore_ascii_case("key")
                    || word.eq_ignore_ascii_case("not")
                    || word.eq_ignore_ascii_case("set")
            }
            4 => {
                word.eq_ignore_ascii_case("case")
                    || word.eq_ignore_ascii_case("drop")
                    || word.eq_ignore_ascii_case("else")
                    || word.eq_ignore_ascii_case("from")
                    || word.eq_ignore_ascii_case("into")
                    || word.eq_ignore_ascii_case("join")
                    || word.eq_ignore_ascii_case("left")
                    || word.eq_ignore_ascii_case("null")
                    || word.eq_ignore_ascii_case("then")
                    || word.eq_ignore_ascii_case("true")
                    || word.eq_ignore_ascii_case("when")
            }
            5 => {
                word.eq_ignore_ascii_case("alter")
                    || word.eq_ignore_ascii_case("false")
                    || word.eq_ignore_ascii_case("group")
                    || word.eq_ignore_ascii_case("index")
                    || word.eq_ignore_ascii_case("inner")
                    || word.eq_ignore_ascii_case("limit")
                    || word.eq_ignore_ascii_case("order")
                    || word.eq_ignore_ascii_case("outer")
                    || word.eq_ignore_ascii_case("right")
                    || word.eq_ignore_ascii_case("table")
                    || word.eq_ignore_ascii_case("union")
                    || word.eq_ignore_ascii_case("where")
            }
            6 => {
                word.eq_ignore_ascii_case("create")
                    || word.eq_ignore_ascii_case("delete")
                    || word.eq_ignore_ascii_case("having")
                    || word.eq_ignore_ascii_case("insert")
                    || word.eq_ignore_ascii_case("offset")
                    || word.eq_ignore_ascii_case("select")
                    || word.eq_ignore_ascii_case("update")
                    || word.eq_ignore_ascii_case("values")
            }
            7 => word.eq_ignore_ascii_case("default") || word.eq_ignore_ascii_case("primary"),
            8 => word.eq_ignore_ascii_case("distinct"),
            10 => word.eq_ignore_ascii_case("references"),
            _ => false,
        },
        "json" | "json5" => match word.len() {
            3 => matches!(word, "NaN"),
            4 => matches!(word, "null" | "true"),
            5 => matches!(word, "false"),
            8 => matches!(word, "Infinity"),
            _ => false,
        },
        "yaml" => match word.len() {
            2 => matches!(word, "no" | "on"),
            3 => matches!(word, "off" | "yes"),
            4 => matches!(word, "null" | "true"),
            5 => matches!(word, "false"),
            _ => false,
        },
        "rust" => match word.len() {
            2 => matches!(word, "as" | "do" | "fn" | "if" | "in"),
            3 => matches!(
                word,
                "box" | "dyn" | "for" | "let" | "mod" | "mut" | "pub" | "ref" | "try" | "use"
            ),
            4 => matches!(
                word,
                "Self"
                    | "else"
                    | "enum"
                    | "impl"
                    | "loop"
                    | "move"
                    | "priv"
                    | "self"
                    | "true"
                    | "type"
            ),
            5 => matches!(
                word,
                "async"
                    | "await"
                    | "break"
                    | "const"
                    | "crate"
                    | "false"
                    | "final"
                    | "macro"
                    | "match"
                    | "super"
                    | "trait"
                    | "union"
                    | "where"
                    | "while"
                    | "yield"
            ),
            6 => matches!(
                word,
                "become" | "extern" | "return" | "static" | "struct" | "typeof" | "unsafe"
            ),
            7 => matches!(word, "unsized" | "virtual"),
            8 => matches!(word, "abstract" | "continue" | "override"),
            _ => false,
        },
        "swift" => match word.len() {
            2 => matches!(word, "as" | "do" | "if" | "in" | "is"),
            3 => matches!(
                word,
                "any" | "for" | "get" | "let" | "nil" | "set" | "try" | "var"
            ),
            4 => matches!(
                word,
                "Self"
                    | "case"
                    | "else"
                    | "enum"
                    | "func"
                    | "init"
                    | "lazy"
                    | "open"
                    | "self"
                    | "some"
                    | "true"
                    | "weak"
            ),
            5 => matches!(
                word,
                "actor"
                    | "async"
                    | "await"
                    | "break"
                    | "catch"
                    | "class"
                    | "defer"
                    | "false"
                    | "final"
                    | "guard"
                    | "inout"
                    | "super"
                    | "throw"
                    | "where"
                    | "while"
            ),
            6 => matches!(
                word,
                "deinit"
                    | "didSet"
                    | "import"
                    | "public"
                    | "repeat"
                    | "return"
                    | "static"
                    | "struct"
                    | "switch"
                    | "throws"
            ),
            7 => matches!(
                word,
                "default" | "dynamic" | "package" | "private" | "unowned" | "willSet"
            ),
            8 => matches!(
                word,
                "continue"
                    | "indirect"
                    | "internal"
                    | "isolated"
                    | "mutating"
                    | "operator"
                    | "optional"
                    | "override"
                    | "protocol"
                    | "required"
                    | "rethrows"
            ),
            9 => matches!(
                word,
                "borrowing" | "consuming" | "extension" | "subscript" | "typealias"
            ),
            11 => matches!(
                word,
                "convenience" | "fallthrough" | "fileprivate" | "nonisolated" | "nonmutating"
            ),
            14 => matches!(word, "associatedtype"),
            _ => false,
        },
        _ => match word.len() {
            2 => matches!(word, "as" | "do" | "go" | "if" | "in" | "is" | "of"),
            3 => matches!(
                word,
                "for"
                    | "int"
                    | "let"
                    | "map"
                    | "new"
                    | "out"
                    | "ref"
                    | "try"
                    | "use"
                    | "val"
                    | "var"
            ),
            4 => matches!(
                word,
                "case"
                    | "char"
                    | "else"
                    | "enum"
                    | "from"
                    | "func"
                    | "long"
                    | "null"
                    | "self"
                    | "this"
                    | "true"
                    | "type"
                    | "void"
                    | "when"
                    | "with"
            ),
            5 => matches!(
                word,
                "async"
                    | "await"
                    | "break"
                    | "catch"
                    | "class"
                    | "const"
                    | "defer"
                    | "false"
                    | "final"
                    | "float"
                    | "match"
                    | "short"
                    | "super"
                    | "throw"
                    | "trait"
                    | "using"
                    | "where"
                    | "while"
                    | "yield"
            ),
            6 => matches!(
                word,
                "delete"
                    | "double"
                    | "export"
                    | "import"
                    | "native"
                    | "object"
                    | "public"
                    | "record"
                    | "return"
                    | "sealed"
                    | "static"
                    | "string"
                    | "struct"
                    | "switch"
                    | "throws"
                    | "typeof"
            ),
            7 => matches!(
                word,
                "boolean" | "default" | "extends" | "finally" | "package" | "private" | "virtual"
            ),
            8 => matches!(
                word,
                "abstract"
                    | "continue"
                    | "debugger"
                    | "function"
                    | "internal"
                    | "override"
                    | "readonly"
                    | "unsigned"
                    | "volatile"
            ),
            9 => matches!(
                word,
                "interface" | "namespace" | "protected" | "satisfies" | "undefined"
            ),
            10 => matches!(word, "implements" | "instanceof"),
            12 => matches!(word, "synchronized"),
            _ => false,
        },
    }
}
fn scan(code: &str, language: &str) -> LexedCode {
    scan_from(code, language, None)
}

fn scan_from(code: &str, language: &str, previous: Option<&LexedCode>) -> LexedCode {
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
        checkpoint: Checkpoint::default(),
        language: language.into(),
        plain: true,
        runs: vec![(code.len(), Tone::Plain)],
        tokens: OnceLock::new(),
    };
    if !supported || code.len() > 1024 * 1024 {
        return plain();
    }
    let bytes = code.as_bytes();
    let mut checkpoint = previous.map_or(Checkpoint::default(), |old| old.checkpoint);
    let mut at = checkpoint.at;
    let mut runs = previous.map_or_else(Vec::new, |old| old.runs[..checkpoint.runs].to_vec());
    let mut in_tag = checkpoint.in_tag;
    let line_start = code.rfind('\n').map_or(0, |at| at + 1);
    let mut future_dependent = false;
    let hash_comment = matches!(language, "python" | "bash" | "yaml");
    let slash_comment = !matches!(language, "python" | "bash" | "yaml" | "html" | "markdown");
    let markup = matches!(language, "html" | "jsx" | "tsx");
    let mut pending_start = checkpoint.pending_start;
    let mut pending_tone = checkpoint.pending_tone.unwrap_or(Tone::Plain);
    while at < bytes.len() {
        // A token crossing the last line (string, comment, raw delimiter) is
        // rechecked from its start. Earlier runs and markup state stay valid.
        if at <= line_start && !future_dependent {
            checkpoint = Checkpoint {
                at,
                in_tag,
                runs: runs.len(),
                pending_start,
                pending_tone: Some(pending_tone),
            };
        }
        let start = at;
        let byte = bytes[at];
        let mut tone = Tone::Plain;
        if (hash_comment && byte == b'#')
            || (slash_comment && byte == b'/' && bytes.get(at + 1) == Some(&b'/'))
            || (language == "sql" && byte == b'-' && bytes.get(at + 1) == Some(&b'-'))
        {
            at += bytes[at..]
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or(bytes.len() - at);
            tone = Tone::Comment;
        } else if slash_comment && byte == b'/' && bytes.get(at + 1) == Some(&b'*') {
            at += 2;
            let mut depth = 1;
            while at < bytes.len() && depth > 0 {
                if language == "rust" && bytes[at] == b'/' && bytes.get(at + 1) == Some(&b'*') {
                    depth += 1;
                    at += 2;
                } else if bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/') {
                    depth -= 1;
                    at += 2;
                } else {
                    at += step_len(bytes[at]);
                }
            }
            tone = Tone::Comment;
        } else if markup && byte == b'<' && bytes.get(at + 1..at + 4) == Some(b"!--".as_slice()) {
            at = code[at + 4..]
                .find("-->")
                .map_or(bytes.len(), |end| at + 4 + end + 3);
            tone = Tone::Comment;
        } else if language == "rust"
            && byte == b'r'
            && matches!(bytes.get(at + 1), Some(b'#') | Some(b'"'))
        {
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
                            at += step_len(bytes[at]);
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
                        at += step_len(bytes[at]);
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
            future_dependent |= after.is_empty();
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
            at += step_len(byte);
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
        checkpoint,
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
    let result = APPEND_SCAN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let previous = slot.as_ref().filter(|(source, old)| {
            old.language == language && !old.plain && code.starts_with(source.as_str())
        });
        let result = Arc::new(match previous {
            Some((_, old)) => scan_from(code, &language, Some(old)),
            None => scan(code, &language),
        });
        *slot = (code.len() <= 1024 * 1024).then(|| (code.to_owned(), result.clone()));
        result
    });
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
        let next = offset + utf16_len(&code[start..end]);
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
        let next = offset + utf16_len(&code[start..end]);
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
        offset += utf16_len(&code[start..end]) as u32;
        let (light, dark) = tone.rgb();
        words.extend([offset, light, dark]);
        start = end;
    }
    words
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
        for &(end, tone) in &self.json.lexed.runs {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_rgb_matches_hex_palette() {
        for tone in [
            Tone::Plain,
            Tone::Comment,
            Tone::String,
            Tone::Number,
            Tone::Keyword,
            Tone::Function,
            Tone::Type,
            Tone::Property,
            Tone::Tag,
            Tone::Punctuation,
        ] {
            let (light, dark) = tone.colors();
            let (light_rgb, dark_rgb) = tone.rgb();
            assert_eq!(light_rgb, u32::from_str_radix(&light[1..], 16).unwrap());
            assert_eq!(dark_rgb, u32::from_str_radix(&dark[1..], 16).unwrap());
        }
    }

    #[test]
    fn utf16_len_matches_encode_count() {
        for text in [
            "",
            "fn main() {}",
            "café",
            "中文 rust",
            "c 🦀",
            "mix 🦀 é 中 end",
        ] {
            assert_eq!(utf16_len(text), text.encode_utf16().count());
        }
    }
}
