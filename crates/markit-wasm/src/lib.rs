use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = renderHtml)]
pub fn render_html(source: &str) -> String {
    markit::render_html(source)
}

#[wasm_bindgen(js_name = renderDocument)]
pub fn render_document(source: &str) -> String {
    markit::render_document_json(source)
}

fn flavor(name: &str) -> Result<markit::Flavor, JsValue> {
    markit::Flavor::from_name(name).ok_or_else(|| JsValue::from_str("Unknown flavor"))
}

/// `renderHtml` in a flavour: `"default"` or `"github"`.
#[wasm_bindgen(js_name = renderHtmlFlavor)]
pub fn render_html_flavor(source: &str, name: &str) -> Result<String, JsValue> {
    Ok(markit::render_html_with(source, flavor(name)?))
}

/// `renderDocument` in a flavour: `"default"` or `"github"`.
#[wasm_bindgen(js_name = renderDocumentFlavor)]
pub fn render_document_flavor(source: &str, name: &str) -> Result<String, JsValue> {
    Ok(markit::render_document_json_with(source, flavor(name)?))
}

#[wasm_bindgen(js_name = highlightCode)]
pub fn highlight_code(source: &str, language: &str) -> String {
    markit::highlight_json(source, language)
}

/// Typed UTF-16 ranges avoid JSON and repeated token text across the web bridge.
#[wasm_bindgen(js_name = highlightCodePacked)]
pub fn highlight_code_packed(source: &str, language: &str) -> Vec<u32> {
    markit::highlight_packed(source, language)
}

/// Diff fence rows: `[additions, deletions]`, then `kind, old + 1, new + 1` per line.
#[wasm_bindgen(js_name = diffRows)]
pub fn diff_rows(source: &str) -> Vec<u32> {
    markit::diff_rows_packed(source)
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
    #[wasm_bindgen(js_name = web)]
    pub fn web() -> Self {
        Self {
            inner: markit::StreamRenderer::new_web(),
        }
    }
    /// A web stream in a flavour: `"default"` or `"github"`.
    #[wasm_bindgen(js_name = webFlavor)]
    pub fn web_flavor(name: &str) -> Result<MarkdownStream, JsValue> {
        Ok(Self {
            inner: markit::StreamRenderer::new_web().with_flavor(flavor(name)?),
        })
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
    /// Binary patch for the web worker. See `encode_web_patch`.
    #[wasm_bindgen(js_name = appendPacked)]
    pub fn append_packed(&mut self, delta: &str, finalized: bool) -> Result<Vec<u8>, JsValue> {
        if self.inner.source().len() + delta.len() > 8 * 1024 * 1024 {
            return Err(JsValue::from_str("Document exceeds 8 MiB"));
        }
        Ok(markit::encode_web_patch(
            &self.inner.append(delta, finalized),
        ))
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
