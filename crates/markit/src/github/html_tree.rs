//! Balances allowlisted raw HTML against the Markdown around it. GitHub lets
//! an element opened in one block hold the blocks after it (`<details>`, a
//! blank line, a code block, `</details>`), and inline tags wrap the inline
//! Markdown between them. Every element is tagged with the Markdown container
//! depth it opened at: it closes at its end tag at that same depth, or when
//! its container ends, so the output is always well nested. An end tag with
//! nothing to close at its depth is dropped; a tag that is not allowed is
//! shown as its source text.
use super::html_allow::{allowed, safe_element, SafeElement};
use super::html_tokens::{pieces, Piece};

#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    Open(SafeElement),
    Close(&'static str),
    /// Text as written; character references are still encoded.
    Text(String),
    /// A tag that is not allowed, drawn as the text it was written as.
    Shown(String),
}

#[derive(Default)]
pub(crate) struct Balancer {
    open: Vec<(&'static str, usize)>,
    /// Index in `open` of a `<video>` whose content is being dropped.
    video: Option<usize>,
}

impl Balancer {
    /// Read one run of raw HTML found at Markdown container `depth`.
    pub(crate) fn feed(&mut self, html: &str, depth: usize, out: &mut Vec<Action>) {
        for piece in pieces(html) {
            if let Some(video) = self.video {
                if matches!(&piece, Piece::Close { tag, .. } if tag == "video") {
                    self.close_to(video, out);
                }
                continue;
            }
            match piece {
                Piece::Comment => {}
                Piece::Text(text) => out.push(Action::Text(text.into())),
                Piece::Close { tag, raw } => match allowed(&tag) {
                    None => out.push(Action::Shown(raw.into())),
                    Some((_, true)) => {}
                    Some((name, false)) => {
                        let at = self
                            .open
                            .iter()
                            .rposition(|&(open, at)| open == name && at == depth);
                        if let Some(at) = at {
                            self.close_to(at, out);
                        }
                    }
                },
                Piece::Open { tag, attrs, raw } => match safe_element(&tag, &attrs) {
                    None => out.push(Action::Shown(raw.into())),
                    Some(element) => {
                        let (name, void) = (element.tag, element.void);
                        out.push(Action::Open(element));
                        // As in a browser, `/>` closes only void elements.
                        if !void {
                            self.open.push((name, depth));
                            if name == "video" {
                                self.video = Some(self.open.len() - 1);
                            }
                        }
                    }
                },
            }
        }
    }

    /// Close every element opened at `depth` or deeper.
    pub(crate) fn close_from(&mut self, depth: usize, out: &mut Vec<Action>) {
        let keep = self.open.partition_point(|&(_, at)| at < depth);
        self.close_to(keep, out);
    }

    /// Whether an element is still open.
    pub(crate) fn is_open(&self) -> bool {
        !self.open.is_empty()
    }

    /// Inside a `<video>`, whose content is never drawn.
    pub(crate) fn dropping(&self) -> bool {
        self.video.is_some()
    }

    /// Whether one of `tags` is open: text inside a raw `<a>` or `<code>`
    /// is never linked again.
    pub(crate) fn inside(&self, tags: &[&str]) -> bool {
        self.open.iter().any(|(tag, _)| tags.contains(tag))
    }

    fn close_to(&mut self, index: usize, out: &mut Vec<Action>) {
        while self.open.len() > index {
            let (tag, _) = self.open.pop().expect("length checked");
            out.push(Action::Close(tag));
        }
        if self.video.is_some_and(|video| video >= index) {
            self.video = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html: &str) -> Vec<String> {
        let mut balancer = Balancer::default();
        let mut out = Vec::new();
        balancer.feed(html, 0, &mut out);
        balancer.close_from(0, &mut out);
        out.iter()
            .map(|action| match action {
                Action::Open(element) => format!("<{}>", element.tag),
                Action::Close(tag) => format!("</{tag}>"),
                Action::Text(text) => text.clone(),
                Action::Shown(raw) => format!("shown:{raw}"),
            })
            .collect()
    }

    #[test]
    fn unclosed_tags_close_at_the_end_and_stray_end_tags_are_dropped() {
        assert_eq!(
            run("before <b>bold <i>both</b> after</i>"),
            ["before ", "<b>", "bold ", "<i>", "both", "</i>", "</b>", " after"]
        );
    }

    #[test]
    fn video_content_is_dropped_and_disallowed_tags_are_shown() {
        assert_eq!(
            run("<video src=\"https://x.test/v.mp4\"><b>no</b><script>x</script></video><script>"),
            ["<video>", "</video>", "shown:<script>"]
        );
    }
}
