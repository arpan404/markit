use crate::{Document, References};
use serde::{Deserialize, Serialize};
use std::{ops::Range, sync::Arc};

/// Code payload for deferred web line rendering. No token objects or HTML rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebCode {
    pub source: String,
    pub info: crate::FenceInfo,
    /// UTF-16 offsets of `source` inside the stream's Markdown, when `source`
    /// is an exact slice. Nested fences that the parser rewrites leave this
    /// empty and the web patch carries the body bytes instead.
    #[serde(skip)]
    pub utf16: Option<(u32, u32)>,
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
    /// Markdown source of each block, kept only while the block can still be
    /// reopened. Identical source with unchanged references reuses the block.
    block_sources: Vec<Arc<str>>,
    settled: usize,
    settled_count: usize,
    references: References,
    html_only: bool,
    native_only: bool,
    web: bool,
    /// The last block is an open fence whose document was sent as an MKNP v2
    /// extension and not rebuilt. It is rebuilt before any other update.
    stale_fence: bool,
    flavor: crate::Flavor,
    /// GitHub flavour: the text defines a footnote, so later text can
    /// renumber or link older blocks. Nothing settles while it does.
    footnotes: bool,
    /// GitHub flavour: task boxes in settled blocks.
    settled_tasks: usize,
    /// GitHub flavour: source byte offset and first task index of each block,
    /// which a reused block must also match.
    block_keys: Vec<(usize, usize)>,
}

/// Result of an append whose open-fence change was not materialized.
enum Appended {
    Patch(RenderPatch),
    Fence {
        index: usize,
        change: crate::fence_doc::FenceChange,
        finalized: bool,
        source_bytes: usize,
        parsed_bytes: usize,
    },
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
    /// Render in `flavor` from the next update on. Resets the stream when
    /// the flavour changes.
    pub fn with_flavor(mut self, flavor: crate::Flavor) -> Self {
        self.set_flavor(flavor);
        self
    }
    /// See [`with_flavor`](Self::with_flavor).
    pub fn set_flavor(&mut self, flavor: crate::Flavor) {
        if flavor != self.flavor {
            self.flavor = flavor;
            self.reset();
        }
    }
    pub fn flavor(&self) -> crate::Flavor {
        self.flavor
    }
    pub fn reset(&mut self) {
        *self = Self {
            html_only: self.html_only,
            web: self.web,
            native_only: self.native_only,
            flavor: self.flavor,
            ..Self::default()
        };
    }
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn append(&mut self, delta: &str, finalized: bool) -> RenderPatch {
        match self.append_with(delta, finalized, false) {
            Appended::Patch(patch) => patch,
            Appended::Fence { .. } => unreachable!("only lazy appends defer fences"),
        }
    }

    /// Append and encode the update as an MKNP version 2 patch. In a native
    /// stream an open fence sends only its changed suffix, so a long live
    /// fence costs time and bytes proportional to each append rather than to
    /// the fence. Other streams send whole documents. See
    /// docs/binary-transport.md.
    pub fn append_native_patch(
        &mut self,
        delta: &str,
        finalized: bool,
    ) -> Result<Vec<u8>, crate::BinaryError> {
        let lazy = self.native_only;
        match self.append_with(delta, finalized, lazy) {
            Appended::Patch(patch) => crate::binary::encode_native_patch_v2(&patch),
            Appended::Fence {
                index,
                change,
                finalized,
                source_bytes,
                parsed_bytes,
            } => crate::binary::encode_fence_patch(
                index,
                &change,
                finalized,
                source_bytes,
                parsed_bytes,
            ),
        }
    }

    fn materialize_fence(&mut self) {
        if !std::mem::take(&mut self.stale_fence) {
            return;
        }
        if let (Some(fence), Some(last)) = (self.open_fence.as_ref(), self.blocks.last_mut()) {
            *last = Arc::new(RenderedBlock {
                codes: Vec::new(),
                html: String::new(),
                document: Some(fence.document(&self.source)),
            });
        }
    }

    fn append_with(&mut self, delta: &str, finalized: bool, lazy: bool) -> Appended {
        self.materialize_fence();
        self.source.push_str(delta);
        if let Some(mut fence) = self.open_fence.take() {
            if lazy && fence.continue_source(&self.source) {
                let fresh = fence.fresh;
                let mut change = fence.native_change(&self.source);
                if let crate::fence_doc::FenceChange::Replace(document) = &change {
                    // The parser already sent this fence; skip an identical copy.
                    if fresh
                        && self.blocks.last().and_then(|block| block.document.as_ref())
                            == Some(document)
                    {
                        change = crate::fence_doc::FenceChange::Unchanged;
                    }
                }
                let index = self.blocks.len() - 1;
                self.stale_fence = true;
                self.open_fence = Some(fence);
                return Appended::Fence {
                    index,
                    change,
                    finalized,
                    source_bytes: self.source.len(),
                    parsed_bytes: self.source.len() - self.settled,
                };
            }
            if !lazy && fence.continue_source(&self.source) {
                let mut codes = Vec::new();
                let fresh = fence.fresh;
                let before = fence.rendered_end();
                let next = fence.render(
                    &self.source,
                    self.html_only,
                    self.native_only,
                    self.web,
                    &mut codes,
                );
                let last = self.blocks.len() - 1;
                // A longer fence body always changes the block. Comparing every
                // span would make each append linear in the fence length.
                let grew = !fresh && fence.rendered_end() != before;
                let changed = grew || self.blocks[last] != next;
                let replace_from = if changed { last } else { last + 1 };
                let blocks = if changed {
                    self.blocks[last] = next.clone();
                    vec![next]
                } else {
                    Vec::new()
                };
                self.open_fence = Some(fence);
                return Appended::Patch(RenderPatch {
                    replace_from,
                    blocks,
                    finalized,
                    source_bytes: self.source.len(),
                    parsed_bytes: self.source.len() - self.settled,
                });
            }
        }
        let github = self.flavor.is_github();
        if github
            && self.settled > 0
            && (self.footnotes || crate::github::may_define(&self.source[self.settled..]))
        {
            self.settled = 0;
            self.settled_count = 0;
            self.settled_tasks = 0;
        }
        let mut prepared = crate::math::prepare(&self.source[self.settled..]);
        // The parser owns block discovery and definition discovery together.
        // Its fallback only needs older definitions: new ones resolve in-parser.
        let previous_references = self.references.clone();
        let mut parser = crate::parser_for(&prepared.source, &previous_references, self.flavor);
        let mut references_changed = false;
        if parser.reference_definitions().iter().next().is_some() {
            references_changed = true;
            if self.settled > 0 {
                drop(parser);
                prepared = crate::math::prepare(&self.source);
                parser = crate::parser_for(&prepared.source, &previous_references, self.flavor);
            }
            self.settled = 0;
            self.settled_count = 0;
            self.settled_tasks = 0;
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
        // GitHub flavour: each event's source range, for task offsets.
        let mut block_spans = Vec::new();
        let mut spans = Vec::new();
        for (event, range) in parser.into_offset_iter() {
            let range = prepared.original_offset(range.start, false)
                ..prepared.original_offset(range.end, true);
            if depth > 0
                || matches!(
                    event,
                    pulldown_cmark::Event::Start(_) | pulldown_cmark::Event::Rule
                )
            {
                events.push(event.clone());
                if github {
                    spans.push(range.clone());
                }
            }
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
                        block_spans.push(std::mem::take(&mut spans));
                    }
                }
                pulldown_cmark::Event::Rule if depth == 0 => {
                    ranges.push(range);
                    block_events.push(std::mem::take(&mut events));
                    block_spans.push(std::mem::take(&mut spans));
                }
                _ => {}
            }
        }
        let plan = github.then(|| {
            let blocks = std::mem::take(&mut ranges)
                .into_iter()
                .zip(std::mem::take(&mut block_events))
                .zip(block_spans)
                .map(|((range, events), spans)| (range, events, spans))
                .collect();
            let mut plan =
                crate::github::stream::plan(&self.source, self.settled, blocks, self.settled_tasks);
            for group in &mut plan.groups {
                ranges.push(group.range.clone());
                block_events.push(std::mem::take(&mut group.events));
            }
            plan
        });
        if let Some(plan) = &plan {
            self.footnotes = plan.definitions;
        }
        if prepared.unclosed.is_none() && !(github && self.footnotes) {
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
        let mut next_sources: Vec<Arc<str>> = Vec::new();
        let mut next_keys: Vec<(usize, usize)> = Vec::new();
        for (at, (range, events)) in ranges.iter().zip(block_events).enumerate() {
            let source = &tail[range.clone()];
            let index = self.settled_count + next.len();
            let group = plan.as_ref().map(|plan| &plan.groups[at]);
            let key = (
                self.settled + range.start,
                group.map_or(0, |group| group.first_task),
            );
            // A footnote's number can change with older text.
            let reusable = !references_changed
                && group.is_none_or(|group| !(group.references && self.footnotes))
                && (!github || self.block_keys.get(index) == Some(&key));
            if reusable {
                if let (Some(previous), Some(existing)) =
                    (self.block_sources.get(index), self.blocks.get(index))
                {
                    if previous.as_ref() == source {
                        next.push(existing.clone());
                        next_sources.push(previous.clone());
                        next_keys.push(key);
                        continue;
                    }
                }
            }
            next_sources.push(Arc::from(source));
            next_keys.push(key);
            if let (Some(plan), Some(group)) = (plan.as_ref(), group) {
                let block = crate::github::stream::render(
                    &crate::github::stream::Target {
                        prepared: &prepared,
                        source: &self.source,
                        start: self.settled + range.start,
                        block: source,
                        web: self.web,
                        html: !self.native_only,
                        native: !self.html_only,
                    },
                    &plan.footnotes,
                    Some((events, group)),
                );
                next.push(Arc::new(block));
                continue;
            }
            let mut html = String::new();
            let mut codes = Vec::new();
            if self.web {
                let block_source = source;
                crate::html::render_html_code(
                    block_source,
                    &mut html,
                    &self.references,
                    &mut |source, info| {
                        let index = codes.len();
                        codes.push(WebCode {
                            source: source.into(),
                            info: info.clone(),
                            utf16: None,
                        });
                        format!("<section class=\"mk-codeblock\" data-markit-code=\"{index}\"></section>\n")
                    },
                );
                place_codes(
                    &self.source,
                    self.settled + range.start,
                    block_source,
                    &mut codes,
                );
            } else if !self.native_only {
                crate::html::render_events(&prepared, events.clone(), &mut html);
            }
            let document = (!self.html_only)
                .then(|| crate::document::parse_events(&prepared, events.into_iter()));
            next.push(Arc::new(RenderedBlock {
                html,
                document,
                codes,
            }));
        }
        if let Some(plan) = plan.as_ref().filter(|plan| !plan.footnotes.is_empty()) {
            let block = crate::github::stream::render(
                &crate::github::stream::Target {
                    prepared: &prepared,
                    source: &self.source,
                    start: self.source.len(),
                    block: "",
                    web: self.web,
                    html: !self.native_only,
                    native: !self.html_only,
                },
                &plan.footnotes,
                None,
            );
            next.push(Arc::new(block));
            next_sources.push(Arc::from(""));
            next_keys.push((usize::MAX, 0));
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
        self.block_sources.truncate(replace_from);
        self.block_sources
            .extend(next_sources.into_iter().skip(common));
        if github {
            self.block_keys.truncate(replace_from);
            self.block_keys.extend(next_keys.into_iter().skip(common));
        }
        let settled = self.settled_count.min(self.block_sources.len());
        for source in &mut self.block_sources[..settled] {
            if !source.is_empty() {
                *source = Arc::from("");
            }
        }
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
        if stable > 0 && !(github && self.footnotes) {
            self.settled += ranges[stable - 1].end;
            self.settled_count += stable;
            if let Some(plan) = &plan {
                self.settled_tasks += plan.groups[..stable]
                    .iter()
                    .map(|group| group.tasks)
                    .sum::<usize>();
            }
        }
        Appended::Patch(RenderPatch {
            replace_from,
            blocks,
            finalized,
            source_bytes: self.source.len(),
            parsed_bytes,
        })
    }
}

/// Give each web code payload the UTF-16 range of its body in the stream's
/// Markdown when the body is an exact slice of the block at `start`.
pub(crate) fn place_codes(markdown: &str, start: usize, block_source: &str, codes: &mut [WebCode]) {
    if codes.is_empty() {
        return;
    }
    let block_utf16 = crate::highlight::utf16_len(&markdown[..start]);
    for code in codes {
        if code.source.is_empty() {
            continue;
        }
        if let Some(at) = block_source.find(&code.source) {
            if at == 0 || block_source.as_bytes()[at - 1] == b'\n' {
                let start = block_utf16 + crate::highlight::utf16_len(&block_source[..at]);
                let end = start + crate::highlight::utf16_len(&code.source);
                code.utf16 = Some((start as u32, end as u32));
            }
        }
    }
}
