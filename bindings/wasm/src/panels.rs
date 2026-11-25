//! Read-only data the editor's panels draw: layer and mask thumbnails for the Layers panel,
//! channel thumbnails for the Channels panel, the composite's histogram for the Histogram panel,
//! and the reduced composite for the Navigator panel. Locks, layer styles, mask states, folded effects, and the history step come with
//! [`kition_kit::KitDocument::inspect`].

use kition_photo_compose::Buffer;
use kition_photo_engine::doc::{ColorMode, Document, Effects, Fill, Layer, LayerContent, LayerId};

use crate::PhotoDocument;

/// The longest side a thumbnail may ask for.
const MAX_THUMBNAIL_SIDE: u32 = 1024;

/// The longest side of the reduced composite the histogram samples.
const HISTOGRAM_SIDE: u32 = 1024;

impl PhotoDocument {
    /// A thumbnail of layer `layer`'s own content laid out on the canvas, as Photoshop's Layers
    /// panel shows it: `[width u32 LE][height u32 LE]` then `width × height` straight RGBA pixels,
    /// the canvas's aspect with its longer side `max`. Pixel, type, shape, and object layers
    /// show their pixels (rendered first when their render cache is empty), fill layers their
    /// fill; groups and adjustment layers, which the panel shows as icons, give a transparent
    /// thumbnail. Opacity, masks, and layer styles are left out.
    pub fn layer_thumbnail(&self, layer: u32, max: u32) -> Result<Vec<u8>, String> {
        let doc = &self.state()?.doc;
        let found = doc.layer(LayerId(u64::from(layer))).ok_or_else(|| format!("no layer {layer}"))?;
        let (width, height) = (doc.size.width, doc.size.height);
        match &found.content {
            LayerContent::Group(_) | LayerContent::Adjustment(_) => Ok(thumbnail(width, height, max, |_, _| [0; 4])),
            LayerContent::Fill(_) => {
                let (w, h) = thumbnail_size(width, height, max);
                let mut alone = alone(doc, found);
                let k = proxy_factor(&alone, w, h);
                // Patterns tile in document pixels: shrink them with the proxy.
                if let Some(layer) = alone.layers.first_mut()
                    && let LayerContent::Fill(Fill::Pattern { scale, phase, .. }) = &mut layer.content
                    && k >= 2
                {
                    *scale /= k as f32;
                    *phase = (phase.0 / k as f32, phase.1 / k as f32);
                    layer.fill_cache = None;
                }
                let reduced = reduced(&alone, w, h);
                Ok(thumbnail(width, height, max, |x, y| sample(&reduced, width, height, x, y)))
            }
            _ => {
                let rendered;
                let surface = match found.surface() {
                    Some(surface) => surface,
                    None => {
                        rendered = render_cache(doc, found);
                        match rendered.as_ref().and_then(Layer::surface) {
                            Some(surface) => surface,
                            None => return Ok(thumbnail(width, height, max, |_, _| [0; 4])),
                        }
                    }
                };
                let mut px = [0.0f32; 8];
                Ok(thumbnail(width, height, max, |x, y| {
                    let n = surface.channels().min(px.len());
                    surface.read_pixel(x, y, &mut px[..n]);
                    kition_photo_raster::to_rgba(&surface.format(), &px[..n]).map(byte)
                }))
            }
        }
    }

    /// A thumbnail of layer `layer`'s mask in gray, encoded like [`Self::layer_thumbnail`]; an
    /// error when the layer has no layer mask.
    pub fn mask_thumbnail(&self, layer: u32, max: u32) -> Result<Vec<u8>, String> {
        let doc = &self.state()?.doc;
        let found = doc.layer(LayerId(u64::from(layer))).ok_or_else(|| format!("no layer {layer}"))?;
        let mask = found.mask.as_ref().ok_or_else(|| format!("layer {layer} has no layer mask"))?;
        let mut value = [0.0f32; 1];
        Ok(thumbnail(doc.size.width, doc.size.height, max, |x, y| {
            mask.surface.read_pixel(x, y, &mut value);
            let v = byte(value[0]);
            [v, v, v, 255]
        }))
    }

    /// The Channels panel's thumbnails, one call per change: `[count u32 LE]`, then `count`
    /// thumbnails each encoded like [`Self::layer_thumbnail`] (its size in front, so each knows
    /// its length), in the panel's order: the composite in color; each color channel of the
    /// document's mode in gray (a CMYK ink dark where it prints, as upstream draws them); each
    /// alpha or spot channel; and the Quick Mask, when the document is in Quick Mask mode. The
    /// composite is rendered once, reduced to the thumbnail size, for all of them.
    pub fn channel_thumbnails(&self, max: u32) -> Result<Vec<u8>, String> {
        let doc = &self.state()?.doc;
        let (width, height) = (doc.size.width, doc.size.height);
        let format = doc.pixel_format();
        let colors = format.mode.color_channels();
        let (w, h) = thumbnail_size(width, height, max);
        let composite = reduced(doc, w, h);
        let cmyk = format.mode == ColorMode::Cmyk;
        let alphas = doc.channels.iter().chain(doc.quick_mask.as_ref());
        let count = 1 + colors + doc.channels.len() + usize::from(doc.quick_mask.is_some());
        let mut out = Vec::with_capacity(4 + count * (8 + w as usize * h as usize * 4));
        out.extend_from_slice(&u32::try_from(count).unwrap_or(u32::MAX).to_le_bytes());
        out.extend(thumbnail(width, height, max, |x, y| sample(&composite, width, height, x, y)));
        for k in 0..colors {
            // The color channels come from the 8-bit sRGB composite converted back to the
            // document's mode: exact for RGB and Gray, an approximation for CMYK and Lab (a
            // thumbnail's worth, as upstream's).
            out.extend(thumbnail(width, height, max, |x, y| {
                let p = sample(&composite, width, height, x, y);
                let rgba = [p[0], p[1], p[2], 255].map(|v| f32::from(v) / 255.0);
                let v = kition_photo_raster::from_rgba(&format, rgba).get(k).copied().unwrap_or(0.0);
                let g = byte(if cmyk { 1.0 - v } else { v });
                [g, g, g, 255]
            }));
        }
        for alpha in alphas {
            out.extend(thumbnail(width, height, max, |x, y| {
                let v = byte(alpha.surface.sample_channel(x, y, 0));
                [v, v, v, 255]
            }));
        }
        Ok(out)
    }

    /// The Navigator panel's image of the whole composite, encoded like [`Self::layer_thumbnail`],
    /// its longer side `max` (1–1024) but never more than the image's own: rendered once, reduced,
    /// from a copy downsampled by a whole factor where that is faithful (as the channel thumbnails
    /// and the histogram are), so a large image costs about as much as the thumbnail rather than
    /// a composite at full resolution.
    pub fn navigator_image(&self, max: u32) -> Result<Vec<u8>, String> {
        let doc = &self.state()?.doc;
        let (width, height) = (doc.size.width.max(1), doc.size.height.max(1));
        let side = max.min(width.max(height));
        let (w, h) = thumbnail_size(width, height, side);
        let composite = reduced(doc, w, h);
        Ok(thumbnail(width, height, side, |x, y| sample(&composite, width, height, x, y)))
    }

    /// The commands the editor aims at a targeted layer mask (`"target": "mask"`, the engine's
    /// [`kition_photo_engine::channel_cmds::follows_target`]), and those of them that cannot run so
    /// aimed now (a layer without a mask, say), as JSON `{follows: [id], disabled: [id]}`: the
    /// editor's menus and keys check these while the mask is the target, as upstream does (#780).
    pub fn mask_target_commands(&self) -> serde_json::Value {
        let target = serde_json::json!({ "target": "mask" });
        let follows: Vec<&str> =
            kition_photo_engine::command_specs().iter().map(|spec| spec.id).filter(|id| kition_photo_engine::channel_cmds::follows_target(id)).collect();
        let disabled: Vec<&str> = follows.iter().copied().filter(|id| !self.session.is_enabled_with(id, &target)).collect();
        serde_json::json!({ "follows": follows, "disabled": disabled })
    }

    /// The composite's 256-bin histogram of `channel`: `rgb` (the mean of red, green, and blue),
    /// `red`, `green`, `blue`, or `luminosity` (0.30 R + 0.59 G + 0.11 B). It is sampled: a
    /// document more than 1024 pixels on its longer side is reduced by the smallest whole factor
    /// that brings it to at most 1024 (rendered from a downsampled proxy where that is faithful,
    /// that is, without layer styles or vector masks), so the bins sum to the reduced image's
    /// pixel count. Fully transparent pixels are not counted.
    pub fn histogram(&self, channel: &str) -> Result<Vec<u32>, String> {
        let pick: fn([u32; 3]) -> u32 = match channel {
            "rgb" => |[r, g, b]| (r + g + b) / 3,
            "red" => |[r, _, _]| r,
            "green" => |[_, g, _]| g,
            "blue" => |[_, _, b]| b,
            "luminosity" => |[r, g, b]| (r * 30 + g * 59 + b * 11 + 50) / 100,
            other => return Err(format!("unknown histogram channel `{other}` (rgb, red, green, blue, luminosity)")),
        };
        let doc = &self.state()?.doc;
        let (width, height) = (doc.size.width.max(1), doc.size.height.max(1));
        let k = width.max(height).div_ceil(HISTOGRAM_SIDE);
        let composite = if k >= 2 && kition_photo_compose::proxy::proxy_faithful(doc) {
            // A proxy at most 1024 pixels on its longer side, composited at its own size.
            let proxy = kition_photo_compose::proxy::proxy_document(doc, k);
            kition_photo_compose::render_reduced(&proxy, proxy.size.width, proxy.size.height)
        } else {
            kition_photo_compose::render_reduced(doc, width.div_ceil(k), height.div_ceil(k))
        };
        let mut bins = vec![0u32; 256];
        for p in composite.px.iter().filter(|p| p[3] > 0.0) {
            let rgb = [p[0], p[1], p[2]].map(|v| u32::from(byte(v)));
            if let Some(bin) = bins.get_mut(pick(rgb).min(255) as usize) {
                *bin = bin.saturating_add(1);
            }
        }
        Ok(bins)
    }
}

/// `doc` with `layer` as its only layer, shown plainly: full opacity, no clipping, mask, or
/// layer style, so its content renders on its own.
fn alone(doc: &Document, layer: &Layer) -> Document {
    let mut alone = Document { layers: Vec::new(), ..doc.clone() };
    alone.selection = None;
    alone.layers.push(Layer {
        visible: true,
        opacity: 1.0,
        fill_opacity: 1.0,
        clipped: false,
        mask: None,
        vector_mask: None,
        effects: Effects::default(),
        ..layer.clone()
    });
    alone
}

/// A copy of a type, shape, or object layer with its render cache filled in, as the
/// engine renders it after an edit; `None` when it cannot render (an object layer whose source
/// is missing).
fn render_cache(doc: &Document, layer: &Layer) -> Option<Layer> {
    let mut copy = layer.clone();
    match &mut copy.content {
        LayerContent::Text(t) => kition_photo_engine::type_cmds::refresh(doc, t),
        LayerContent::Shape(sh) => kition_photo_engine::vector_cmds::refresh_shape(doc, sh),
        LayerContent::Smart(_) => {
            kition_photo_engine::object_layer_cmds::refresh_layer(doc, &mut copy).ok()?;
        }
        _ => return None,
    }
    Some(copy)
}

/// The whole factor [`reduced`] shrinks `doc` by for a `w` × `h` render (1 when it renders
/// the document itself).
fn proxy_factor(doc: &Document, w: u32, h: u32) -> u32 {
    let k = doc.size.width.max(doc.size.height) / w.max(h).max(1);
    if k >= 2 && kition_photo_compose::proxy::proxy_faithful(doc) { k } else { 1 }
}

/// `doc`'s composite reduced to `w` × `h` (at most its size), rendered from a copy downsampled by
/// [`proxy_factor`] when that is faithful (no layer styles or vector masks), so a large document
/// costs about as much as the reduced image.
fn reduced(doc: &Document, w: u32, h: u32) -> Buffer {
    match proxy_factor(doc, w, h) {
        1 => kition_photo_compose::render_reduced(doc, w, h),
        k => kition_photo_compose::render_reduced(&kition_photo_compose::proxy::proxy_document(doc, k), w, h),
    }
}

/// The pixel of `buffer` (a reduced render of a `width` × `height` canvas) at canvas `(x, y)`.
fn sample(buffer: &Buffer, width: u32, height: u32, x: i32, y: i32) -> [u8; 4] {
    let (bw, bh) = (buffer.rect.width().max(1), buffer.rect.height().max(1));
    let bx = (u64::from(x.max(0) as u32) * u64::from(bw) / u64::from(width.max(1))).min(u64::from(bw - 1)) as usize;
    let by = (u64::from(y.max(0) as u32) * u64::from(bh) / u64::from(height.max(1))).min(u64::from(bh - 1)) as usize;
    buffer.px.get(by * bw as usize + bx).map_or([0; 4], |p| p.map(byte))
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// The size of a thumbnail of a `width` × `height` canvas whose longer side is `max` (clamped to
/// 1–1024).
fn thumbnail_size(width: u32, height: u32, max: u32) -> (u32, u32) {
    let max = max.clamp(1, MAX_THUMBNAIL_SIDE);
    let (cw, ch) = (width.max(1), height.max(1));
    let scale = f64::from(cw.max(ch)) / f64::from(max);
    (((f64::from(cw) / scale).round() as u32).clamp(1, max), ((f64::from(ch) / scale).round() as u32).clamp(1, max))
}

/// A thumbnail of a `width` × `height` canvas (see [`thumbnail_size`]), sampling `pixel(x, y)` at
/// each thumbnail pixel's center in canvas coordinates, with its size in front.
fn thumbnail(width: u32, height: u32, max: u32, mut pixel: impl FnMut(i32, i32) -> [u8; 4]) -> Vec<u8> {
    let (tw, th) = thumbnail_size(width, height, max);
    let (sx, sy) = (f64::from(width.max(1)) / f64::from(tw), f64::from(height.max(1)) / f64::from(th));
    let mut out = Vec::with_capacity(8 + tw as usize * th as usize * 4);
    out.extend_from_slice(&tw.to_le_bytes());
    out.extend_from_slice(&th.to_le_bytes());
    for ty in 0..th {
        let y = ((f64::from(ty) + 0.5) * sy) as i32;
        for tx in 0..tw {
            let x = ((f64::from(tx) + 0.5) * sx) as i32;
            out.extend_from_slice(&pixel(x, y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use kition_kit::KitDocument;
    use serde_json::{Value, json};

    fn size(bytes: &[u8]) -> (u32, u32) {
        (u32::from_le_bytes(bytes[0..4].try_into().unwrap()), u32::from_le_bytes(bytes[4..8].try_into().unwrap()))
    }

    /// A 200 × 100 white image with a painted layer "Ink" on top; returns it and Ink's id.
    fn painted() -> (PhotoDocument, u32) {
        let mut doc = PhotoDocument::create("new.psd", 200, 100).unwrap();
        doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap();
        doc.execute("paint.stroke", json!({ "points": [[10, 10, 1], [190, 90, 1]], "size": 8, "color": "#ff0000" })).unwrap();
        let id = doc.inspect().unwrap()["activeLayer"].as_u64().unwrap() as u32;
        (doc, id)
    }

    fn top_layer(doc: &PhotoDocument) -> Value {
        doc.inspect().unwrap()["layers"][0].clone()
    }

    #[test]
    fn thumbnails_a_layer_at_the_canvas_aspect() {
        let (doc, ink) = painted();
        let thumb = doc.layer_thumbnail(ink, 64).unwrap();
        let (w, h) = size(&thumb);
        assert_eq!((w, h), (64, 32));
        assert_eq!(thumb.len(), 8 + 64 * 32 * 4);
        let pixels = &thumb[8..];
        assert!(pixels.chunks(4).any(|p| p == [255, 0, 0, 255]), "the stroke shows");
        assert!(pixels.chunks(4).any(|p| p[3] == 0), "the rest of Ink is transparent");
        assert!(doc.layer_thumbnail(999_999, 64).is_err());
        let tall = PhotoDocument::create("tall.psd", 30, 90).unwrap();
        let background = tall.inspect().unwrap()["layers"][0]["id"].as_u64().unwrap() as u32;
        assert_eq!(size(&tall.layer_thumbnail(background, 64).unwrap()), (21, 64));
    }

    fn active(doc: &PhotoDocument) -> u32 {
        doc.inspect().unwrap()["activeLayer"].as_u64().unwrap() as u32
    }

    fn pixels(thumb: &[u8]) -> impl Iterator<Item = &[u8]> {
        thumb[8..].chunks(4)
    }

    #[test]
    fn thumbnails_fill_layers_from_their_fill() {
        let mut doc = PhotoDocument::create("new.psd", 200, 100).unwrap();
        doc.execute("layer.newFillLayer.solidColor", json!({ "color": "#00ff00" })).unwrap();
        let solid = doc.layer_thumbnail(active(&doc), 64).unwrap();
        assert_eq!(size(&solid), (64, 32));
        assert!(pixels(&solid).all(|p| p == [0, 255, 0, 255]), "a solid fill is opaque in its color");
        doc.execute("layer.newFillLayer.gradient", json!({ "from": "#000000", "to": "#ffffff", "angle": 0 })).unwrap();
        let gradient = doc.layer_thumbnail(active(&doc), 64).unwrap();
        assert!(pixels(&gradient).all(|p| p[3] == 255));
        assert!(pixels(&gradient).any(|p| p[0] < 64) && pixels(&gradient).any(|p| p[0] > 192), "the gradient runs dark to light");
        doc.execute("layer.newFillLayer.pattern", json!({ "pattern": "Checkerboard" })).unwrap();
        let mean = |thumb: Vec<u8>| {
            let n = (thumb.len() - 8) / 4;
            pixels(&thumb).map(|p| u64::from(p[0]) * u64::from(p[3]) / 255).sum::<u64>() / n as u64
        };
        let (small, exact) = (doc.layer_thumbnail(active(&doc), 64).unwrap(), doc.layer_thumbnail(active(&doc), 200).unwrap());
        assert!(pixels(&small).any(|p| p[3] > 0), "a pattern fill shows");
        assert!(mean(small).abs_diff(mean(exact)) < 24, "a reduced pattern keeps its tone");
    }

    #[test]
    fn thumbnails_type_and_shape_layers_with_or_without_their_render_cache() {
        let mut doc = PhotoDocument::create("new.psd", 200, 100).unwrap();
        doc.execute("text.add", json!({ "x": 10, "y": 70, "text": "Kition", "size": 60, "color": "#000000" })).unwrap();
        let text = active(&doc);
        doc.execute("shape.add", json!({ "kind": "rect", "rect": [120, 20, 60, 60], "fill": "#0000ff" })).unwrap();
        let shape = active(&doc);
        for id in [text, shape] {
            assert!(pixels(&doc.layer_thumbnail(id, 64).unwrap()).any(|p| p[3] > 0), "layer {id} shows at once");
        }
        // A layer whose render cache is not populated (as after some imports) still draws.
        let state = doc.session.active_mut().unwrap();
        let tree = std::sync::Arc::make_mut(&mut state.doc);
        for id in [text, shape] {
            match &mut tree.layer_mut(kition_photo_engine::doc::LayerId(u64::from(id))).unwrap().content {
                kition_photo_engine::doc::LayerContent::Text(t) => t.cache = None,
                kition_photo_engine::doc::LayerContent::Shape(sh) => sh.cache = None,
                other => panic!("not a type or shape layer: {}", other.kind_name()),
            }
        }
        for id in [text, shape] {
            assert!(pixels(&doc.layer_thumbnail(id, 64).unwrap()).any(|p| p[3] > 0), "layer {id} shows without its cache");
        }
        let thumb = doc.layer_thumbnail(shape, 200).unwrap();
        assert_eq!(&thumb[8 + (50 * 200 + 150) * 4..][..4], &[0, 0, 255, 255], "the rectangle is blue");
    }

    #[test]
    fn leaves_groups_and_adjustments_to_the_panel_s_icons() {
        let mut doc = PhotoDocument::create("new.psd", 200, 100).unwrap();
        doc.execute("layer.new.group", json!({})).unwrap();
        let group = active(&doc);
        doc.execute("layer.newAdjustmentLayer.curves", json!({})).unwrap();
        let adjustment = active(&doc);
        for id in [group, adjustment] {
            let thumb = doc.layer_thumbnail(id, 64).unwrap();
            assert_eq!(size(&thumb), (64, 32));
            assert!(pixels(&thumb).all(|p| p[3] == 0), "layer {id} is transparent");
        }
    }

    #[test]
    fn thumbnails_a_mask_once_there_is_one() {
        let (mut doc, ink) = painted();
        assert!(doc.mask_thumbnail(ink, 64).is_err());
        doc.execute("layer.layerMask.revealAll", json!({})).unwrap();
        let thumb = doc.mask_thumbnail(ink, 64).unwrap();
        assert_eq!(size(&thumb), (64, 32));
        assert!(thumb[8..].chunks(4).all(|p| p == [255, 255, 255, 255]), "reveal all is white");
        assert_eq!(top_layer(&doc)["maskEnabled"], true);
        doc.execute("layer.layerMask.enabled", json!({ "layer": ink })).unwrap();
        assert_eq!(top_layer(&doc)["maskEnabled"], false, "the command toggles it");
        assert!(doc.mask_thumbnail(999_999, 64).is_err());
    }

    /// `channel_thumbnails`'s thumbnails, split.
    fn channels(doc: &PhotoDocument) -> Vec<Vec<u8>> {
        let bytes = doc.channel_thumbnails(64).unwrap();
        let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let mut at = 4;
        let mut out = Vec::new();
        for _ in 0..count {
            let (w, h) = size(&bytes[at..]);
            let end = at + 8 + (w * h * 4) as usize;
            out.push(bytes[at..end].to_vec());
            at = end;
        }
        assert_eq!(at, bytes.len(), "nothing after the last thumbnail");
        out
    }

    #[test]
    fn thumbnails_channels() {
        let (mut doc, _) = painted();
        let thumbs = channels(&doc);
        assert_eq!(thumbs.len(), 4, "the composite and red, green, blue");
        assert_eq!(size(&thumbs[0]), (64, 32));
        assert!(pixels(&thumbs[0]).any(|p| p == [255, 0, 0, 255]), "the red stroke shows");
        assert!(pixels(&thumbs[2]).any(|p| p == [0, 0, 0, 255]) && pixels(&thumbs[2]).any(|p| p == [255, 255, 255, 255]), "red has no green");
        doc.execute("select.rect", json!({ "x": 0, "y": 0, "width": 100, "height": 100 })).unwrap();
        doc.execute("select.saveSelection", json!({})).unwrap();
        let thumbs = channels(&doc);
        assert_eq!(thumbs.len(), 5, "and the alpha channel");
        assert_eq!(&thumbs[4][8..12], &[255, 255, 255, 255], "selected is white");
        assert_eq!(&thumbs[4][thumbs[4].len() - 4..], &[0, 0, 0, 255], "the rest is black");
        doc.execute("select.editInQuickMaskMode", json!({ "on": true })).unwrap();
        let thumbs = channels(&doc);
        assert_eq!(thumbs.len(), 6, "and the Quick Mask");
        assert!(pixels(&thumbs[5]).any(|p| p == [255, 255, 255, 255]), "the selection shows in the Quick Mask: {:?}", &thumbs[5][8..12]);
        assert_ne!(thumbs[5], thumbs[0]);
    }

    #[test]
    fn thumbnails_cmyk_and_lab_channels() {
        let (mut doc, _) = painted();
        doc.execute("image.mode.cmyk", json!({})).unwrap();
        let thumbs = channels(&doc);
        assert_eq!(thumbs.len(), 5, "the composite and cyan, magenta, yellow, black");
        // White paper takes no ink (light); the red stroke takes magenta and yellow, little cyan.
        let at = |thumb: &[u8], x: usize, y: usize| thumb[8 + (y * 64 + x) * 4];
        let (paper, ink) = ((2, 30), (32, 16));
        for (k, ink) in thumbs.iter().enumerate().skip(1) {
            assert!(at(ink, paper.0, paper.1) > 240, "channel {k} is light on white paper");
        }
        assert!(at(&thumbs[1], ink.0, ink.1) > 128, "little cyan in red");
        assert!(at(&thumbs[2], ink.0, ink.1) < 128 && at(&thumbs[3], ink.0, ink.1) < 128, "magenta and yellow print red");
        doc.execute("edit.undo", json!({})).unwrap();
        doc.execute("image.mode.lab", json!({})).unwrap();
        let thumbs = channels(&doc);
        assert_eq!(thumbs.len(), 4, "the composite and L, a, b");
        assert!(at(&thumbs[1], paper.0, paper.1) > 240, "white is full lightness");
        assert!(at(&thumbs[2], ink.0, ink.1) > at(&thumbs[2], paper.0, paper.1), "red sits on the a axis's positive side");
    }

    #[test]
    fn names_the_commands_a_mask_target_aims() {
        let (mut doc, _) = painted();
        let answer = doc.mask_target_commands();
        let list = |key: &str| answer[key].as_array().unwrap().iter().map(|id| id.as_str().unwrap().to_owned()).collect::<Vec<_>>();
        let follows = list("follows");
        for id in ["paint.stroke", "edit.fill", "edit.paste", "image.adjustments.invert", "filter.blur.gaussianBlur"] {
            assert!(follows.iter().any(|f| f == id), "{id} follows the target");
        }
        assert!(!follows.iter().any(|f| f == "layer.new.layer"));
        assert!(list("disabled").iter().any(|id| id == "image.adjustments.invert"), "no mask yet");
        doc.execute("layer.layerMask.revealAll", json!({})).unwrap();
        let answer = doc.mask_target_commands();
        assert!(!answer["disabled"].as_array().unwrap().iter().any(|id| id == "image.adjustments.invert"), "with a mask, Invert can edit it");
    }

    #[test]
    fn counts_the_composite_per_channel() {
        let (doc, _) = painted();
        for channel in ["rgb", "red", "green", "blue", "luminosity"] {
            let bins = doc.histogram(channel).unwrap();
            assert_eq!(bins.len(), 256, "{channel}");
            assert_eq!(bins.iter().map(|&n| u64::from(n)).sum::<u64>(), 200 * 100, "{channel}");
        }
        let red = doc.histogram("red").unwrap();
        assert_eq!(red[255], 200 * 100, "white and red both have full red");
        let green = doc.histogram("green").unwrap();
        assert!(green[0] > 0 && green[255] > 0);
        assert!(doc.histogram("alpha").is_err());
    }

    #[test]
    fn draws_the_navigator_image_from_a_reduced_render() {
        let (doc, _) = painted();
        let image = doc.navigator_image(64).unwrap();
        assert_eq!(size(&image), (64, 32));
        assert_eq!(image.len(), 8 + 64 * 32 * 4);
        assert!(pixels(&image).any(|p| p == [255, 0, 0, 255]), "the red stroke shows");
        assert!(pixels(&image).any(|p| p == [255, 255, 255, 255]), "the white background shows");
        // Never larger than the image itself, and at most 1024 on its longer side.
        assert_eq!(size(&doc.navigator_image(4000).unwrap()), (200, 100));
        let wide = PhotoDocument::create("wide.psd", 3000, 1500).unwrap();
        let image = wide.navigator_image(5000).unwrap();
        assert_eq!(size(&image), (1024, 512));
        assert!(pixels(&image).all(|p| p == [255, 255, 255, 255]), "all white");
        assert_eq!(size(&wide.navigator_image(0).unwrap()), (1, 1));
    }

    #[test]
    fn the_navigator_image_matches_the_full_render() {
        let mut doc = PhotoDocument::create("big.psd", 1200, 800).unwrap();
        doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap();
        doc.execute("paint.stroke", json!({ "points": [[0, 400, 1], [1200, 400, 1]], "size": 200, "color": "#0000ff" })).unwrap();
        let image = doc.navigator_image(120).unwrap();
        assert_eq!(size(&image), (120, 80));
        let at = |x: usize, y: usize| &image[8 + (y * 120 + x) * 4..][..4];
        assert_eq!(at(60, 40), [0, 0, 255, 255], "the band across the middle is blue");
        assert_eq!(at(60, 2), [255, 255, 255, 255], "above it is white");
    }

    #[test]
    fn samples_large_images_for_the_histogram() {
        let doc = PhotoDocument::create("wide.psd", 3000, 1500).unwrap();
        let bins = doc.histogram("rgb").unwrap();
        assert_eq!(bins.iter().map(|&n| u64::from(n)).sum::<u64>(), 1000 * 500, "a render reduced by 3, at most 1024 wide");
        assert_eq!(bins[255], 1000 * 500, "all white");
    }

    /// Timing for the report: `cargo test --release -p kition-photo-kit -- --ignored --nocapture
    /// histogram_timing`.
    #[test]
    #[ignore = "timing only"]
    fn histogram_timing() {
        let mut doc = PhotoDocument::create("big.psd", 4000, 3000).unwrap();
        doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap();
        doc.execute("paint.stroke", json!({ "points": [[100, 100, 1], [3900, 2900, 1]], "size": 200, "color": "#ff0000" })).unwrap();
        let ink = active(&doc);
        let start = std::time::Instant::now();
        doc.histogram("rgb").unwrap();
        let histogram = start.elapsed();
        let start = std::time::Instant::now();
        doc.layer_thumbnail(ink, 64).unwrap();
        let thumbnail = start.elapsed();
        doc.execute("layer.newFillLayer.gradient", json!({ "from": "#000000", "to": "#ffffff", "angle": 0 })).unwrap();
        let start = std::time::Instant::now();
        doc.layer_thumbnail(active(&doc), 64).unwrap();
        let fill = start.elapsed();
        doc.execute("layer.newFillLayer.pattern", json!({ "pattern": "Checkerboard" })).unwrap();
        let start = std::time::Instant::now();
        doc.layer_thumbnail(active(&doc), 64).unwrap();
        let pattern = start.elapsed();
        println!(
            "4000x3000: histogram {histogram:?}, pixel layer thumbnail {thumbnail:?}, gradient fill thumbnail {fill:?}, pattern fill thumbnail {pattern:?}"
        );
    }

    #[test]
    fn inspects_locks_and_layer_styles() {
        let (mut doc, ink) = painted();
        assert_eq!(top_layer(&doc)["locks"], json!({ "transparency": false, "pixels": false, "position": false, "artboard": false, "all": false }));
        doc.execute("layer.setProps", json!({ "layer": ink, "locks": { "position": true } })).unwrap();
        assert_eq!(top_layer(&doc)["locks"]["position"], true);
        doc.execute("layer.layerStyle.dropShadow", json!({ "add": true })).unwrap();
        let effects = &top_layer(&doc)["effects"];
        assert_eq!(effects["items"][0]["key"], "dropShadow", "{effects}");
        assert_eq!(effects["items"][0]["kind"], "Drop Shadow");
        assert_eq!(effects["items"][0]["enabled"], true);
    }

    #[test]
    fn inspects_folded_effects() {
        let (mut doc, ink) = painted();
        doc.execute("layer.layerStyle.dropShadow", json!({ "add": true })).unwrap();
        assert_eq!(doc.inspect().unwrap()["effectsCollapsed"], json!([]));
        doc.execute("layer.setEffectsExpanded", json!({ "layer": ink, "expanded": false })).unwrap();
        assert_eq!(doc.inspect().unwrap()["effectsCollapsed"], json!([ink]));
    }

    #[test]
    fn inspects_the_history_step() {
        let mut doc = PhotoDocument::create("new.psd", 20, 10).unwrap();
        assert_eq!(doc.inspect().unwrap()["historyIndex"], 0);
        doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap();
        assert_eq!(doc.inspect().unwrap()["historyIndex"], 1);
        doc.execute("edit.undo", json!({})).unwrap();
        assert_eq!(doc.inspect().unwrap()["historyIndex"], 0);
        assert_eq!(doc.inspect().unwrap()["historyRedo"], json!(["New Layer"]), "the states Redo steps through");
    }

    #[test]
    fn inspects_an_adjustment_layers_settings() {
        let mut doc = PhotoDocument::create("new.psd", 20, 10).unwrap();
        doc.execute("layer.newAdjustmentLayer.posterize", json!({ "levels": 6 })).unwrap();
        let layer = top_layer(&doc);
        assert_eq!(layer["adjustmentKind"], "posterize");
        assert_eq!(layer["adjustmentParams"], json!({ "levels": 6 }));
    }
}
