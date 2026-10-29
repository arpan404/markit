//! Native math rasterizer. A port of `latex-rust` 2.1.1's PNG backend that
//! produces the same pixels and PNG bytes (same `tiny-skia` calls, same
//! encoder), with two differences: glyph outlines are converted once per
//! thread instead of once per formula, and fills borrow cached paths instead
//! of cloning them. Wasm builds do not compile this module or link tiny-skia.
use latex_rust::{ttf_parser, BoxContent, Color, Dim, Error, FontError, MathBox, MathFont};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tiny_skia::{FillRule, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, Transform};

thread_local! {
    // STIX glyph ids are bounded by the font, so this cannot grow without limit.
    static OUTLINES: RefCell<HashMap<u16, Option<Rc<Path>>>> = RefCell::new(HashMap::new());
}

/// Same defaults as `latex_rust::PngOptions::new()`: 144 dpi, black, transparent.
pub(crate) fn render_png(
    tree: &MathBox,
    font: &MathFont,
    font_size_pt: i64,
) -> Result<Vec<u8>, Error> {
    let em_px = &Dim::from_i64(font_size_pt) * &Dim::from_i64(144) / Dim::from_i64(72);
    let fu_px = &em_px / &Dim::from_i64(i64::from(font.units_per_em()));
    let w = (&tree.width * &em_px).ceil_to_u32()?.max(1);
    let h = ((&tree.height + &tree.depth) * &em_px)
        .ceil_to_u32()?
        .max(1);
    let mut pixmap = Pixmap::new(w, h).ok_or_else(|| Error::InvalidOption {
        what: format!("png pixmap {w}x{h}"),
    })?;
    let baseline = &tree.height * &em_px;
    let mut painter = Painter {
        font,
        em_px: &em_px,
        fu_px: &fu_px,
        pixmap: &mut pixmap,
    };
    painter.emit(tree, &Dim::zero(), &baseline, Color::rgb(0, 0, 0))?;
    pixmap.encode_png().map_err(|_| Error::Unsupported {
        what: "png encode".into(),
    })
}

struct Painter<'a> {
    font: &'a MathFont,
    em_px: &'a Dim,
    fu_px: &'a Dim,
    pixmap: &'a mut Pixmap,
}

fn skia_color(c: Color, a: u8) -> tiny_skia::Color {
    let [r, g, b, _] = c.to_rgba8();
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(skia_color(c, 255));
    p.anti_alias = true;
    p
}

fn px(d: &Dim) -> f32 {
    f32::from_bits(d.to_ieee32_bits())
}

impl Painter<'_> {
    fn emit(
        &mut self,
        bx: &MathBox,
        origin_x: &Dim,
        parent_baseline: &Dim,
        fill: Color,
    ) -> Result<(), Error> {
        let em_px = self.em_px;
        let baseline = parent_baseline - &(bx.shift.clone() * em_px);
        match &bx.content {
            BoxContent::Empty | BoxContent::Kern(_) => Ok(()),
            BoxContent::Glyph {
                glyph_id, scale, ..
            } => {
                let path = glyph_path(self.font, *glyph_id)?;
                let k = self.fu_px * scale;
                let t = Transform::from_row(px(&k), 0.0, 0.0, px(&-k), px(origin_x), px(&baseline));
                self.pixmap
                    .fill_path(&path, &paint(fill), FillRule::Winding, t, None);
                Ok(())
            }
            BoxContent::Rule => self.fill_box_rect(bx, origin_x, &baseline, fill),
            BoxContent::HList(kids) => {
                let mut x = origin_x.clone();
                for k in kids {
                    self.emit(k, &x, &baseline, fill)?;
                    x = &x + &(k.width.clone() * em_px);
                }
                Ok(())
            }
            BoxContent::VList(kids) => {
                let Some(first) = kids.first() else {
                    return Ok(());
                };
                self.emit(first, origin_x, &baseline, fill)?;
                let mut y_below = first.depth.clone() * em_px;
                for k in kids.iter().skip(1) {
                    let child_base = &baseline + &y_below + &(k.height.clone() * em_px);
                    self.emit(k, origin_x, &child_base, fill)?;
                    y_below = &y_below + &((&k.height + &k.depth) * em_px);
                }
                Ok(())
            }
            BoxContent::Overlap(kids) => {
                for k in kids {
                    self.emit(k, origin_x, &baseline, fill)?;
                }
                Ok(())
            }
            BoxContent::Color(c, inner) => self.emit(inner, origin_x, &baseline, *c),
            BoxContent::BackColor(c, inner) => {
                self.fill_box_rect(bx, origin_x, &baseline, *c)?;
                self.emit(inner, origin_x, &baseline, fill)
            }
            BoxContent::Line {
                x1,
                y1,
                x2,
                y2,
                thickness,
            } => {
                let x1p = origin_x + &(x1.clone() * em_px);
                let y1p = &baseline - &(y1.clone() * em_px);
                let x2p = origin_x + &(x2.clone() * em_px);
                let y2p = &baseline - &(y2.clone() * em_px);
                let mut pb = PathBuilder::new();
                pb.move_to(px(&x1p), px(&y1p));
                pb.line_to(px(&x2p), px(&y2p));
                let Some(path) = pb.finish() else {
                    return Ok(());
                };
                let stroke = Stroke {
                    width: px(&(thickness.clone() * em_px)),
                    ..Stroke::default()
                };
                self.pixmap
                    .stroke_path(&path, &paint(fill), &stroke, Transform::identity(), None);
                Ok(())
            }
            BoxContent::Frame {
                thickness,
                stroke,
                inner,
            } => {
                let w = &bx.width * em_px;
                let h_ink = (&bx.height + &bx.depth) * em_px;
                let t = thickness.clone() * em_px;
                let half = &t / &Dim::from_i64(2);
                let x = origin_x + &half;
                let y = &(&baseline - &(bx.height.clone() * em_px)) + &half;
                let rw = (&w - &t).max(&Dim::zero());
                let rh = (&h_ink - &t).max(&Dim::zero());
                if !rw.is_zero() && !rh.is_zero() {
                    if let Some(rect) = Rect::from_xywh(px(&x), px(&y), px(&rw), px(&rh)) {
                        let st = Stroke {
                            width: px(&t),
                            ..Stroke::default()
                        };
                        let border = stroke.unwrap_or(fill);
                        self.pixmap.stroke_path(
                            &PathBuilder::from_rect(rect),
                            &paint(border),
                            &st,
                            Transform::identity(),
                            None,
                        );
                    }
                }
                self.emit(inner, origin_x, &baseline, fill)
            }
        }
    }

    fn fill_box_rect(
        &mut self,
        bx: &MathBox,
        origin_x: &Dim,
        baseline: &Dim,
        fill: Color,
    ) -> Result<(), Error> {
        let em_px = self.em_px;
        let w = &bx.width * em_px;
        let h_ink = (&bx.height + &bx.depth) * em_px;
        if w.is_zero() || h_ink.is_zero() {
            return Ok(());
        }
        let y = baseline - &(bx.height.clone() * em_px);
        if let Some(rect) = Rect::from_xywh(px(origin_x), px(&y), px(&w), px(&h_ink)) {
            self.pixmap
                .fill_rect(rect, &paint(fill), Transform::identity(), None);
        }
        Ok(())
    }
}

fn glyph_path(font: &MathFont, glyph_id: u16) -> Result<Rc<Path>, Error> {
    OUTLINES
        .with(|cache| {
            cache
                .borrow_mut()
                .entry(glyph_id)
                .or_insert_with(|| outline_path(font, glyph_id).map(Rc::new))
                .clone()
        })
        .ok_or_else(|| FontError::MissingGlyph { ch: '\u{FFFD}' }.into())
}

fn outline_path(font: &MathFont, glyph_id: u16) -> Option<Path> {
    let mut b = DimOutline {
        pb: PathBuilder::new(),
    };
    font.face()
        .outline_glyph(ttf_parser::GlyphId(glyph_id), &mut b)?;
    b.pb.finish()
}

/// TrueType `f32` points round-trip through [`Dim`] exactly as upstream does.
struct DimOutline {
    pb: PathBuilder,
}

impl DimOutline {
    fn pt(x: f32) -> f32 {
        f32::from_bits(Dim::from_ieee32_bits(x.to_bits()).to_ieee32_bits())
    }
}

impl ttf_parser::OutlineBuilder for DimOutline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(Self::pt(x), Self::pt(y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(Self::pt(x), Self::pt(y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.pb
            .quad_to(Self::pt(x1), Self::pt(y1), Self::pt(x), Self::pt(y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            Self::pt(x1),
            Self::pt(y1),
            Self::pt(x2),
            Self::pt(y2),
            Self::pt(x),
            Self::pt(y),
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}
