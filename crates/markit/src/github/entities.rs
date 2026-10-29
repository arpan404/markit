//! Character references in raw forge HTML. Attribute values are decoded so
//! URL checks judge what a browser reads; text keeps its well-formed
//! references, which can only ever produce text.
use std::borrow::Cow;

/// Named references the decoder knows. Anything else stays as written, which
/// is still safe: callers re-escape the decoded text.
const NAMED: &[(&str, char)] = &[
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
    ("colon", ':'),
    ("semi", ';'),
    ("tab", '\t'),
    ("newline", '\n'),
    ("sol", '/'),
    ("bsol", '\\'),
    ("lpar", '('),
    ("rpar", ')'),
    ("period", '.'),
    ("comma", ','),
    ("excl", '!'),
    ("num", '#'),
    ("dollar", '$'),
    ("percnt", '%'),
    ("ast", '*'),
    ("plus", '+'),
    ("equals", '='),
    ("quest", '?'),
    ("commat", '@'),
    ("lsqb", '['),
    ("rsqb", ']'),
    ("lowbar", '_'),
    ("grave", '`'),
    ("lcub", '{'),
    ("rcub", '}'),
    ("verbar", '|'),
    ("hyphen", '-'),
    ("copy", '©'),
    ("reg", '®'),
    ("trade", '™'),
    ("hellip", '…'),
    ("mdash", '—'),
    ("ndash", '–'),
    ("lsquo", '‘'),
    ("rsquo", '’'),
    ("ldquo", '“'),
    ("rdquo", '”'),
    ("laquo", '«'),
    ("raquo", '»'),
    ("middot", '·'),
    ("bull", '•'),
    ("times", '×'),
    ("divide", '÷'),
    ("deg", '°'),
    ("plusmn", '±'),
    ("larr", '←'),
    ("rarr", '→'),
    ("uarr", '↑'),
    ("darr", '↓'),
    ("harr", '↔'),
    ("check", '✓'),
    ("hearts", '♥'),
    ("star", '☆'),
    ("zwj", '\u{200d}'),
    ("zwnj", '\u{200c}'),
    ("shy", '\u{ad}'),
    ("ensp", '\u{2002}'),
    ("emsp", '\u{2003}'),
    ("thinsp", '\u{2009}'),
];

/// One character reference at the start of `text` (just after `&`): the
/// character and the bytes it spans, `;` included.
fn reference(text: &str) -> Option<(char, usize)> {
    let bytes = text.as_bytes();
    if let Some(rest) = text.strip_prefix('#') {
        let (hex, digits) = match rest.as_bytes().first() {
            Some(b'x' | b'X') => (true, &rest[1..]),
            _ => (false, rest),
        };
        let len = digits
            .bytes()
            .take_while(|b| {
                if hex {
                    b.is_ascii_hexdigit()
                } else {
                    b.is_ascii_digit()
                }
            })
            .count();
        if len == 0 || len > 8 {
            return None;
        }
        let value = u32::from_str_radix(&digits[..len], if hex { 16 } else { 10 }).ok()?;
        let mut used = 1 + usize::from(hex) + len;
        if bytes.get(used) == Some(&b';') {
            used += 1;
        }
        let c = match value {
            0 => '\u{fffd}',
            value => char::from_u32(value).unwrap_or('\u{fffd}'),
        };
        return Some((c, used));
    }
    let len = bytes
        .iter()
        .take(32)
        .take_while(|b| b.is_ascii_alphanumeric())
        .count();
    if len == 0 || bytes.get(len) != Some(&b';') {
        return None;
    }
    NAMED
        .iter()
        .find(|(name, _)| *name == &text[..len])
        .map(|&(_, c)| (c, len + 1))
}

/// `text` with the character references this decoder knows replaced.
pub(crate) fn decode_entities(text: &str) -> Cow<'_, str> {
    if !text.contains('&') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        match reference(&rest[at + 1..]) {
            Some((c, used)) => {
                out.push(c);
                rest = &rest[at + 1 + used..];
            }
            None => {
                out.push('&');
                rest = &rest[at + 1..];
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// Whether `text` (just after `&`) starts with a well-formed reference that a
/// browser reads as one character of text.
fn well_formed_reference(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = if let Some(rest) = text.strip_prefix('#') {
        let (prefix, digits) = match rest.as_bytes().first() {
            Some(b'x' | b'X') => (2, &rest[1..]),
            _ => (1, rest),
        };
        let hex = prefix == 2;
        let count = digits
            .bytes()
            .take_while(|b| {
                if hex {
                    b.is_ascii_hexdigit()
                } else {
                    b.is_ascii_digit()
                }
            })
            .count();
        if count == 0 || count > 8 {
            return None;
        }
        prefix + count
    } else {
        let count = bytes
            .iter()
            .take(32)
            .take_while(|b| b.is_ascii_alphanumeric())
            .count();
        if count == 0 || !bytes[0].is_ascii_alphabetic() {
            return None;
        }
        count
    };
    (bytes.get(len) == Some(&b';')).then_some(len + 1)
}

/// HTML-escape raw `text` for a text position, keeping well-formed
/// character references so `&copy;` still reads as ©. A reference in text
/// can only ever produce text.
pub(crate) fn push_text(text: &str, out: &mut String) {
    let mut rest = text;
    while let Some(at) = rest.find(['&', '<', '>', '"', '\'']) {
        crate::html::escape_into(&rest[..at], out);
        let c = rest.as_bytes()[at];
        if c == b'&' {
            if let Some(len) = well_formed_reference(&rest[at + 1..]) {
                out.push_str(&rest[at..at + 1 + len]);
                rest = &rest[at + 1 + len..];
                continue;
            }
        }
        crate::html::escape_into(&rest[at..at + 1], out);
        rest = &rest[at + 1..];
    }
    crate::html::escape_into(rest, out);
}
