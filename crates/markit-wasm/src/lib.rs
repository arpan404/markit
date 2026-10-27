use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = renderHtml)]
pub fn render_html(source: &str) -> String {
    markit::render_html(source)
}

#[wasm_bindgen(js_name = renderDocument)]
pub fn render_document(source: &str) -> String {
    markit::render_document_json(source)
}

#[wasm_bindgen(js_name = highlightCode)]
pub fn highlight_code(source: &str, language: &str) -> String {
    serde_json::to_string(markit::highlight(source, language).as_ref()).expect("serializable code")
}

/// Typed UTF-16 ranges avoid JSON and repeated token text across the web bridge.
#[wasm_bindgen(js_name = highlightCodePacked)]
pub fn highlight_code_packed(source: &str, language: &str) -> Vec<u32> {
    markit::highlight_packed(source, language)
}

#[wasm_bindgen]
pub struct MarkdownStream {
    inner: markit::StreamRenderer,
}

#[wasm_bindgen]
impl MarkdownStream {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: markit::StreamRenderer::new_html(),
        }
    }
    pub fn reset(&mut self) {
        self.inner.reset();
    }
    pub fn append(&mut self, delta: &str, finalized: bool) -> Result<String, JsValue> {
        if self.inner.source().len() + delta.len() > 8 * 1024 * 1024 {
            return Err(JsValue::from_str("Document exceeds 8 MiB"));
        }
        Ok(
            serde_json::to_string(&self.inner.append(delta, finalized))
                .expect("serializable patch"),
        )
    }
}

impl Default for MarkdownStream {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen(js_name = highlightGrammar)]
pub fn highlight_grammar(source: &str, language: &str) -> Result<String, JsValue> {
    let code = markit::highlight_grammar(source, language)
        .ok_or_else(|| JsValue::from_str("Build with the grammars feature"))?;
    Ok(serde_json::to_string(code.as_ref()).expect("serializable code"))
}
