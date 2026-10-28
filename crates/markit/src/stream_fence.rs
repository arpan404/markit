//! Fast continuation only for unindented, top-level open fences. Markdown
//! containers, normalized CR/NUL input and closing fences use the parser.
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use std::sync::Arc;

pub(crate) struct OpenFence {
    body: usize,
    recheck: usize,
    marker: u8,
    width: usize,
    info: String,
}

impl OpenFence {
    pub(crate) fn new(source: &str, start: usize, info: &str) -> Option<Self> {
        let opening_end = source[start..].find('\n')? + start + 1;
        let opening = &source[start..opening_end - 1];
        let marker = *opening.as_bytes().first()?;
        if !matches!(marker, b'`' | b'~') {
            return None;
        }
        let width = opening.bytes().take_while(|&b| b == marker).count();
        if width < 3 {
            return None;
        }
        let mut fence = Self {
            body: opening_end,
            recheck: opening_end,
            marker,
            width,
            info: info.to_owned(),
        };
        fence.continue_source(source).then_some(fence)
    }

    pub(crate) fn continue_source(&mut self, source: &str) -> bool {
        let suffix = &source[self.recheck..];
        if suffix.contains(['\r', '\0']) || suffix.split('\n').any(|line| self.closes(line)) {
            return false;
        }
        self.recheck = source[self.body..]
            .rfind('\n')
            .map_or(self.body, |at| self.body + at + 1);
        true
    }

    fn closes(&self, line: &str) -> bool {
        let indent = line.bytes().take_while(|&b| b == b' ').count();
        if indent > 3 {
            return false;
        }
        let line = &line[indent..];
        let width = line.bytes().take_while(|&b| b == self.marker).count();
        width >= self.width && line[width..].trim_matches([' ', '\t']).is_empty()
    }

    pub(crate) fn render(
        &self,
        source: &str,
        html_only: bool,
        native_only: bool,
        web: bool,
        codes: &mut Vec<super::WebCode>,
    ) -> Arc<super::RenderedBlock> {
        let code = &source[self.body..];
        // pulldown omits an unfinished whitespace-only last line.
        let line = code.rfind('\n').map_or(0, |at| at + 1);
        let code = if code[line..].bytes().all(|b| matches!(b, b' ' | b'\t')) {
            &code[..line]
        } else {
            code
        };
        let code_text: &str = code;
        let code = CowStr::Borrowed(code_text);
        let events = [
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
                self.info.as_str().into(),
            ))),
            Event::Text(code),
            Event::End(TagEnd::CodeBlock),
        ];
        let prepared = crate::math::prepare("");
        let mut html = String::new();
        if web {
            let info = crate::fence_info(&self.info);
            let index = codes.len();
            codes.push(super::WebCode {
                source: code_text.to_owned(),
                info,
            });
            html = format!(
                "<section class=\"mk-codeblock\" data-markit-code=\"{index}\"></section>\n"
            );
        } else if !native_only {
            crate::html::render_events(&prepared, events.iter().cloned(), &mut html);
        }
        let document =
            (!html_only).then(|| crate::document::parse_events(&prepared, events.into_iter()));
        Arc::new(super::RenderedBlock {
            html,
            document,
            codes: std::mem::take(codes),
        })
    }
}
