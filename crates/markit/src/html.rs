use pulldown_cmark::{html, CodeBlockKind, CowStr, Event, Tag};

pub(crate) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
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
    escaped
}

pub(crate) fn bare_links(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
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
    let mut input = super::parser_with_refs(&prepared.source, references)
        .filter(|event| !matches!(event, Event::InlineHtml(text) if prepared.is_separator(text)));
    let mut link_depth = 0usize;
    let events = std::iter::from_fn(|| {
        let event = input.next()?;
        Some(match event {
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
                    let mut at = 0;
                    for (range, url) in links {
                        html.push_str(&escape(&text[at..range.start]));
                        html.push_str(&format!(
                            "<a href=\"{}\">{}</a>",
                            escape(&url),
                            escape(&text[range.clone()])
                        ));
                        at = range.end;
                    }
                    html.push_str(&escape(&text[at..]));
                    Event::Html(html.into())
                }
            }
            other => other,
        })
    });
    html::push_html(output, events);
}

fn render_code(code: &str, info: &crate::FenceInfo) -> String {
    let mut output = format!("<section class=\"mk-codeblock\"><header><span>{}</span><button type=\"button\" data-markit-copy aria-label=\"Copy code\">Copy</button></header><pre><code class=\"language-{}\">",
        escape(info.file.as_deref().unwrap_or(&info.language)), escape(&info.language));
    let mut line = 1usize;
    output.push_str("<span class=\"mk-line\" data-line=\"1\">");
    let mut at_line_start = true;
    crate::highlight::for_each_run(code, &info.language, |text, light, dark| {
        for part in text.split_inclusive('\n') {
            if at_line_start && info.diff {
                let kind = if part.starts_with("+++")
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
                };
                output.push_str(&format!("<span class=\"mk-diff-{kind}\">"));
            }
            output.push_str(&format!(
                "<span style=\"--mk-light:{};--mk-dark:{}\">{}</span>",
                light,
                dark,
                escape(part)
            ));
            if part.ends_with('\n') {
                if info.diff {
                    output.push_str("</span>");
                }
                output.push_str("</span>");
                line += 1;
                output.push_str(&format!("<span class=\"mk-line\" data-line=\"{line}\">"));
                at_line_start = true;
            } else {
                at_line_start = false;
            }
        }
    });
    if info.diff && !at_line_start {
        output.push_str("</span>");
    }
    output.push_str("</span></code></pre></section>\n");
    output
}

pub fn render_html(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    render_html_into(source, &mut output);
    output
}
