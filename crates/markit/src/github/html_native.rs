//! Allowlisted raw HTML as native text and spans: formatting tags map onto
//! the existing kinds where one fits (`<b>` is `strong`, `<kbd>` is `code`
//! plus `html`), the rest onto `html` spans named by tag. Images and videos
//! become `image` / `video` spans the host resolves; a `<picture>` dark
//! source becomes a `dark_image` span over the same image.
use super::html_allow::SafeElement;
use super::html_tree::Action;
use crate::document::{Builder, Style};

const BLOCK: &[&str] = &[
    "blockquote",
    "caption",
    "center",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "ol",
    "p",
    "pre",
    "summary",
    "table",
    "tbody",
    "tfoot",
    "thead",
    "tr",
    "ul",
];
/// Tags without a kind of their own that hosts may still style.
const NAMED: &[&str] = &[
    "abbr",
    "bdo",
    "center",
    "details",
    "figcaption",
    "figure",
    "ins",
    "kbd",
    "mark",
    "q",
    "rp",
    "rt",
    "ruby",
    "small",
    "sub",
    "summary",
    "sup",
    "time",
    "u",
];

#[derive(Default)]
pub(crate) struct NativeHtml {
    open: Vec<(SafeElement, usize)>,
    /// A `<picture>` dark-mode source waiting for its `<img>`.
    dark: Option<String>,
}

impl NativeHtml {
    pub(crate) fn apply(&mut self, actions: Vec<Action>, builder: &mut Builder) {
        for action in actions {
            match action {
                Action::Open(element) => self.open(element, builder),
                Action::Close(tag) => self.close(tag, builder),
                Action::Text(text) => text_run(&text, builder),
                Action::Shown(raw) => builder.text(&raw),
            }
        }
    }

    fn open(&mut self, element: SafeElement, builder: &mut Builder) {
        if BLOCK.contains(&element.tag) {
            builder.newline();
        }
        match element.tag {
            "br" => builder.text("\n"),
            "hr" => {
                builder.newline();
                let start = builder.position;
                builder.text("────────\n");
                builder.span(start, Style::Rule);
            }
            "wbr" => {}
            "source" => {
                let dark = element
                    .attr("media")
                    .is_some_and(|media| media.contains("dark"));
                if let Some(url) = element.attr("data-markit-srcset").filter(|_| dark) {
                    self.dark = Some(url.into());
                }
            }
            "img" => {
                let start = builder.position;
                let alt = element.attr("alt").unwrap_or_default();
                builder.text(if alt.is_empty() { "[image]" } else { alt });
                let url = element.attr("data-markit-src").unwrap_or_default();
                let dark = self.dark.take();
                if !url.is_empty() {
                    builder.span(start, Style::Image { url: url.into() });
                    if let Some(url) = dark {
                        builder.span(start, Style::DarkImage { url });
                    }
                }
            }
            "li" => {
                builder.text("• ");
                self.open.push((element, builder.position));
            }
            "td" | "th" => {
                if !builder.document.text.is_empty() && !builder.document.text.ends_with('\n') {
                    builder.text("\t");
                }
                self.open.push((element, builder.position));
            }
            "video" => {
                let start = builder.position;
                builder.text("[video]");
                let url = element.attr("data-markit-src").unwrap_or_default();
                if !url.is_empty() {
                    builder.span(start, Style::Video { url: url.into() });
                }
                self.open.push((element, builder.position));
            }
            _ => self.open.push((element, builder.position)),
        }
    }

    fn close(&mut self, tag: &str, builder: &mut Builder) {
        let Some(at) = self
            .open
            .iter()
            .rposition(|(element, _)| element.tag == tag)
        else {
            return;
        };
        let (element, start) = self.open.remove(at);
        let mut styles = Vec::new();
        match element.tag {
            "b" | "strong" => styles.push(Style::Strong),
            "i" | "em" | "cite" | "dfn" | "var" => styles.push(Style::Emphasis),
            "del" | "s" | "strike" => styles.push(Style::Strikethrough),
            "code" | "tt" | "samp" | "kbd" => styles.push(Style::Code),
            "a" => {
                if let Some(href) = element.attr("href") {
                    styles.push(match crate::reference_target(href) {
                        Some(reference) => Style::Reference {
                            reference_kind: reference.kind,
                            target: reference.target,
                        },
                        None => Style::Link { url: href.into() },
                    });
                }
            }
            "picture" => self.dark = None,
            heading if heading.len() == 2 && heading.starts_with('h') => {
                let level = heading.as_bytes()[1] - b'0';
                styles.push(Style::Heading { level });
            }
            _ => {}
        }
        if NAMED.contains(&element.tag) {
            styles.push(Style::Html {
                tag: element.tag.into(),
            });
        }
        for style in styles {
            builder.span(start, style);
        }
        if BLOCK.contains(&element.tag) {
            builder.newline();
        }
    }
}

/// Raw HTML text with references decoded and whitespace collapsed as a
/// browser would.
fn text_run(raw: &str, builder: &mut Builder) {
    let decoded = super::entities::decode_entities(raw);
    let mut text = String::with_capacity(decoded.len());
    let mut space =
        builder.document.text.is_empty() || builder.document.text.ends_with(['\n', ' ', '\t']);
    for c in decoded.chars() {
        if c.is_ascii_whitespace() {
            if !space {
                text.push(' ');
                space = true;
            }
        } else {
            text.push(c);
            space = false;
        }
    }
    if !text.is_empty() {
        builder.text(&text);
    }
}
