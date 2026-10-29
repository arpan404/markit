//! GitHub flavour planning for [`crate::StreamRenderer`]: which top-level
//! blocks render together, where their task boxes sit, and the footnotes.
//!
//! Raw HTML opened at the top level (`<details>`) holds the blocks after it
//! until it closes, so those blocks become one rendered block: each rendered
//! block is then well-formed HTML on its own. Top-level footnote definitions
//! render nothing in place; the notes are drawn in a final block.
use super::footnotes::Footnotes;
use super::front::Front;
use super::tasks::{MarkScanner, Marks};
use pulldown_cmark::{Event, Tag};
use std::ops::Range;

pub(crate) struct Group<'a> {
    /// Range in the parsed tail.
    pub range: Range<usize>,
    pub events: Vec<Event<'a>>,
    pub marks: Marks,
    pub first_task: usize,
    pub tasks: usize,
    /// Holds a footnote reference, whose number depends on earlier blocks.
    pub references: bool,
}

/// A top-level block: its range, events and each event's range.
pub(crate) type Block<'a> = (Range<usize>, Vec<Event<'a>>, Vec<Range<usize>>);

pub(crate) struct Plan<'a> {
    pub groups: Vec<Group<'a>>,
    pub footnotes: Footnotes<'a>,
    /// The parsed text defines a footnote: older text can depend on later.
    pub definitions: bool,
}

/// `blocks` are top-level blocks of the tail starting at byte `base` of
/// `source`, each with its events and their tail-relative ranges.
pub(crate) fn plan<'a>(
    source: &str,
    base: usize,
    blocks: Vec<Block<'a>>,
    first_task: usize,
) -> Plan<'a> {
    let footnotes = Footnotes::collect(blocks.iter().flat_map(|(_, events, _)| events.iter()));
    let definition = |event: &Event<'_>| matches!(event, Event::Start(Tag::FootnoteDefinition(_)));
    let definitions = blocks
        .iter()
        .any(|(_, events, _)| events.iter().any(definition));
    let none = Footnotes::default();
    let mut html = Front::new(&none, Marks::default(), 0);
    let mut scanner = MarkScanner::new(source, first_task);
    let mut groups: Vec<Group<'a>> = Vec::new();
    let mut open = false;
    for (range, events, ranges) in blocks {
        if !open && events.first().is_some_and(definition) {
            continue;
        }
        let first = scanner.next_index();
        for (event, range) in events.iter().zip(ranges) {
            scanner.event(event, base + range.start..base + range.end);
        }
        let marks = scanner.take();
        let tasks = scanner.next_index() - first;
        let references = events
            .iter()
            .any(|event| matches!(event, Event::FootnoteReference(_)));
        let has_html = events
            .iter()
            .any(|event| matches!(event, Event::Html(_) | Event::InlineHtml(_)));
        if open || has_html {
            let mut sink = Vec::new();
            for event in events.iter().cloned() {
                html.push(event, &mut sink);
            }
        }
        match groups.last_mut().filter(|_| open) {
            Some(group) => {
                group.range.end = range.end;
                group.events.extend(events);
                group.marks.tasks.extend(marks.tasks);
                group.marks.fences.extend(marks.fences);
                group.tasks += tasks;
                group.references |= references;
            }
            None => groups.push(Group {
                range,
                events,
                marks,
                first_task: first,
                tasks,
                references,
            }),
        }
        open = html.html_open();
    }
    Plan {
        groups,
        footnotes,
        definitions,
    }
}

/// Where and how one stream block renders.
pub(crate) struct Target<'t, 'p> {
    pub prepared: &'t crate::math::Prepared<'p>,
    /// The stream's whole Markdown.
    pub source: &'t str,
    /// Byte offset of the block in `source`.
    pub start: usize,
    /// The block's own Markdown.
    pub block: &'t str,
    pub web: bool,
    pub html: bool,
    pub native: bool,
}

/// One rendered block: a group of blocks, or the footnotes when `group` is
/// `None`.
pub(crate) fn render<'a>(
    target: &Target<'_, '_>,
    footnotes: &Footnotes<'a>,
    group: Option<(Vec<Event<'a>>, &Group<'a>)>,
) -> crate::RenderedBlock {
    let mut html = String::new();
    let mut codes = Vec::new();
    let section = group.is_none();
    let (mut events, marks, first_task) = match group {
        Some((events, group)) => (events, group.marks.clone(), group.first_task),
        None => (Vec::new(), Marks::default(), 0),
    };
    if target.html {
        let mut placeholder = |source: &str, info: &crate::FenceInfo| {
            let index = codes.len();
            codes.push(crate::WebCode {
                source: source.into(),
                info: info.clone(),
                utf16: None,
            });
            format!("<section class=\"mk-codeblock\" data-markit-code=\"{index}\"></section>\n")
        };
        let mut highlighted = crate::html::render_code;
        let code: &mut super::CodeRenderer<'_> = if target.web {
            &mut placeholder
        } else {
            &mut highlighted
        };
        if section {
            super::section_html(target.prepared, footnotes, &mut html, code);
        } else {
            let events = if target.native {
                events.clone()
            } else {
                std::mem::take(&mut events)
            };
            super::render_block(
                target.prepared,
                footnotes,
                events,
                marks.clone(),
                first_task,
                &mut html,
                code,
            );
        }
    }
    if !section {
        crate::stream::place_codes(target.source, target.start, target.block, &mut codes);
    }
    let document = target.native.then(|| {
        if section {
            super::section_document(target.prepared, footnotes)
        } else {
            super::block_document(target.prepared, footnotes, events, marks, first_task)
        }
    });
    crate::RenderedBlock {
        codes,
        html,
        document,
    }
}
