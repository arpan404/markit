use crate::{Document, References};
use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};

/// Code payload for deferred web line rendering. No token objects or HTML rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebCode {
    pub source: String,
    pub info: crate::FenceInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderedBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codes: Vec<WebCode>,
    pub html: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<Document>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderPatch {
    pub replace_from: usize,
    /// Immutable blocks shared with the renderer and any retained patches.
    /// Serialization is identical to an owned block vector.
    pub blocks: Vec<Arc<RenderedBlock>>,
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
    open_fence: Option<crate::stream_fence::OpenFence>,
    blocks: Vec<Arc<RenderedBlock>>,
    settled: usize,
    settled_count: usize,
    references: References,
    html_only: bool,
    native_only: bool,
    web: bool,
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
    pub fn new_web() -> Self {
        Self {
            web: true,
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
            web: self.web,
            native_only: self.native_only,
            ..Self::default()
        };
    }
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn append(&mut self, delta: &str, finalized: bool) -> RenderPatch {
        self.source.push_str(delta);
        if let Some(mut fence) = self.open_fence.take() {
            if fence.continue_source(&self.source) {
                let mut codes = Vec::new();
                let next = fence.render(
                    &self.source,
                    self.html_only,
                    self.native_only,
                    self.web,
                    &mut codes,
                );
                let last = self.blocks.len() - 1;
                let changed = self.blocks[last] != next;
                let replace_from = if changed { last } else { last + 1 };
                let blocks = if changed {
                    self.blocks[last] = next.clone();
                    vec![next]
                } else {
                    Vec::new()
                };
                self.open_fence = Some(fence);
                return RenderPatch {
                    replace_from,
                    blocks,
                    finalized,
                    source_bytes: self.source.len(),
                    parsed_bytes: self.source.len() - self.settled,
                };
            }
        }
        let mut prepared = crate::math::prepare(&self.source[self.settled..]);
        // The parser owns block discovery and definition discovery together.
        // Its fallback only needs older definitions: new ones resolve in-parser.
        let previous_references = self.references.clone();
        let mut parser = crate::parser_with_refs(&prepared.source, &previous_references);
        if parser.reference_definitions().iter().next().is_some() {
            if self.settled > 0 {
                drop(parser);
                prepared = crate::math::prepare(&self.source);
                parser = crate::parser_with_refs(&prepared.source, &previous_references);
            }
            self.settled = 0;
            self.settled_count = 0;
            self.references.clear();
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
        let mut block_events = Vec::new();
        let mut events = Vec::new();
        for (event, range) in parser.into_offset_iter() {
            if depth > 0
                || matches!(
                    event,
                    pulldown_cmark::Event::Start(_) | pulldown_cmark::Event::Rule
                )
            {
                events.push(event.clone());
            }
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
                        block_events.push(std::mem::take(&mut events));
                    }
                }
                pulldown_cmark::Event::Rule if depth == 0 => {
                    ranges.push(range);
                    block_events.push(std::mem::take(&mut events));
                }
                _ => {}
            }
        }
        if prepared.unclosed.is_none() {
            if let (Some(range), Some(events)) = (ranges.last(), block_events.last()) {
                if let Some(pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(
                    pulldown_cmark::CodeBlockKind::Fenced(info),
                ))) = events.first()
                {
                    self.open_fence = crate::stream_fence::OpenFence::new(
                        &self.source,
                        self.settled + range.start,
                        info,
                    );
                }
            }
        }
        let mut next: Vec<Arc<RenderedBlock>> = Vec::new();
        for (range, events) in ranges.iter().zip(block_events) {
            let mut html = String::new();
            let mut codes = Vec::new();
            if self.web {
                let source = &tail[range.clone()];
                crate::html::render_html_code(
                    source,
                    &mut html,
                    &self.references,
                    &mut |source, info| {
                        let index = codes.len();
                        codes.push(WebCode {
                            source: source.into(),
                            info: info.clone(),
                        });
                        format!("<section class=\"mk-codeblock\" data-markit-code=\"{index}\"></section>\n")
                    },
                );
            } else if !self.native_only {
                crate::html::render_events(&prepared, events.iter().cloned(), &mut html);
            }
            let document = (!self.html_only)
                .then(|| crate::document::parse_events(&prepared, events.into_iter()));
            next.push(Arc::new(RenderedBlock {
                html,
                document,
                codes,
            }));
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
