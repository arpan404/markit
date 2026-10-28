use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// All positions are UTF-16 code units, matching NSRange and Android spans.
/// Ranges are half-open and index the rendered `text`, not the Markdown source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub version: u32,
    pub text: String,
    pub spans: Vec<Span>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub start: usize,
    pub end: usize,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    #[serde(flatten)]
    pub style: Style,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Style {
    Heading {
        level: u8,
    },
    Paragraph,
    Strong,
    Emphasis,
    Strikethrough,
    Code,
    CodeBlock {
        language: String,
    },
    Link {
        url: String,
    },
    Image {
        url: String,
    },
    Quote {
        depth: usize,
    },
    ListItem {
        depth: usize,
    },
    Table,
    TableHeader,
    Rule,
    Reference {
        reference_kind: String,
        target: String,
    },
    CodeMetadata {
        file: Option<String>,
        diff: bool,
    },
    Syntax {
        // Borrowed palette entries clone without copying the color text.
        // Grammar colors that miss the palette stay owned.
        light: Cow<'static, str>,
        dark: Cow<'static, str>,
    },
    Diff {
        change: Cow<'static, str>,
    },
    Math {
        tex: String,
        raw: String,
        display: bool,
        png: String,
        width: u32,
        height: u32,
    },
    TableCell {
        row: usize,
        column: usize,
        header: bool,
        alignment: String,
    },
}

struct Frame {
    end: TagEnd,
    start: usize,
    style: Option<Style>,
    byte_start: usize,
    fence: Option<crate::FenceInfo>,
}

struct Builder {
    document: Document,
    position: usize,
    frames: Vec<Frame>,
    lists: Vec<Option<u64>>,
    quote_depth: usize,
    link_suppression: usize,
    cell: usize,
    row: usize,
    header: bool,
    alignments: Vec<String>,
    block: Option<(usize, String)>,
}

impl Builder {
    fn text(&mut self, text: &str) {
        self.document.text.push_str(text);
        self.position += crate::highlight::utf16_len(text);
    }

    fn newline(&mut self) {
        if !self.document.text.is_empty() && !self.document.text.ends_with('\n') {
            self.text("\n");
        }
    }

    fn span(&mut self, start: usize, style: Style) {
        if start < self.position || matches!(style, Style::TableCell { .. }) {
            self.document.spans.push(Span {
                start,
                end: self.position,
                style,
            });
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        if self.frames.is_empty() {
            let kind = match tag {
                Tag::CodeBlock(_) => "code",
                Tag::Table(_) => "table",
                _ => "text",
            };
            self.block = Some((self.position, kind.into()));
        }
        let end = tag.to_end();
        if matches!(end, TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) {
            self.link_suppression += 1;
        }
        let mut start = self.position;
        let mut fence = None;
        let style = match tag {
            Tag::Heading { level, .. } => Some(Style::Heading { level: level as u8 }),
            Tag::Paragraph => Some(Style::Paragraph),
            Tag::Strong => Some(Style::Strong),
            Tag::Emphasis => Some(Style::Emphasis),
            Tag::Strikethrough => Some(Style::Strikethrough),
            Tag::CodeBlock(kind) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => crate::fence_info(&info),
                    CodeBlockKind::Indented => crate::FenceInfo::default(),
                };
                let style = Style::CodeBlock {
                    language: info.language.clone(),
                };
                fence = Some(info);
                Some(style)
            }
            Tag::Link { ref dest_url, .. } if crate::reference_target(dest_url).is_some() => {
                let reference = crate::reference_target(dest_url).expect("guard checked reference");
                Some(Style::Reference {
                    reference_kind: reference.kind,
                    target: reference.target,
                })
            }
            Tag::Link { dest_url, .. } if super::is_safe_url(&dest_url) => Some(Style::Link {
                url: dest_url.into_string(),
            }),
            Tag::Image { dest_url, .. } if super::is_safe_url(&dest_url) => Some(Style::Image {
                url: dest_url.into_string(),
            }),
            Tag::BlockQuote(_) => {
                self.quote_depth += 1;
                Some(Style::Quote {
                    depth: self.quote_depth,
                })
            }
            Tag::List(first) => {
                self.newline();
                self.lists.push(first);
                None
            }
            Tag::Item => {
                self.newline();
                start = self.position;
                for _ in 1..self.lists.len() {
                    self.text("  ");
                }
                match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let prefix = format!("{}. ", *number);
                        *number = number.saturating_add(1);
                        self.text(&prefix);
                    }
                    _ => self.text("• "),
                }
                Some(Style::ListItem {
                    depth: self.lists.len(),
                })
            }
            Tag::Table(alignments) => {
                self.alignments = alignments
                    .iter()
                    .map(|alignment| format!("{alignment:?}").to_lowercase())
                    .collect();
                Some(Style::Table)
            }
            Tag::TableHead => {
                self.cell = 0;
                self.row = 0;
                self.header = true;
                Some(Style::TableHeader)
            }
            Tag::TableRow => {
                self.cell = 0;
                self.row += 1;
                self.header = false;
                None
            }
            Tag::TableCell => {
                if self.cell > 0 {
                    self.text("\t");
                }
                self.cell += 1;
                start = self.position;
                Some(Style::TableCell {
                    row: self.row,
                    column: self.cell - 1,
                    header: self.header,
                    alignment: self
                        .alignments
                        .get(self.cell - 1)
                        .cloned()
                        .unwrap_or_default(),
                })
            }
            _ => None,
        };
        self.frames.push(Frame {
            end,
            start,
            style,
            byte_start: self.document.text.len(),
            fence,
        });
    }

    fn end(&mut self, end: TagEnd) {
        let frame = self.frames.pop().expect("parser emits balanced events");
        debug_assert_eq!(frame.end, end);
        if matches!(end, TagEnd::Link | TagEnd::Image | TagEnd::CodeBlock) {
            self.link_suppression -= 1;
        }
        if frame.fence.is_some() && self.position == frame.start {
            self.text("\n");
        }
        if end == TagEnd::Image && self.position == frame.start {
            self.text("[image]");
        }
        if let Some(info) = &frame.fence {
            let code = &self.document.text[frame.byte_start..];
            crate::highlight::append_syntax_spans(
                code,
                &info.language,
                frame.start,
                &mut self.document.spans,
            );
            if info.diff {
                let mut position = frame.start;
                for line in code.split_inclusive('\n') {
                    let end = position + crate::highlight::utf16_len(line);
                    let change = if line.starts_with("+++")
                        || line.starts_with("---")
                        || line.starts_with("@@")
                        || line.starts_with("diff ")
                    {
                        "meta"
                    } else if line.starts_with('+') {
                        "addition"
                    } else if line.starts_with('-') {
                        "deletion"
                    } else {
                        "context"
                    };
                    if end > position {
                        self.document.spans.push(Span {
                            start: position,
                            end,
                            style: Style::Diff {
                                change: change.into(),
                            },
                        });
                    }
                    position = end;
                }
            }
            self.span(
                frame.start,
                Style::CodeMetadata {
                    file: info.file.clone(),
                    diff: info.diff,
                },
            );
        }
        // Include the paragraph terminator so native paragraph spans never spill
        // across two adjacent blocks. All storage and positions stay UTF-16.
        match end {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::CodeBlock
            | TagEnd::Item
            | TagEnd::BlockQuote(_)
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::Table
            | TagEnd::HtmlBlock => self.newline(),
            _ => {}
        }
        if self.frames.is_empty() {
            if let Some((start, kind)) = self.block.take() {
                if self.position > start {
                    self.document.blocks.push(Block {
                        start,
                        end: self.position,
                        kind,
                    });
                }
            }
        }
        if let Some(style) = frame.style {
            self.span(frame.start, style);
        }
        match end {
            TagEnd::List(_) => {
                self.lists.pop();
            }
            TagEnd::BlockQuote(_) => self.quote_depth -= 1,
            _ => {}
        }
    }
}

/// Build the native render plan in a single pass without a recursive AST.
pub fn parse(source: &str) -> Document {
    parse_with_refs(source, &crate::References::new())
}

pub(crate) fn parse_with_refs(source: &str, references: &crate::References) -> Document {
    let prepared = crate::math::prepare(source);
    parse_events(
        &prepared,
        super::parser_with_refs(&prepared.source, references),
    )
}

pub(crate) fn parse_events<'a>(
    prepared: &crate::math::Prepared<'_>,
    events: impl Iterator<Item = Event<'a>>,
) -> Document {
    let mut builder = Builder {
        document: Document {
            version: 2,
            text: String::new(),
            spans: Vec::new(),
            blocks: Vec::new(),
        },
        position: 0,
        frames: Vec::new(),
        lists: Vec::new(),
        quote_depth: 0,
        link_suppression: 0,
        cell: 0,
        row: 0,
        header: false,
        alignments: Vec::new(),
        block: None,
    };
    for event in events {
        match event {
            Event::Start(tag) => builder.start(tag),
            Event::End(end) => builder.end(end),
            Event::Text(text) => {
                let start = builder.position;
                let in_link_or_code = builder.link_suppression > 0;
                if !in_link_or_code {
                    for (range, url) in crate::html::bare_links(&text) {
                        let link_start = start + crate::highlight::utf16_len(&text[..range.start]);
                        let end = link_start + crate::highlight::utf16_len(&text[range]);
                        builder.document.spans.push(Span {
                            start: link_start,
                            end,
                            style: Style::Link { url },
                        });
                    }
                }
                builder.text(&text);
            }
            Event::InlineHtml(text) if prepared.is_separator(&text) => {}
            Event::Html(text) | Event::InlineHtml(text) => builder.text(&text),
            Event::Code(text) => {
                let start = builder.position;
                if let Some(formula) = prepared.formula(&text) {
                    if let Some(visual) = crate::math::render_pixels(&formula.tex, formula.display)
                    {
                        builder.text("\u{fffc}");
                        builder.span(
                            start,
                            Style::Math {
                                tex: formula.tex.clone(),
                                raw: formula.raw.clone(),
                                display: formula.display,
                                png: visual.png.clone(),
                                width: visual.width,
                                height: visual.height,
                            },
                        );
                    } else {
                        builder.text(&formula.raw);
                    }
                } else {
                    builder.text(&text);
                    builder.span(start, Style::Code);
                    if let Some(reference) = crate::reference::inline_file(&text) {
                        builder.span(
                            start,
                            Style::Reference {
                                reference_kind: reference.kind,
                                target: reference.target,
                            },
                        );
                    }
                }
            }
            Event::SoftBreak => builder.text(" "),
            Event::HardBreak => builder.text("\n"),
            Event::Rule => {
                builder.newline();
                let start = builder.position;
                builder.text("────────\n");
                builder.span(start, Style::Rule);
                if builder.frames.is_empty() {
                    builder.document.blocks.push(Block {
                        start,
                        end: builder.position,
                        kind: "rule".into(),
                    });
                }
            }
            Event::TaskListMarker(checked) => builder.text(if checked { "☑ " } else { "☐ " }),
            // These extensions are deliberately disabled in parser().
            Event::FootnoteReference(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                builder.text(&text)
            }
        }
    }
    // Outer spans first: native font/style overrides can then compose inner marks.
    // Stable ordering preserves parser order for ranges with equal boundaries.
    builder
        .document
        .spans
        .sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    // Native panes also apply to fences/tables nested inside lists or quotes.
    // Walk sorted ranges once so segmentation stays proportional to the document.
    let special: Vec<_> = builder
        .document
        .spans
        .iter()
        .filter_map(|span| match span.style {
            Style::CodeBlock { .. } => Some(Block {
                start: span.start,
                end: span.end,
                kind: "code".into(),
            }),
            Style::Table => Some(Block {
                start: span.start,
                end: span.end,
                kind: "table".into(),
            }),
            _ => None,
        })
        .collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    for root in &builder.document.blocks {
        while special
            .get(index)
            .is_some_and(|span| span.end <= root.start)
        {
            index += 1;
        }
        if root.kind != "text" {
            blocks.push(root.clone());
            continue;
        }
        let mut at = root.start;
        while let Some(span) = special
            .get(index)
            .filter(|span| span.start >= at && span.end <= root.end)
        {
            if span.start > at {
                blocks.push(Block {
                    start: at,
                    end: span.start,
                    kind: "text".into(),
                });
            }
            blocks.push(span.clone());
            at = span.end;
            index += 1;
        }
        if at < root.end {
            blocks.push(Block {
                start: at,
                end: root.end,
                kind: "text".into(),
            });
        }
    }
    builder.document.blocks = blocks;
    builder.document
}

/// Native code-fence document built from lexical runs. Output matches
/// [`parse_events`] for a single fenced block, including the empty-fence
/// newline and the trailing newline pulldown does not treat as a diff line.
pub(crate) struct FenceDoc {
    syntax: Vec<Span>,
    diffs: Vec<Span>,
    kept_runs: usize,
    kept_diffs: usize,
    stable: usize,
    language: String,
    diff: bool,
    /// `(bytes, UTF-16 units)` of the last rendered code, for an O(appended)
    /// UTF-16 length. The rendered code only grows by appending.
    measured: (usize, usize),
    /// The last rendered document came from this state (not the empty-fence
    /// special case), so a host holding it can apply an extension.
    extendable: bool,
}

/// Change from the last rendered fence document to the current one. Spans
/// starting before `cut` are unchanged. See docs/binary-transport.md (MKNP v2).
pub(crate) enum FenceChange {
    Unchanged,
    Replace(Document),
    Extend(FenceExtension),
}

/// The new document is `old.text[..keep_text_utf16] + text` with spans
/// `spans[..head] + old.spans[keep_from..keep_to] + spans[head..]` and `blocks`.
/// Span and block offsets are positions in the full new text.
pub(crate) struct FenceExtension {
    pub keep_text_utf16: usize,
    pub head: usize,
    pub keep_from: usize,
    pub keep_to: usize,
    pub document: Document,
}

impl FenceDoc {
    pub(crate) fn new(info: &crate::FenceInfo) -> Self {
        Self {
            syntax: Vec::new(),
            diffs: Vec::new(),
            kept_runs: 0,
            kept_diffs: 0,
            stable: 0,
            language: info.language.clone(),
            diff: info.diff,
            measured: (0, 0),
            extendable: false,
        }
    }

    pub(crate) fn render(
        &mut self,
        code: &str,
        info: &crate::FenceInfo,
        runs: &crate::highlight::LexRuns,
    ) -> Document {
        if code.is_empty() {
            self.extendable = false;
            return empty_fence_document(info);
        }
        self.update(code, info, runs);
        self.extendable = true;
        self.document(code, info)
    }

    /// Same state transition as [`render`](Self::render), but returns only
    /// what changed since the previous call. Linear in the changed suffix,
    /// not the fence length.
    pub(crate) fn change(
        &mut self,
        code: &str,
        info: &crate::FenceInfo,
        runs: &crate::highlight::LexRuns,
    ) -> FenceChange {
        if code.is_empty() {
            let unchanged = !self.extendable && self.measured.0 == 0 && self.syntax.is_empty();
            self.extendable = false;
            self.measured = (0, 0);
            return if unchanged {
                FenceChange::Unchanged
            } else {
                FenceChange::Replace(empty_fence_document(info))
            };
        }
        let was_extendable = self.extendable;
        let old_len = self.measured.0;
        let old_utf16 = self.measured.1;
        let old_zero = self.zero_starts();
        let cut = self.update(code, info, runs);
        self.extendable = true;
        if !was_extendable || cut == 0 {
            return FenceChange::Replace(self.document(code, info));
        }
        if code.len() == old_len {
            // Spans are a function of the code text alone.
            return FenceChange::Unchanged;
        }
        debug_assert!(code.len() > old_len);
        let zero = self.zero_starts();
        debug_assert_eq!(zero, old_zero);
        let (meta_end, block_end) = self.ends(code);
        let syntax_at = self.syntax.partition_point(|span| span.start < cut);
        let diff_at = self.diffs.partition_point(|span| span.start < cut);
        let mut spans = Vec::with_capacity(
            zero + 2 + self.syntax.len() - syntax_at + self.diffs.len() - diff_at,
        );
        let syntax_zero = self.syntax.partition_point(|span| span.start == 0);
        let diff_zero = self.diffs.partition_point(|span| span.start == 0);
        merge_fence_spans(
            &self.syntax[..syntax_zero],
            &self.diffs[..diff_zero],
            &metadata_span(info, meta_end),
            &code_block_span(info, block_end),
            &mut spans,
            true,
        );
        let head = spans.len();
        merge_fence_spans(
            &self.syntax[syntax_at..],
            &self.diffs[diff_at..],
            &metadata_span(info, meta_end),
            &code_block_span(info, block_end),
            &mut spans,
            false,
        );
        let middle = syntax_at - syntax_zero + diff_at - diff_zero;
        let mut text = String::with_capacity(code.len() - old_len + 1);
        text.push_str(&code[old_len..]);
        if !code.ends_with('\n') {
            text.push('\n');
        }
        FenceChange::Extend(FenceExtension {
            keep_text_utf16: old_utf16,
            head,
            keep_from: head,
            keep_to: head + middle,
            document: Document {
                version: 2,
                text,
                spans,
                blocks: vec![Block {
                    start: 0,
                    end: block_end,
                    kind: "code".into(),
                }],
            },
        })
    }

    /// Document for `code`, the code of the last update.
    pub(crate) fn current(&self, code: &str, info: &crate::FenceInfo) -> Document {
        if code.is_empty() {
            empty_fence_document(info)
        } else {
            self.document(code, info)
        }
    }

    fn zero_starts(&self) -> usize {
        self.syntax.partition_point(|span| span.start == 0)
            + self.diffs.partition_point(|span| span.start == 0)
    }

    /// UTF-16 ends of the metadata span (the code) and the block (the code
    /// plus the newline pulldown adds when the open line has none).
    fn ends(&self, code: &str) -> (usize, usize) {
        let meta_end = self.measured.1;
        debug_assert_eq!(self.measured.0, code.len());
        let block_end = if code.ends_with('\n') {
            meta_end
        } else {
            meta_end + 1
        };
        (meta_end, block_end)
    }

    /// Advance the incremental state to `code`. Returns the smallest UTF-16
    /// start of any removed or added span: spans before it are unchanged.
    fn update(
        &mut self,
        code: &str,
        info: &crate::FenceInfo,
        runs: &crate::highlight::LexRuns,
    ) -> usize {
        let mut cut = usize::MAX;
        if runs.stable_byte < self.stable
            || info.language != self.language
            || info.diff != self.diff
            || code.len() < self.measured.0
        {
            self.syntax.clear();
            self.diffs.clear();
            self.kept_runs = 0;
            self.kept_diffs = 0;
            self.stable = 0;
            self.measured = (0, 0);
            self.language.clone_from(&info.language);
            self.diff = info.diff;
            cut = 0;
        }
        self.measured = if runs.ascii() {
            (code.len(), code.len())
        } else {
            let (bytes, units) = self.measured;
            (
                code.len(),
                units + crate::highlight::utf16_len(&code[bytes..]),
            )
        };
        // Run ends strictly increase, so this is the count of final runs.
        let keep = if runs.is_plain() {
            0
        } else {
            runs.first_ending_after(runs.stable_byte)
        };
        let kept_spans = self.kept_runs.min(keep);
        if let Some(removed) = self.syntax.get(kept_spans) {
            cut = cut.min(removed.start);
        }
        self.syntax.truncate(kept_spans);
        let start_run = self.syntax.len();
        let start_byte = if start_run == 0 {
            0
        } else {
            runs.end(start_run - 1)
        };
        let ascii = runs.ascii();
        let mut byte = start_byte;
        let mut utf16 = if ascii {
            start_byte
        } else {
            crate::highlight::utf16_len(&code[..start_byte])
        };
        if !runs.is_plain() {
            for (end, id) in runs.from(start_run) {
                let next = if ascii {
                    end
                } else {
                    utf16 + crate::highlight::utf16_len(&code[byte..end])
                };
                if next > utf16 {
                    let (light, dark, _, _) = crate::highlight::PALETTE[id as usize];
                    self.syntax.push(Span {
                        start: utf16,
                        end: next,
                        style: Style::Syntax {
                            light: light.into(),
                            dark: dark.into(),
                        },
                    });
                }
                byte = end;
                utf16 = next;
            }
        }
        if let Some(added) = self.syntax.get(start_run) {
            cut = cut.min(added.start);
        }
        self.kept_runs = keep.min(self.syntax.len());

        let complete_lines = if info.diff {
            code[..runs.stable_byte.min(code.len())]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
        } else {
            0
        };
        if complete_lines < self.kept_diffs {
            self.diffs.truncate(complete_lines);
            self.stable = 0;
            self.kept_diffs = 0;
        }
        if let Some(removed) = self.diffs.get(self.kept_diffs) {
            cut = cut.min(removed.start);
        }
        self.diffs.truncate(self.kept_diffs);
        let first_new_diff = self.diffs.len();
        if info.diff {
            let from = self.stable.min(code.len());
            let mut pos = crate::highlight::utf16_len(&code[..from]);
            for line in code[from..].split_inclusive('\n') {
                let end = pos + crate::highlight::utf16_len(line);
                if end > pos {
                    self.diffs.push(Span {
                        start: pos,
                        end,
                        style: Style::Diff {
                            change: crate::html::diff_kind(line).into(),
                        },
                    });
                }
                pos = end;
            }
        }
        if let Some(added) = self.diffs.get(first_new_diff) {
            cut = cut.min(added.start);
        }
        self.kept_diffs = complete_lines;
        self.stable = runs.stable_byte.min(code.len());
        cut
    }

    fn document(&self, code: &str, info: &crate::FenceInfo) -> Document {
        let (meta_end, block_end) = self.ends(code);
        let mut text = String::with_capacity(code.len() + 1);
        text.push_str(code);
        if !code.ends_with('\n') {
            text.push('\n');
        }
        let mut spans = Vec::with_capacity(self.syntax.len() + self.diffs.len() + 2);
        merge_fence_spans(
            &self.syntax,
            &self.diffs,
            &metadata_span(info, meta_end),
            &code_block_span(info, block_end),
            &mut spans,
            true,
        );
        Document {
            version: 2,
            text,
            spans,
            blocks: vec![Block {
                start: 0,
                end: block_end,
                kind: "code".into(),
            }],
        }
    }
}

fn metadata_span(info: &crate::FenceInfo, end: usize) -> Span {
    Span {
        start: 0,
        end,
        style: Style::CodeMetadata {
            file: info.file.clone(),
            diff: info.diff,
        },
    }
}

fn code_block_span(info: &crate::FenceInfo, end: usize) -> Span {
    Span {
        start: 0,
        end,
        style: Style::CodeBlock {
            language: info.language.clone(),
        },
    }
}

/// Same order as parse_events: stable sort by (start asc, end desc), with ties
/// keeping syntax, diff, metadata, code block insertion order. `with_head`
/// includes the metadata and code-block spans.
fn merge_fence_spans(
    syntax: &[Span],
    diffs: &[Span],
    metadata: &Span,
    code_block: &Span,
    spans: &mut Vec<Span>,
    with_head: bool,
) {
    let mut syntax_at = 0;
    let mut diff_at = 0;
    let mut metadata_used = !with_head;
    let mut code_used = !with_head;
    while syntax_at < syntax.len() || diff_at < diffs.len() || !metadata_used || !code_used {
        let mut best: Option<(u8, usize, usize)> = None;
        let mut consider = |kind: u8, start: usize, end: usize| {
            let replace = match best {
                None => true,
                Some((best_kind, best_start, best_end)) => {
                    start < best_start
                        || (start == best_start && end > best_end)
                        || (start == best_start && end == best_end && kind < best_kind)
                }
            };
            if replace {
                best = Some((kind, start, end));
            }
        };
        if let Some(span) = syntax.get(syntax_at) {
            consider(0, span.start, span.end);
        }
        if let Some(span) = diffs.get(diff_at) {
            consider(1, span.start, span.end);
        }
        if !metadata_used {
            consider(2, metadata.start, metadata.end);
        }
        if !code_used {
            consider(3, code_block.start, code_block.end);
        }
        match best.expect("a fence span remains").0 {
            0 => {
                spans.push(syntax[syntax_at].clone());
                syntax_at += 1;
            }
            1 => {
                spans.push(diffs[diff_at].clone());
                diff_at += 1;
            }
            2 => {
                spans.push(metadata.clone());
                metadata_used = true;
            }
            _ => {
                spans.push(code_block.clone());
                code_used = true;
            }
        }
    }
}

fn empty_fence_document(info: &crate::FenceInfo) -> Document {
    let mut spans = vec![Span {
        start: 0,
        end: 1,
        style: Style::CodeMetadata {
            file: info.file.clone(),
            diff: info.diff,
        },
    }];
    if !info.language.is_empty() {
        // Match the parser: an empty fence is a single newline before highlighting.
        crate::highlight::append_syntax_spans("\n", &info.language, 0, &mut spans);
    } else {
        crate::highlight::append_syntax_spans("\n", "", 0, &mut spans);
    }
    if info.diff {
        spans.push(Span {
            start: 0,
            end: 1,
            style: Style::Diff {
                change: "context".into(),
            },
        });
    }
    spans.push(Span {
        start: 0,
        end: 1,
        style: Style::CodeBlock {
            language: info.language.clone(),
        },
    });
    // append_syntax_spans pushes after metadata. Parser order is syntax, diff,
    // metadata, code block. Rebuild in that order before the same stable sort.
    let syntax: Vec<_> = spans
        .iter()
        .filter(|span| matches!(span.style, Style::Syntax { .. }))
        .cloned()
        .collect();
    let mut ordered = syntax;
    if info.diff {
        ordered.push(Span {
            start: 0,
            end: 1,
            style: Style::Diff {
                change: "context".into(),
            },
        });
    }
    ordered.push(Span {
        start: 0,
        end: 1,
        style: Style::CodeMetadata {
            file: info.file.clone(),
            diff: info.diff,
        },
    });
    ordered.push(Span {
        start: 0,
        end: 1,
        style: Style::CodeBlock {
            language: info.language.clone(),
        },
    });
    ordered.sort_by(|left, right| left.start.cmp(&right.start).then(right.end.cmp(&left.end)));
    Document {
        version: 2,
        text: "\n".into(),
        spans: ordered,
        blocks: vec![Block {
            start: 0,
            end: 1,
            kind: "code".into(),
        }],
    }
}

#[cfg(test)]
mod fence_doc_tests {
    use super::*;
    use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};

    fn reference(info: &str, code: &str) -> Document {
        let prepared = crate::math::prepare("");
        let events = [
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info.into()))),
            Event::Text(code.into()),
            Event::End(TagEnd::CodeBlock),
        ];
        parse_events(&prepared, events.into_iter())
    }

    #[test]
    fn fence_documents_match_the_parser_while_a_block_grows() {
        let samples = [
            ("rust", ""),
            ("rust", "let x = 1;"),
            ("rust", "let x = 1;\n"),
            ("rust", "fn main() {\n    let s = \"😀\";\n}\n"),
            ("rust", "/* comment\nstill */\nlet s = r#\"a\nb\"#;\n"),
            ("python", "x = \"\"\"long\nstr\"\"\"\ny\n"),
            (
                "diff",
                "diff --git a/f.rs b/f.rs\n--- a/f.rs\n+++ b/f.rs\n@@ -1 +1 @@\n-old\n+new\n context\n",
            ),
            ("tsx", "<A\n b=\"v\"/>\n<!-- c\nomment -->\n😀\n"),
            ("sql", "SELECT name FROM t WHERE x IS NOT NULL\n"),
            ("plain", "not highlighted\nsecond\n"),
            ("rust file=src/lib.rs", "fn answer() -> i32 { 42 }\n"),
        ];
        for (info, sample) in samples {
            let parsed = crate::fence_info(info);
            let mut doc = FenceDoc::new(&parsed);
            let mut built = String::new();
            let incremental = doc.render(
                &built,
                &parsed,
                &crate::highlight::lex_runs(&built, &parsed.language),
            );
            assert_eq!(incremental, reference(info, &built), "{info} empty");
            for ch in sample.chars() {
                built.push(ch);
                let runs = crate::highlight::lex_runs(&built, &parsed.language);
                let incremental = doc.render(&built, &parsed, &runs);
                assert_eq!(incremental, reference(info, &built), "{info} {built:?}");
            }
        }
    }
}
