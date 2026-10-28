//! Compact web-worker patch. Prose HTML is still text. Top-level fence bodies
//! are UTF-16 offsets into the Markdown the host already has, plus packed
//! highlight words, so the worker does not JSON-encode or re-scan the source.
use crate::{RenderPatch, WebCode};

const MAGIC: &[u8; 4] = b"MKWP";

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_u32(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}

/// Version 1 little-endian patch. `flags` bit 0 embeds source bytes, bit 1 is
/// a diff fence, bit 2 embeds a file name. Packed words match `highlight_packed`.
pub fn encode_web_patch(patch: &RenderPatch) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(MAGIC);
    put_u32(&mut out, 1);
    put_u32(&mut out, patch.replace_from as u32);
    put_u32(&mut out, u32::from(patch.finalized));
    put_u32(&mut out, patch.source_bytes as u32);
    put_u32(&mut out, patch.parsed_bytes as u32);
    put_u32(&mut out, patch.blocks.len() as u32);
    for block in &patch.blocks {
        put_bytes(&mut out, block.html.as_bytes());
        put_u32(&mut out, block.codes.len() as u32);
        for code in &block.codes {
            encode_code(&mut out, code);
        }
    }
    out
}

fn encode_code(out: &mut Vec<u8>, code: &WebCode) {
    let embed = code.utf16.is_none();
    let mut flags = 0u32;
    if embed {
        flags |= 1;
    }
    if code.info.diff {
        flags |= 2;
    }
    if code.info.file.is_some() {
        flags |= 4;
    }
    put_u32(out, flags);
    let (start, end) = code.utf16.unwrap_or((0, 0));
    put_u32(out, start);
    put_u32(out, end);
    put_bytes(out, code.info.language.as_bytes());
    if let Some(file) = &code.info.file {
        put_bytes(out, file.as_bytes());
    }
    if embed {
        put_bytes(out, code.source.as_bytes());
    }
    let packed = crate::highlight_packed(&code.source, &code.info.language);
    put_u32(out, packed.len() as u32);
    for word in packed {
        put_u32(out, word);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StreamRenderer;

    fn take_u32(bytes: &[u8], at: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
        *at += 4;
        value
    }
    fn take_bytes<'a>(bytes: &'a [u8], at: &mut usize) -> &'a [u8] {
        let len = take_u32(bytes, at) as usize;
        let slice = &bytes[*at..*at + len];
        *at += len;
        slice
    }

    #[test]
    fn top_level_fence_patch_uses_utf16_offsets_and_packed_ranges() {
        let markdown = "```rust\nfn café() {\n    let s = \"😀\";\n}\n";
        let mut stream = StreamRenderer::new_web();
        let patch = stream.append(markdown, false);
        let code = &patch.blocks[0].codes[0];
        let (start, end) = code.utf16.expect("top-level body is a slice");
        let units: Vec<u16> = markdown.encode_utf16().collect();
        let body = String::from_utf16(&units[start as usize..end as usize]).unwrap();
        assert_eq!(body, code.source);
        let encoded = encode_web_patch(&patch);
        assert_eq!(&encoded[..4], b"MKWP");
        let mut at = 4;
        assert_eq!(take_u32(&encoded, &mut at), 1);
        assert_eq!(take_u32(&encoded, &mut at), 0);
        let _finalized = take_u32(&encoded, &mut at);
        let _source_bytes = take_u32(&encoded, &mut at);
        let _parsed = take_u32(&encoded, &mut at);
        assert_eq!(take_u32(&encoded, &mut at), 1);
        let html = take_bytes(&encoded, &mut at);
        assert!(std::str::from_utf8(html).unwrap().contains("mk-codeblock"));
        assert_eq!(take_u32(&encoded, &mut at), 1);
        let flags = take_u32(&encoded, &mut at);
        assert_eq!(flags & 1, 0, "source bytes stay on the host");
        assert_eq!(take_u32(&encoded, &mut at), start);
        assert_eq!(take_u32(&encoded, &mut at), end);
        assert_eq!(take_bytes(&encoded, &mut at), b"rust");
        let words = take_u32(&encoded, &mut at) as usize;
        let mut packed = Vec::with_capacity(words);
        for _ in 0..words {
            packed.push(take_u32(&encoded, &mut at));
        }
        assert_eq!(packed, crate::highlight_packed(&code.source, "rust"));
        assert_eq!(at, encoded.len());
    }

    #[test]
    fn nested_fence_patch_embeds_rewritten_source() {
        let markdown = "> ```ts title=\"nested.ts\"\n> const x = 1;\n> const y = 2;\n> ```\n";
        let mut stream = StreamRenderer::new_web();
        let patch = stream.append(markdown, true);
        let code = patch
            .blocks
            .iter()
            .flat_map(|block| block.codes.iter())
            .next()
            .expect("nested fence");
        assert!(code.utf16.is_none());
        let encoded = encode_web_patch(&patch);
        assert!(encoded
            .windows(code.source.len())
            .any(|window| window == code.source.as_bytes()));
    }
}
