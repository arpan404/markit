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

pub(crate) fn render_html_code(
    source: &str,
    output: &mut String,
    references: &crate::References,
    code_renderer: &mut impl FnMut(&str, &crate::FenceInfo) -> String,
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

pub(crate) fn render_events<'a>(
    prepared: &crate::math::Prepared<'_>,
    events: impl Iterator<Item = Event<'a>>,
    output: &mut String,
) {
    output.clear();
    push_events(prepared, events, output, &mut render_code, false);
}

/// The one HTML stage behind every renderer. Raw HTML is escaped unless
/// `trusted_html` is set, which only the GitHub flavour does after its
/// allowlist has rebuilt every tag (see `github::html`).
pub(crate) fn push_events<'a>(
    prepared: &crate::math::Prepared<'_>,
    events: impl Iterator<Item = Event<'a>>,
    output: &mut String,
    code_renderer: &mut impl FnMut(&str, &crate::FenceInfo) -> String,
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

fn open_line(line: usize, output: &mut String) {
    output.push_str("<span class=\"mk-line\" data-line=\"");
    push_usize(line, output);
    output.push_str("\">");
}

fn write_header(info: &crate::FenceInfo, output: &mut String) {
    output.push_str("<section class=\"mk-codeblock\"><header><span>");
    escape_into(info.file.as_deref().unwrap_or(&info.language), output);
    output.push_str(
        "</span><button type=\"button\" data-markit-copy aria-label=\"Copy code\">Copy</button></header><pre><code class=\"language-",
    );
    escape_into(&info.language, output);
    output.push_str("\">");
}

/// Retains HTML for every line the highlighter has closed. Append-only fences
/// rewrite the open line instead of the whole block.
pub(crate) struct CodeHtml {
    html: String,
    prefix_len: usize,
    stable: usize,
    line: usize,
}

impl CodeHtml {
    pub(crate) fn new(info: &crate::FenceInfo) -> Self {
        let mut html = String::new();
        write_header(info, &mut html);
        open_line(1, &mut html);
        let prefix_len = html.len();
        Self {
            html,
            prefix_len,
            stable: 0,
            line: 1,
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
            let (line, at_line_start) = write_runs(
                &mut self.html,
                code,
                info,
                runs,
                self.stable,
                stable,
                self.line,
            );
            debug_assert!(at_line_start);
            self.line = line;
            self.stable = stable;
            self.prefix_len = self.html.len();
        }
        let saved = self.prefix_len;
        let (_line, at_line_start) = write_runs(
            &mut self.html,
            code,
            info,
            runs,
            stable,
            code.len(),
            self.line,
        );
        if info.diff && !at_line_start {
            self.html.push_str("</span>");
        }
        self.html.push_str("</span></code></pre></section>\n");
        let result = self.html.clone();
        self.html.truncate(saved);
        result
    }
}

/// Write highlighted `code[from..to]` into an already-open line span.
/// Returns the line number of the span left open, and whether that span is
/// still at column zero.
fn write_runs(
    output: &mut String,
    code: &str,
    info: &crate::FenceInfo,
    runs: &crate::highlight::LexRuns,
    from: usize,
    to: usize,
    mut line: usize,
) -> (usize, bool) {
    let start = runs.first_ending_after(from);
    let mut byte = if start == 0 { 0 } else { runs.end(start - 1) };
    let mut at_line_start = true;
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
            if at_line_start && info.diff {
                output.push_str("<span class=\"mk-diff-");
                output.push_str(diff_kind(part));
                output.push_str("\">");
            }
            output.push_str("<span style=\"--mk-light:");
            output.push_str(light);
            output.push_str(";--mk-dark:");
            output.push_str(dark);
            output.push_str("\">");
            escape_into(part, output);
            output.push_str("</span>");
            if part.ends_with('\n') {
                if info.diff {
                    output.push_str("</span>");
                }
                output.push_str("</span>");
                line += 1;
                open_line(line, output);
                at_line_start = true;
            } else {
                at_line_start = false;
            }
        }
        byte = end;
    }
    (line, at_line_start)
}

fn render_code(code: &str, info: &crate::FenceInfo) -> String {
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
