//! Version 1 native binary transport. See docs/binary-transport.md.
use crate::{Document, Style};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryError {
    TooLarge,
    InvalidMath,
}
fn word(n: usize) -> Result<u32, BinaryError> {
    u32::try_from(n).map_err(|_| BinaryError::TooLarge)
}
fn put(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
#[derive(Default)]
struct Table<'a> {
    strings: HashMap<&'a str, u32>,
    blobs: HashMap<Vec<u8>, u32>,
    entries: Vec<(u32, u32)>,
    bytes: Vec<u8>,
}
impl<'a> Table<'a> {
    fn add(&mut self, bytes: &[u8]) -> Result<u32, BinaryError> {
        let id = word(self.entries.len())?;
        let offset = word(self.bytes.len())?;
        let len = word(bytes.len())?;
        self.entries.push((offset, len));
        self.bytes.extend_from_slice(bytes);
        word(self.bytes.len())?;
        Ok(id)
    }
    fn string(&mut self, s: &'a str) -> Result<u32, BinaryError> {
        if let Some(id) = self.strings.get(s) {
            return Ok(*id);
        }
        let id = self.add(s.as_bytes())?;
        self.strings.insert(s, id);
        Ok(id)
    }
    fn png(&mut self, s: &str) -> Result<u32, BinaryError> {
        let bytes = STANDARD.decode(s).map_err(|_| BinaryError::InvalidMath)?;
        if let Some(id) = self.blobs.get(&bytes) {
            return Ok(*id);
        }
        let id = self.add(&bytes)?;
        self.blobs.insert(bytes, id);
        Ok(id)
    }
}

pub fn render_document_binary(source: &str) -> Result<Vec<u8>, BinaryError> {
    encode_document_binary(&crate::parse(source))
}

/// The JSON APIs remain unchanged. All native callers share this encoder.
pub fn encode_document_binary(doc: &Document) -> Result<Vec<u8>, BinaryError> {
    let mut table = Table::default();
    let mut out = Vec::with_capacity(32 + doc.text.len() + doc.spans.len() * 40);
    out.extend_from_slice(b"MKBP");
    for n in [
        1,
        doc.version,
        word(doc.text.len())?,
        word(doc.spans.len())?,
        word(doc.blocks.len())?,
        0,
        0,
    ] {
        put(&mut out, n);
    }
    out.extend_from_slice(doc.text.as_bytes());
    for span in &doc.spans {
        let mut args = [0u32; 7];
        let tag = match &span.style {
            Style::Heading { level } => {
                args[0] = (*level).into();
                1
            }
            Style::Paragraph => 2,
            Style::Strong => 3,
            Style::Emphasis => 4,
            Style::Strikethrough => 5,
            Style::Code => 6,
            Style::CodeBlock { language } => {
                args[0] = table.string(language)?;
                7
            }
            Style::Link { url } => {
                args[0] = table.string(url)?;
                8
            }
            Style::Image { url } => {
                args[0] = table.string(url)?;
                9
            }
            Style::Quote { depth } => {
                args[0] = word(*depth)?;
                10
            }
            Style::ListItem { depth } => {
                args[0] = word(*depth)?;
                11
            }
            Style::Table => 12,
            Style::TableHeader => 13,
            Style::Rule => 14,
            Style::Reference {
                reference_kind,
                target,
            } => {
                args[0] = table.string(reference_kind)?;
                args[1] = table.string(target)?;
                15
            }
            Style::CodeMetadata { file, diff } => {
                args[0] = match file {
                    Some(file) => table.string(file)?,
                    None => u32::MAX,
                };
                args[1] = u32::from(*diff);
                16
            }
            Style::Syntax { light, dark } => {
                if let Some(id) = crate::highlight::PALETTE
                    .iter()
                    .position(|p| p.0 == light && p.1 == dark)
                {
                    args[0] = id as u32;
                    17
                } else if let (Some(l), Some(d)) = (rgb(light), rgb(dark)) {
                    args[0] = l;
                    args[1] = d;
                    21
                } else {
                    args[0] = table.string(light)?;
                    args[1] = table.string(dark)?;
                    22
                }
            }
            Style::Diff { change } => {
                args[0] = table.string(change)?;
                18
            }
            Style::Math {
                tex,
                raw,
                display,
                png,
                width,
                height,
            } => {
                args = [
                    table.string(tex)?,
                    table.string(raw)?,
                    u32::from(*display),
                    table.png(png)?,
                    *width,
                    *height,
                    0,
                ];
                19
            }
            Style::TableCell {
                row,
                column,
                header,
                alignment,
            } => {
                args[0] = word(*row)?;
                args[1] = word(*column)?;
                args[2] = u32::from(*header);
                args[3] = table.string(alignment)?;
                20
            }
        };
        for n in [word(span.start)?, word(span.end)?, tag]
            .into_iter()
            .chain(args)
        {
            put(&mut out, n);
        }
    }
    for block in &doc.blocks {
        for n in [
            word(block.start)?,
            word(block.end)?,
            table.string(&block.kind)?,
        ] {
            put(&mut out, n);
        }
    }
    out[24..28].copy_from_slice(&word(table.entries.len())?.to_le_bytes());
    out[28..32].copy_from_slice(&word(table.bytes.len())?.to_le_bytes());
    for (offset, len) in table.entries {
        put(&mut out, offset);
        put(&mut out, len);
    }
    out.extend_from_slice(&table.bytes);
    word(out.len())?;
    Ok(out)
}
/// Patch wrapper around [`encode_document_binary`]. Magic `MKNP`, version 1.
/// Each block is a length-prefixed MKBP document. Native streams only; a block
/// without a document is an error.
pub fn encode_native_patch(patch: &crate::RenderPatch) -> Result<Vec<u8>, BinaryError> {
    let mut out = Vec::new();
    out.extend_from_slice(b"MKNP");
    put(&mut out, 1);
    put(&mut out, word(patch.replace_from)?);
    put(&mut out, u32::from(patch.finalized));
    put(&mut out, word(patch.source_bytes)?);
    put(&mut out, word(patch.parsed_bytes)?);
    put(&mut out, word(patch.blocks.len())?);
    for block in &patch.blocks {
        let Some(document) = block.document.as_ref() else {
            return Err(BinaryError::TooLarge);
        };
        let bytes = encode_document_binary(document)?;
        put(&mut out, word(bytes.len())?);
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

/// MKNP version 2 header: version, replace_from, finalized, source_bytes,
/// parsed_bytes, block count. Each block then starts with a kind word.
fn patch_v2_header(
    out: &mut Vec<u8>,
    replace_from: usize,
    finalized: bool,
    source_bytes: usize,
    parsed_bytes: usize,
    blocks: usize,
) -> Result<(), BinaryError> {
    out.extend_from_slice(b"MKNP");
    put(out, 2);
    put(out, word(replace_from)?);
    put(out, u32::from(finalized));
    put(out, word(source_bytes)?);
    put(out, word(parsed_bytes)?);
    put(out, word(blocks)?);
    Ok(())
}

fn put_document(out: &mut Vec<u8>, document: &Document) -> Result<(), BinaryError> {
    let bytes = encode_document_binary(document)?;
    put(out, word(bytes.len())?);
    out.extend_from_slice(&bytes);
    Ok(())
}

/// Version 2 of [`encode_native_patch`] with whole documents only (kind 0).
pub(crate) fn encode_native_patch_v2(patch: &crate::RenderPatch) -> Result<Vec<u8>, BinaryError> {
    let mut out = Vec::new();
    patch_v2_header(
        &mut out,
        patch.replace_from,
        patch.finalized,
        patch.source_bytes,
        patch.parsed_bytes,
        patch.blocks.len(),
    )?;
    for block in &patch.blocks {
        let Some(document) = block.document.as_ref() else {
            return Err(BinaryError::TooLarge);
        };
        put(&mut out, 0);
        put_document(&mut out, document)?;
    }
    Ok(out)
}

/// One open-fence update. An extension (kind 1) carries the kept UTF-16
/// prefix of the host's text, the head span count, the kept range of the
/// host's spans, and an MKBP document holding the appended text, the new
/// head and tail spans, and the blocks.
pub(crate) fn encode_fence_patch(
    index: usize,
    change: &crate::document::FenceChange,
    finalized: bool,
    source_bytes: usize,
    parsed_bytes: usize,
) -> Result<Vec<u8>, BinaryError> {
    use crate::document::FenceChange;
    let mut out = Vec::new();
    match change {
        FenceChange::Unchanged => {
            patch_v2_header(
                &mut out,
                index + 1,
                finalized,
                source_bytes,
                parsed_bytes,
                0,
            )?;
        }
        FenceChange::Replace(document) => {
            patch_v2_header(&mut out, index, finalized, source_bytes, parsed_bytes, 1)?;
            put(&mut out, 0);
            put_document(&mut out, document)?;
        }
        FenceChange::Extend(extension) => {
            patch_v2_header(&mut out, index, finalized, source_bytes, parsed_bytes, 1)?;
            put(&mut out, 1);
            for n in [
                extension.keep_text_utf16,
                extension.head,
                extension.keep_from,
                extension.keep_to,
            ] {
                put(&mut out, word(n)?);
            }
            put_document(&mut out, &extension.document)?;
        }
    }
    Ok(out)
}

fn rgb(s: &str) -> Option<u32> {
    // Only canonical lowercase RGB can be reconstructed field-for-field.
    if s.len() != 7 || !s.starts_with('#') || s.bytes().any(|b| b.is_ascii_uppercase()) {
        return None;
    }
    u32::from_str_radix(&s[1..], 16).ok()
}

#[cfg(test)]
mod patch_tests {
    #[test]
    fn native_patch_embeds_each_document_unchanged() {
        let mut stream = crate::StreamRenderer::new_native();
        let patch = stream.append("# Title\n\n```rust\nlet x = 1;\n", false);
        let bytes = crate::encode_native_patch(&patch).unwrap();
        assert_eq!(&bytes[..4], b"MKNP");
        let mut at = 4usize;
        let word = |bytes: &[u8], at: &mut usize| {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&bytes, &mut at), 1);
        assert_eq!(word(&bytes, &mut at) as usize, patch.replace_from);
        let _finalized = word(&bytes, &mut at);
        let _source = word(&bytes, &mut at);
        let _parsed = word(&bytes, &mut at);
        assert_eq!(word(&bytes, &mut at) as usize, patch.blocks.len());
        for block in &patch.blocks {
            let len = word(&bytes, &mut at) as usize;
            let embedded = &bytes[at..at + len];
            at += len;
            let direct = crate::encode_document_binary(block.document.as_ref().unwrap()).unwrap();
            assert_eq!(embedded, direct);
        }
        assert_eq!(at, bytes.len());
    }
}
