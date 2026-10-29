//! Where task boxes and closed code fences sit in the Markdown source. Hosts
//! tick a box by rewriting the character inside its `[ ]` at `offset` (a
//! UTF-16 index into the source, as JavaScript strings and NSString count),
//! the same mark ace's forge renderer places: `source[offset - 1] == '['`
//! and `source[offset + 1] == ']'`. Boxes inside footnote definitions are
//! drawn but not placed.
use pulldown_cmark::{CodeBlockKind, Event, Tag, TagEnd};
use std::ops::Range;

/// Task box positions and fence closure for one run of events, in order.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Marks {
    /// `(index, offset)` of each placed box.
    pub tasks: Vec<(usize, usize)>,
    /// For each fenced code block outside footnote definitions: whether its
    /// closing fence was written. Only closed fences become diagrams, so a
    /// stream never swaps a half-written diagram in.
    pub fences: Vec<bool>,
}

/// Reads events with their byte ranges in the original source.
pub(crate) struct MarkScanner<'s> {
    source: &'s str,
    byte: usize,
    utf16: usize,
    next_index: usize,
    definitions: usize,
    /// End of the fence block and end of its last content seen so far.
    fence: Option<(usize, usize)>,
    marks: Marks,
}

impl<'s> MarkScanner<'s> {
    /// `first_index` numbers the first box: the boxes of earlier blocks.
    pub(crate) fn new(source: &'s str, first_index: usize) -> Self {
        Self {
            source,
            byte: 0,
            utf16: 0,
            next_index: first_index,
            definitions: 0,
            fence: None,
            marks: Marks::default(),
        }
    }

    fn utf16_at(&mut self, byte: usize) -> usize {
        if byte < self.byte {
            self.byte = 0;
            self.utf16 = 0;
        }
        self.utf16 += crate::highlight::utf16_len(&self.source[self.byte..byte]);
        self.byte = byte;
        self.utf16
    }

    pub(crate) fn event(&mut self, event: &Event<'_>, range: Range<usize>) {
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => self.definitions += 1,
            Event::End(TagEnd::FootnoteDefinition) => self.definitions -= 1,
            _ if self.definitions > 0 => {}
            Event::TaskListMarker(_) => {
                // The range covers `[ ]`; the mark is the character inside.
                let inside = range.start + 1;
                let index = self.next_index;
                self.next_index += 1;
                if self.source.as_bytes().get(inside.wrapping_sub(1)) == Some(&b'[') {
                    let offset = self.utf16_at(inside);
                    self.marks.tasks.push((index, offset));
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                let block = &self.source[range.clone()];
                let opened = block
                    .find('\n')
                    .map_or(range.end, |at| range.start + at + 1);
                self.fence = Some((range.end, opened));
            }
            Event::Text(_) => {
                if let Some((_, content)) = self.fence.as_mut() {
                    *content = range.end;
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((end, content)) = self.fence.take() {
                    let after = &self.source[content.min(end)..end];
                    self.marks
                        .fences
                        .push(after.contains("```") || after.contains("~~~"));
                }
            }
            _ => {}
        }
    }

    /// The marks found since the last take.
    pub(crate) fn take(&mut self) -> Marks {
        std::mem::take(&mut self.marks)
    }

    /// The index the next box takes.
    pub(crate) fn next_index(&self) -> usize {
        self.next_index
    }
}

/// `source` with the box at `offset` (UTF-16) set to `checked`, or `None`
/// when the text there is no longer a box.
pub fn with_task(source: &str, offset: usize, checked: bool) -> Option<String> {
    let units: Vec<u16> = source.encode_utf16().collect();
    let before = *units.get(offset.checked_sub(1)?)?;
    let inside = *units.get(offset)?;
    let after = *units.get(offset + 1)?;
    if before != u16::from(b'[')
        || after != u16::from(b']')
        || !b" xX".iter().any(|&b| u16::from(b) == inside)
    {
        return None;
    }
    let mut next = units;
    next[offset] = u16::from(if checked { b'x' } else { b' ' });
    String::from_utf16(&next).ok()
}
