//! GitHub footnotes. pulldown-cmark only emits a reference for a label that
//! has a definition. Notes are numbered in the order they are first
//! referenced, drawn together after the last block, and a definition nobody
//! references is not drawn. Labels match case-insensitively.
use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::collections::HashMap;

pub(crate) struct Definition<'a> {
    pub label: CowStr<'a>,
    pub number: usize,
    /// The definition's content, without its own start and end events.
    pub events: Vec<Event<'a>>,
}

#[derive(Default)]
pub(crate) struct Footnotes<'a> {
    numbers: HashMap<String, usize>,
    /// Referenced definitions, by number.
    pub definitions: Vec<Definition<'a>>,
}

fn key(label: &str) -> String {
    label.to_lowercase()
}

impl<'a> Footnotes<'a> {
    /// Number and gather the notes of a whole document, in event order.
    pub(crate) fn collect<'e>(events: impl Iterator<Item = &'e Event<'a>>) -> Self
    where
        'a: 'e,
    {
        let mut numbers: HashMap<String, usize> = HashMap::new();
        let mut found: Vec<(CowStr<'a>, Vec<Event<'a>>)> = Vec::new();
        let mut open: Option<(CowStr<'a>, Vec<Event<'a>>, usize)> = None;
        for event in events {
            if let Event::FootnoteReference(label) = event {
                let next = numbers.len() + 1;
                numbers.entry(key(label)).or_insert(next);
            }
            if let Some((_, content, depth)) = open.as_mut() {
                match event {
                    Event::Start(_) => *depth += 1,
                    Event::End(TagEnd::FootnoteDefinition) if *depth == 0 => {
                        let (label, content, _) = open.take().expect("open definition");
                        found.push((label, content));
                        continue;
                    }
                    Event::End(_) => *depth -= 1,
                    _ => {}
                }
                content.push(event.clone());
            } else if let Event::Start(Tag::FootnoteDefinition(label)) = event {
                open = Some((label.clone(), Vec::new(), 0));
            }
        }
        let mut definitions: Vec<Definition<'a>> = Vec::new();
        for (label, events) in found {
            let Some(&number) = numbers.get(&key(&label)) else {
                continue;
            };
            if definitions
                .iter()
                .all(|definition| definition.number != number)
            {
                definitions.push(Definition {
                    label,
                    number,
                    events,
                });
            }
        }
        definitions.sort_by_key(|definition| definition.number);
        Self {
            numbers,
            definitions,
        }
    }

    pub(crate) fn number(&self, label: &str) -> Option<usize> {
        self.numbers.get(&key(label)).copied()
    }

    /// Whether a footnote section is drawn.
    pub(crate) fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// Whether `source` may define a footnote (`[^label]:`). A cheap, generous
/// test that lets streams skip footnote bookkeeping for ordinary prose.
pub(crate) fn may_define(source: &str) -> bool {
    source
        .find("[^")
        .is_some_and(|at| source[at..].contains("]:"))
}
