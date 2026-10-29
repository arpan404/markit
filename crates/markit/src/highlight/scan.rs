//! The lexical scanner: one forward pass that splits a source into
//! contiguous tone runs. Rules come from a [`Profile`]; no language names
//! are compared here.
use super::language::{BlockComment, LineMarker, Profile};
use super::tone::{step_len, word_byte, Run, Tone};

/// Resumable scanner state at a token start. Tokens are scanned whole, so
/// resuming here reproduces exactly the runs a cold scan would emit.
#[derive(Clone, Copy, Default)]
pub(super) struct Checkpoint {
    pub at: usize,
    pub in_tag: bool,
    pub runs: usize,
    pub pending_start: usize,
    pub pending_tone: Option<Tone>,
}

pub(super) struct Scanned {
    pub checkpoint: Checkpoint,
    pub runs: Vec<Run>,
}

/// Scan `code`, resuming from `previous` (the checkpoint of a scan of a
/// prefix of `code` and that scan's runs truncated to it). After `max_runs`
/// runs the rest of the source becomes one plain run: colour stays exact up
/// to that point, and memory is bounded on adversarial input.
pub(super) fn scan(
    code: &str,
    profile: &Profile,
    previous: Option<(Checkpoint, Vec<Run>)>,
    max_runs: usize,
) -> Scanned {
    let bytes = code.as_bytes();
    let (mut checkpoint, mut runs) = previous.unwrap_or_default();
    let mut at = checkpoint.at;
    let mut in_tag = checkpoint.in_tag;
    let line_start = code.rfind('\n').map_or(0, |at| at + 1);
    let mut future_dependent = false;
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
        let tone;
        (at, tone) = token(code, start, profile, &mut in_tag, &mut future_dependent);
        if tone != pending_tone {
            if start > pending_start {
                runs.push(run(start, pending_tone));
            }
            pending_start = start;
            pending_tone = tone;
        }
        if runs.len() >= max_runs && at < bytes.len() {
            if pending_tone != Tone::Plain {
                runs.push(run(at, pending_tone));
                pending_start = at;
                pending_tone = Tone::Plain;
            }
            at = bytes.len();
            // Capped runs depend on the whole source: never resume them.
            checkpoint = Checkpoint::default();
            future_dependent = true;
        }
    }
    if at > pending_start {
        runs.push(run(at, pending_tone));
    }
    Scanned { checkpoint, runs }
}

fn run(end: usize, tone: Tone) -> Run {
    Run {
        end: u32::try_from(end).expect("lexed sources are capped below 4 GiB"),
        tone,
    }
}

/// Scan one token starting at `at`; returns its end and tone.
fn token(
    code: &str,
    at: usize,
    profile: &Profile,
    in_tag: &mut bool,
    future_dependent: &mut bool,
) -> (usize, Tone) {
    let bytes = code.as_bytes();
    let byte = bytes[at];
    if let Some(end) = comment_end(code, at, profile) {
        return (end, Tone::Comment);
    }
    if profile.rust && byte == b'r' && matches!(bytes.get(at + 1), Some(b'#' | b'"')) {
        return match raw_string_end(bytes, at + 1) {
            Some(end) => (end, Tone::String),
            None => (at + 1, Tone::Plain),
        };
    }
    if profile.long_brackets && byte == b'[' {
        if let Some(end) = long_bracket_end(code, at) {
            return (end, Tone::String);
        }
    }
    if matches!(byte, b'"' | b'\'' | b'`') {
        let after_word = at > 0 && word_byte(bytes[at - 1]);
        // Rust lifetimes are identifiers rather than unterminated strings.
        let lifetime = profile.rust
            && byte == b'\''
            && bytes.get(at + 1).is_some_and(|b| b.is_ascii_alphabetic())
            && bytes.get(at + 2) != Some(&b'\'');
        if lifetime {
            return (word_end(bytes, at + 1), Tone::Plain);
        }
        if profile.apostrophe_text && byte == b'\'' && after_word {
            return (at + 1, Tone::Plain);
        }
        return (string_end(bytes, at, profile), Tone::String);
    }
    if byte.is_ascii_digit() && (at == 0 || !word_byte(bytes[at - 1])) {
        let mut end = at + 1;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'.' | b'_'))
        {
            end += 1;
        }
        return (end, Tone::Number);
    }
    if byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$') {
        let end = word_end(bytes, at + 1);
        let word = &code[at..end];
        let after = code[end..].trim_start();
        *future_dependent |= after.is_empty();
        let tone = if (profile.keywords)(word) {
            Tone::Keyword
        } else if *in_tag && profile.markup {
            if after.starts_with('=') {
                Tone::Property
            } else {
                Tone::Tag
            }
        } else if after.starts_with('(') {
            Tone::Function
        } else if after.starts_with(':')
            || (profile.assign_property && after.starts_with('=') && !after.starts_with("=="))
        {
            Tone::Property
        } else if word.as_bytes()[0].is_ascii_uppercase() {
            Tone::Type
        } else {
            Tone::Plain
        };
        return (end, tone);
    }
    if profile.markup {
        if byte == b'<' {
            *in_tag = true;
        } else if byte == b'>' {
            *in_tag = false;
        }
    }
    // Punctuation shares the plain palette entry and merges into its run.
    (at + step_len(byte), Tone::Plain)
}

fn word_end(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && word_byte(bytes[at]) {
        at += 1;
    }
    at
}

fn line_end(bytes: &[u8], at: usize) -> usize {
    bytes[at..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |n| at + n)
}

fn after_space(bytes: &[u8], at: usize) -> bool {
    at == 0 || matches!(bytes[at - 1], b' ' | b'\t' | b'\n' | b'\r')
}

fn marker(rule: LineMarker, bytes: &[u8], at: usize) -> bool {
    match rule {
        LineMarker::Never => false,
        LineMarker::Always => true,
        LineMarker::AfterSpace => after_space(bytes, at),
    }
}

/// End of a comment starting at `at`, if one does.
fn comment_end(code: &str, at: usize, profile: &Profile) -> Option<usize> {
    let bytes = code.as_bytes();
    let next = bytes.get(at + 1).copied();
    let line = match bytes[at] {
        b'#' => marker(profile.hash, bytes, at),
        b'/' => profile.slash && next == Some(b'/'),
        b';' => marker(profile.semicolon, bytes, at),
        b'-' if next == Some(b'-') => match profile.dash {
            LineMarker::AfterSpace => {
                after_space(bytes, at) && bytes.get(at + 2).is_none_or(|b| b.is_ascii_whitespace())
            }
            rule => marker(rule, bytes, at),
        },
        _ => false,
    };
    if line {
        if bytes[at] == b'-' && profile.long_brackets {
            if let Some(end) = long_bracket_end(code, at + 2) {
                return Some(end);
            }
        }
        return Some(line_end(bytes, at));
    }
    if bytes[at] == b'/' && next == Some(b'*') && profile.block != BlockComment::Never {
        return Some(block_comment_end(
            bytes,
            at + 2,
            profile.block == BlockComment::Nested,
        ));
    }
    if profile.markup && bytes[at] == b'<' && bytes.get(at + 1..at + 4) == Some(b"!--".as_slice()) {
        return Some(
            code[at + 4..]
                .find("-->")
                .map_or(bytes.len(), |end| at + 4 + end + 3),
        );
    }
    None
}

fn block_comment_end(bytes: &[u8], mut at: usize, nested: bool) -> usize {
    let mut depth = 1;
    while at < bytes.len() && depth > 0 {
        if nested && bytes[at] == b'/' && bytes.get(at + 1) == Some(&b'*') {
            depth += 1;
            at += 2;
        } else if bytes[at] == b'*' && bytes.get(at + 1) == Some(&b'/') {
            depth -= 1;
            at += 2;
        } else {
            at += step_len(bytes[at]);
        }
    }
    at
}

/// `r"…"`, `r#"…"#`: `at` is just after the `r`. `None` when no quote
/// follows the hashes (then the `r` is an ordinary identifier start).
fn raw_string_end(bytes: &[u8], at: usize) -> Option<usize> {
    let hashes = bytes[at..].iter().take_while(|&&b| b == b'#').count();
    let quote = at + hashes;
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let mut from = quote + 1;
    while let Some(offset) = bytes[from..].iter().position(|&b| b == b'"') {
        let close = from + offset;
        let tail = &bytes[close + 1..];
        if tail.len() >= hashes && tail[..hashes].iter().all(|&b| b == b'#') {
            return Some(close + 1 + hashes);
        }
        from = close + 1;
    }
    Some(bytes.len())
}

/// Lua long bracket `[[…]]` or `[==[…]==]` opening at `at`.
fn long_bracket_end(code: &str, at: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    if bytes.get(at) != Some(&b'[') {
        return None;
    }
    let level = bytes[at + 1..].iter().take_while(|&&b| b == b'=').count();
    if bytes.get(at + 1 + level) != Some(&b'[') {
        return None;
    }
    let mut from = at + 2 + level;
    while let Some(offset) = bytes[from..].iter().position(|&b| b == b']') {
        let close = from + offset;
        let tail = &bytes[close + 1..];
        if tail.len() > level && tail[..level].iter().all(|&b| b == b'=') && tail[level] == b']' {
            return Some(close + level + 2);
        }
        from = close + 1;
    }
    Some(bytes.len())
}

/// End of a quoted string opening at `at`, including escapes and, where the
/// profile allows it, triple quotes.
fn string_end(bytes: &[u8], at: usize, profile: &Profile) -> usize {
    let quote = bytes[at];
    let triple = profile.triple_quotes
        && bytes
            .get(at..at + 3)
            .is_some_and(|part| part == [quote, quote, quote]);
    let marker = if triple { 3 } else { 1 };
    let multiline = triple || profile.multiline_strings;
    let mut at = at + marker;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte == b'\\' {
            at += 1;
            if at < bytes.len() {
                at += step_len(bytes[at]);
            }
        } else if byte == quote
            && (!triple
                || bytes
                    .get(at..at + 3)
                    .is_some_and(|part| part == [quote, quote, quote]))
        {
            return at + marker;
        } else if byte == b'\n' && !multiline {
            return at;
        } else {
            at += step_len(byte);
        }
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust() -> &'static Profile {
        super::super::language::profile("rust").unwrap()
    }

    #[test]
    fn runs_past_the_cap_become_one_plain_tail_and_earlier_runs_are_unchanged() {
        let code = "let a = 1; // one\nlet b = \"two\";\nfn c() {}\n".repeat(20);
        let full = scan(&code, rust(), None, usize::MAX).runs;
        let capped = scan(&code, rust(), None, 10).runs;
        assert!(full.len() > 20);
        assert!(capped.len() <= 12);
        let last = *capped.last().unwrap();
        assert_eq!(last.end(), code.len());
        assert_eq!(last.tone, Tone::Plain);
        let kept = &capped[..capped.len() - 1];
        assert_eq!(kept, &full[..kept.len()]);
    }

    #[test]
    fn a_capped_scan_is_never_resumed_from_a_checkpoint() {
        let code = "let a = 1;\n".repeat(50);
        let capped = scan(&code, rust(), None, 4);
        assert_eq!(capped.checkpoint.at, 0);
        assert_eq!(capped.checkpoint.runs, 0);
    }
}
