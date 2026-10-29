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
    /// GitHub flavour: a `> [!NOTE]` alert; `alert` is `note`, `tip`,
    /// `important`, `warning` or `caution`. Its title is the first line.
    Alert {
        alert: String,
    },
    /// GitHub flavour: a footnote marker, drawn as `[number]`.
    FootnoteReference {
        label: String,
        number: usize,
    },
    /// GitHub flavour: one note in the footnotes list after the last block.
    FootnoteDefinition {
        label: String,
        number: usize,
    },
    /// GitHub flavour: a task box. `offset` is the UTF-16 index of the
    /// character inside its `[ ]` in the Markdown source, `index` its order.
    Task {
        index: usize,
        offset: usize,
        checked: bool,
    },
    /// GitHub flavour: a closed diagram fence. The code block inside is the
    /// fallback; hosts may draw `source` instead.
    Diagram {
        language: String,
        source: String,
    },
    /// GitHub flavour: an HTML `<video>`, drawn as `[video]`. The host
    /// resolves `url`; markit never loads it.
    Video {
        url: String,
    },
    /// GitHub flavour: the dark-mode source of the image on the same range,
    /// from a `<picture>` `<source media="(prefers-color-scheme: dark)">`.
    DarkImage {
        url: String,
    },
    /// GitHub flavour: an allowlisted HTML element without its own kind
    /// (`kbd`, `sub`, `sup`, `mark`, `ins`, `u`, `small`, `abbr`, `details`,
    /// `summary`, `ruby`, `rt`, `rp`, `q`, `time`, `bdo`, `center`, ...).
    Html {
        tag: String,
    },
}

struct Frame {
    end: TagEnd,
    start: usize,
    style: Option<Style>,
    byte_start: usize,
    fence: Option<crate::FenceInfo>,
}

pub(crate) struct Builder {
    pub(crate) document: Document,
    pub(crate) position: usize,
    frames: Vec<Frame>,
    lists: Vec<Option<u64>>,
    quote_depth: usize,
    pub(crate) link_suppression: usize,
    cell: usize,
    row: usize,
    header: bool,
    alignments: Vec<String>,
    block: Option<(usize, String)>,
}

impl Builder {
    pub(crate) fn text(&mut self, text: &str) {
        self.document.text.push_str(text);
        self.position += crate::highlight::utf16_len(text);
    }

    pub(crate) fn newline(&mut self) {
        if !self.document.text.is_empty() && !self.document.text.ends_with('\n') {
            self.text("\n");
        }
    }

    pub(crate) fn span(&mut self, start: usize, style: Style) {
        if start < self.position || matches!(style, Style::TableCell { .. }) {
            self.document.spans.push(Span {
                start,
                end: self.position,
                style,
            });
        }
    }

    pub(crate) fn start(&mut self, tag: Tag<'_>) {
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

    pub(crate) fn end(&mut self, end: TagEnd) {
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
    let mut builder = Builder::new();
    for event in events {
        builder.event(event, prepared);
    }
    builder.finish()
}

impl Builder {
    pub(crate) fn new() -> Self {
        Builder {
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
        }
    }

    /// Apply one parser event. Footnote and math events are inert here.
    pub(crate) fn event(&mut self, event: Event<'_>, prepared: &crate::math::Prepared<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(end) => self.end(end),
            Event::Text(text) => {
                let start = self.position;
                let in_link_or_code = self.link_suppression > 0;
                if !in_link_or_code {
                    for (range, url) in crate::html::bare_links(&text) {
                        let link_start = start + crate::highlight::utf16_len(&text[..range.start]);
                        let end = link_start + crate::highlight::utf16_len(&text[range]);
                        self.document.spans.push(Span {
                            start: link_start,
                            end,
                            style: Style::Link { url },
                        });
                    }
                }
                self.text(&text);
            }
            Event::InlineHtml(text) if prepared.is_separator(&text) => {}
            Event::Html(text) | Event::InlineHtml(text) => self.text(&text),
            Event::Code(text) => {
                let start = self.position;
                if let Some(formula) = prepared.formula(&text) {
                    if let Some(visual) = crate::math::render_pixels(&formula.tex, formula.display)
                    {
                        self.text("\u{fffc}");
                        self.span(
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
                        self.text(&formula.raw);
                    }
                } else {
                    self.text(&text);
                    self.span(start, Style::Code);
                    if let Some(reference) = crate::reference::inline_file(&text) {
                        self.span(
                            start,
                            Style::Reference {
                                reference_kind: reference.kind,
                                target: reference.target,
                            },
                        );
                    }
                }
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.text("\n"),
            Event::Rule => {
                self.newline();
                let start = self.position;
                self.text("────────\n");
                self.span(start, Style::Rule);
                if self.frames.is_empty() {
                    self.document.blocks.push(Block {
                        start,
                        end: self.position,
                        kind: "rule".into(),
                    });
                }
            }
            Event::TaskListMarker(checked) => self.text(if checked { "☑ " } else { "☐ " }),
            // These extensions are deliberately disabled in parser().
            Event::FootnoteReference(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                self.text(&text)
            }
        }
    }

    /// Sort spans and split native panes. Consumes the builder.
    pub(crate) fn finish(mut self) -> Document {
        // Outer spans first: native font/style overrides can then compose inner marks.
        // Stable ordering preserves parser order for ranges with equal boundaries.
        self.document
            .spans
            .sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
        // Native panes also apply to fences/tables nested inside lists or quotes.
        // Walk sorted ranges once so segmentation stays proportional to the document.
        let special: Vec<_> = self
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
        for root in &self.document.blocks {
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
        self.document.blocks = blocks;
        self.document
    }
}
