//! Native document output for GitHub flavour items: the same reading as the
//! HTML output, as UTF-16 text ranges.
use super::footnotes::Footnotes;
use super::front::{Front, Item, TextMode};
use super::html_native::NativeHtml;
use super::text::Part;
use crate::document::{Builder, Span, Style};
use pulldown_cmark::{BlockQuoteKind, CowStr, Event, Tag, TagEnd};

/// Native state that lives across the items of one run.
#[derive(Default)]
pub(crate) struct Native {
    html: NativeHtml,
    alerts: Vec<(usize, BlockQuoteKind)>,
    diagrams: Vec<(usize, String, String)>,
}

/// Build one run of events into `builder`.
pub(crate) fn build<'a>(
    prepared: &crate::math::Prepared<'_>,
    events: impl Iterator<Item = Event<'a>>,
    front: &mut Front<'_, 'a>,
    finish: bool,
    builder: &mut Builder,
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
    let mut native = Native::default();
    for item in items {
        native.item(item, prepared, builder);
    }
}

impl Native {
    fn item(
        &mut self,
        item: Item<'_>,
        prepared: &crate::math::Prepared<'_>,
        builder: &mut Builder,
    ) {
        match item {
            Item::Event(event) => builder.event(event, prepared),
            Item::Text(text, mode) => text_item(text, mode, prepared, builder),
            Item::AlertStart(kind) => {
                builder.start(Tag::BlockQuote(None));
                let start = builder.position;
                self.alerts.push((start, kind));
                builder.text(super::alerts::alert(kind).1);
                builder.span(start, Style::Strong);
                builder.text("\n");
            }
            Item::AlertEnd => {
                builder.end(TagEnd::BlockQuote(None));
                if let Some((start, kind)) = self.alerts.pop() {
                    let alert = super::alerts::alert(kind).0.into();
                    builder.span(start, Style::Alert { alert });
                }
            }
            Item::Task { checked, mark } => {
                let start = builder.position;
                builder.text(if checked { "☑ " } else { "☐ " });
                if let Some((index, offset)) = mark {
                    builder.span(
                        start,
                        Style::Task {
                            index,
                            offset,
                            checked,
                        },
                    );
                }
            }
            Item::FootnoteRef { label, number } => match number {
                Some(number) => {
                    let start = builder.position;
                    builder.text(&format!("[{number}]"));
                    builder.span(
                        start,
                        Style::FootnoteReference {
                            label: label.into_string(),
                            number,
                        },
                    );
                }
                None => builder.text(&format!("[^{label}]")),
            },
            Item::Html { actions, block } => {
                self.html.apply(actions, builder);
                if block {
                    builder.newline();
                }
            }
            Item::DiagramStart { language, source } => {
                self.diagrams.push((builder.position, language, source));
            }
            // Spans the fallback code block, which has just ended.
            Item::DiagramEnd => {
                if let Some((start, language, source)) = self.diagrams.pop() {
                    builder.span(start, Style::Diagram { language, source });
                }
            }
        }
    }
}

/// Draw `text`: bare links, emoji and forge references as spans.
fn text_item(
    text: CowStr<'_>,
    mode: TextMode,
    prepared: &crate::math::Prepared<'_>,
    builder: &mut Builder,
) {
    let Some(parts) = super::text::parts(&text, mode) else {
        if matches!(mode, TextMode::Prose { .. }) {
            builder.event(Event::Text(text), prepared);
        } else {
            builder.text(&text);
        }
        return;
    };
    let mut at = 0;
    for part in parts {
        let range = part.range().clone();
        builder.text(&text[at..range.start]);
        let start = builder.position;
        match part {
            Part::Link { url, .. } => {
                builder.text(&text[range.clone()]);
                builder.document.spans.push(Span {
                    start,
                    end: builder.position,
                    style: Style::Link { url },
                });
            }
            Part::Segment(super::autolink::Segment::Emoji { emoji, .. }) => builder.text(emoji),
            Part::Segment(super::autolink::Segment::Reference {
                kind,
                target,
                display,
                ..
            }) => {
                builder.text(&display);
                if kind == "commit" {
                    builder.span(start, Style::Code);
                }
                builder.span(
                    start,
                    Style::Reference {
                        reference_kind: kind.into(),
                        target,
                    },
                );
            }
        }
        at = range.end;
    }
    builder.text(&text[at..]);
}

/// The footnotes list after the last block, as an ordered list after a rule.
pub(crate) fn build_section(
    prepared: &crate::math::Prepared<'_>,
    footnotes: &Footnotes<'_>,
    flavor: crate::Flavor,
    builder: &mut Builder,
) {
    if footnotes.is_empty() {
        return;
    }
    builder.event(Event::Rule, prepared);
    builder.start(Tag::List(Some(1)));
    for definition in &footnotes.definitions {
        builder.newline();
        let start = builder.position;
        builder.start(Tag::Item);
        let mut front = Front::definition(footnotes, flavor);
        build(
            prepared,
            definition.events.iter().cloned(),
            &mut front,
            true,
            builder,
        );
        builder.end(TagEnd::Item);
        builder.span(
            start,
            Style::FootnoteDefinition {
                label: definition.label.to_string(),
                number: definition.number,
            },
        );
    }
    builder.end(TagEnd::List(true));
}
