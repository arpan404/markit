//! What GitHub links or draws inside plain forge text: emoji shortcodes
//! (`:tada:`), `@user` and `@org/team` mentions, `#123` and `owner/repo#123`
//! issue references, and commit SHAs (`owner/repo@sha` too). The same
//! alternatives, order and guards as ace's forge text segments: at each
//! position the first alternative that matches wins and its text is consumed,
//! even when it draws nothing (an unknown shortcode, a SHA-like word).
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Segment {
    Emoji {
        range: Range<usize>,
        emoji: &'static str,
    },
    /// A forge reference: `kind` is `mention`, `issue` or `commit`.
    Reference {
        range: Range<usize>,
        kind: &'static str,
        target: String,
        display: String,
    },
}

impl Segment {
    pub(crate) fn range(&self) -> &Range<usize> {
        match self {
            Self::Emoji { range, .. } | Self::Reference { range, .. } => range,
        }
    }
}

/// JavaScript's `\w`.
fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Segments of `text`, in order and never overlapping. With `references`
/// unset (inside a link) only emoji are found.
pub(crate) fn segments(text: &str, references: bool) -> Vec<Segment> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    // Every alternative starts with one of these bytes or a hex digit.
    if !bytes
        .iter()
        .any(|&b| matches!(b, b':' | b'@' | b'#' | b'[') || b.is_ascii_hexdigit())
    {
        return found;
    }
    let mut at = 0;
    while at < bytes.len() {
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let matched = emoji(text, at)
            .or_else(|| footnote(text, at))
            .or_else(|| {
                references
                    .then(|| cross_reference(text, at, before))
                    .flatten()
            })
            .or_else(|| references.then(|| mention(text, at, before)).flatten())
            .or_else(|| references.then(|| issue(text, at, before)).flatten())
            .or_else(|| references.then(|| commit(text, at, before)).flatten());
        match matched {
            Some((end, segment)) => {
                found.extend(segment);
                at = end;
            }
            None => at += text[at..].chars().next().map_or(1, char::len_utf8),
        }
    }
    found
}

type Match = Option<(usize, Option<Segment>)>;

/// `:name:` with 1 to 64 of `[a-z0-9_+-]`.
fn emoji(text: &str, at: usize) -> Match {
    let bytes = text.as_bytes();
    if bytes[at] != b':' {
        return None;
    }
    let len = bytes[at + 1..]
        .iter()
        .take(65)
        .take_while(|&&b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'+' | b'-')
        })
        .count();
    if !(1..=64).contains(&len) || bytes.get(at + 1 + len) != Some(&b':') {
        return None;
    }
    let end = at + len + 2;
    let segment = super::emoji::lookup(&text[at + 1..end - 1]).map(|emoji| Segment::Emoji {
        range: at..end,
        emoji,
    });
    Some((end, segment))
}

/// `[^label]`: consumed so a label is never read as a SHA or mention. Labels
/// that name a footnote were already parsed as references.
fn footnote(text: &str, at: usize) -> Match {
    let rest = text[at..].strip_prefix("[^")?;
    for (count, (offset, c)) in rest.char_indices().enumerate() {
        if c == ']' {
            return (count > 0).then_some((at + 2 + offset + 1, None));
        }
        if c.is_whitespace() || count >= 64 {
            return None;
        }
    }
    None
}

/// Length of the longest owner (`[A-Za-z0-9]` then up to 38 of alphanumerics
/// or a hyphen followed by an alphanumeric) starting at `at`.
fn owner_len(bytes: &[u8], at: usize) -> usize {
    if !bytes.get(at).is_some_and(u8::is_ascii_alphanumeric) {
        return 0;
    }
    let mut len = 1;
    while len < 39 {
        match bytes.get(at + len) {
            Some(b) if b.is_ascii_alphanumeric() => len += 1,
            Some(b'-')
                if bytes
                    .get(at + len + 1)
                    .is_some_and(u8::is_ascii_alphanumeric) =>
            {
                len += 1
            }
            _ => break,
        }
    }
    len
}

/// `[1-9]\d{0,9}\b` at `at`: the end of the number.
fn number_end(bytes: &[u8], at: usize) -> Option<usize> {
    let len = bytes[at..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .count();
    if !(1..=10).contains(&len) || bytes[at] == b'0' {
        return None;
    }
    let end = at + len;
    (!bytes.get(end).copied().is_some_and(word)).then_some(end)
}

/// `[0-9a-f]{7,40}` at `at` not followed by a word byte (or `extra`).
fn sha_end(bytes: &[u8], at: usize, extra: u8) -> Option<usize> {
    let len = bytes[at..]
        .iter()
        .take_while(|&&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        .count();
    if !(7..=40).contains(&len) {
        return None;
    }
    let end = at + len;
    (!bytes.get(end).is_some_and(|&b| word(b) || b == extra)).then_some(end)
}

/// `owner/repo#123` and `owner/repo@sha`, not inside a path or URL.
fn cross_reference(text: &str, at: usize, before: Option<u8>) -> Match {
    let bytes = text.as_bytes();
    if before.is_some_and(|b| word(b) || matches!(b, b'/' | b'.' | b'@' | b'-')) {
        return None;
    }
    let owner = owner_len(bytes, at);
    if owner == 0 || bytes.get(at + owner) != Some(&b'/') {
        return None;
    }
    let repo_at = at + owner + 1;
    let repo = bytes[repo_at..]
        .iter()
        .take_while(|&&b| word(b) || b == b'.' || b == b'-')
        .count();
    if !(1..=100).contains(&repo) {
        return None;
    }
    let mark = repo_at + repo;
    let name = &text[at..mark];
    match bytes.get(mark) {
        Some(b'#') => {
            let end = number_end(bytes, mark + 1)?;
            let shown = &text[at..end];
            Some((
                end,
                Some(Segment::Reference {
                    range: at..end,
                    kind: "issue",
                    target: shown.into(),
                    display: shown.into(),
                }),
            ))
        }
        Some(b'@') => {
            let end = sha_end(bytes, mark + 1, 0)?;
            let sha = &text[mark + 1..end];
            Some((
                end,
                Some(Segment::Reference {
                    range: at..end,
                    kind: "commit",
                    target: format!("{name}@{sha}"),
                    display: format!("{name}@{}", &sha[..7]),
                }),
            ))
        }
        _ => None,
    }
}

/// `@login` or `@org/team`, not inside an e-mail address, path or code span.
fn mention(text: &str, at: usize, before: Option<u8>) -> Match {
    let bytes = text.as_bytes();
    if bytes[at] != b'@'
        || before.is_some_and(|b| word(b) || matches!(b, b'@' | b'/' | b'.' | b'`'))
    {
        return None;
    }
    let free = |end: usize| !bytes.get(end).is_some_and(|&b| word(b) || b == b'@');
    let owner_at = at + 1;
    for owner in (1..=owner_len(bytes, owner_at)).rev() {
        let after = owner_at + owner;
        if bytes.get(after) == Some(&b'/')
            && bytes.get(after + 1).is_some_and(u8::is_ascii_alphanumeric)
        {
            let run = bytes[after + 1..]
                .iter()
                .take(101)
                .take_while(|&&b| word(b) || b == b'.' || b == b'-')
                .count()
                .min(100);
            if let Some(team) = (1..=run).rev().find(|&team| free(after + 1 + team)) {
                let end = after + 1 + team;
                return Some((end, Some(reference(text, at, end, "mention", 1))));
            }
        }
        if free(after) {
            return Some((after, Some(reference(text, at, after, "mention", 1))));
        }
    }
    None
}

/// `#123`, not inside a word, path, entity or another reference.
fn issue(text: &str, at: usize, before: Option<u8>) -> Match {
    let bytes = text.as_bytes();
    if bytes[at] != b'#' || before.is_some_and(|b| word(b) || matches!(b, b'/' | b'#' | b'&')) {
        return None;
    }
    let end = number_end(bytes, at + 1)?;
    Some((end, Some(reference(text, at, end, "issue", 0))))
}

/// A commit SHA standing alone. Consumed even when it does not read as a
/// commit, so its letters are never re-read as something else.
fn commit(text: &str, at: usize, before: Option<u8>) -> Match {
    let bytes = text.as_bytes();
    if before.is_some_and(|b| word(b) || matches!(b, b'/' | b'@' | b'#' | b'.' | b'-')) {
        return None;
    }
    let end = sha_end(bytes, at, b'-')?;
    let sha = &text[at..end];
    // A full SHA, or a short one mixing digits and letters: `deadbeef`,
    // `1234567` and colours stay text.
    let commit_like = sha.len() == 40
        || (sha.bytes().any(|b| b.is_ascii_digit()) && sha.bytes().any(|b| b.is_ascii_lowercase()));
    let segment = commit_like.then(|| Segment::Reference {
        range: at..end,
        kind: "commit",
        target: sha.into(),
        display: sha[..7].into(),
    });
    Some((end, segment))
}

/// A reference whose target is its text without the first `skip` bytes.
fn reference(text: &str, start: usize, end: usize, kind: &'static str, skip: usize) -> Segment {
    Segment::Reference {
        range: start..end,
        kind,
        target: text[start + skip..end].into(),
        display: text[start..end].into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<(String, &'static str, String)> {
        segments(text, true)
            .into_iter()
            .map(|segment| match segment {
                Segment::Emoji { range, emoji } => {
                    (text[range].to_string(), "emoji", emoji.to_string())
                }
                Segment::Reference {
                    range,
                    kind,
                    target,
                    ..
                } => (text[range].to_string(), kind, target),
            })
            .collect()
    }

    #[test]
    fn mentions_teams_issues_and_cross_references_are_found() {
        assert_eq!(
            found("cc @octocat and @github/docs-team on #12 and rust-lang/rust#99"),
            [
                ("@octocat".into(), "mention", "octocat".into()),
                (
                    "@github/docs-team".into(),
                    "mention",
                    "github/docs-team".into()
                ),
                ("#12".into(), "issue", "#12".into()),
                (
                    "rust-lang/rust#99".into(),
                    "issue",
                    "rust-lang/rust#99".into()
                ),
            ]
        );
    }

    #[test]
    fn emails_paths_entities_and_words_are_not_references() {
        for text in [
            "mail a@b.com",
            "see path/to/#12",
            "&#123; entity",
            "issue#12",
            "@user_name",
            "a/b/c#1",
            "x.y/z#1",
            "#0",
            "#12345678901",
            "`@code",
        ] {
            assert_eq!(found(text), [], "{text}");
        }
    }

    #[test]
    fn commit_shas_need_digits_and_letters_unless_full_length() {
        let full = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            found(&format!("fixed in a1b2c3d and {full}")),
            [
                ("a1b2c3d".into(), "commit", "a1b2c3d".into()),
                (full.into(), "commit", full.into()),
            ]
        );
        for text in [
            "deadbeef",
            "1234567",
            "#a1b2c3",
            "a1b2c3d-x",
            "a1b2c3dz",
            "A1B2C3D",
            "abc",
        ] {
            assert_eq!(found(text), [], "{text}");
        }
        assert_eq!(
            found("owner/repo@a1b2c3d4e5"),
            [(
                "owner/repo@a1b2c3d4e5".into(),
                "commit",
                "owner/repo@a1b2c3d4e5".into()
            )]
        );
    }

    #[test]
    fn known_shortcodes_become_emoji_and_unknown_ones_stay() {
        assert_eq!(
            found(":tada: :not_an_emoji_name: 10:30:45"),
            [(":tada:".into(), "emoji", "🎉".into())]
        );
        // Inside a link only emoji are drawn.
        assert_eq!(segments("@octocat :+1:", false).len(), 1);
    }
}
