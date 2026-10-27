//! Markdown to safe HTML or native UTF-16 text ranges. No global state or UI dependency.
mod cache;
mod document;
mod fence;
#[cfg(feature = "grammars")]
mod grammar;
mod highlight;
mod html;
mod math;
mod reference;
mod stream;

pub use document::{parse, Block, Document, Span, Style};
pub use fence::{fence_info, FenceInfo};
pub use highlight::{highlight, highlight_packed, language_alias, CodeToken, HighlightedCode};

/// Optional full Sublime grammars. Enable the `grammars` Cargo feature.
pub fn highlight_grammar(source: &str, language: &str) -> Option<std::sync::Arc<HighlightedCode>> {
    #[cfg(feature = "grammars")]
    {
        Some(grammar::highlight(source, language))
    }
    #[cfg(not(feature = "grammars"))]
    {
        let _ = (source, language);
        None
    }
}
pub use html::{render_html, render_html_into};
pub use math::{render_math, MathVisual};
pub use reference::{reference_target, Reference};
pub use stream::{RenderPatch, RenderedBlock, StreamRenderer};

/// Highlight a complete source file without wrapping it in a Markdown fence.
pub fn code_document(source: &str, language: &str) -> Document {
    highlight::code_document(source, language)
}

pub fn code_document_grammar(source: &str, language: &str) -> Option<Document> {
    highlight_grammar(source, language).map(|code| document_from_highlight(source, &code))
}

fn document_from_highlight(source: &str, highlighted: &HighlightedCode) -> Document {
    let end = source.encode_utf16().count();
    let mut spans = vec![Span {
        start: 0,
        end,
        style: Style::CodeBlock {
            language: highlighted.language.clone(),
        },
    }];
    let mut at = 0;
    for token in &highlighted.tokens {
        let end = at + token.text.encode_utf16().count();
        if !highlighted.plain && at < end {
            spans.push(Span {
                start: at,
                end,
                style: Style::Syntax {
                    light: token.light.clone(),
                    dark: token.dark.clone(),
                },
            });
        }
        at = end;
    }
    Document {
        version: 2,
        text: source.into(),
        spans,
        blocks: vec![Block {
            start: 0,
            end,
            kind: "code".into(),
        }],
    }
}

use pulldown_cmark::{Options, Parser};

fn parser(source: &str) -> Parser<'_> {
    Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    )
}

type References = std::collections::HashMap<unicase::UniCase<String>, (String, String)>;
fn reference_key(label: &str) -> unicase::UniCase<String> {
    unicase::UniCase::unicode(label.split_whitespace().collect::<Vec<_>>().join(" "))
}
fn parser_with_refs<'a>(
    source: &'a str,
    references: &'a References,
) -> Parser<
    'a,
    impl FnMut(
            pulldown_cmark::BrokenLink<'a>,
        ) -> Option<(pulldown_cmark::CowStr<'a>, pulldown_cmark::CowStr<'a>)>
        + 'a,
> {
    Parser::new_with_broken_link_callback(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
        Some(move |link: pulldown_cmark::BrokenLink<'a>| {
            references
                .get(&reference_key(&link.reference))
                .map(|(url, title)| (url.clone().into(), title.clone().into()))
        }),
    )
}

/// One serialization and one bridge call per document, never per Markdown token.
pub fn render_document_json(source: &str) -> String {
    serde_json::to_string(&parse(source)).expect("document serialization is infallible")
}

/// Allow explicit web/mail/phone schemes and relative URLs. Reject browser-normalized
/// control characters, backslashes and protocol-relative destinations.
pub fn is_safe_url(url: &str) -> bool {
    let url = url.trim();
    if url.chars().any(|c| c.is_control() || c == '\\') || url.starts_with("//") {
        return false;
    }
    let first = url.split(['/', '?', '#']).next().unwrap_or_default();
    match first.split_once(':') {
        Some((scheme, _)) => ["http", "https", "mailto", "tel"]
            .iter()
            .any(|allowed| scheme.eq_ignore_ascii_case(allowed)),
        None => true,
    }
}
