//! Colour glyphs (OpenType `COLR` version 0 and 1 with `CPAL`, e.g. Noto Color Emoji): the paint
//! graph skrifa walks ([`skrifa::color::ColorGlyph::paint`]) drawn into premultiplied RGBA.
//!
//! Supported: solid fills, linear, radial and sweep gradients (pad, repeat and reflect),
//! transforms, glyph and box clips, and layers composited with the Porter-Duff modes, Plus,
//! Multiply and Screen; the other blend modes composite as source-over. Colours are the font's
//! first palette, with the text colour for the foreground index (0xFFFF), in RGB.

use skrifa::GlyphId;
use skrifa::color::{Brush, ColorPainter, ColorStop, CompositeMode, Extend, Transform};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::DrawSettings;
use skrifa::raw::types::BoundingBox;
use skrifa::{FontRef, MetadataProvider};

use crate::raster::{Coverage, Pen, Xform, rect_to};

/// Deepest stack of transforms, clips or layers a glyph may push (the paint graph is bounded by
/// skrifa too; this caps memory).
const MAX_DEPTH: usize = 64;

/// The colour glyph for `id`, if the font has one.
pub(crate) fn color_glyph<'a>(font: &FontRef<'a>, id: u32) -> Option<skrifa::color::ColorGlyph<'a>> {
    font.color_glyphs().get(GlyphId::new(id))
}

/// A colour glyph's extent in font units (y up): its COLRv1 clip box, else its advance from the
/// descender to the ascender.
pub(crate) fn extent(font: &FontRef<'_>, id: u32, coords: LocationRef<'_>) -> Option<[f64; 4]> {
    let glyph = color_glyph(font, id)?;
    if let Some(b) = glyph.bounding_box(coords, Size::unscaled()) {
        return Some([f64::from(b.x_min), f64::from(b.y_min), f64::from(b.x_max), f64::from(b.y_max)]);
    }
    let metrics = font.metrics(Size::unscaled(), coords);
    let advance = font.glyph_metrics(Size::unscaled(), coords).advance_width(GlyphId::new(id)).unwrap_or(f32::from(metrics.units_per_em));
    Some([0.0, f64::from(metrics.descent), f64::from(advance), f64::from(metrics.ascent)])
}

/// Premultiplied RGBA pixels, `width × height`, that colour glyphs are painted into.
pub(crate) struct Target<'a> {
    pub rgba: &'a mut [f32],
    pub width: usize,
    pub height: usize,
}

/// Paints colour glyph `id` of `font` into `target` through `xf` (font units, y up → the target's
/// pixels), source-over. `foreground` is the text colour (straight RGBA). Returns false when the
/// font has no colour glyph `id` or its paint graph is broken (nothing is drawn then).
pub(crate) fn paint(font: &FontRef<'_>, id: u32, coords: LocationRef<'_>, xf: Xform, foreground: [f32; 4], target: Target<'_>) -> bool {
    let Target { rgba: out, width, height } = target;
    let Some(glyph) = color_glyph(font, id) else { return false };
    if width == 0 || height == 0 || out.len() < width * height * 4 {
        return false;
    }
    let palette: Vec<[f32; 4]> = font
        .color_palettes()
        .get(0)
        .map(|p| {
            p.colors().iter().map(|c| [f32::from(c.red) / 255.0, f32::from(c.green) / 255.0, f32::from(c.blue) / 255.0, f32::from(c.alpha) / 255.0]).collect()
        })
        .unwrap_or_default();
    let mut painter = Painter {
        font,
        coords,
        palette,
        foreground,
        width,
        height,
        transforms: vec![xf],
        clips: vec![vec![1.0; width * height]],
        layers: vec![(vec![0.0; width * height * 4], CompositeMode::SrcOver)],
        overflow: 0,
    };
    if glyph.paint(coords, &mut painter).is_err() {
        return false;
    }
    let Some((layer, _)) = painter.layers.into_iter().next() else { return false };
    for (dst, src) in out.as_chunks_mut::<4>().0.iter_mut().zip(layer.as_chunks::<4>().0) {
        let keep = 1.0 - src[3];
        for (d, s) in dst.iter_mut().zip(src) {
            *d = s + *d * keep;
        }
    }
    true
}

struct Painter<'a, 'f> {
    font: &'a FontRef<'f>,
    coords: LocationRef<'a>,
    palette: Vec<[f32; 4]>,
    foreground: [f32; 4],
    width: usize,
    height: usize,
    /// Font units → pixels, innermost last.
    transforms: Vec<Xform>,
    /// Coverage of the clips so far (each the product of those below it).
    clips: Vec<Vec<f32>>,
    /// Premultiplied RGBA layers and how each merges down.
    layers: Vec<(Vec<f32>, CompositeMode)>,
    /// Pushes refused past [`MAX_DEPTH`], so their pops are ignored too.
    overflow: usize,
}

impl Painter<'_, '_> {
    fn xf(&self) -> Xform {
        self.transforms.last().copied().unwrap_or(Xform::IDENTITY)
    }

    fn full(&self) -> bool {
        self.transforms.len() + self.clips.len() + self.layers.len() > 3 * MAX_DEPTH
    }

    fn push_clip(&mut self, coverage: Vec<f32>) {
        let mut clip = self.clips.last().cloned().unwrap_or_else(|| vec![1.0; self.width * self.height]);
        for (c, v) in clip.iter_mut().zip(coverage) {
            *c *= v;
        }
        self.clips.push(clip);
    }

    /// A palette colour times `alpha`, straight RGBA.
    fn color(&self, index: u16, alpha: f32) -> [f32; 4] {
        let [r, g, b, a] = if index == 0xFFFF { self.foreground } else { self.palette.get(usize::from(index)).copied().unwrap_or([0.0, 0.0, 0.0, 1.0]) };
        [r, g, b, (a * alpha).clamp(0.0, 1.0)]
    }

    /// The colour at `t` (0..1 along the stops) of a colour line, after `extend`.
    fn stop_color(&self, stops: &[ColorStop], t: f32, extend: Extend) -> [f32; 4] {
        let t = match extend {
            Extend::Repeat => t - t.floor(),
            Extend::Reflect => {
                let m = t.rem_euclid(2.0);
                if m > 1.0 { 2.0 - m } else { m }
            }
            _ => t.clamp(0.0, 1.0),
        };
        let Some(first) = stops.first() else { return [0.0; 4] };
        if t <= first.offset {
            return self.color(first.palette_index, first.alpha);
        }
        for pair in stops.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if t <= b.offset {
                let span = b.offset - a.offset;
                let f = if span > 1e-6 { (t - a.offset) / span } else { 1.0 };
                // Interpolate premultiplied, as the COLR specification asks.
                let (ca, cb) = (self.color(a.palette_index, a.alpha), self.color(b.palette_index, b.alpha));
                let pa = [ca[0] * ca[3], ca[1] * ca[3], ca[2] * ca[3], ca[3]];
                let pb = [cb[0] * cb[3], cb[1] * cb[3], cb[2] * cb[3], cb[3]];
                let p: Vec<f32> = pa.iter().zip(pb).map(|(x, y)| x + (y - x) * f).collect();
                let alpha = p[3];
                return if alpha > 0.0 { [p[0] / alpha, p[1] / alpha, p[2] / alpha, alpha] } else { [0.0; 4] };
            }
        }
        stops.last().map_or([0.0; 4], |s| self.color(s.palette_index, s.alpha))
    }

    /// The brush's straight RGBA at paint-space point (x, y), or None outside it (a radial
    /// gradient's undefined area).
    fn brush_at(&self, brush: &Brush<'_>, x: f64, y: f64) -> Option<[f32; 4]> {
        Some(match brush {
            Brush::Solid { palette_index, alpha } => self.color(*palette_index, *alpha),
            Brush::LinearGradient { p0, p1, color_stops, extend } => {
                let (dx, dy) = (f64::from(p1.x - p0.x), f64::from(p1.y - p0.y));
                let len2 = dx * dx + dy * dy;
                let t = if len2 > 1e-12 { ((x - f64::from(p0.x)) * dx + (y - f64::from(p0.y)) * dy) / len2 } else { 0.0 };
                self.stop_color(color_stops, t as f32, *extend)
            }
            Brush::RadialGradient { c0, r0, c1, r1, color_stops, extend } => {
                let t = radial_t(x, y, (f64::from(c0.x), f64::from(c0.y), f64::from(*r0)), (f64::from(c1.x), f64::from(c1.y), f64::from(*r1)))?;
                self.stop_color(color_stops, t as f32, *extend)
            }
            Brush::SweepGradient { c0, start_angle, end_angle, color_stops, extend } => {
                // Angles run clockwise in a y-up space, as skrifa hands them over.
                let mut angle = (-(y - f64::from(c0.y))).atan2(x - f64::from(c0.x)).to_degrees();
                if angle < 0.0 {
                    angle += 360.0;
                }
                let span = f64::from(end_angle - start_angle);
                let t = if span.abs() > 1e-9 { (angle - f64::from(*start_angle)) / span } else { 0.0 };
                self.stop_color(color_stops, t as f32, *extend)
            }
        })
    }
}

/// The largest `t` whose circle (centre and radius interpolated from `a` to `b`, radius ≥ 0)
/// passes through (x, y): the two-point conical gradient.
fn radial_t(x: f64, y: f64, a: (f64, f64, f64), b: (f64, f64, f64)) -> Option<f64> {
    let (cdx, cdy, dr) = (b.0 - a.0, b.1 - a.1, b.2 - a.2);
    let (pdx, pdy) = (x - a.0, y - a.1);
    let qa = cdx * cdx + cdy * cdy - dr * dr;
    let qb = pdx * cdx + pdy * cdy + a.2 * dr;
    let qc = pdx * pdx + pdy * pdy - a.2 * a.2;
    let ok = |t: f64| a.2 + t * dr >= 0.0;
    if qa.abs() < 1e-9 {
        if qb.abs() < 1e-12 {
            return None;
        }
        let t = qc / (2.0 * qb);
        return ok(t).then_some(t);
    }
    let disc = qb * qb - qa * qc;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let (t1, t2) = ((qb + root) / qa, (qb - root) / qa);
    let (hi, lo) = if t1 > t2 { (t1, t2) } else { (t2, t1) };
    if ok(hi) {
        Some(hi)
    } else if ok(lo) {
        Some(lo)
    } else {
        None
    }
}

fn inverse(xf: &Xform) -> Option<Xform> {
    let [a, b, c, d, e, f] = xf.0;
    let det = a * d - b * c;
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    Some(Xform([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)]))
}

/// Merges premultiplied `src` onto `dst` with `mode`.
fn composite(dst: &mut [f32], src: &[f32], mode: CompositeMode) {
    for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
        let (sa, da) = (s[3], d[3]);
        // Porter-Duff factors (Fa for the source, Fb for the destination).
        let pd = |fa: f32, fb: f32, d: &mut [f32]| {
            for i in 0..4 {
                d[i] = s[i] * fa + d[i] * fb;
            }
        };
        match mode {
            CompositeMode::Clear => d.fill(0.0),
            CompositeMode::Src => d.copy_from_slice(s),
            CompositeMode::Dest => {}
            CompositeMode::DestOver => pd(1.0 - da, 1.0, d),
            CompositeMode::SrcIn => pd(da, 0.0, d),
            CompositeMode::DestIn => pd(0.0, sa, d),
            CompositeMode::SrcOut => pd(1.0 - da, 0.0, d),
            CompositeMode::DestOut => pd(0.0, 1.0 - sa, d),
            CompositeMode::SrcAtop => pd(da, 1.0 - sa, d),
            CompositeMode::DestAtop => pd(1.0 - da, sa, d),
            CompositeMode::Xor => pd(1.0 - da, 1.0 - sa, d),
            CompositeMode::Plus => {
                for i in 0..4 {
                    d[i] = (d[i] + s[i]).min(1.0);
                }
            }
            CompositeMode::Multiply => {
                for i in 0..3 {
                    d[i] = s[i] * d[i] + s[i] * (1.0 - da) + d[i] * (1.0 - sa);
                }
                d[3] = sa + da - sa * da;
            }
            CompositeMode::Screen => {
                for i in 0..3 {
                    d[i] = s[i] + d[i] - s[i] * d[i];
                }
                d[3] = sa + da - sa * da;
            }
            // Source-over, and the separable and non-separable blend modes not drawn yet.
            _ => pd(1.0, 1.0 - sa, d),
        }
    }
}

impl ColorPainter for Painter<'_, '_> {
    fn push_transform(&mut self, t: Transform) {
        if self.full() {
            self.overflow += 1;
            return;
        }
        let local = Xform([f64::from(t.xx), f64::from(t.yx), f64::from(t.xy), f64::from(t.yy), f64::from(t.dx), f64::from(t.dy)]);
        let xf = self.xf().mul(&local);
        self.transforms.push(xf);
    }

    fn pop_transform(&mut self) {
        if self.overflow > 0 {
            self.overflow -= 1;
        } else if self.transforms.len() > 1 {
            self.transforms.pop();
        }
    }

    fn push_clip_glyph(&mut self, glyph_id: GlyphId) {
        if self.full() {
            self.overflow += 1;
            return;
        }
        let mut cov = Coverage::new(self.width, self.height);
        if let Some(outline) = self.font.outline_glyphs().get(glyph_id) {
            let mut pen = Pen::new(&mut cov, self.xf());
            if outline.draw(DrawSettings::unhinted(Size::unscaled(), self.coords), &mut pen).is_ok() {
                skrifa::outline::OutlinePen::close(&mut pen);
            }
        }
        self.push_clip(cov.finish());
    }

    fn push_clip_box(&mut self, b: BoundingBox<f32>) {
        if self.full() {
            self.overflow += 1;
            return;
        }
        let mut cov = Coverage::new(self.width, self.height);
        rect_to(&mut cov, &self.xf(), f64::from(b.x_min), f64::from(b.y_min), f64::from(b.x_max), f64::from(b.y_max));
        self.push_clip(cov.finish());
    }

    fn pop_clip(&mut self) {
        if self.overflow > 0 {
            self.overflow -= 1;
        } else if self.clips.len() > 1 {
            self.clips.pop();
        }
    }

    fn fill(&mut self, brush: Brush<'_>) {
        let Some(back) = inverse(&self.xf()) else { return };
        let (Some(clip), Some((layer, _))) = (self.clips.last(), self.layers.last()) else { return };
        let mut painted = vec![0.0f32; layer.len()];
        for (i, &cover) in clip.iter().enumerate() {
            if cover <= 0.0 {
                continue;
            }
            let (px, py) = ((i % self.width) as f64 + 0.5, (i / self.width) as f64 + 0.5);
            let (x, y) = back.apply(px, py);
            let Some([r, g, b, a]) = self.brush_at(&brush, x, y) else { continue };
            let a = a * cover;
            if let Some(out) = painted.get_mut(i * 4..i * 4 + 4) {
                out.copy_from_slice(&[r * a, g * a, b * a, a]);
            }
        }
        if let Some((layer, _)) = self.layers.last_mut() {
            composite(layer, &painted, CompositeMode::SrcOver);
        }
    }

    fn push_layer(&mut self, mode: CompositeMode) {
        if self.full() {
            self.overflow += 1;
            return;
        }
        self.layers.push((vec![0.0; self.width * self.height * 4], mode));
    }

    fn pop_layer(&mut self) {
        if self.overflow > 0 {
            self.overflow -= 1;
            return;
        }
        if self.layers.len() < 2 {
            return;
        }
        let Some((top, mode)) = self.layers.pop() else { return };
        if let Some((below, _)) = self.layers.last_mut() {
            composite(below, &top, mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_t_finds_the_outer_circle() {
        // Concentric: t is the distance over the radius growth.
        let t = radial_t(5.0, 0.0, (0.0, 0.0, 0.0), (0.0, 0.0, 10.0)).unwrap();
        assert!((t - 0.5).abs() < 1e-9);
        // Same radii and centres: undefined.
        assert!(radial_t(1.0, 1.0, (0.0, 0.0, 2.0), (0.0, 0.0, 2.0)).is_none());
    }

    #[test]
    fn layers_merge_with_their_mode() {
        let mut dst = vec![0.0, 0.0, 1.0, 1.0];
        composite(&mut dst, &[1.0, 0.0, 0.0, 1.0], CompositeMode::SrcOver);
        assert_eq!(dst, [1.0, 0.0, 0.0, 1.0]);
        let mut dst = vec![0.5, 0.5, 0.5, 1.0];
        composite(&mut dst, &[1.0, 0.0, 0.0, 1.0], CompositeMode::DestIn);
        assert_eq!(dst, [0.5, 0.5, 0.5, 1.0]);
        composite(&mut dst, &[0.0, 0.0, 0.0, 0.0], CompositeMode::SrcIn);
        assert_eq!(dst, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn inverse_undoes_a_transform() {
        let xf = Xform([2.0, 0.5, -0.25, 3.0, 7.0, -4.0]);
        let back = inverse(&xf).unwrap();
        let (x, y) = xf.apply(1.5, -2.0);
        let (bx, by) = back.apply(x, y);
        assert!((bx - 1.5).abs() < 1e-9 && (by + 2.0).abs() < 1e-9);
        assert!(inverse(&Xform([0.0; 6])).is_none());
    }
}
