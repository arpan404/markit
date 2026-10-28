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
fn rgb(s: &str) -> Option<u32> {
    // Only canonical lowercase RGB can be reconstructed field-for-field.
    if s.len() != 7 || !s.starts_with('#') || s.bytes().any(|b| b.is_ascii_uppercase()) {
        return None;
    }
    u32::from_str_radix(&s[1..], 16).ok()
}
