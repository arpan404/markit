use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Tag};

pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    escape_into(text, &mut escaped);
    escaped
}

pub(crate) fn escape_into(text: &str, escaped: &mut String) {
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
}

pub(crate) fn bare_links(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
    // linkify only emits schemes, e-mail addresses, and www-prefixed hosts.
    if !text.bytes().any(|b| b == b':' || b == b'@') && !contains_www(text.as_bytes()) {
        return Vec::new();
    }
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url, linkify::LinkKind::Email]);
    finder
        .links(text)
        .filter_map(|link| {
            let raw = link.as_str();
            let url = if link.kind() == &linkify::LinkKind::Email {
                format!("mailto:{raw}")
            } else if raw.starts_with("www.") {
                format!("https://{raw}")
            } else {
                raw.into()
            };
            crate::is_safe_url(&url).then_some((link.start()..link.end(), url))
        })
        .collect()
}

fn contains_www(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|window| {
        window[0].eq_ignore_ascii_case(&b'w')
            && window[1].eq_ignore_ascii_case(&b'w')
            && window[2].eq_ignore_ascii_case(&b'w')
            && window[3] == b'.'
    })
}

/// Reuse the caller's allocation. Raw HTML is always escaped. Code, math and
/// image placeholders below are generated solely from escaped or numeric data.
pub fn render_html_into(source: &str, output: &mut String) {
    render_html_with_refs(source, output, &crate::References::new());
}

pub(crate) fn render_html_with_refs(
    source: &str,
    output: &mut String,
    references: &crate::References,
) {
    render_html_code(source, output, references, &mut render_code);
}

/// Renders one fenced or indented code block's HTML.
pub(crate) type CodeRenderer<'r> = dyn FnMut(&str, &crate::FenceInfo) -> String + 'r;

pub(crate) fn render_html_code(
    source: &str,
    output: &mut String,
    references: &crate::References,
    code_renderer: &mut CodeRenderer<'_>,
) {
    output.clear();
    output.reserve(source.len());
    let prepared = crate::math::prepare(source);
    push_events(
        &prepared,
        super::parser_with_refs(&prepared.source, references),
        output,
        code_renderer,
        false,
    );
}

pub(crate) fn render_events(
    prepared: &crate::math::Prepared<'_>,
    events: Vec<Event<'_>>,
    output: &mut String,
) {
    output.clear();
    push_events(
        prepared,
        events.into_iter(),
        output,
        &mut render_code,
        false,
    );
}

/// The one HTML stage behind every renderer. Raw HTML is escaped unless
/// `trusted_html` is set, which only the GitHub flavour does after its
/// allowlist has rebuilt every tag (see `github::html`).
pub(crate) fn push_events<'a>(
    prepared: &crate::math::Prepared<'_>,
    events: impl Iterator<Item = Event<'a>>,
    output: &mut String,
    code_renderer: &mut CodeRenderer<'_>,
    trusted_html: bool,
) {
    let mut input = events
        .filter(|event| !matches!(event, Event::InlineHtml(text) if prepared.is_separator(text)));
    let mut link_depth = 0usize;
    let events = std::iter::from_fn(|| {
        let event = input.next()?;
        Some(match event {
            Event::Html(text) | Event::InlineHtml(text) if trusted_html => Event::Html(text),
            Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => crate::fence_info(&info),
                    CodeBlockKind::Indented => crate::FenceInfo::default(),
                };
                let mut code = String::new();
                for event in input.by_ref() {
                    match event {
                        Event::End(pulldown_cmark::TagEnd::CodeBlock) => break,
                        Event::Text(text) => code.push_str(&text),
                        _ => {}
                    }
                }
                Event::Html(code_renderer(&code, &info).into())
            }
            Event::Code(code) => {
                if let Some(formula) = prepared.formula(&code) {
                    if let Some(visual) = crate::math::render_svg(&formula.tex, formula.display) {
                        Event::Html(format!("<span class=\"mk-math{}\" role=\"math\" aria-label=\"{}\" data-tex=\"{}\">{}</span>",
                            if formula.display { " mk-display" } else { "" }, escape(&formula.tex), escape(&formula.raw), visual.svg).into())
                    } else {
                        Event::Text(formula.raw.clone().into())
                    }
                } else if let Some(reference) = crate::reference::inline_file(&code) {
                    Event::Html(format!("<code role=\"link\" tabindex=\"0\" data-markit-reference=\"{}\" data-reference-kind=\"file\">{}</code>", escape(&reference.target), escape(&code)).into())
                } else {
                    Event::Code(code)
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let mut alt = String::new();
                let mut depth = 1;
                for event in input.by_ref() {
                    match event {
                        Event::Start(Tag::Image { .. }) => depth += 1,
                        Event::End(pulldown_cmark::TagEnd::Image) => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        Event::Text(text) | Event::Code(text) => alt.push_str(&text),
                        Event::SoftBreak | Event::HardBreak => alt.push(' '),
                        _ => {}
                    }
                }
                let url = if crate::is_safe_url(&dest_url) {
                    dest_url.trim()
                } else {
                    ""
                };
                Event::Html(
                    format!(
                        "<img data-markit-src=\"{}\" alt=\"{}\" loading=\"lazy\">",
                        escape(url),
                        escape(&alt)
                    )
                    .into(),
                )
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                link_depth += 1;
                if let Some(reference) = crate::reference_target(&dest_url) {
                    let href = if crate::is_safe_url(&dest_url) {
                        format!(" href=\"{}\"", escape(&dest_url))
                    } else {
                        String::new()
                    };
                    return Some(Event::Html(format!("<a{href} role=\"link\" tabindex=\"0\" data-markit-reference=\"{}\" data-reference-kind=\"{}\">", escape(&reference.target), escape(&reference.kind)).into()));
                }
                let dest_url = if super::is_safe_url(&dest_url) {
                    CowStr::from(dest_url.trim().to_owned())
                } else {
                    CowStr::Borrowed("")
                };
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
            Event::End(pulldown_cmark::TagEnd::Link) => {
                link_depth = link_depth.saturating_sub(1);
                Event::End(pulldown_cmark::TagEnd::Link)
            }
            Event::Text(text) if link_depth == 0 => {
                let links = bare_links(&text);
                if links.is_empty() {
                    Event::Text(text)
                } else {
                    let mut html = String::new();
                    push_linked_text(&text, &links, &mut html);
                    Event::Html(html.into())
                }
            }
            other => other,
        })
    });
    html::push_html(output, events);
}

/// `text` escaped, with each bare link from [`bare_links`] as an anchor.
pub(crate) fn push_linked_text(
    text: &str,
    links: &[(std::ops::Range<usize>, String)],
    html: &mut String,
) {
    let mut at = 0;
    for (range, url) in links {
        escape_into(&text[at..range.start], html);
        html.push_str("<a href=\"");
        escape_into(url, html);
        html.push_str("\">");
        escape_into(&text[range.clone()], html);
        html.push_str("</a>");
        at = range.end;
    }
    escape_into(&text[at..], html);
}

pub(crate) fn diff_kind(part: &str) -> &'static str {
    if part.starts_with("+++")
        || part.starts_with("---")
        || part.starts_with("@@")
        || part.starts_with("diff ")
    {
        "meta"
    } else if part.starts_with('+') {
        "addition"
    } else if part.starts_with('-') {
        "deletion"
    } else {
        "context"
    }
}

fn push_usize(value: usize, output: &mut String) {
    let mut buf = [0u8; 20];
    let mut n = value;
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    output.push_str(std::str::from_utf8(&buf[i..]).expect("digits are utf-8"));
}

fn write_header(info: &crate::FenceInfo, diff: Option<&crate::DiffCursor>, output: &mut String) {
    output.push_str("<section class=\"mk-codeblock\"><header>");
    match info.file.as_deref() {
        Some(file) => {
            output.push_str("<span class=\"mk-code-title\" title=\"");
            escape_into(file, output);
            output.push_str("\"><bdi>");
            escape_into(file, output);
            output.push_str("</bdi></span>");
        }
        None => {
            output.push_str("<span class=\"mk-code-title\">");
            let label = if info.language.is_empty() {
                "text"
            } else {
                &info.language
            };
            escape_into(label, output);
            output.push_str("</span>");
        }
    }
    if let Some(diff) = diff {
        output.push_str("<span class=\"mk-diff-stat\"><span class=\"mk-diff-added\">+");
        push_usize(diff.additions as usize, output);
        output.push_str("</span> <span class=\"mk-diff-removed\">\u{2212}");
        push_usize(diff.deletions as usize, output);
        output.push_str("</span></span>");
    }
    output.push_str(
        "<button type=\"button\" data-markit-copy aria-label=\"Copy code\">Copy</button><span class=\"mk-copy-status\" role=\"status\"></span></header><pre><code class=\"language-",
    );
    escape_into(&info.language, output);
    output.push_str("\">");
}

/// Line numbering and diff state where the next line starts.
#[derive(Clone)]
struct Lines {
    /// Number of the next line, from 1.
    next: usize,
    diff: crate::DiffCursor,
    /// Closing markup of the line left open, if any.
    open: Option<&'static str>,
}

impl Lines {
    fn new() -> Self {
        Self {
            next: 1,
            diff: crate::DiffCursor::default(),
            open: None,
        }
    }

    /// Opens the line whose full text (as known so far) is `text`.
    fn open(&mut self, text: &str, diff: bool, output: &mut String) {
        if !diff {
            output.push_str("<span class=\"mk-line\" data-line=\"");
            push_usize(self.next, output);
            output.push_str("\">");
            self.open = Some("</span>");
        } else {
            let row = self.diff.row(text);
            output.push_str("<span class=\"mk-line mk-diff-");
            output.push_str(row.kind.class());
            output.push('"');
            if let Some(old) = row.old {
                output.push_str(" data-old=\"");
                push_usize(old as usize, output);
                output.push('"');
            }
            let (tag, close) = match row.kind {
                crate::DiffKind::Addition => ("><ins", "</ins></span>"),
                crate::DiffKind::Deletion => ("><del", "</del></span>"),
                _ => ("><span", "</span></span>"),
            };
            output.push_str(tag);
            if let Some(new) = row.new {
                output.push_str(" data-new=\"");
                push_usize(new as usize, output);
                output.push('"');
            }
            output.push('>');
            self.open = Some(close);
        }
        self.next += 1;
    }

    fn close(&mut self, output: &mut String) {
        if let Some(close) = self.open.take() {
            output.push_str(close);
        }
    }

    /// Closes the open line, or writes the empty line after a final newline.
    fn finish(&mut self, diff: bool, output: &mut String) {
        if self.open.is_some() {
            self.close(output);
        } else if diff {
            output.push_str("<span class=\"mk-line mk-diff-context\"></span>");
        } else {
            output.push_str("<span class=\"mk-line\" data-line=\"");
            push_usize(self.next, output);
            output.push_str("\"></span>");
        }
    }
}

/// Retains HTML for every line the highlighter has closed. Append-only fences
/// rewrite the open line instead of the whole block. The header is written last
/// because a diff's +/- counts grow with the fence.
pub(crate) struct CodeHtml {
    body: String,
    stable: usize,
    lines: Lines,
}

impl CodeHtml {
    pub(crate) fn new(_info: &crate::FenceInfo) -> Self {
        Self {
            body: String::new(),
            stable: 0,
            lines: Lines::new(),
        }
    }

    pub(crate) fn render(
        &mut self,
        code: &str,
        info: &crate::FenceInfo,
        runs: &crate::highlight::LexRuns,
    ) -> String {
        let stable = runs.stable_byte.min(code.len());
        if stable < self.stable {
            *self = Self::new(info);
        }
        if stable > self.stable {
            write_runs(
                &mut self.body,
                code,
                info,
                runs,
                self.stable,
                stable,
                &mut self.lines,
            );
            debug_assert!(self.lines.open.is_none());
            self.stable = stable;
        }
        let mut tail = String::new();
        let mut lines = self.lines.clone();
        write_runs(&mut tail, code, info, runs, stable, code.len(), &mut lines);
        lines.finish(info.diff, &mut tail);
        let mut html = String::with_capacity(self.body.len() + tail.len() + 320);
        write_header(info, info.diff.then_some(&lines.diff), &mut html);
        html.push_str(&self.body);
        html.push_str(&tail);
        html.push_str("</code></pre></section>\n");
        html
    }
}

/// Write highlighted `code[from..to]`, where `from` starts a line. Lines open
/// at their first character, once their kind is known, and close at `\n`.
fn write_runs(
    output: &mut String,
    code: &str,
    info: &crate::FenceInfo,
    runs: &crate::highlight::LexRuns,
    from: usize,
    to: usize,
    lines: &mut Lines,
) {
    let start = runs.first_ending_after(from);
    let mut byte = if start == 0 { 0 } else { runs.end(start - 1) };
    for (end, id) in runs.from(start) {
        if byte >= to {
            break;
        }
        let (light, dark, _, _) = crate::highlight::PALETTE[id as usize];
        let text = &code[byte..end];
        let mut local = 0usize;
        for part in text.split_inclusive('\n') {
            let part_start = byte + local;
            let part_end = part_start + part.len();
            local = part_end - byte;
            if part_end <= from {
                continue;
            }
            if part_start >= to {
                break;
            }
            if lines.open.is_none() {
                let line_end = code[part_start..]
                    .find('\n')
                    .map_or(code.len(), |at| part_start + at + 1);
                lines.open(&code[part_start..line_end], info.diff, output);
            }
            output.push_str("<span style=\"--mk-light:");
            output.push_str(light);
            output.push_str(";--mk-dark:");
            output.push_str(dark);
            output.push_str("\">");
            escape_into(part, output);
            output.push_str("</span>");
            if part.ends_with('\n') {
                lines.close(output);
            }
        }
        byte = end;
    }
}

pub(crate) fn render_code(code: &str, info: &crate::FenceInfo) -> String {
    let runs = crate::highlight::lex_runs(code, &info.language);
    CodeHtml::new(info).render(code, info, &runs)
}

pub fn render_html(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    render_html_into(source, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::{render_code, CodeHtml};

    #[test]
    fn diff_fences_mark_insertions_deletions_and_hunk_line_numbers() {
        let html = crate::render_html("```diff\n@@ -4,2 +4,2 @@\n keep\n-old\n+new\n```\n");
        assert!(html.contains("<span class=\"mk-diff-added\">+1</span> <span class=\"mk-diff-removed\">\u{2212}1</span>"), "{html}");
        assert!(
            html.contains(
                "<span class=\"mk-line mk-diff-context\" data-old=\"4\"><span data-new=\"4\">"
            ),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"mk-line mk-diff-deletion\" data-old=\"5\"><del>"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"mk-line mk-diff-addition\"><ins data-new=\"5\">"),
            "{html}"
        );
        assert!(
            html.contains("role=\"status\""),
            "copy is announced: {html}"
        );
    }

    #[test]
    fn file_labels_keep_their_full_path_as_a_title() {
        let html = crate::render_html("```ts title=\"src/deep/a.ts\"\nx\n```\n");
        assert!(html.contains("<span class=\"mk-code-title\" title=\"src/deep/a.ts\"><bdi>src/deep/a.ts</bdi></span>"), "{html}");
        let plain = crate::render_html("```\nx\n```\n");
        assert!(
            plain.contains("<span class=\"mk-code-title\">text</span>"),
            "{plain}"
        );
    }

    #[test]
    fn incremental_code_html_matches_a_fresh_render() {
        let samples = [
            ("rust", ""),
            ("rust", "let x = 1;"),
            ("rust", "let x = 1;\n"),
            ("rust", "fn main() {\n    let s = \"hi\";\n}\n"),
            ("rust", "/* comment\nstill */\nfn main() {\nlet s = r#\"a\nb\"#;\n}\n"),
            ("python", "x = \"\"\"long\nstr\"\"\"\ny\n"),
            ("diff", "diff --git a/f.rs b/f.rs\n--- a/f.rs\n+++ b/f.rs\n@@ -1 +1 @@\n-old\n+new\n context\n"),
            ("tsx", "<Component\n property=\"value\"/>\n<!-- c\nomment -->\n😀\n"),
            ("sql", "SELECT name FROM t WHERE x IS NOT NULL\n"),
            ("html", "<div title=\"😀\">hello & world</div>\n"),
            ("plain", "not a language\nsecond\n"),
        ];
        for (language, sample) in samples {
            let info = crate::fence_info(language);
            let mut html = CodeHtml::new(&info);
            let mut built = String::new();
            for ch in sample.chars() {
                built.push(ch);
                let runs = crate::highlight::lex_runs(&built, &info.language);
                let incremental = html.render(&built, &info, &runs);
                let full = render_code(&built, &info);
                assert_eq!(incremental, full, "{language} {built:?}");
            }
        }
    }
}
