//! The GitHub flavour's reading of parser events, shared by the HTML and
//! native outputs: alerts, placed task boxes, numbered footnote references,
//! balanced allowlisted raw HTML, diagrams, and which text may hold forge
//! references. Footnote definitions are skipped here and drawn afterwards.
use super::footnotes::Footnotes;
use super::html_tree::{Action, Balancer};
use super::tasks::Marks;
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, CowStr, Event, Tag, TagEnd};

/// What text may turn into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum TextMode {
    /// Bare links, forge references and emoji.
    Prose,
    /// Inside a link: emoji only, never another link.
    Link,
    /// Inside raw `<code>`-like HTML: exactly as written.
    Literal,
}

pub(crate) enum Item<'a> {
    Event(Event<'a>),
    Text(CowStr<'a>, TextMode),
    AlertStart(BlockQuoteKind),
    AlertEnd,
    Task {
        checked: bool,
        /// `(index, offset)` when the box is placed in the source.
        mark: Option<(usize, usize)>,
    },
    FootnoteRef {
        label: CowStr<'a>,
        number: Option<usize>,
    },
    /// Allowlisted raw HTML. `block` is a whole HTML block.
    Html {
        actions: Vec<Action>,
        block: bool,
    },
    /// Around a closed ```` ```mermaid ```` fence, which stays inside as the
    /// fallback.
    DiagramStart {
        language: String,
        source: String,
    },
    DiagramEnd,
}

const LITERAL: &[&str] = &["code", "kbd", "pre", "samp", "tt"];

pub(crate) struct Front<'f, 'a> {
    footnotes: &'f Footnotes<'a>,
    marks: Marks,
    next_mark: usize,
    next_fence: usize,
    next_task: usize,
    /// Rendering a footnote definition: boxes are not placed and fences are
    /// never diagrams.
    in_definition: bool,
    balancer: Balancer,
    depth: usize,
    html_block: Option<String>,
    skip: Option<usize>,
    links: usize,
    images: usize,
    code: bool,
    diagram: Option<(String, Vec<Event<'a>>)>,
}

impl<'f, 'a> Front<'f, 'a> {
    /// `first_task` is the index of this run's first box.
    pub(crate) fn new(footnotes: &'f Footnotes<'a>, marks: Marks, first_task: usize) -> Self {
        Self {
            footnotes,
            marks,
            next_mark: 0,
            next_fence: 0,
            next_task: first_task,
            in_definition: false,
            balancer: Balancer::default(),
            depth: 0,
            html_block: None,
            skip: None,
            links: 0,
            images: 0,
            code: false,
            diagram: None,
        }
    }

    pub(crate) fn definition(footnotes: &'f Footnotes<'a>) -> Self {
        Self {
            in_definition: true,
            ..Self::new(footnotes, Marks::default(), 0)
        }
    }

    /// Whether raw HTML opened at the top level is still open: a stream
    /// keeps the following blocks in the same rendered block.
    pub(crate) fn html_open(&self) -> bool {
        self.balancer.is_open()
    }

    pub(crate) fn push(&mut self, event: Event<'a>, out: &mut Vec<Item<'a>>) {
        if let Some(depth) = self.skip.as_mut() {
            match event {
                Event::Start(_) => *depth += 1,
                Event::End(_) if *depth == 0 => self.skip = None,
                Event::End(_) => *depth -= 1,
                _ => {}
            }
            return;
        }
        if let Some((_, buffered)) = self.diagram.as_mut() {
            let end = matches!(event, Event::End(TagEnd::CodeBlock));
            buffered.push(event);
            if end {
                let (language, events) = self.diagram.take().expect("open diagram");
                let source = events
                    .iter()
                    .filter_map(|event| match event {
                        Event::Text(text) => Some(text.as_ref()),
                        _ => None,
                    })
                    .collect();
                out.push(Item::DiagramStart { language, source });
                out.extend(events.into_iter().map(Item::Event));
                out.push(Item::DiagramEnd);
            }
            return;
        }
        if self.balancer.dropping()
            && matches!(
                event,
                Event::Text(_)
                    | Event::Code(_)
                    | Event::SoftBreak
                    | Event::HardBreak
                    | Event::FootnoteReference(_)
                    | Event::InlineMath(_)
                    | Event::DisplayMath(_)
            )
        {
            // A `<video>`'s content is dropped, Markdown inside it too.
            return;
        }
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => self.skip = Some(0),
            Event::Start(Tag::HtmlBlock) => {
                self.html_block = Some(String::new());
                out.push(Item::Event(Event::Start(Tag::HtmlBlock)));
            }
            Event::End(TagEnd::HtmlBlock) => {
                let html = self.html_block.take().unwrap_or_default();
                let mut actions = Vec::new();
                self.balancer.feed(&html, self.depth, &mut actions);
                out.push(Item::Html {
                    actions,
                    block: true,
                });
                out.push(Item::Event(Event::End(TagEnd::HtmlBlock)));
            }
            Event::Html(text) | Event::InlineHtml(text) => {
                if let Some(block) = self.html_block.as_mut() {
                    block.push_str(&text);
                } else {
                    let mut actions = Vec::new();
                    self.balancer.feed(&text, self.depth, &mut actions);
                    out.push(Item::Html {
                        actions,
                        block: false,
                    });
                }
            }
            Event::Start(tag) => self.start(tag, out),
            Event::End(end) => self.end(end, out),
            Event::Text(text) if self.code || self.images > 0 => {
                out.push(Item::Event(Event::Text(text)))
            }
            Event::Text(text) => {
                let mode = if self.balancer.inside(LITERAL) {
                    TextMode::Literal
                } else if self.links > 0 || self.balancer.inside(&["a"]) {
                    TextMode::Link
                } else {
                    TextMode::Prose
                };
                out.push(Item::Text(text, mode));
            }
            Event::TaskListMarker(checked) => {
                let mark = if self.in_definition {
                    None
                } else {
                    let index = self.next_task;
                    self.next_task += 1;
                    let mark = self.marks.tasks.get(self.next_mark).copied();
                    mark.filter(|&(at, _)| at == index)
                        .inspect(|_| self.next_mark += 1)
                };
                out.push(Item::Task { checked, mark });
            }
            Event::FootnoteReference(label) => {
                let number = self.footnotes.number(&label);
                out.push(Item::FootnoteRef { label, number });
            }
            other => out.push(Item::Event(other)),
        }
    }

    fn start(&mut self, tag: Tag<'a>, out: &mut Vec<Item<'a>>) {
        self.depth += 1;
        match tag {
            Tag::BlockQuote(Some(kind)) => {
                out.push(Item::AlertStart(kind));
                return;
            }
            Tag::Link { .. } => self.links += 1,
            Tag::Image { .. } => self.images += 1,
            Tag::CodeBlock(CodeBlockKind::Fenced(ref info)) if !self.in_definition => {
                let closed = self
                    .marks
                    .fences
                    .get(self.next_fence)
                    .copied()
                    .unwrap_or(false);
                self.next_fence += 1;
                let language = crate::fence_info(info).language;
                if closed && super::diagram::is_diagram(&language) {
                    self.diagram = Some((language, vec![Event::Start(tag)]));
                    return;
                }
                self.code = true;
            }
            Tag::CodeBlock(_) => self.code = true,
            _ => {}
        }
        out.push(Item::Event(Event::Start(tag)));
    }

    fn end(&mut self, end: TagEnd, out: &mut Vec<Item<'a>>) {
        let mut actions = Vec::new();
        self.balancer.close_from(self.depth, &mut actions);
        if !actions.is_empty() {
            out.push(Item::Html {
                actions,
                block: false,
            });
        }
        self.depth -= 1;
        match end {
            TagEnd::BlockQuote(Some(_)) => {
                out.push(Item::AlertEnd);
                return;
            }
            TagEnd::Link => self.links -= 1,
            TagEnd::Image => self.images -= 1,
            TagEnd::CodeBlock => self.code = false,
            _ => {}
        }
        out.push(Item::Event(Event::End(end)));
    }

    /// Close raw HTML still open at the end of the run.
    pub(crate) fn finish(&mut self, out: &mut Vec<Item<'a>>) {
        let mut actions = Vec::new();
        self.balancer.close_from(0, &mut actions);
        if !actions.is_empty() {
            out.push(Item::Html {
                actions,
                block: false,
            });
        }
    }
}
