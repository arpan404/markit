//! Fast continuation only for unindented, top-level open fences. Markdown
//! containers, normalized CR/NUL input and closing fences use the parser.
use std::sync::Arc;

pub(crate) struct OpenFence {
    body: usize,
    recheck: usize,
    marker: u8,
    width: usize,
    info: String,
    html: Option<crate::html::CodeHtml>,
    doc: Option<crate::fence_doc::FenceDoc>,
    body_utf16: usize,
    /// Source end of the code last rendered, initially the code the parser
    /// rendered when the fence was found.
    rendered_end: usize,
    /// No fast-path render yet: the last block came from the parser.
    pub(crate) fresh: bool,
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
            html: None,
            doc: None,
            body_utf16: crate::highlight::utf16_len(&source[..opening_end]),
            rendered_end: opening_end,
            fresh: true,
        };
        fence.rendered_end = opening_end + fence.code(source).len();
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

    /// Fence body as pulldown reports it. It only grows while the fence is open.
    fn code<'a>(&self, source: &'a str) -> &'a str {
        let code = &source[self.body..];
        // pulldown omits an unfinished whitespace-only last line.
        let line = code.rfind('\n').map_or(0, |at| at + 1);
        if code[line..].bytes().all(|b| matches!(b, b' ' | b'\t')) {
            &code[..line]
        } else {
            code
        }
    }

    /// Native-only update that reports what changed instead of building the
    /// whole document. [`document`](Self::document) rebuilds it on demand.
    pub(crate) fn rendered_end(&self) -> usize {
        self.rendered_end
    }

    pub(crate) fn native_change(&mut self, source: &str) -> crate::fence_doc::FenceChange {
        self.fresh = false;
        let code = self.code(source);
        self.rendered_end = self.body + code.len();
        let info = crate::fence_info(&self.info);
        let runs = crate::highlight::lex_runs(code, &info.language);
        self.doc
            .get_or_insert_with(|| crate::fence_doc::FenceDoc::new(&info))
            .change(code, &info, &runs)
    }

    /// The native document of the last [`native_change`](Self::native_change).
    pub(crate) fn document(&self, source: &str) -> crate::Document {
        let info = crate::fence_info(&self.info);
        let code = &source[self.body..self.rendered_end];
        match self.doc.as_ref() {
            Some(doc) => doc.current(code, &info),
            None => crate::fence_doc::FenceDoc::new(&info).render(
                code,
                &info,
                &crate::highlight::lex_runs(code, &info.language),
            ),
        }
    }

    pub(crate) fn render(
        &mut self,
        source: &str,
        html_only: bool,
        native_only: bool,
        web: bool,
        codes: &mut Vec<super::WebCode>,
    ) -> Arc<super::RenderedBlock> {
        self.fresh = false;
        let code_text = self.code(source);
        self.rendered_end = self.body + code_text.len();
        let info = crate::fence_info(&self.info);
        let runs = (!web).then(|| crate::highlight::lex_runs(code_text, &info.language));
        let mut html = String::new();
        if web {
            let index = codes.len();
            let end_utf16 = self.body_utf16 + crate::highlight::utf16_len(code_text);
            codes.push(super::WebCode {
                source: code_text.to_owned(),
                info: info.clone(),
                utf16: Some((self.body_utf16 as u32, end_utf16 as u32)),
            });
            html = format!(
                "<section class=\"mk-codeblock\" data-markit-code=\"{index}\"></section>\n"
            );
        }
        let document = if let Some(runs) = runs.as_ref() {
            if !native_only {
                html = self
                    .html
                    .get_or_insert_with(|| crate::html::CodeHtml::new(&info))
                    .render(code_text, &info, runs);
            }
            (!html_only).then(|| {
                self.doc
                    .get_or_insert_with(|| crate::fence_doc::FenceDoc::new(&info))
                    .render(code_text, &info, runs)
            })
        } else {
            None
        };
        Arc::new(super::RenderedBlock {
            html,
            document,
            codes: std::mem::take(codes),
        })
    }
}
