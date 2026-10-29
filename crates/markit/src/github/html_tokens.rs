//! Raw HTML from forge text as a flat run of pieces: start and end tags with
//! their attributes, text and comments. Close to the HTML tokenizer, but it
//! never builds a tree and decides nothing about what is allowed. Every piece
//! is re-serialized by the caller, so a reading that differs from a browser's
//! can change what is drawn but never lets markup through unescaped.
use super::entities::decode_entities;
use std::borrow::Cow;

#[derive(Debug, PartialEq)]
pub(crate) enum Piece<'a> {
    /// Lowercase tag name, entity-decoded attribute values (first of a
    /// repeated name wins) and the tag as written.
    Open {
        tag: String,
        attrs: Vec<(String, String)>,
        raw: &'a str,
    },
    Close {
        tag: String,
        raw: &'a str,
    },
    /// Text as written; character references are still encoded.
    Text(&'a str),
    /// Comments, processing instructions, doctypes and bogus comments.
    Comment,
}

/// `source` as pieces, in order. A tag cut off by the end of the input is text.
pub(crate) fn pieces(source: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut text_start = 0;
    let mut at = 0;
    while let Some(offset) = source[at..].find('<') {
        let lt = at + offset;
        let Some((piece, end)) = markup(source, lt) else {
            at = lt + 1;
            continue;
        };
        if lt > text_start {
            pieces.push(Piece::Text(&source[text_start..lt]));
        }
        pieces.push(piece);
        at = end;
        text_start = end;
    }
    if text_start < source.len() {
        pieces.push(Piece::Text(&source[text_start..]));
    }
    pieces
}

/// The markup starting at `lt` (a `<`) and the byte after it, or `None` when
/// that `<` is plain text.
fn markup(source: &str, lt: usize) -> Option<(Piece<'_>, usize)> {
    let bytes = source.as_bytes();
    let next = *bytes.get(lt + 1)?;
    if next.is_ascii_alphabetic() {
        let (tag, attrs, end) = tag_body(source, lt + 1)?;
        let raw = &source[lt..end];
        return Some((Piece::Open { tag, attrs, raw }, end));
    }
    if next == b'/' {
        if bytes.get(lt + 2).is_some_and(u8::is_ascii_alphabetic) {
            // Attributes on an end tag are read and ignored, as in a browser.
            let (tag, _, end) = tag_body(source, lt + 2)?;
            let raw = &source[lt..end];
            return Some((Piece::Close { tag, raw }, end));
        }
        return bogus_comment(source, lt + 2);
    }
    if let Some(rest) = source[lt..].strip_prefix("<!--") {
        let body = lt + 4;
        // `<!-->` and `<!--->` are complete, empty comments.
        let end = if rest.starts_with('>') {
            body + 1
        } else if rest.starts_with("->") {
            body + 2
        } else {
            rest.find("-->").map_or(source.len(), |at| body + at + 3)
        };
        return Some((Piece::Comment, end));
    }
    if next == b'!' || next == b'?' {
        return bogus_comment(source, lt + 2);
    }
    None
}

fn bogus_comment(source: &str, from: usize) -> Option<(Piece<'_>, usize)> {
    let end = source[from..]
        .find('>')
        .map_or(source.len(), |at| from + at + 1);
    Some((Piece::Comment, end))
}

/// A lowercase tag or attribute name starting at `at`, and the byte after it.
fn name_at(source: &str, at: usize) -> (String, usize) {
    let end = source[at..]
        .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        .map_or(source.len(), |offset| at + offset);
    (source[at..end].to_ascii_lowercase(), end)
}

type Attributes = Vec<(String, String)>;

/// The name and attributes of a tag whose name starts at `name`, and the byte
/// after its `>`. `None` when the input ends inside the tag.
fn tag_body(source: &str, name: usize) -> Option<(String, Attributes, usize)> {
    let bytes = source.as_bytes();
    let (tag, mut at) = name_at(source, name);
    let mut attrs: Vec<(String, String)> = Vec::new();
    loop {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') {
            at += 1;
        }
        let &byte = bytes.get(at)?;
        if byte == b'>' {
            return Some((tag, attrs, at + 1));
        }
        // An attribute name may begin with `=`; it ends at space, `/`, `>` or `=`.
        let name_end = source[at + 1..]
            .find(|c: char| c.is_ascii_whitespace() || matches!(c, '/' | '>' | '='))
            .map_or(source.len(), |offset| at + 1 + offset);
        let name = source[at..name_end].to_ascii_lowercase();
        at = name_end;
        let mut probe = at;
        while probe < bytes.len() && bytes[probe].is_ascii_whitespace() {
            probe += 1;
        }
        let mut value = Cow::Borrowed("");
        if bytes.get(probe) == Some(&b'=') {
            at = probe + 1;
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            let &first = bytes.get(at)?;
            let raw = if first == b'"' || first == b'\'' {
                let close = source[at + 1..].find(first as char)? + at + 1;
                let raw = &source[at + 1..close];
                at = close + 1;
                raw
            } else {
                let end = source[at..]
                    .find(|c: char| c.is_ascii_whitespace() || c == '>')
                    .map_or(source.len(), |offset| at + offset);
                let raw = &source[at..end];
                at = end;
                raw
            };
            value = decode_entities(raw);
        }
        if !attrs.iter().any(|(existing, _)| *existing == name) {
            attrs.push((name, value.into_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::entities::push_text;

    fn open(source: &str) -> (String, Vec<(String, String)>) {
        match pieces(source).into_iter().next() {
            Some(Piece::Open { tag, attrs, .. }) => (tag, attrs),
            other => panic!("{source}: {other:?}"),
        }
    }

    #[test]
    fn attributes_decode_quote_styles_entities_and_keep_the_first_repeat() {
        let (tag, attrs) = open("<IMG Src='a.png' alt=\"x &amp; y\" w=10 src=b.png>");
        assert_eq!(tag, "img");
        assert_eq!(
            attrs,
            [
                ("src".into(), "a.png".into()),
                ("alt".into(), "x & y".into()),
                ("w".into(), "10".into()),
            ]
        );
        let (_, attrs) = open("<a href=\"jav&#x09;ascript&colon;x\">");
        assert_eq!(attrs[0].1, "jav\tascript:x");
    }

    #[test]
    fn attributes_joined_by_a_quote_still_split_into_two() {
        let (_, attrs) = open("<img src=\"https://ok.example/a.png\"onerror=alert(1)>");
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[1].0, "onerror");
    }

    #[test]
    fn cut_off_tags_are_text_and_comments_disappear() {
        assert_eq!(pieces("a <b"), [Piece::Text("a <b")]);
        assert_eq!(
            pieces("x<!-- <b> -->y<!-->z"),
            [
                Piece::Text("x"),
                Piece::Comment,
                Piece::Text("y"),
                Piece::Comment,
                Piece::Text("z")
            ]
        );
        assert_eq!(pieces("1 < 2"), [Piece::Text("1 < 2")]);
    }

    #[test]
    fn text_escaping_keeps_references_but_not_markup() {
        let mut out = String::new();
        push_text("&copy; &#x1F389; & <b> &bogus", &mut out);
        assert_eq!(out, "&copy; &#x1F389; &amp; &lt;b&gt; &amp;bogus");
        assert_eq!(decode_entities("&lt;&#65;&#x42;&unknown;"), "<AB&unknown;");
    }
}
