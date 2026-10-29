//! The web build embeds `assets/math-core.otf` in place of the full STIX Two
//! Math face. These checks show that it cannot change any output: every glyph
//! keeps its advance, every glyph latex-rust can reach keeps its bounding box,
//! kept glyphs keep their outlines, and formulas lay out identically. Merging
//! the glyph chunks restores every other reachable outline.
use latex_rust::{ttf_parser, BoxContent, MathBox, MathFont, MathStyle};

static CORE: &[u8] = include_bytes!("../assets/math-core.otf");
static GLYPHS: &str = include_str!("../assets/math-core-glyphs.txt");
static GROUPS: &[u8] = include_bytes!("../assets/math-core-groups.bin");

#[derive(Default, PartialEq, Debug)]
struct Path(Vec<(char, [i32; 6])>);
impl ttf_parser::OutlineBuilder for Path {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0
            .push(('M', [x.to_bits() as i32, y.to_bits() as i32, 0, 0, 0, 0]));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0
            .push(('L', [x.to_bits() as i32, y.to_bits() as i32, 0, 0, 0, 0]));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0
            .push(('Q', [x1, y1, x, y, 0.0, 0.0].map(|v| v.to_bits() as i32)));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0
            .push(('C', [x1, y1, x2, y2, x, y].map(|v| v.to_bits() as i32)));
    }
    fn close(&mut self) {
        self.0.push(('Z', [0; 6]));
    }
}

fn fonts() -> (MathFont, MathFont, Vec<bool>) {
    let full = MathFont::stix_two_math().unwrap();
    let core = MathFont::from_bytes(CORE).unwrap();
    let mut kept = vec![false; usize::from(full.face().number_of_glyphs())];
    for gid in GLYPHS
        .lines()
        .filter_map(|line| line.trim().parse::<usize>().ok())
    {
        kept[gid] = true;
    }
    kept[0] = true; // .notdef
    assert_eq!(GROUPS.len(), kept.len());
    for (gid, kept) in kept.iter().enumerate() {
        if *kept {
            assert_eq!(GROUPS[gid], 0, "group {gid}");
        }
    }
    (full, core, kept)
}

/// Every glyph latex-rust can reach: cmap targets closed over the size
/// variants, assembly parts and ssty alternates that its layout reads.
fn reachable(font: &MathFont) -> Vec<bool> {
    let face = font.face();
    let mut seen = vec![false; usize::from(face.number_of_glyphs())];
    let mut frontier: Vec<u16> = (0..0x110000)
        .filter_map(char::from_u32)
        .filter_map(|ch| face.glyph_index(ch))
        .map(|gid| gid.0)
        .collect();
    let gsub = face.tables().gsub.unwrap();
    let ssty = gsub
        .features
        .find(ttf_parser::Tag::from_bytes(b"ssty"))
        .unwrap();
    while let Some(gid) = frontier.pop() {
        if std::mem::replace(&mut seen[usize::from(gid)], true) {
            continue;
        }
        frontier.extend(font.vertical_variants(gid));
        frontier.extend(font.horizontal_variants(gid));
        frontier.extend(
            font.horizontal_assembly_parts(gid)
                .into_iter()
                .map(|part| part.0),
        );
        for index in ssty.lookup_indices {
            for subtable in gsub
                .lookups
                .get(index)
                .unwrap()
                .subtables
                .into_iter::<ttf_parser::gsub::SubstitutionSubtable>()
            {
                if let ttf_parser::gsub::SubstitutionSubtable::Alternate(alternate) = subtable {
                    if let Some(set) = alternate
                        .coverage
                        .get(ttf_parser::GlyphId(gid))
                        .and_then(|i| alternate.alternate_sets.get(i))
                    {
                        frontier.extend(set.alternates.into_iter().map(|g| g.0));
                    }
                }
            }
        }
    }
    seen
}

#[test]
fn core_font_keeps_all_metrics_and_kept_outlines() {
    let (full, core, kept) = fonts();
    let (a, b) = (full.face(), core.face());
    let reachable = reachable(&full);
    assert!(reachable.iter().filter(|r| **r).count() > 6000);
    assert_eq!(a.number_of_glyphs(), b.number_of_glyphs());
    assert_eq!(full.units_per_em(), core.units_per_em());
    assert_eq!(
        (full.ascender_fu(), full.descender_fu()),
        (core.ascender_fu(), core.descender_fu())
    );
    let mut drawn = 0;
    for gid in 0..a.number_of_glyphs() {
        let id = ttf_parser::GlyphId(gid);
        assert_eq!(
            a.glyph_hor_advance(id),
            b.glyph_hor_advance(id),
            "advance {gid}"
        );
        if reachable[usize::from(gid)] {
            assert_eq!(
                a.glyph_bounding_box(id),
                b.glyph_bounding_box(id),
                "bbox {gid}"
            );
        }
        assert_eq!(full.italic_correction(gid), core.italic_correction(gid));
        assert_eq!(
            full.top_accent_attachment(gid),
            core.top_accent_attachment(gid)
        );
        assert_eq!(full.vertical_variants(gid), core.vertical_variants(gid));
        assert_eq!(full.horizontal_variants(gid), core.horizontal_variants(gid));
        assert_eq!(
            full.horizontal_assembly_parts(gid),
            core.horizontal_assembly_parts(gid)
        );
        if kept[usize::from(gid)] {
            let (mut p, mut q) = (Path::default(), Path::default());
            a.outline_glyph(id, &mut p);
            b.outline_glyph(id, &mut q);
            assert_eq!(p, q, "outline {gid}");
            drawn += 1;
        }
    }
    assert!(drawn > 1000);
    for ch in (0..0x110000).filter_map(char::from_u32) {
        assert_eq!(a.glyph_index(ch), b.glyph_index(ch), "cmap {ch:?}");
    }
}

fn glyphs(node: &MathBox, out: &mut Vec<u16>) {
    match &node.content {
        BoxContent::Glyph { glyph_id, .. } => out.push(*glyph_id),
        BoxContent::HList(c) | BoxContent::VList(c) | BoxContent::Overlap(c) => {
            c.iter().for_each(|n| glyphs(n, out))
        }
        BoxContent::Color(_, inner)
        | BoxContent::BackColor(_, inner)
        | BoxContent::Frame { inner, .. } => glyphs(inner, out),
        _ => {}
    }
}

#[test]
fn core_font_lays_out_and_draws_formulas_identically() {
    let (full, core, kept) = fonts();
    let mut formulas: Vec<String> = [
        "x^2", "\\frac{a+b}{c-d}", "\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}", "\\int_0^\\infty e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
        "\\sqrt[3]{x} + \\sqrt{y}", "\\alpha\\beta\\gamma \\Delta \\Omega", "\\left( \\frac{1}{2} \\right)^{n}",
        "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", "\\mathbb{R} \\to \\mathcal{L}", "f'(x) = \\lim_{h \\to 0} \\frac{f(x+h)-f(x)}{h}",
        "\\mathfrak{g} \\oplus \\mathscr{H}", "\\phantom{\\mathfrak{X}} x \\hphantom{\\Finv} y", "\\text{café ∮ ☃}", "\\left\\langle \\begin{matrix} 1\\\\2\\\\3\\\\4\\\\5\\\\6\\\\7\\\\8\\\\9 \\end{matrix} \\right\\rangle",
        "\\overbrace{a+b+c+d+e+f+g+h+i+j+k+l+m+n+o+p+q+r+s+t+u+v+w+x+y+z}^{26}", "\\xrightarrow[\\text{under}]{\\text{over over over over}}",
        "\\boldsymbol{\\nabla} \\times \\mathbf{B} = \\mu_0 \\mathbf{J}", "\\binom{n}{k} = \\frac{n!}{k!(n-k)!}", "\\sqrt{\\frac{\\frac{a}{b}}{\\frac{c}{\\frac{d}{e}}}}",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for entry in latex_rust::symbols() {
        let latex = entry.latex.replace("{}", "{x}");
        formulas.push(latex.clone());
        formulas.push(format!("x_{{{latex}}}^{{y^{{{latex}}}}}"));
    }
    let (mut same, mut needs_full) = (0, 0);
    for tex in &formulas {
        let Ok(ast) = latex_rust::parse(tex) else {
            continue;
        };
        for style in [MathStyle::Display, MathStyle::Text] {
            let (Ok(a), Ok(b)) = (
                latex_rust::layout(&ast, &full, style),
                latex_rust::layout(&ast, &core, style),
            ) else {
                assert!(
                    latex_rust::layout(&ast, &full, style).is_err()
                        && latex_rust::layout(&ast, &core, style).is_err()
                );
                continue;
            };
            assert_eq!(a, b, "layout {tex}");
            let mut used = Vec::new();
            glyphs(&a, &mut used);
            // SVG output is a function of the layout and the glyph outlines.
            // latex-rust caches glyph paths process-wide by glyph id, so the
            // outline test above, not a second render here, covers the paths.
            if used.iter().all(|gid| kept[usize::from(*gid)]) {
                same += 1;
            } else {
                needs_full += 1;
            }
        }
    }
    eprintln!("{same} drawn with the core font, {needs_full} need the full font");
    assert!(same > 500 && needs_full > 0);
}

fn chunk(group: u8) -> Vec<u8> {
    let path = format!(
        "{}/assets/math-glyphs/math-glyphs-{group}.bin",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(path).unwrap()
}

#[test]
fn glyph_chunks_restore_every_reachable_outline() {
    let (full, _, kept) = fonts();
    let reachable = reachable(&full);
    let groups = *GROUPS.iter().max().unwrap();
    assert!(groups > 5 && groups < 32);
    let mut merged = CORE.to_vec();
    for group in 1..=groups {
        let chunk = chunk(group);
        let glyphs = markit::math_glyphs::chunk_glyphs(&chunk).unwrap();
        assert!(!glyphs.is_empty());
        for (gid, _) in &glyphs {
            assert_eq!(GROUPS[*gid], group, "chunk {group} glyph {gid}");
        }
        // Each chunk holds exactly the glyphs of its group.
        let count = GROUPS.iter().filter(|g| **g == group).count();
        assert_eq!(glyphs.len(), count);
        merged = markit::math_glyphs::merge(&merged, &chunk).unwrap();
    }
    let merged: &'static [u8] = merged.leak();
    let merged = MathFont::from_bytes(merged).unwrap();
    let (a, b) = (full.face(), merged.face());
    for gid in 0..a.number_of_glyphs() {
        let index = usize::from(gid);
        assert!(GROUPS[index] == 0 || !kept[index], "group {gid}");
        let id = ttf_parser::GlyphId(gid);
        assert_eq!(a.glyph_hor_advance(id), b.glyph_hor_advance(id));
        if reachable[index] {
            assert_eq!(
                a.glyph_bounding_box(id),
                b.glyph_bounding_box(id),
                "bbox {gid}"
            );
            let (mut p, mut q) = (Path::default(), Path::default());
            a.outline_glyph(id, &mut p);
            b.outline_glyph(id, &mut q);
            assert_eq!(p, q, "outline {gid}");
        }
    }
    // Every reachable glyph that the core does not draw is in some chunk.
    for (gid, reachable) in reachable.iter().enumerate() {
        if *reachable && !kept[gid] {
            assert_ne!(GROUPS[gid], 0, "unassigned {gid}");
        }
    }
    assert!(markit::math_glyphs::merge(CORE, b"MKGC").is_none());
    assert!(markit::math_glyphs::merge(&CORE[..CORE.len() - 8], &chunk(1)).is_none());
}

#[test]
fn rare_formulas_need_only_their_alphabet() {
    let full = MathFont::stix_two_math().unwrap();
    for (tex, group) in [("\\mathfrak{g}", 1), ("\\mathscr{h}", 2)] {
        let ast = latex_rust::parse(tex).unwrap();
        let tree = latex_rust::layout(&ast, &full, MathStyle::Text).unwrap();
        let mut used = Vec::new();
        glyphs(&tree, &mut used);
        let needed: std::collections::BTreeSet<u8> = used
            .iter()
            .map(|g| GROUPS[usize::from(*g)])
            .filter(|g| *g != 0)
            .collect();
        assert_eq!(needed.into_iter().collect::<Vec<_>>(), vec![group], "{tex}");
    }
}
