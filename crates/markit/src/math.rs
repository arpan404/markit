use base64::{engine::general_purpose::STANDARD, Engine};
use latex_rust::{BoxContent, Dim, MathBox, MathFont, MathStyle, SvgOptions};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cell::{OnceCell, RefCell},
    rc::Rc,
    sync::{Arc, OnceLock},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MathVisual {
    pub svg: String,
    pub png: String,
    pub width: u32,
    pub height: u32,
}
/// Native builds embed latex-rust's full STIX Two Math face.
#[cfg(not(target_arch = "wasm32"))]
fn font() -> Option<&'static MathFont> {
    static FONT: OnceLock<MathFont> = OnceLock::new();
    Some(FONT.get_or_init(|| MathFont::stix_two_math().expect("bundled STIX font is valid")))
}

/// Wasm embeds a core cut of the same face instead of the 839 KB original; see
/// `scripts/math-font-core.py`. All of its metrics equal the full face, so every
/// layout is identical, but only the glyphs in `math-core-glyphs.txt` keep their
/// outlines. A formula that would draw another glyph is not drawn: it sets
/// [`web_font::take_missing`] so the host can load the full face with
/// [`web_font::install_full`] and render again.
#[cfg(target_arch = "wasm32")]
pub mod web_font {
    use super::*;
    use std::cell::Cell;

    static CORE_OTF: &[u8] = include_bytes!("../assets/math-core.otf");
    /// One bit per glyph id: set when the core face keeps that glyph's outline.
    static CORE_GLYPHS: &[u8] = include_bytes!("../assets/math-core-glyphs.bin");
    static CORE: OnceLock<MathFont> = OnceLock::new();
    static FULL: OnceLock<MathFont> = OnceLock::new();
    thread_local! {
        static MISSING: Cell<bool> = const { Cell::new(false) };
    }

    fn core() -> &'static MathFont {
        CORE.get_or_init(|| MathFont::from_bytes(CORE_OTF).expect("core math font is valid"))
    }

    pub(super) fn font() -> Option<&'static MathFont> {
        Some(FULL.get().unwrap_or_else(core))
    }

    /// Whether `tree` draws only glyphs whose outlines the current face holds.
    pub(super) fn drawable(tree: &MathBox) -> bool {
        if FULL.get().is_some() {
            return true;
        }
        let mut ok = true;
        visit(tree, &mut |gid| {
            let gid = usize::from(gid);
            ok &= CORE_GLYPHS
                .get(gid / 8)
                .is_some_and(|bits| bits & (1 << (gid % 8)) != 0);
        });
        if !ok {
            MISSING.with(|missing| missing.set(true));
        }
        ok
    }

    fn visit(node: &MathBox, f: &mut impl FnMut(u16)) {
        match &node.content {
            BoxContent::Glyph { glyph_id, .. } => f(*glyph_id),
            BoxContent::HList(children)
            | BoxContent::VList(children)
            | BoxContent::Overlap(children) => children.iter().for_each(|child| visit(child, f)),
            BoxContent::Color(_, inner)
            | BoxContent::BackColor(_, inner)
            | BoxContent::Frame { inner, .. } => visit(inner, f),
            _ => {}
        }
    }

    /// True once since the last call if a formula needed the full face.
    pub fn take_missing() -> bool {
        MISSING.with(|missing| missing.replace(false))
    }

    /// Installs the full STIX Two Math face and drops formulas cached with the core face.
    pub fn install_full(bytes: Vec<u8>) -> bool {
        if FULL.get().is_none() {
            let Ok(font) = MathFont::from_bytes(Vec::leak(bytes)) else {
                return false;
            };
            let _ = FULL.set(font);
        }
        CACHE.with(|cache| *cache.borrow_mut() = crate::cache::Cache::new(4 * 1024 * 1024));
        MISSING.with(|missing| missing.set(false));
        true
    }
}
#[cfg(target_arch = "wasm32")]
use web_font::font;
thread_local! {
    static CACHE: RefCell<crate::cache::Cache<(String, bool), Rc<CachedFormula>>> =
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

pub(crate) fn render_svg(tex: &str, display: bool) -> Option<Arc<MathVisual>> {
    render_visual(tex, display, MathOutput::Svg)
}

pub(crate) fn render_pixels(tex: &str, display: bool) -> Option<Arc<MathVisual>> {
    render_visual(tex, display, MathOutput::Pixels)
}

// Layouts are immutable. Each output, including a failed output, is derived at
// most once while the formula is resident in the thread's bounded cache.
struct CachedFormula {
    tree: Option<MathBox>,
    layout_weight: usize,
    svg: OnceCell<Option<Arc<MathVisual>>>,
    pixels: OnceCell<Option<Arc<MathVisual>>>,
    both: OnceCell<Option<Arc<MathVisual>>>,
}

impl CachedFormula {
    fn output(&self, display: bool, output: MathOutput) -> Option<Arc<MathVisual>> {
        let slot = match output {
            MathOutput::Svg => &self.svg,
            MathOutput::Pixels => &self.pixels,
            MathOutput::Both => &self.both,
        };
        slot.get_or_init(|| {
            let tree = self.tree.as_ref()?;
            if output == MathOutput::Both {
                let svg = self.output(display, MathOutput::Svg)?;
                let pixels = self.output(display, MathOutput::Pixels)?;
                return Some(Arc::new(MathVisual {
                    svg: svg.svg.clone(),
                    png: pixels.png.clone(),
                    width: pixels.width,
                    height: pixels.height,
                }));
            }
            derive_visual(tree, display, output)
        })
        .clone()
    }

    fn weight(&self) -> usize {
        self.layout_weight
            + [&self.svg, &self.pixels, &self.both]
                .into_iter()
                .filter_map(|slot| slot.get().and_then(Option::as_ref))
                .map(|v| {
                    v.svg.capacity() + v.png.capacity() + std::mem::size_of::<MathVisual>() + 32
                })
                .sum::<usize>()
    }
}

fn layout_weight(tree: &MathBox) -> usize {
    std::mem::size_of::<MathBox>()
        + match &tree.content {
            BoxContent::HList(children)
            | BoxContent::VList(children)
            | BoxContent::Overlap(children) => {
                (children.capacity() - children.len()) * std::mem::size_of::<MathBox>()
                    + children.iter().map(layout_weight).sum::<usize>()
            }
            BoxContent::Color(_, inner)
            | BoxContent::BackColor(_, inner)
            | BoxContent::Frame { inner, .. } => layout_weight(inner),
            _ => 0,
        }
}

fn render_visual(tex: &str, display: bool, output: MathOutput) -> Option<Arc<MathVisual>> {
    if tex.len() > 2048 || tex.trim().is_empty() {
        return None;
    }
    let cached = CACHE.with(|cache| {
        cache
            .borrow_mut()
            .get_by(&(tex, display), |(source, style)| {
                source == tex && *style == display
            })
    });
    let is_new = cached.is_none();
    let formula = cached.unwrap_or_else(|| {
        let tree = prepare_layout(tex, display);
        let layout_weight = tree.as_ref().map_or(0, layout_weight)
            + tex.len()
            + std::mem::size_of::<CachedFormula>()
            + 128;
        Rc::new(CachedFormula {
            tree,
            layout_weight,
            svg: OnceCell::new(),
            pixels: OnceCell::new(),
            both: OnceCell::new(),
        })
    });
    let before = formula.weight();
    let result = formula.output(display, output);
    // Reinsert to account for newly materialized strings. Layout and outputs
    // share one eviction lifetime and one total 4 MiB budget.
    if is_new || formula.weight() != before {
        CACHE.with(|cache| {
            let key = (tex.to_owned(), display);
            let mut cache = cache.borrow_mut();
            cache.remove(&key);
            cache.insert(key, formula.clone(), formula.weight());
        });
    }
    result
}

fn prepare_layout(tex: &str, display: bool) -> Option<MathBox> {
    let font = font()?;
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
    Some(tree)
}

fn derive_visual(tree: &MathBox, display: bool, output: MathOutput) -> Option<Arc<MathVisual>> {
    let font = font()?;
    let svg = if output != MathOutput::Pixels {
        #[cfg(target_arch = "wasm32")]
        if !web_font::drawable(tree) {
            return None;
        }
        let svg_options = SvgOptions {
            font_size_pt: Dim::from_i64(16),
            display,
            ..SvgOptions::new()
        };
        let svg = latex_rust::render_svg(tree, font, &svg_options).ok()?;
        svg[svg.find("<svg")?..].replace("#000000", "currentColor")
    } else {
        String::new()
    };
    let (png, width, height) = if output != MathOutput::Svg {
        let png = rasterize(tree, font)?;
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
    Some(visual)
}

#[cfg(not(target_arch = "wasm32"))]
fn rasterize(tree: &MathBox, font: &MathFont) -> Option<Vec<u8>> {
    crate::math_raster::render_png(tree, font, 16).ok()
}

/// Wasm renders math as SVG only and does not link a rasterizer. A native
/// document built in Wasm keeps math as source text, the same fallback used
/// for any formula that cannot be drawn.
#[cfg(target_arch = "wasm32")]
fn rasterize(_: &MathBox, _: &MathFont) -> Option<Vec<u8>> {
    None
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

#[cfg(test)]
#[path = "math_probe.rs"]
mod probe;

#[cfg(test)]
mod parity {
    use super::*;
    #[test]
    fn mixed_outputs_preserve_baseline_svg_and_png_bytes() {
        for (tex, display, svg, png_hash) in [
            (
                "x^2+1",
                false,
                include_str!("math_fixtures/x2.svg"),
                "015d8991f070e38f45570e32300119af7a2c11a781c16b9d066e84de05dcf681",
            ),
            (
                r"\sum_{i=1}^{n}\frac{i^2}{2}+1",
                false,
                include_str!("math_fixtures/sum_fraction.svg"),
                "e22f62363434c6f41aaab538aef6fb000b676abb9965e9b274924cf953747bc1",
            ),
            (
                r"\int_0^\infty e^{-x^2}\,dx=\frac{\sqrt{\pi}}{2}",
                true,
                include_str!("math_fixtures/display_math.svg"),
                "5595943b2aeae756861dcdb3dd79b95e3243635fe269bb4da62007da21ddc34e",
            ),
        ] {
            for order in [
                [MathOutput::Svg, MathOutput::Pixels, MathOutput::Both],
                [MathOutput::Both, MathOutput::Pixels, MathOutput::Svg],
                [MathOutput::Pixels, MathOutput::Both, MathOutput::Svg],
            ] {
                CACHE.with(|c| *c.borrow_mut() = crate::cache::Cache::new(4 * 1024 * 1024));
                for output in order {
                    let visual = render_visual(tex, display, output).unwrap();
                    if output != MathOutput::Pixels {
                        assert_eq!(visual.svg, svg);
                    } else {
                        assert!(visual.svg.is_empty());
                    }
                    if output != MathOutput::Svg {
                        assert_eq!(
                            MathFont::sha256_hex(&STANDARD.decode(&visual.png).unwrap()),
                            png_hash
                        );
                    } else {
                        assert!(visual.png.is_empty());
                    }
                }
            }
        }
        for _ in 0..3 {
            assert!(render_svg(r"\frac{", false).is_none());
            assert!(render_pixels(r"\frac{", false).is_none());
            assert!(render_math(r"\frac{", false).is_none());
        }
    }
}
