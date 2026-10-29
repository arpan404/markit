//! Merges a glyph chunk into the web build's core math font.
//!
//! `scripts/math-font-web.py` writes the core face with its CFF table last in
//! the file and the CharStrings INDEX last in that table, and every Top DICT
//! offset in a fixed five-byte form. A chunk (`math-glyphs-N.bin`) holds the
//! full outlines of one group of glyphs that the core face leaves out:
//!
//! ```text
//! "MKGC" u32 version (1) u32 count, then count × (u16 glyph id, u32 length, charstring)
//! ```
//!
//! Merging replaces those glyphs' charstrings and rewrites only the trailing
//! INDEX, so every other table and offset stays put.

fn u16_at(data: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_be_bytes(
        data.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u32_at(data: &[u8], at: usize) -> Option<usize> {
    usize::try_from(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?)).ok()
}

fn uint(data: &[u8], at: usize, size: usize) -> Option<usize> {
    data.get(at..at + size)?
        .iter()
        .try_fold(0usize, |value, byte| {
            value.checked_mul(256).map(|v| v | usize::from(*byte))
        })
}

/// Items of the CFF INDEX at `at`, and the offset just past it.
fn index(data: &[u8], at: usize) -> Option<(Vec<&[u8]>, usize)> {
    let count = u16_at(data, at)?;
    if count == 0 {
        return Some((Vec::new(), at + 2));
    }
    let size = usize::from(*data.get(at + 2)?);
    if !(1..=4).contains(&size) {
        return None;
    }
    let base = at + 3 + (count + 1) * size - 1;
    let mut items = Vec::with_capacity(count);
    let mut start = uint(data, at + 3, size)?;
    for i in 1..=count {
        let end = uint(data, at + 3 + i * size, size)?;
        items.push(data.get(base + start..base + end)?);
        start = end;
    }
    Some((items, base + start))
}

/// The CharStrings offset in a Top DICT written as fixed five-byte integers.
fn charstrings_offset(dict: &[u8]) -> Option<usize> {
    let mut operand = None;
    let mut at = 0;
    while at < dict.len() {
        match dict[at] {
            29 => {
                operand = Some(u32_at(dict, at + 1)?);
                at += 5;
            }
            17 => return operand,
            12 => {
                operand = None;
                at += 2;
            }
            0..=21 => {
                operand = None;
                at += 1;
            }
            _ => return None,
        }
    }
    None
}

/// Glyph ids and charstrings in a chunk.
pub fn chunk_glyphs(chunk: &[u8]) -> Option<Vec<(usize, &[u8])>> {
    if chunk.get(..4)? != b"MKGC" || u32_at(chunk, 4)? != 1 {
        return None;
    }
    let count = u32_at(chunk, 8)?;
    let mut at = 12;
    let mut glyphs = Vec::with_capacity(count.min(chunk.len() / 6));
    for _ in 0..count {
        let gid = u16_at(chunk, at)?;
        let len = u32_at(chunk, at + 2)?;
        glyphs.push((gid, chunk.get(at + 6..at + 6 + len)?));
        at += 6 + len;
    }
    (at == chunk.len()).then_some(glyphs)
}

/// The core font `otf` with the outlines in `chunk`, or `None` if either is
/// not in the layout described above.
pub fn merge(otf: &[u8], chunk: &[u8]) -> Option<Vec<u8>> {
    let glyphs = chunk_glyphs(chunk)?;
    let tables = u16_at(otf, 4)?;
    let entry = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&at| otf.get(at..at + 4) == Some(b"CFF "))?;
    let cff_at = u32_at(otf, entry + 8)?;
    let cff = otf.get(cff_at..cff_at + u32_at(otf, entry + 12)?)?;
    if otf.len() - (cff_at + cff.len()) > 3 {
        return None; // CFF must be the last table.
    }
    let (_, after_names) = index(cff, usize::from(*cff.get(2)?))?;
    let (top, _) = index(cff, after_names)?;
    let at = charstrings_offset(top.first()?)?;
    let (mut strings, end) = index(cff, at)?;
    if end != cff.len() {
        return None;
    }
    for (gid, code) in glyphs {
        *strings.get_mut(gid)? = code;
    }
    let total = 1 + strings.iter().map(|s| s.len()).sum::<usize>();
    let size = (1..=4).find(|n| total < 1 << (8 * n))?;
    let mut out = Vec::with_capacity(cff_at + at + total + (strings.len() + 1) * size + 6);
    out.extend_from_slice(&otf[..cff_at + at]);
    out.extend_from_slice(&u16::try_from(strings.len()).ok()?.to_be_bytes());
    out.push(size as u8);
    let mut offset = 1usize;
    out.extend_from_slice(&offset.to_be_bytes()[std::mem::size_of::<usize>() - size..]);
    for item in &strings {
        offset += item.len();
        out.extend_from_slice(&offset.to_be_bytes()[std::mem::size_of::<usize>() - size..]);
    }
    for item in &strings {
        out.extend_from_slice(item);
    }
    let length = u32::try_from(out.len() - cff_at).ok()?;
    out[entry + 12..entry + 16].copy_from_slice(&length.to_be_bytes());
    out.resize(out.len().next_multiple_of(4), 0);
    Some(out)
}
