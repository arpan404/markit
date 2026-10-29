//! GitHub flavour: forge Markdown (PR, issue and comment bodies) as GitHub
//! draws it. Selected with [`crate::Flavor::Github`]; the default flavour
//! never reaches this module.
mod alerts;
mod autolink;
mod diagram;
mod emoji;
mod emoji_table;
mod entities;
mod footnotes;
mod front;
mod html_allow;
mod html_native;
mod html_out;
mod html_tokens;
mod html_tree;
mod native;
pub(crate) mod stream;
mod tasks;
mod text;

pub(crate) use footnotes::{may_define, Footnotes};
pub(crate) use html_out::CodeRenderer;
pub use tasks::with_task;
pub(crate) use tasks::Marks;

use crate::document::{Builder, Document};
use front::Front;
use pulldown_cmark::Event;

/// Parse a whole document: its events, their task and fence marks, and its
/// footnotes.
fn read<'a>(
    source: &str,
    prepared: &'a crate::math::Prepared<'_>,
    references: &'a crate::References,
) -> (Vec<Event<'a>>, Marks, Footnotes<'a>) {
    let parser = crate::parser_for(&prepared.source, references, crate::Flavor::Github);
    let mut scanner = tasks::MarkScanner::new(source, 0);
    let mut events = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        let range =
            prepared.original_offset(range.start, false)..prepared.original_offset(range.end, true);
        scanner.event(&event, range);
        events.push(event);
    }
    let footnotes = Footnotes::collect(events.iter());
    (events, scanner.take(), footnotes)
}

/// One-shot HTML. Equal to the concatenated blocks of a GitHub stream.
pub(crate) fn render_html(source: &str, output: &mut String) {
    output.clear();
    output.reserve(source.len());
    let prepared = crate::math::prepare(source);
    let references = crate::References::new();
    let (events, marks, footnotes) = read(source, &prepared, &references);
    let mut code = crate::html::render_code;
    render_block(&prepared, &footnotes, events, marks, 0, output, &mut code);
    html_out::render_section(&prepared, &footnotes, output, &mut code);
}

/// One-shot native document, footnotes last.
pub(crate) fn parse(source: &str) -> Document {
    let prepared = crate::math::prepare(source);
    let references = crate::References::new();
    let (events, marks, footnotes) = read(source, &prepared, &references);
    let mut builder = Builder::new();
    let mut front = Front::new(&footnotes, marks, 0);
    native::build(
        &prepared,
        events.into_iter(),
        &mut front,
        true,
        &mut builder,
    );
    native::build_section(&prepared, &footnotes, &mut builder);
    builder.finish()
}

/// HTML for one rendered block, appended to `output`.
pub(crate) fn render_block<'a>(
    prepared: &crate::math::Prepared<'_>,
    footnotes: &Footnotes<'a>,
    events: Vec<Event<'a>>,
    marks: Marks,
    first_task: usize,
    output: &mut String,
    code: &mut CodeRenderer<'_>,
) {
    let mut front = Front::new(footnotes, marks, first_task);
    html_out::render(prepared, events.into_iter(), &mut front, true, output, code);
}

/// Native document for one rendered block.
pub(crate) fn block_document<'a>(
    prepared: &crate::math::Prepared<'_>,
    footnotes: &Footnotes<'a>,
    events: Vec<Event<'a>>,
    marks: Marks,
    first_task: usize,
) -> Document {
    let mut builder = Builder::new();
    let mut front = Front::new(footnotes, marks, first_task);
    native::build(prepared, events.into_iter(), &mut front, true, &mut builder);
    builder.finish()
}

/// The footnotes block's HTML, appended to `output`.
pub(crate) fn section_html(
    prepared: &crate::math::Prepared<'_>,
    footnotes: &Footnotes<'_>,
    output: &mut String,
    code: &mut CodeRenderer<'_>,
) {
    html_out::render_section(prepared, footnotes, output, code);
}

/// The footnotes block's native document.
pub(crate) fn section_document(
    prepared: &crate::math::Prepared<'_>,
    footnotes: &Footnotes<'_>,
) -> Document {
    let mut builder = Builder::new();
    native::build_section(prepared, footnotes, &mut builder);
    builder.finish()
}
