use crate::{Document, References};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderedBlock {
    pub html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<Document>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderPatch {
    pub replace_from: usize,
    pub blocks: Vec<RenderedBlock>,
    pub finalized: bool,
    pub source_bytes: usize,
    /// Source bytes reconsidered this update. Excludes repeated parser passes.
    pub parsed_bytes: usize,
}

/// Append-only renderer with stable top-level blocks. The last block stays open
/// for setext headings, tables, loose lists and incomplete fences. Definitions
/// can change older references, so updates containing them invalidate the prefix.
/// Hosts retain prior blocks and replace only the suffix named by each patch.
#[derive(Default)]
pub struct StreamRenderer {
    source: String,
    blocks: Vec<RenderedBlock>,
    settled: usize,
    settled_count: usize,
    references: References,
    html_only: bool,
    native_only: bool,
}

impl StreamRenderer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn new_html() -> Self {
        Self {
            html_only: true,
            ..Self::default()
        }
    }
    pub fn new_native() -> Self {
        Self {
            native_only: true,
            ..Self::default()
        }
    }
    pub fn reset(&mut self) {
        *self = Self {
            html_only: self.html_only,
            native_only: self.native_only,
            ..Self::default()
        };
    }
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn append(&mut self, delta: &str, finalized: bool) -> RenderPatch {
        self.source.push_str(delta);
        let initial_prepared = crate::math::prepare(&self.source[self.settled..]);
        let has_definitions = crate::parser(&initial_prepared.source)
            .reference_definitions()
            .iter()
            .next()
            .is_some();
        if has_definitions {
            self.settled = 0;
            self.settled_count = 0;
            self.references.clear();
            let full_prepared = crate::math::prepare(&self.source);
            let parser = crate::parser(&full_prepared.source);
            for (label, definition) in parser.reference_definitions().iter() {
                self.references
                    .entry(crate::reference_key(label))
                    .or_insert_with(|| {
                        (
                            definition.dest.to_string(),
                            definition.title.as_deref().unwrap_or("").to_owned(),
                        )
                    });
            }
        }
        let tail = &self.source[self.settled..];
        let parsed_bytes = tail.len();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut depth = 0usize;
        let mut start = 0;
        let prepared = crate::math::prepare(tail);
        for (event, range) in crate::parser(&prepared.source).into_offset_iter() {
            let range = prepared.original_offset(range.start, false)
                ..prepared.original_offset(range.end, true);
            match event {
                pulldown_cmark::Event::Start(_) => {
                    if depth == 0 {
                        start = range.start;
                    }
                    depth += 1;
                }
                pulldown_cmark::Event::End(_) => {
                    depth -= 1;
                    if depth == 0 {
                        ranges.push(start..range.end);
                    }
                }
                pulldown_cmark::Event::Rule if depth == 0 => ranges.push(range),
                _ => {}
            }
        }
        let mut next: Vec<RenderedBlock> = Vec::new();
        for range in &ranges {
            let source = &tail[range.clone()];
            let mut html = String::new();
            if !self.native_only {
                crate::html::render_html_with_refs(source, &mut html, &self.references);
            }
            let document = (!self.html_only)
                .then(|| crate::document::parse_with_refs(source, &self.references));
            next.push(RenderedBlock { html, document });
        }
        let common = self
            .blocks
            .iter()
            .skip(self.settled_count)
            .zip(&next)
            .take_while(|(old, new)| old == new)
            .count();
        let replace_from = self.settled_count + common;
        let blocks = next[common..].to_vec();
        self.blocks.truncate(replace_from);
        self.blocks.extend(next.into_iter().skip(common));
        // A partial first line can still join the preceding table/list. Hold
        // two blocks until that line completes, and one block afterward.
        let hold = if ranges
            .last()
            .is_some_and(|last| tail[last.start..].contains('\n'))
        {
            1
        } else {
            2
        };
        let available = ranges.len().saturating_sub(hold);
        let stable = ranges[..available]
            .iter()
            .take_while(|range| {
                prepared
                    .unclosed
                    .is_none_or(|unclosed| range.end <= unclosed)
            })
            .count();
        if stable > 0 {
            self.settled += ranges[stable - 1].end;
            self.settled_count += stable;
        }
        RenderPatch {
            replace_from,
            blocks,
            finalized,
            source_bytes: self.source.len(),
            parsed_bytes,
        }
    }
}
