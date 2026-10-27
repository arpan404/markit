use base64::{engine::general_purpose::STANDARD, Engine};
use latex_rust::{Dim, MathFont, MathStyle, PngOptions, SvgOptions};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cell::RefCell,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MathVisual {
    pub svg: String,
    pub png: String,
    pub width: u32,
    pub height: u32,
}
static FONT: OnceLock<MathFont> = OnceLock::new();
thread_local! {
    static CACHE: RefCell<crate::cache::Cache<(String, bool, MathOutput), Arc<MathVisual>>> =
        RefCell::new(crate::cache::Cache::new(4 * 1024 * 1024));
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum MathOutput {
    Svg,
    Pixels,
    Both,
}

/// Native math is rasterized by Rust, with no WebView or platform TeX engine.
/// Invalid, unsupported, oversized, or excessively deep formulas stay source text.
pub fn render_math(tex: &str, display: bool) -> Option<Arc<MathVisual>> {
    render_visual(tex, display, MathOutput::Both)
}

// HTML embeds SVG paths and never consumes the native bitmap. Keep its cache
// separate so a web render cannot satisfy a later native request without pixels.
pub(crate) fn render_svg(tex: &str, display: bool) -> Option<Arc<MathVisual>> {
    render_visual(tex, display, MathOutput::Svg)
}

pub(crate) fn render_pixels(tex: &str, display: bool) -> Option<Arc<MathVisual>> {
    render_visual(tex, display, MathOutput::Pixels)
}

fn render_visual(tex: &str, display: bool, output: MathOutput) -> Option<Arc<MathVisual>> {
    if tex.len() > 2048 || tex.trim().is_empty() {
        return None;
    }
    let key = (tex.to_owned(), display, output);
    if let Some(cached) = CACHE.with(|cache| cache.borrow_mut().get(&key)) {
        return Some(cached);
    }
    let font = FONT.get_or_init(|| MathFont::stix_two_math().expect("bundled STIX font is valid"));
    let ast = latex_rust::parse(tex).ok()?;
    let tree = latex_rust::layout(
        &ast,
        font,
        if display {
            MathStyle::Display
        } else {
            MathStyle::Text
        },
    )
    .ok()?;
    // Bound pixel area before allocating a bitmap. Dimensions here are em units.
    let bound = Dim::from_i64(100);
    if tree.width.cmp(&bound).is_none_or(|order| order.is_gt())
        || tree.height.cmp(&bound).is_none_or(|order| order.is_gt())
        || tree.depth.cmp(&bound).is_none_or(|order| order.is_gt())
    {
        return None;
    }
    let width_em = tree.width.ceil_to_u32().ok()?.max(1);
    let height_em = tree
        .height
        .ceil_to_u32()
        .ok()?
        .saturating_add(tree.depth.ceil_to_u32().ok()?)
        .max(1);
    if height_em > 48 || width_em.saturating_mul(height_em) > 2048 {
        return None;
    }
    let svg = if output != MathOutput::Pixels {
        let svg_options = SvgOptions {
            font_size_pt: Dim::from_i64(16),
            display,
            ..SvgOptions::new()
        };
        let svg = latex_rust::render_svg(&tree, font, &svg_options).ok()?;
        svg[svg.find("<svg")?..].replace("#000000", "currentColor")
    } else {
        String::new()
    };
    let (png, width, height) = if output != MathOutput::Svg {
        let png_options = PngOptions {
            font_size_pt: Dim::from_i64(16),
            display,
            ..PngOptions::new()
        };
        let png = latex_rust::render_png(&tree, font, &png_options).ok()?;
        let width = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
        let height = u32::from_be_bytes(png.get(20..24)?.try_into().ok()?);
        (STANDARD.encode(png), width, height)
    } else {
        (String::new(), 0, 0)
    };
    let visual = Arc::new(MathVisual {
        svg,
        png,
        width,
        height,
    });
    let weight = key.0.len() + visual.svg.len() + visual.png.len() + 128;
    CACHE.with(|cache| cache.borrow_mut().insert(key, visual.clone(), weight));
    Some(visual)
}

pub(crate) struct Formula {
    pub tex: String,
    pub raw: String,
    pub display: bool,
}
pub(crate) struct Prepared<'a> {
    pub source: Cow<'a, str>,
    prefix: String,
    pub formulas: Vec<Formula>,
    mappings: Vec<(std::ops::Range<usize>, std::ops::Range<usize>)>,
    pub unclosed: Option<usize>,
}

impl Prepared<'_> {
    pub fn is_separator(&self, text: &str) -> bool {
        text.strip_prefix("<!--")
            .and_then(|text| text.strip_suffix("-->"))
            == Some(self.prefix.as_str())
    }
    pub fn original_offset(&self, offset: usize, end: bool) -> usize {
        let before = self
            .mappings
            .partition_point(|(prepared, _)| prepared.end <= offset);
        if let Some((prepared, original)) = self
            .mappings
            .get(before)
            .filter(|(prepared, _)| prepared.start <= offset)
        {
            if offset == prepared.start {
                return original.start;
            }
            return if end { original.end } else { original.start };
        }
        if before == 0 {
            offset
        } else {
            let (prepared, original) = &self.mappings[before - 1];
            offset - prepared.end + original.end
        }
    }
    pub fn formula(&self, code: &str) -> Option<&Formula> {
        let index: usize = code.strip_prefix(&self.prefix)?.parse().ok()?;
        self.formulas.get(index)
    }
}

/// Protect Markdown code first, then shield math from emphasis/escape parsing.
/// The uncommon math path makes a prepass; ordinary prose keeps borrowed input.
pub(crate) fn prepare(source: &str) -> Prepared<'_> {
    let mut prepared = Prepared {
        source: Cow::Borrowed(source),
        prefix: "MARKITMATH".into(),
        formulas: Vec::new(),
        mappings: Vec::new(),
        unclosed: None,
    };
    if !source.contains('$') && !source.contains("\\(") && !source.contains("\\[") {
        return prepared;
    }
    while source.contains(&prepared.prefix) {
        prepared.prefix.push('X');
    }
    let mut protected = Vec::new();
    let parser = crate::parser(source);
    protected.extend(
        parser
            .reference_definitions()
            .iter()
            .map(|(_, definition)| definition.span.clone()),
    );
    protected.extend(
        crate::html::bare_links(source)
            .into_iter()
            .map(|(range, _)| range),
    );
    let mut fence_start = None;
    for (event, range) in parser.into_offset_iter() {
        match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Image { .. }) => {
                protected.push(range)
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { link_type, .. }) => {
                if link_type == pulldown_cmark::LinkType::Inline {
                    if let Some(destination) = source[range.clone()].rfind("](") {
                        protected.push(range.start + destination + 1..range.end);
                    }
                } else if matches!(
                    link_type,
                    pulldown_cmark::LinkType::Autolink | pulldown_cmark::LinkType::Email
                ) {
                    protected.push(range);
                }
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::CodeBlock(_)) => {
                fence_start = Some(range.start)
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::CodeBlock) => {
                if let Some(start) = fence_start.take() {
                    protected.push(start..range.end);
                }
            }
            pulldown_cmark::Event::Code(_)
            | pulldown_cmark::Event::Html(_)
            | pulldown_cmark::Event::InlineHtml(_)
                if fence_start.is_none() =>
            {
                protected.push(range)
            }
            _ => {}
        }
    }
    protected.sort_by_key(|range| range.start);
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    let mut protected_index = 0;
    let mut missing = std::collections::HashSet::new();
    while index < bytes.len() {
        while protected
            .get(protected_index)
            .is_some_and(|range| range.end <= index)
        {
            protected_index += 1;
        }
        if let Some(range) = protected
            .get(protected_index)
            .filter(|range| range.start <= index)
        {
            output.push_str(&source[index..range.end]);
            index = range.end;
            continue;
        }
        let next_protected = protected
            .get(protected_index)
            .map_or(bytes.len(), |range| range.start);
        let tail = &source[index..next_protected];
        let opening = if tail.starts_with("\\(") {
            Some(("\\)", 2, false))
        } else if tail.starts_with("\\[") {
            Some(("\\]", 2, true))
        } else if tail.starts_with("$$") {
            Some(("$$", 2, true))
        } else if tail.starts_with('$')
            && tail[1..]
                .chars()
                .next()
                .is_some_and(|c| !c.is_whitespace() && c != '$')
        {
            Some(("$", 1, false))
        } else {
            None
        };
        if let Some((closing, open_len, display)) = opening {
            let tail = if display {
                tail
            } else {
                tail.split("\n\n").next().unwrap_or(tail)
            };
            let key = (closing, index + tail.len());
            let mut found = None;
            let mut search = open_len;
            while !missing.contains(&key) {
                let Some(offset) = tail[search..].find(closing) else {
                    missing.insert(key);
                    break;
                };
                let end = search + offset;
                let escaped = tail[..end]
                    .bytes()
                    .rev()
                    .take_while(|&b| b == b'\\')
                    .count()
                    % 2
                    == 1;
                let content = &tail[open_len..end];
                let after = tail[end + closing.len()..].chars().next();
                let valid = !escaped
                    && !content.trim().is_empty()
                    && (open_len != 1
                        || (!content.ends_with(char::is_whitespace)
                            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
                            && !content.contains("\\$")));
                if valid {
                    found = Some(end);
                    break;
                }
                if open_len == 1 {
                    break;
                }
                search = end + closing.len();
            }
            if let Some(end) = found {
                let finish = end + closing.len();
                let number = prepared.formulas.len();
                prepared.formulas.push(Formula {
                    tex: tail[open_len..end].into(),
                    raw: tail[..finish].into(),
                    display,
                });
                let prepared_start = output.len();
                output.push_str(&format!(
                    "`{}{number}`<!--{}-->",
                    prepared.prefix, prepared.prefix
                ));
                prepared
                    .mappings
                    .push((prepared_start..output.len(), index..index + finish));
                index += finish;
                continue;
            } else if tail.starts_with("\\(") || tail.starts_with("\\[") || tail.starts_with("$$") {
                // Preserve an unfinished opener during streaming instead of losing '\'.
                let number = prepared.formulas.len();
                prepared.formulas.push(Formula {
                    tex: String::new(),
                    raw: tail[..2].into(),
                    display,
                });
                if display {
                    prepared.unclosed.get_or_insert(index);
                }
                let prepared_start = output.len();
                output.push_str(&format!(
                    "`{}{number}`<!--{}-->",
                    prepared.prefix, prepared.prefix
                ));
                prepared
                    .mappings
                    .push((prepared_start..output.len(), index..index + 2));
                index += 2;
                continue;
            }
        }
        // An escaped dollar or backslash cannot open a formula on the next step.
        if bytes[index] == b'\\'
            && index + 1 < next_protected
            && matches!(bytes[index + 1], b'\\' | b'$')
        {
            output.push_str(&source[index..index + 2]);
            index += 2;
        } else {
            let c = source[index..]
                .chars()
                .next()
                .expect("index is a character boundary");
            output.push(c);
            index += c.len_utf8();
        }
    }
    if !prepared.formulas.is_empty() {
        prepared.source = Cow::Owned(output);
    }
    prepared
}

#[cfg(test)]
mod tests {
    #[test]
    fn svg_math_skips_pixels_without_changing_paths_or_native_cache_entries() {
        for (tex, display) in [(r"\frac{x^2}{2}+1", false), (r"\sum_{i=1}^{n} i", true)] {
            let vector = super::render_svg(tex, display).unwrap();
            assert!(vector.png.is_empty());
            let native = super::render_math(tex, display).unwrap();
            assert!(!native.png.is_empty());
            assert!(native.width > 0 && native.height > 0);
            assert_eq!(vector.svg, native.svg);
            let pixels = super::render_pixels(tex, display).unwrap();
            assert!(pixels.svg.is_empty());
            assert_eq!(pixels.png, native.png);
            assert_eq!((pixels.width, pixels.height), (native.width, native.height));
            assert!(std::sync::Arc::ptr_eq(
                &vector,
                &super::render_svg(tex, display).unwrap()
            ));
        }
    }
}
