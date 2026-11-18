//! The agent's preview and measurements (task card C-photo): `file.render` draws an explicit view
//! of the image, never the editor's, and `image.measure` answers what can be told exactly beside
//! the pixels.
//!
//! **`file.render`** (kit.json `agent.render`): `{target?: {layer?, layers?, objectId?, region?},
//! scale?, maxSize?, background?}`. Without a target it draws the composite of the whole canvas as
//! the file shows it. With layers (`layer`, `layers`, or `objectId`, a layer's id in the one id
//! space) it draws only those layers, with what they hold, as they look in the image (their
//! opacity, blending, masks and effects, over the layers' own transparency), each shown even when
//! hidden; framed to `region` (`[x, y, width, height]` in image pixels) when given, else to the
//! layers' non-transparent bounds, else the canvas. `scale` (image pixels to output pixels, up to
//! 2) is lowered so the longer side fits `maxSize` (4096 at most, 1024 by default). It reads no
//! editor state and changes nothing; the same document and call answer the same bytes. It
//! answers `png` (base64), `width`, `height`, `scale` (output pixels per image pixel, per axis
//! as drawn), `region` (the image area drawn), the mapping `x = region[0] + px / scale[0]` (and
//! for y), and `drawn` (`layers`: the ids drawn, or `composite: true`).
//!
//! **`image.measure`**: `{layer?, region?, with?}`: over `region` (default: the canvas), the
//! non-transparent `bounds` of the layer's own pixels (or of the composite without `layer`), how
//! many pixels are not fully transparent, their mean color (straight RGBA, 0..255) and a 256-bin
//! luminosity histogram of them, and with `with` (another layer) the rectangle and the count of
//! pixels both cover (`overlap`). Exact on 8-bit values of the layers as drawn alone.

use std::collections::BTreeSet;

use kition_kit::KitCommand;
use kition_photo_engine::doc::{Document, LayerContent, LayerId};
use kition_photo_engine::inspect::xywh;
use kition_photo_geom::Rect;
use serde_json::{Value, json};

use crate::PhotoDocument;
use crate::context::{disabled, invalid};

pub const RENDER: &str = "file.render";
pub const MEASURE: &str = "image.measure";

/// The most pixels `image.measure` reads at once.
const MEASURE_LIMIT: u64 = 64 * 1024 * 1024;

/// The largest scale and output side `file.render` draws.
const MAX_SCALE: f64 = 2.0;
const MAX_SIDE: u32 = 4096;
const DEFAULT_SIDE: u32 = 1024;

/// The two commands, for [`kition_kit::KitDocument::commands`].
pub fn commands(enabled: bool) -> Vec<KitCommand> {
    let mut render = KitCommand {
        id: RENDER.to_owned(),
        title: "Render View".to_owned(),
        description: "Draws an explicit view of the image as a PNG, without the editor's state: the composite, or only the given layers, framed to a region; answers how its pixels map to image coordinates and what it drew. Changes nothing.".to_owned(),
        menu: None,
        params: Some(r##"{"target":{"layer":id|"#n"?,"layers":[id,…]?,"objectId":id?,"region":[x,y,w,h]?}?,"scale":0..2=1,"maxSize":1..4096=1024,"background":"transparent|white|#rrggbb"="transparent"} → {png, width, height, scale, region, mapping, drawn}"##.to_owned()),
        mutates: false,
        enabled,
        ..Default::default()
    }
    .with_kit_schema();
    if let Some(Value::Object(schema)) = render.schema.as_mut() {
        schema.insert(
            "target".to_owned(),
            json!({
                "type": "object",
                "additionalProperties": false,
                "description": "The view: `layer` (an id, or \"#n\", the n-th of the top level from the top), `layers` (ids), or `objectId` (a layer id) to draw only those layers; `region` [x, y, width, height] in image pixels to frame it.",
                "properties": {
                    "layer": { "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": "One layer, by id or \"#n\"." },
                    "layers": { "type": "array", "items": { "type": "integer" }, "description": "Layers, by id." },
                    "objectId": { "oneOf": [{ "type": "integer" }, { "type": "string" }], "description": "A layer's id." },
                    "region": { "type": "array", "items": { "type": "number" }, "description": "[x, y, width, height] in image pixels." },
                },
            }),
        );
    }
    render.output_schema = None;
    let mut measure = KitCommand {
        id: MEASURE.to_owned(),
        title: "Measure".to_owned(),
        description: "Answers what can be told exactly beside the pixels: a layer's (or the composite's) non-transparent bounds, pixel count, mean color and luminosity histogram inside a region, and how much it overlaps another layer. Changes nothing.".to_owned(),
        menu: None,
        params: Some(r#"{"layer":id?,"region":[x,y,w,h]?,"with":id?} → {bounds, pixels, mean, histogram, overlap?}"#.to_owned()),
        mutates: false,
        enabled,
        objects: &["layer", "with"],
        ..Default::default()
    }
    .with_kit_schema();
    if let Some(schema) = measure.schema.as_mut().and_then(Value::as_object_mut) {
        for key in ["layer", "with"] {
            schema.insert(
                key.to_owned(),
                json!({"oneOf":[{"type":"integer"},{"type":"string"}],"description":"A layer's id, whole unique name, or #n from the top."}),
            );
        }
    }
    if let Some(items) = render
        .schema
        .as_mut()
        .and_then(|s| s.get_mut("target"))
        .and_then(|s| s.get_mut("properties"))
        .and_then(|s| s.get_mut("layers"))
        .and_then(|s| s.get_mut("items"))
    {
        *items = json!({"oneOf":[{"type":"integer"},{"type":"string"}]});
    }
    vec![render, measure]
}

/// `[x, y, width, height]` as a rectangle, width and height above 0.
fn rect_of(value: &Value, what: &str) -> Result<Rect, String> {
    let numbers: Option<Vec<i64>> = value
        .as_array()
        .filter(|items| items.len() == 4)
        .and_then(|items| items.iter().map(|item| item.as_f64().filter(|n| n.is_finite() && n.abs() < 1e8).map(|n| n.round() as i64)).collect());
    match numbers.as_deref() {
        Some([x, y, w, h]) if *w > 0 && *h > 0 => {
            let (x, y) = (i32::try_from(*x).unwrap_or(0), i32::try_from(*y).unwrap_or(0));
            Ok(Rect::from_xywh(x, y, u32::try_from(*w).unwrap_or(1), u32::try_from(*h).unwrap_or(1)))
        }
        _ => Err(invalid(format!("`{what}` is [x, y, width, height] in image pixels, width and height above 0, not {value}"))),
    }
}

/// A layer a view or a measurement names: an id, or `"#n"`.
fn layer_named(doc: &Document, value: &Value, what: &str) -> Result<LayerId, String> {
    crate::place::layer_value(doc, value).map_err(|error| format!("`{what}`: {error}"))
}

/// `doc` drawing only `shown` (and what they hold), each shown even when hidden, and the groups
/// holding them; every other layer hidden.
fn only(doc: &Document, shown: &BTreeSet<LayerId>) -> Document {
    fn walk(layers: &mut [kition_photo_engine::doc::Layer], shown: &BTreeSet<LayerId>, inside: bool) -> bool {
        let mut any = false;
        for layer in layers {
            let this = inside || shown.contains(&layer.id);
            let held = match &mut layer.content {
                LayerContent::Group(group) => walk(&mut group.children, shown, this),
                _ => false,
            };
            layer.visible = this || held;
            any |= layer.visible;
        }
        any
    }
    let mut out = doc.clone();
    walk(&mut out.layers, shown, false);
    out
}

/// Straight 8-bit RGBA of `doc` over `area` at `width` x `height`.
fn draw(doc: &Document, area: Rect, width: u32, height: u32) -> Vec<u8> {
    if width == area.width() && height == area.height() {
        kition_photo_compose::render(doc, area).to_rgba8().pixels
    } else if width <= area.width() {
        kition_photo_compose::render_reduced_rect(doc, area, width, height).to_rgba8().pixels
    } else {
        // Enlarged: each image pixel repeated.
        let source = kition_photo_compose::render(doc, area).to_rgba8();
        let (sw, sh) = (source.width as usize, source.height as usize);
        let mut out = vec![0u8; width as usize * height as usize * 4];
        for row in 0..height as usize {
            let sy = (row * sh / height as usize).min(sh.saturating_sub(1));
            for col in 0..width as usize {
                let sx = (col * sw / width as usize).min(sw.saturating_sub(1));
                let (from, to) = ((sy * sw + sx) * 4, (row * width as usize + col) * 4);
                if let (Some(dst), Some(src)) = (out.get_mut(to..to + 4), source.pixels.get(from..from + 4)) {
                    dst.copy_from_slice(src);
                }
            }
        }
        out
    }
}

/// Straight RGBA over an opaque color.
fn flatten(pixels: &mut [u8], color: [u8; 3]) {
    for px in pixels.as_chunks_mut::<4>().0 {
        let a = u32::from(px[3]);
        for (c, back) in px.iter_mut().take(3).zip(color) {
            *c = ((u32::from(*c) * a + u32::from(back) * (255 - a) + 127) / 255) as u8;
        }
        px[3] = 255;
    }
}

fn png(pixels: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, String> {
    use kition_photo_codecs::{ChannelLayout, EncodeOptions, Format, Image, SampleType, encode};
    let image = Image::from_raw(width, height, ChannelLayout::Rgba, SampleType::U8, pixels).map_err(|error| error.to_string())?;
    encode(&image, Format::Png, &EncodeOptions::default()).map_err(|error| error.to_string())
}

/// The union of the non-transparent bounds of `ids` (and what they hold).
fn bounds_of(doc: &Document, ids: &BTreeSet<LayerId>) -> Rect {
    let mut out = Rect::EMPTY;
    for (_, _, layer) in doc.walk() {
        let held = doc.walk().iter().any(|(_, _, group)| ids.contains(&group.id) && crate::place::holds_layer(group, layer.id));
        if ids.contains(&layer.id) || held {
            if let Some(surface) = layer.surface() {
                out = out.union(&surface.content_bounds());
            } else if matches!(layer.content, LayerContent::Fill(_)) {
                out = out.union(&doc.bounds());
            }
        }
    }
    out
}

impl PhotoDocument {
    /// `file.render`: the module docs.
    pub(crate) fn render_view(&self, params: &Value) -> Result<Value, String> {
        let st = self.session.active().ok_or_else(|| disabled("no image is open"))?;
        let doc = &st.doc;
        let target = params.get("target").filter(|target| !target.is_null());
        if let Some(target) = target {
            let fields = target.as_object().ok_or_else(|| invalid(format!("`target` is {target}: an object of `layer`, `layers`, `objectId`, `region`")))?;
            if let Some(key) = fields.keys().find(|key| !matches!(key.as_str(), "layer" | "layers" | "objectId" | "region")) {
                return Err(invalid(format!("`target` has `{key}`: a view takes `layer`, `layers`, `objectId` and `region`")));
            }
        }
        let field = |key: &str| target.and_then(|target| target.get(key)).filter(|value| !value.is_null());
        let mut shown = BTreeSet::new();
        for key in ["layer", "objectId"] {
            if let Some(value) = field(key) {
                shown.insert(layer_named(doc, value, key)?);
            }
        }
        if let Some(list) = field("layers") {
            for value in list.as_array().ok_or_else(|| invalid(format!("`layers` is {list}: a list of layer ids")))? {
                shown.insert(layer_named(doc, value, "layers")?);
            }
        }
        let area = match field("region") {
            Some(region) => rect_of(region, "region")?,
            None if !shown.is_empty() => {
                let bounds = bounds_of(doc, &shown);
                if bounds.is_empty() { doc.bounds() } else { bounds }
            }
            None => doc.bounds(),
        };
        let scale = match params.get("scale").filter(|scale| !scale.is_null()) {
            None => 1.0,
            Some(scale) => scale
                .as_f64()
                .filter(|s| s.is_finite() && *s > 0.0 && *s <= MAX_SCALE)
                .ok_or_else(|| invalid(format!("`scale` is {scale}: above 0, at most {MAX_SCALE}")))?,
        };
        let max = match params.get("maxSize").filter(|max| !max.is_null()) {
            None => DEFAULT_SIDE,
            Some(max) => max
                .as_u64()
                .and_then(|m| u32::try_from(m).ok())
                .filter(|m| (1..=MAX_SIDE).contains(m))
                .ok_or_else(|| invalid(format!("`maxSize` is {max}: 1 to {MAX_SIDE}")))?,
        };
        let longer = f64::from(area.width().max(area.height()).max(1));
        let scale = scale.min(f64::from(max) / longer);
        let width = ((f64::from(area.width()) * scale).round() as u32).clamp(1, MAX_SIDE);
        let height = ((f64::from(area.height()) * scale).round() as u32).clamp(1, MAX_SIDE);
        let drawn_doc;
        let source = if shown.is_empty() {
            doc.as_ref()
        } else {
            drawn_doc = only(doc, &shown);
            &drawn_doc
        };
        let mut pixels = draw(source, area, width, height);
        let background = params.get("background").and_then(Value::as_str).unwrap_or("transparent");
        match background {
            "transparent" => {}
            "white" => flatten(&mut pixels, [255, 255, 255]),
            hex => {
                let parse = |at: usize| hex.trim_start_matches('#').get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
                match (hex.trim_start_matches('#').len(), parse(0), parse(2), parse(4)) {
                    (6, Some(r), Some(g), Some(b)) => flatten(&mut pixels, [r, g, b]),
                    _ => return Err(invalid(format!("`background` is \"{hex}\": \"transparent\", \"white\" or \"#rrggbb\""))),
                }
            }
        }
        let bytes = png(pixels, width, height)?;
        let (sx, sy) = (f64::from(width) / f64::from(area.width().max(1)), f64::from(height) / f64::from(area.height().max(1)));
        let drawn = if shown.is_empty() { json!({ "composite": true }) } else { json!({ "layers": shown.iter().map(|id| id.0).collect::<Vec<_>>() }) };
        Ok(json!({
            "png": crate::base64_encode(&bytes),
            "width": width,
            "height": height,
            "scale": [sx, sy],
            "region": xywh(area),
            "mapping": format!("image x = {} + px / {sx}, image y = {} + py / {sy}", area.x0, area.y0),
            "drawn": drawn,
            "background": background,
        }))
    }

    /// `image.measure`: the module docs.
    pub(crate) fn measure(&self, params: &Value) -> Result<Value, String> {
        let st = self.session.active().ok_or_else(|| disabled("no image is open"))?;
        let doc = &st.doc;
        let layer = params.get("layer").filter(|value| !value.is_null()).map(|value| layer_named(doc, value, "layer")).transpose()?;
        let with = params.get("with").filter(|value| !value.is_null()).map(|value| layer_named(doc, value, "with")).transpose()?;
        let area = match params.get("region").filter(|value| !value.is_null()) {
            Some(region) => rect_of(region, "region")?,
            None => doc.bounds(),
        };
        if u64::from(area.width()) * u64::from(area.height()) > MEASURE_LIMIT {
            return Err(invalid(format!("the region holds more than {MEASURE_LIMIT} pixels: measure a smaller one")));
        }
        let alone = |id: Option<LayerId>| -> Vec<u8> {
            match id {
                Some(id) => draw(&only(doc, &BTreeSet::from([id])), area, area.width(), area.height()),
                None => draw(doc, area, area.width(), area.height()),
            }
        };
        let pixels = alone(layer);
        let (w, h) = (area.width() as usize, area.height() as usize);
        let mut count = 0u64;
        let mut sums = [0u64; 4];
        let mut histogram = vec![0u64; 256];
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        for (index, px) in pixels.as_chunks::<4>().0.iter().enumerate() {
            if px[3] == 0 {
                continue;
            }
            let (x, y) = (index % w.max(1), index / w.max(1));
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            count += 1;
            for (sum, value) in sums.iter_mut().zip(px) {
                *sum += u64::from(*value);
            }
            let luminosity = (299 * u32::from(px[0]) + 587 * u32::from(px[1]) + 114 * u32::from(px[2]) + 500) / 1000;
            if let Some(bin) = histogram.get_mut(luminosity.min(255) as usize) {
                *bin += 1;
            }
        }
        let bounds = (count > 0).then(|| [area.x0 + x0 as i32, area.y0 + y0 as i32, (x1 - x0) as i32, (y1 - y0) as i32]);
        let mean = (count > 0).then(|| sums.map(|sum| (sum as f64 / count as f64 * 100.0).round() / 100.0));
        let mut answer = json!({
            "layer": layer.map(|id| id.0),
            "region": xywh(area),
            "bounds": bounds,
            "pixels": count,
            "mean": mean,
            "histogram": histogram,
        });
        if let Some(other) = with {
            let others = alone(Some(other));
            let mut both = 0u64;
            let (mut ox0, mut oy0, mut ox1, mut oy1) = (usize::MAX, usize::MAX, 0usize, 0usize);
            for (index, (a, b)) in pixels.as_chunks::<4>().0.iter().zip(others.as_chunks::<4>().0).enumerate() {
                if a[3] > 0 && b[3] > 0 {
                    let (x, y) = (index % w.max(1), index / w.max(1));
                    (ox0, oy0, ox1, oy1) = (ox0.min(x), oy0.min(y), ox1.max(x + 1), oy1.max(y + 1));
                    both += 1;
                }
            }
            let _ = h;
            answer["overlap"] = json!({
                "with": other.0,
                "pixels": both,
                "bounds": (both > 0).then(|| [area.x0 + ox0 as i32, area.y0 + oy0 as i32, (ox1 - ox0) as i32, (oy1 - oy0) as i32]),
            });
        }
        Ok(answer)
    }
}
