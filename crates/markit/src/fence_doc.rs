//! Incremental native document for one open code fence. Output matches
//! [`crate::document::parse_events`] for a single fenced block.
use crate::document::{Block, Document, Span, Style};

/// Native code-fence document built from lexical runs. Output matches
/// [`crate::document::parse_events`] for a single fenced block, including the empty-fence
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
        crate::document::parse_events(&prepared, events.into_iter())
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
