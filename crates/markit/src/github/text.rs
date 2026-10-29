//! How one run of forge text is drawn: bare links first (as in the default
//! flavour), then emoji and forge references in the text between them.
use super::autolink::{segments, Segment};
use super::front::TextMode;
use std::ops::Range;

pub(crate) enum Part {
    Link { range: Range<usize>, url: String },
    Segment(Segment),
}

impl Part {
    pub(crate) fn range(&self) -> &Range<usize> {
        match self {
            Self::Link { range, .. } => range,
            Self::Segment(segment) => segment.range(),
        }
    }
}

/// The parts of `text` in order, or `None` when it holds no emoji or
/// reference, so callers can keep their ordinary text path.
pub(crate) fn parts(text: &str, mode: TextMode) -> Option<Vec<Part>> {
    match mode {
        TextMode::Literal => None,
        TextMode::Link => {
            let found = segments(text, false);
            (!found.is_empty()).then(|| found.into_iter().map(Part::Segment).collect())
        }
        TextMode::Prose { references } => {
            let links = crate::html::bare_links(text);
            let mut parts = Vec::new();
            let mut at = 0;
            for (range, url) in links
                .into_iter()
                .map(|(range, url)| (range, Some(url)))
                .chain(std::iter::once((text.len()..text.len(), None)))
            {
                for segment in segments(&text[at..range.start], references) {
                    parts.push(Part::Segment(shift(segment, at)));
                }
                at = range.end;
                if let Some(url) = url {
                    parts.push(Part::Link { range, url });
                }
            }
            parts
                .iter()
                .any(|part| matches!(part, Part::Segment(_)))
                .then_some(parts)
        }
    }
}

fn shift(segment: Segment, by: usize) -> Segment {
    match segment {
        Segment::Emoji { range, emoji } => Segment::Emoji {
            range: range.start + by..range.end + by,
            emoji,
        },
        Segment::Reference {
            range,
            kind,
            target,
            display,
        } => Segment::Reference {
            range: range.start + by..range.end + by,
            kind,
            target,
            display,
        },
    }
}
