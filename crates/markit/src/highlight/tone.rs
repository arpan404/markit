//! Token tones, their wire palette, and the byte helpers the scanner shares.

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(super) enum Tone {
    Plain,
    Comment,
    String,
    Number,
    Keyword,
    Function,
    Type,
    Property,
    Tag,
}

impl Tone {
    pub(super) fn palette_id(self) -> usize {
        match self {
            Self::Plain => 0,
            Self::Comment => 1,
            Self::String => 2,
            Self::Number | Self::Property => 3,
            Self::Keyword => 4,
            Self::Function => 5,
            Self::Type => 6,
            Self::Tag => 7,
        }
    }
    pub(super) fn colors(self) -> (&'static str, &'static str) {
        let p = PALETTE[self.palette_id()];
        (p.0, p.1)
    }
    pub(super) fn rgb(self) -> (u32, u32) {
        let p = PALETTE[self.palette_id()];
        (p.2, p.3)
    }
}

/// One lexical run: its exclusive byte end and tone. Ends are `u32` because
/// lexed sources are capped far below 4 GiB, which halves the memory of a
/// large file's runs compared with `usize` ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Run {
    pub end: u32,
    pub tone: Tone,
}

impl Run {
    pub(super) fn end(self) -> usize {
        self.end as usize
    }
}

pub(super) fn word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

/// UTF-16 length with a SIMD-friendly ASCII fast path. Code is usually ASCII;
/// `encode_utf16().count()` fully decodes UTF-8, which costs a second pass
/// over every run. Identical result on all inputs.
pub(crate) fn utf16_len(text: &str) -> usize {
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
pub(super) fn step_len(byte: u8) -> usize {
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
