use crate::{Document, Span};
use std::ops::Range;

/// A borrowed index cannot become stale when the public document is edited.
/// Leaves retain original indices; a max-end tree prunes enclosing spans without
/// assuming that ends are sorted. Queries always preserve document span order.
pub struct SpanIndex<'a> {
    document: &'a Document,
    order: Vec<usize>,
    max_end: Vec<usize>,
    lines: Vec<usize>,
}

impl<'a> SpanIndex<'a> {
    pub fn new(document: &'a Document) -> Self {
        let mut order: Vec<_> = (0..document.spans.len()).collect();
        order.sort_by_key(|&i| document.spans[i].start);
        let mut max_end = vec![0; order.len().next_power_of_two() * 2];
        let base = max_end.len() / 2;
        for (at, &i) in order.iter().enumerate() {
            max_end[base + at] = document.spans[i].end;
        }
        for at in (1..base).rev() {
            max_end[at] = max_end[at * 2].max(max_end[at * 2 + 1]);
        }
        let mut lines = vec![0];
        let mut position = 0;
        for c in document.text.chars() {
            position += c.len_utf16();
            if c == '\n' {
                lines.push(position);
            }
        }
        lines.push(position);
        Self {
            document,
            order,
            max_end,
            lines,
        }
    }

    pub fn query(&self, range: Range<usize>) -> Vec<&'a Span> {
        if range.start >= range.end {
            return Vec::new();
        }
        let limit = self
            .order
            .partition_point(|&i| self.document.spans[i].start < range.end);
        let mut found = Vec::new();
        self.visit(1, 0, self.max_end.len() / 2, limit, range.start, &mut found);
        found.sort_unstable();
        found.into_iter().map(|i| &self.document.spans[i]).collect()
    }

    fn visit(
        &self,
        node: usize,
        start: usize,
        end: usize,
        limit: usize,
        from: usize,
        found: &mut Vec<usize>,
    ) {
        if start >= limit || self.max_end[node] <= from {
            return;
        }
        if end - start == 1 {
            let i = self.order[start];
            let span = &self.document.spans[i];
            if span.start < span.end {
                found.push(i);
            }
        } else {
            let mid = (start + end) / 2;
            self.visit(node * 2, start, mid, limit, from, found);
            self.visit(node * 2 + 1, mid, end, limit, from, found);
        }
    }

    pub fn block(&self, block: usize) -> Vec<&'a Span> {
        self.document
            .blocks
            .get(block)
            .map_or_else(Vec::new, |b| self.query(b.start..b.end))
    }

    /// Zero-based rendered text line, including its newline when present.
    pub fn line(&self, line: usize) -> Vec<&'a Span> {
        self.lines
            .get(line..line.saturating_add(2))
            .map_or_else(Vec::new, |r| self.query(r[0]..r[1]))
    }
}
