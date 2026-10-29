//! GitHub alerts: a blockquote opening with `[!NOTE]`, `[!TIP]`,
//! `[!IMPORTANT]`, `[!WARNING]` or `[!CAUTION]`. pulldown-cmark recognises
//! the marker with `ENABLE_GFM`; this names each kind and its title.
use pulldown_cmark::BlockQuoteKind;

/// The kind's class suffix (`note`) and its visible title (`Note`).
pub(crate) fn alert(kind: BlockQuoteKind) -> (&'static str, &'static str) {
    match kind {
        BlockQuoteKind::Note => ("note", "Note"),
        BlockQuoteKind::Tip => ("tip", "Tip"),
        BlockQuoteKind::Important => ("important", "Important"),
        BlockQuoteKind::Warning => ("warning", "Warning"),
        BlockQuoteKind::Caution => ("caution", "Caution"),
    }
}
