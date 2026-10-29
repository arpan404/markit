//! Lexical syntax highlighting. One scan produces compact tone runs that
//! every output (tokens, packed UTF-16 ranges, native spans, HTML, JSON)
//! reads without rescanning.
//!
//! Limits: explicit highlight calls colour sources up to [`MAX_SOURCE`]
//! bytes, with no run-count cliff (a cap of [`MAX_RUNS`] runs only bounds
//! memory on adversarial input; real code stays far below it). Fences
//! rendered inside Markdown keep a smaller budget ([`FENCE_RUNS`],
//! [`FENCE_SOURCE`]) because the HTML and JSON stream modes rebuild a
//! fence's whole output on every append.
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    sync::{Arc, OnceLock},
};

mod json;
mod keywords;
mod keywords_extra;
mod language;
mod scan;
mod spans;
mod tone;

pub use json::{code_document_json, highlight_json};
pub use language::language_alias;
pub(crate) use spans::code_document;
pub use spans::{for_each_code_span, BorrowedCodeSpan};
pub(crate) use tone::{utf16_len, PALETTE};

use scan::Checkpoint;
use tone::{Run, Tone};

/// Largest source an explicit highlight call colours, matching the 8 MiB
/// stream cap of every bridge. Larger sources are returned as plain text.
pub(crate) const MAX_SOURCE: usize = 8 * 1024 * 1024;
/// Run cap for one source. Real code has well under one run per two bytes,
/// so an 8 MiB file stays below it; past the cap the tail is one plain run.
pub(crate) const MAX_RUNS: usize = 1 << 22;
/// Fences inside rendered Markdown fall back to plain text above this many
/// runs or bytes. HTML and JSON stream modes rebuild a fence's full output on
/// every append, so their per-append cost grows with coloured runs.
pub(crate) const FENCE_RUNS: usize = 8192;
pub(crate) const FENCE_SOURCE: usize = 1024 * 1024;
/// Results with more runs are not memoized as tokens or packed words: they
/// are rebuilt from runs on demand instead of doubling retained memory.
const MEMO_RUNS: usize = 16 * 1024;
/// Largest cold result kept for append-only rescans and the shared cache.
const RETAINED_SOURCE: usize = 1024 * 1024;

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
struct LexedCode {
    checkpoint: Checkpoint,
    language: String,
    plain: bool,
    /// Plain for this source and for every source that extends it (no
    /// language, a plain-text language, or the size limit).
    sticky: bool,
    /// Every byte is ASCII, so UTF-16 offsets equal byte offsets.
    ascii: bool,
    runs: Vec<Run>,
    tokens: OnceLock<Arc<HighlightedCode>>,
    packed: OnceLock<Vec<u32>>,
}

impl LexedCode {
    /// UTF-16 start, end and tone of every run, without decoding ASCII sources.
    fn utf16_ends<'a>(&'a self, code: &'a str) -> impl Iterator<Item = (usize, usize, Tone)> + 'a {
        let mut start = 0;
        let mut offset = 0;
        let ascii = self.ascii;
        self.runs.iter().map(move |run| {
            let end = run.end();
            let begin = offset;
            offset = if ascii {
                end
            } else {
                offset + utf16_len(&code[start..end])
            };
            start = end;
            (begin, offset, run.tone)
        })
    }

    fn plain(code: &str, language: &str) -> Self {
        LexedCode {
            checkpoint: Checkpoint::default(),
            language: language.into(),
            plain: true,
            sticky: true,
            ascii: code.is_ascii(),
            runs: vec![Run {
                end: u32::try_from(code.len()).expect("highlighted sources are below 4 GiB"),
                tone: Tone::Plain,
            }],
            tokens: OnceLock::new(),
            packed: OnceLock::new(),
        }
    }
}

/// `previous` is the checkpoint of a scan of a prefix of `code` and that
/// scan's runs, already truncated to the checkpoint.
fn scan_from(code: &str, language: &str, previous: Option<(Checkpoint, Vec<Run>)>) -> LexedCode {
    let profile = match language::profile(language) {
        Some(profile) if code.len() <= MAX_SOURCE => profile,
        _ => return LexedCode::plain(code, language),
    };
    let scanned = scan::scan(code, profile, previous, MAX_RUNS);
    LexedCode {
        checkpoint: scanned.checkpoint,
        language: language.into(),
        plain: false,
        sticky: false,
        ascii: code.is_ascii(),
        runs: scanned.runs,
        tokens: OnceLock::new(),
        packed: OnceLock::new(),
    }
}

fn lex(code: &str, language: &str) -> Arc<LexedCode> {
    let language = language::canonical_language(language);
    let language = language.as_ref();
    // The hot buffer is checked first: a streaming fence extends it on every
    // append, and hashing the whole source for the shared cache would make
    // each append linear in the fence length.
    let hot = APPEND_SCAN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let (source, old) = slot.as_ref()?;
        if old.language != language || !code.starts_with(source.as_str()) {
            return None;
        }
        if source.len() == code.len() {
            return Some(old.clone());
        }
        let (mut source, old) = slot.take()?;
        let result = Arc::new(if old.sticky {
            LexedCode::plain(code, language)
        } else {
            // `extended` scans only the new suffix. Those prefixes are not
            // inserted into the shared cache: a streaming fence would
            // otherwise copy the whole source on every line and evict useful
            // entries. A unique previous result donates its runs.
            let checkpoint = old.checkpoint;
            let runs = match Arc::try_unwrap(old) {
                Ok(mut owned) => {
                    owned.runs.truncate(checkpoint.runs);
                    owned.runs
                }
                Err(shared) => shared.runs[..checkpoint.runs].to_vec(),
            };
            scan_from(code, language, Some((checkpoint, runs)))
        });
        source.push_str(&code[source.len()..]);
        *slot = Some((source, result.clone()));
        Some(result)
    });
    if let Some(result) = hot {
        return result;
    }
    if let Some(cached) = CACHE.with(|cache| {
        cache
            .borrow_mut()
            .get_by(&(language, code), |key| key.0 == language && key.1 == code)
    }) {
        return cached;
    }
    let result = Arc::new(scan_from(code, language, None));
    let store = code.len() <= RETAINED_SOURCE;
    APPEND_SCAN.with(|slot| {
        *slot.borrow_mut() = store.then(|| (code.to_owned(), result.clone()));
    });
    if store {
        // Reserve the budget for lazy token materialization as well as compact runs.
        let weight = language.len() + code.len() * 2 + result.runs.len() * 128;
        CACHE.with(|cache| {
            cache.borrow_mut().insert(
                (language.to_owned(), code.to_owned()),
                result.clone(),
                weight,
            )
        });
    }
    result
}

/// The lexer result for a fence inside rendered Markdown, or `None` when the
/// fence is shown as plain text (no language, or over the fence budget).
fn lex_fence(code: &str, language: &str) -> Option<Arc<LexedCode>> {
    if code.len() > FENCE_SOURCE {
        return None;
    }
    let lexed = lex(code, language);
    (!lexed.plain && lexed.runs.len() <= FENCE_RUNS).then_some(lexed)
}

/// Compact runs for one source. `stable_byte` is a line boundary: every run
/// that ends there is final for an append-only buffer. The open tail is the
/// last line, or the line that still contains a multiline token. Runs are
/// borrowed from the lexer result, so a streaming append does not copy them.
pub(crate) struct LexRuns {
    lexed: Arc<LexedCode>,
    pub plain: bool,
    pub stable_byte: usize,
}

impl LexRuns {
    pub(crate) fn is_plain(&self) -> bool {
        self.plain
    }
    /// Byte end of run `index`.
    pub(crate) fn end(&self, index: usize) -> usize {
        self.lexed.runs[index].end()
    }
    /// Index of the first run whose end is greater than `byte`.
    pub(crate) fn first_ending_after(&self, byte: usize) -> usize {
        self.lexed.runs.partition_point(|run| run.end() <= byte)
    }
    /// `(byte end, palette id)` of every run from `index`.
    pub(crate) fn from(&self, index: usize) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.lexed.runs[index..]
            .iter()
            .map(|run| (run.end(), run.tone.palette_id() as u8))
    }
    /// The source has no multi-byte characters.
    pub(crate) fn ascii(&self) -> bool {
        self.lexed.ascii
    }
}

/// Runs for a fence inside rendered Markdown, with the fence budget applied.
pub(crate) fn lex_runs(code: &str, language: &str) -> LexRuns {
    let Some(lexed) = lex_fence(code, language) else {
        let language = language::canonical_language(language);
        return LexRuns {
            plain: true,
            stable_byte: code.rfind('\n').map_or(0, |at| at + 1),
            lexed: Arc::new(LexedCode::plain(code, &language)),
        };
    };
    let at = lexed.checkpoint.at.min(code.len());
    let stable_byte = if at == 0 || code.as_bytes().get(at - 1) == Some(&b'\n') {
        at
    } else {
        code[..at].rfind('\n').map_or(0, |nl| nl + 1)
    };
    LexRuns {
        plain: false,
        stable_byte,
        lexed,
    }
}

pub(crate) fn append_syntax_spans(
    code: &str,
    language: &str,
    base: usize,
    spans: &mut Vec<crate::Span>,
) {
    let Some(lexed) = lex_fence(code, language) else {
        return;
    };
    spans.reserve(lexed.runs.len());
    for (from, to, tone) in lexed.utf16_ends(code) {
        if to > from {
            let (light, dark) = tone.colors();
            spans.push(crate::Span {
                start: base + from,
                end: base + to,
                style: crate::Style::Syntax {
                    light: light.into(),
                    dark: dark.into(),
                },
            });
        }
    }
}

/// Owned tokens for a whole source. Sources up to 8 MiB are
/// coloured in full; an unknown non-empty language uses a generic lexer.
pub fn highlight(code: &str, language: &str) -> Arc<HighlightedCode> {
    let lexed = lex(code, language);
    let build = || {
        let mut start = 0;
        let tokens = lexed
            .runs
            .iter()
            .map(|run| {
                let (light, dark) = run.tone.colors();
                let token = CodeToken {
                    text: code[start..run.end()].into(),
                    light: light.into(),
                    dark: dark.into(),
                };
                start = run.end();
                token
            })
            .collect();
        Arc::new(HighlightedCode {
            language: lexed.language.clone(),
            plain: lexed.plain,
            tokens,
        })
    };
    if lexed.runs.len() > MEMO_RUNS {
        return build();
    }
    lexed.tokens.get_or_init(build).clone()
}

/// Compact lexical result for Wasm. Words are version 1, plain flag, language
/// UTF-16 length, language UTF-16 units, then triples of end UTF-16 offset and
/// light/dark 24-bit RGB. Runs are contiguous and start at offset zero.
/// Sources up to 8 MiB are coloured in full (3 words per run).
pub fn highlight_packed(code: &str, language: &str) -> Vec<u32> {
    let lexed = lex(code, language);
    let build = || {
        let language_units = lexed.language.encode_utf16().count();
        let mut words = Vec::with_capacity(3 + language_units + lexed.runs.len() * 3);
        words.extend([1, u32::from(lexed.plain), language_units as u32]);
        words.extend(lexed.language.encode_utf16().map(u32::from));
        for (_, offset, tone) in lexed.utf16_ends(code) {
            let (light, dark) = tone.rgb();
            words.extend([offset as u32, light, dark]);
        }
        words
    };
    if lexed.runs.len() > MEMO_RUNS {
        return build();
    }
    // A repeated file copies the retained words instead of walking runs.
    lexed.packed.get_or_init(build).clone()
}
