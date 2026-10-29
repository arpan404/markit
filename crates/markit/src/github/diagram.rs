//! Diagram fences (```` ```mermaid ````). Markit never draws them: it marks a
//! closed fence as a diagram with its source, keeps the code block inside as
//! the fallback, and the host swaps a drawing in (web `renderDiagram`, native
//! `diagram` spans).

/// Languages drawn as diagrams.
pub(crate) fn is_diagram(language: &str) -> bool {
    language == "mermaid"
}

/// The wrapper opened before the fallback code block.
pub(crate) fn open_html(language: &str, source: &str, out: &mut String) {
    out.push_str("<div class=\"mk-diagram\" data-markit-diagram=\"");
    crate::html::escape_into(language, out);
    out.push_str("\" data-markit-diagram-source=\"");
    crate::html::escape_into(source, out);
    out.push_str("\">\n");
}

pub(crate) const CLOSE_HTML: &str = "</div>\n";
