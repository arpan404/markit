//! HTML for GitHub flavour items. Every raw tag reaching the output was
//! rebuilt from the allowlist with escaped attribute values; text is escaped.
use super::autolink::Segment;
use super::footnotes::Footnotes;
use super::front::{Front, Item, TextMode};
use super::html_tree::Action;
use super::text::Part;
use crate::html::{escape_into, push_linked_text};
use pulldown_cmark::{CowStr, Event};

pub(crate) use crate::html::CodeRenderer;

/// Render one run of events. `finish` closes raw HTML still open at its end.
pub(crate) fn render<'a>(
    prepared: &crate::math::Prepared<'_>,
    events: impl Iterator<Item = Event<'a>>,
    front: &mut Front<'_, 'a>,
    finish: bool,
    output: &mut String,
    code: &mut CodeRenderer<'_>,
) {
    let mut items = Vec::new();
    for event in events {
        if matches!(&event, Event::InlineHtml(text) if prepared.is_separator(text)) {
            continue;
        }
        front.push(event, &mut items);
    }
    if finish {
        front.finish(&mut items);
    }
    let events: Vec<Event<'a>> = items.into_iter().filter_map(item_event).collect();
    crate::html::push_events(prepared, events.into_iter(), output, code, true);
}

fn html(text: String) -> Option<Event<'static>> {
    (!text.is_empty()).then(|| Event::Html(text.into()))
}

fn item_event(item: Item<'_>) -> Option<Event<'_>> {
    match item {
        Item::Event(event) => Some(event),
        Item::Text(text, mode) => text_event(text, mode),
        Item::AlertStart(kind) => {
            let (class, title) = super::alerts::alert(kind);
            html(format!(
                "<div class=\"mk-alert mk-alert-{class}\" data-markit-alert=\"{class}\"><p class=\"mk-alert-title\">{title}</p>\n"
            ))
        }
        Item::AlertEnd => html("</div>\n".into()),
        Item::Task { checked, mark } => {
            let mut out = String::from("<input disabled=\"\" type=\"checkbox\"");
            if checked {
                out.push_str(" checked=\"\"");
            }
            if let Some((index, offset)) = mark {
                out.push_str(&format!(
                    " class=\"mk-task\" data-markit-task=\"{index}\" data-markit-task-offset=\"{offset}\""
                ));
            }
            out.push_str("/>\n");
            html(out)
        }
        Item::FootnoteRef { label, number } => match number {
            Some(number) => html(format!(
                "<sup class=\"mk-footnote-ref\"><a role=\"doc-noteref\" tabindex=\"0\" aria-label=\"Footnote {number}\" data-markit-footnote=\"{number}\">{number}</a></sup>"
            )),
            None => Some(Event::Text(format!("[^{label}]").into())),
        },
        Item::Html { actions, block } => {
            let mut out = String::new();
            for action in &actions {
                push_action(action, &mut out);
            }
            if block && !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            html(out)
        }
        Item::DiagramStart { language, source } => {
            let mut out = String::new();
            super::diagram::open_html(&language, &source, &mut out);
            html(out)
        }
        Item::DiagramEnd => html(super::diagram::CLOSE_HTML.into()),
    }
}

/// Prose without anything to draw stays a text event, so the shared stage
/// links bare URLs exactly as in the default flavour.
fn text_event(text: CowStr<'_>, mode: TextMode) -> Option<Event<'_>> {
    if text.is_empty() {
        return None;
    }
    let Some(parts) = super::text::parts(&text, mode) else {
        if mode == TextMode::Prose {
            return Some(Event::Text(text));
        }
        // Inside a raw `<a>` or `<code>` there is no Markdown link around the
        // text: escape it here so it is never linked again.
        let mut out = String::new();
        escape_into(&text, &mut out);
        return html(out);
    };
    let mut out = String::new();
    let mut at = 0;
    for part in &parts {
        let range = part.range();
        escape_into(&text[at..range.start], &mut out);
        match part {
            Part::Link { range, url } => push_linked_text(
                &text[range.clone()],
                &[(0..range.len(), url.clone())],
                &mut out,
            ),
            Part::Segment(segment) => push_segment(&text, segment, &mut out),
        }
        at = range.end;
    }
    escape_into(&text[at..], &mut out);
    html(out)
}

fn push_segment(text: &str, segment: &Segment, out: &mut String) {
    match segment {
        Segment::Emoji { emoji, range } => {
            out.push_str("<span role=\"img\" aria-label=\"");
            escape_into(&text[range.start + 1..range.end - 1].replace('_', " "), out);
            out.push_str("\">");
            out.push_str(emoji);
            out.push_str("</span>");
        }
        Segment::Reference {
            kind,
            target,
            display,
            ..
        } => {
            out.push_str("<a role=\"link\" tabindex=\"0\" class=\"mk-");
            out.push_str(kind);
            out.push_str("\" data-markit-reference=\"");
            escape_into(target, out);
            out.push_str("\" data-reference-kind=\"");
            out.push_str(kind);
            out.push_str("\">");
            if *kind == "commit" {
                out.push_str("<code>");
                escape_into(display, out);
                out.push_str("</code>");
            } else {
                escape_into(display, out);
            }
            out.push_str("</a>");
        }
    }
}

fn push_action(action: &Action, out: &mut String) {
    match action {
        Action::Open(element) => {
            out.push('<');
            out.push_str(element.tag);
            let reference = element
                .attr("href")
                .filter(|_| element.tag == "a")
                .and_then(crate::reference_target);
            for (name, value) in &element.attrs {
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                escape_into(value, out);
                out.push('"');
            }
            match element.tag {
                "img" => out.push_str(" loading=\"lazy\""),
                "video" => out.push_str(" controls=\"\" preload=\"none\""),
                _ => {}
            }
            if let Some(reference) = reference {
                out.push_str(" role=\"link\" tabindex=\"0\" data-markit-reference=\"");
                escape_into(&reference.target, out);
                out.push_str("\" data-reference-kind=\"");
                escape_into(&reference.kind, out);
                out.push('"');
            }
            out.push('>');
        }
        Action::Close(tag) => {
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
        Action::Text(text) => super::entities::push_text(text, out),
        Action::Shown(raw) => escape_into(raw, out),
    }
}

/// The footnotes section drawn after the last block, or nothing.
pub(crate) fn render_section(
    prepared: &crate::math::Prepared<'_>,
    footnotes: &Footnotes<'_>,
    output: &mut String,
    code: &mut CodeRenderer<'_>,
) {
    if footnotes.is_empty() {
        return;
    }
    output.push_str("<section class=\"mk-footnotes\" data-markit-footnotes=\"\">\n<ol>\n");
    for definition in &footnotes.definitions {
        let number = definition.number;
        let mut content = String::new();
        let mut front = Front::definition(footnotes);
        render(
            prepared,
            definition.events.iter().cloned(),
            &mut front,
            true,
            &mut content,
            code,
        );
        let back = format!(
            "<a class=\"mk-footnote-back\" role=\"link\" tabindex=\"0\" aria-label=\"Back to reference {number}\" data-markit-footnote-back=\"{number}\">↩</a>"
        );
        output.push_str(&format!("<li data-markit-footnote-def=\"{number}\">\n"));
        match content.strip_suffix("</p>\n") {
            Some(body) => {
                output.push_str(body);
                output.push(' ');
                output.push_str(&back);
                output.push_str("</p>\n");
            }
            None => {
                output.push_str(&content);
                output.push_str(&back);
                output.push('\n');
            }
        }
        output.push_str("</li>\n");
    }
    output.push_str("</ol>\n</section>\n");
}
