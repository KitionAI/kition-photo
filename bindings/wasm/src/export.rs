//! The image as a flat file with Export As's options (plan 2026-11-24, stage 6): format, quality,
//! size and transparency for `file.exportImage`, which the editor's Export dialog, Quick Export and
//! the agent's `<prefix>_export` call; and the engine's clipboard as a PNG, which the editor puts on
//! the system clipboard after Copy. Nothing here writes a file or changes the document.

use kition_photo_engine::Session;
use kition_photo_engine::doc::{ColorMode, Document, Layer, LayerContent, PixelFormat, Rect, Size, Surface};
use serde_json::{Value, json};

/// The formats `file.exportImage` writes, with their MIME types.
pub const FORMATS: [(&str, &str); 4] = [("png", "image/png"), ("jpg", "image/jpeg"), ("webp", "image/webp"), ("tiff", "image/tiff")];

/// The largest side an export may have, as Image Size's own limit keeps a resample bounded.
const MAX_SIDE: u32 = 30_000;

/// What `file.exportImage` was asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub format: &'static str,
    pub mime: &'static str,
    /// JPEG and lossy WebP quality, 1–100.
    pub quality: u8,
    /// WebP: lossless instead of `quality`.
    pub lossless: bool,
    /// The output size; `None` keeps the image's.
    pub size: Option<(u32, u32)>,
    /// Keep the transparent pixels (PNG, WebP, TIFF); off, or for JPEG, the image is flattened onto `matte`.
    pub transparency: bool,
    /// The colour under transparent pixels when they are not kept, straight RGB 0–1.
    pub matte: [f32; 3],
}

fn number(params: &Value, key: &str) -> Result<Option<f64>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_f64().filter(|v| v.is_finite()).map(Some).ok_or_else(|| format!("`{key}` must be a number")),
    }
}

fn hex_color(text: &str) -> Option<[f32; 3]> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |at: usize| hex.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok()).map(|v| f32::from(v) / 255.0);
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// Reads `file.exportImage`'s parameters for an image of `width` × `height`: a missing side keeps
/// the aspect ratio of the other.
pub fn options(params: &Value, width: u32, height: u32) -> Result<Options, String> {
    let asked = params.get("format").and_then(Value::as_str).unwrap_or("png");
    let asked = if asked == "jpeg" {
        "jpg"
    } else if asked == "tif" {
        "tiff"
    } else {
        asked
    };
    let (format, mime) =
        FORMATS.iter().copied().find(|(format, _)| *format == asked).ok_or_else(|| format!("Photo exports png, jpg, tiff or webp, not {asked}"))?;
    let quality = number(params, "quality")?.unwrap_or(if format == "webp" { 80.0 } else { 90.0 });
    if !(1.0..=100.0).contains(&quality) {
        return Err("`quality` must be from 1 to 100".to_owned());
    }
    let lossless = params.get("lossless").and_then(Value::as_bool).unwrap_or(false);
    let side = |key: &str| -> Result<Option<u32>, String> {
        match number(params, key)? {
            None => Ok(None),
            Some(v) if (1.0..=f64::from(MAX_SIDE)).contains(&v.round()) => Ok(Some(v.round() as u32)),
            Some(_) => Err(format!("`{key}` must be from 1 to {MAX_SIDE} pixels")),
        }
    };
    let (w, h) = (side("width")?, side("height")?);
    let (ow, oh) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let scaled = |v: f64| (v.round().max(1.0) as u32).min(MAX_SIDE);
    let size = match (w, h) {
        (None, None) => None,
        (Some(w), Some(h)) => Some((w, h)),
        (Some(w), None) => Some((w, scaled(f64::from(w) * oh / ow))),
        (None, Some(h)) => Some((scaled(f64::from(h) * ow / oh), h)),
    }
    .filter(|&(w, h)| (w, h) != (width, height));
    let transparency = format != "jpg" && params.get("transparency").and_then(Value::as_bool).unwrap_or(true);
    let matte = match params.get("matte") {
        None | Some(Value::Null) => [1.0; 3],
        Some(Value::String(text)) => hex_color(text).ok_or_else(|| format!("`matte` must be a colour as #rrggbb, not {text}"))?,
        Some(other) => return Err(format!("`matte` must be a colour as #rrggbb, not {other}")),
    };
    Ok(Options { format, mime, quality: quality.round() as u8, lossless, size, transparency, matte })
}

/// `doc` resized to `size` (bicubic, every layer, as Image Size does).
fn resized(doc: &Document, (width, height): (u32, u32)) -> Result<Document, String> {
    let mut scratch = Session::new();
    scratch.add_document(doc.clone(), None);
    scratch.execute("image.imageSize", json!({"width": width, "height": height, "resample": "bicubic"})).map_err(|error| error.to_string())?;
    scratch.active().map(|state| (*state.doc).clone()).ok_or_else(|| "the resized image is gone".to_owned())
}

/// `doc` flattened onto `matte`: one opaque layer with the composite over the colour.
fn matted(doc: &Document, matte: [f32; 3]) -> Document {
    let area = doc.bounds();
    let buffer = kition_photo_compose::render(doc, area);
    let format = doc.pixel_format();
    let opaque = PixelFormat::new(format.mode, format.sample, true);
    let samples: Vec<f32> = buffer
        .px
        .iter()
        .flat_map(|[r, g, b, a]| {
            let a = a.clamp(0.0, 1.0);
            let over = |c: f32, m: f32| c * a + m * (1.0 - a);
            kition_photo_raster::from_rgba(&opaque, [over(*r, matte[0]), over(*g, matte[1]), over(*b, matte[2]), 1.0])
        })
        .collect();
    let mut surface = Surface::new(opaque);
    surface.write_region(area, &samples);
    let mut out = Document::new(doc.name.clone(), doc.size, doc.mode, doc.depth);
    out.resolution_dpi = doc.resolution_dpi;
    out.icc_profile = doc.icc_profile.clone();
    out.metadata = doc.metadata.clone();
    out.layers.push(Layer::new("Background", LayerContent::Raster(surface)));
    out
}

/// The encoded file and its size in pixels.
pub struct Exported {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// `doc` as a flat file with `options`.
pub fn export(doc: &Document, options: &Options) -> Result<Exported, String> {
    let mut sized = match options.size {
        Some(size) => Some(resized(doc, size)?),
        None => None,
    };
    // JPEG has no alpha, and the export mattes it onto white by itself; any other colour, or a
    // format with alpha whose transparency is off, is flattened onto the matte here.
    let white = options.matte == [1.0; 3];
    if (options.format == "jpg" && !white) || (options.format != "jpg" && !options.transparency) {
        sized = Some(matted(sized.as_ref().unwrap_or(doc), options.matte));
    }
    let image = sized.as_ref().unwrap_or(doc);
    let encode = kition_photo_codecs::EncodeOptions {
        jpeg_quality: options.quality,
        webp_quality: options.quality,
        webp_lossless: options.lossless,
        ..Default::default()
    };
    let export = kition_photo_io::ExportOptions { encode, ..Default::default() };
    let exported = kition_photo_io::export(image, &format!("export.{}", options.format), &export).map_err(|error| error.to_string())?;
    Ok(Exported { bytes: exported.bytes, width: image.size.width, height: image.size.height })
}

/// The engine's clipboard (after Copy, Copy Merged or Cut) as a document its size, or `None` when it is empty.
pub fn clipboard_document(session: &Session) -> Option<Document> {
    let clip = session.clipboard.as_ref()?;
    let bounds = clip.bounds;
    if bounds.is_empty() {
        return None;
    }
    let format = clip.surface.format();
    let (width, height) = (bounds.width(), bounds.height());
    let samples = clip.surface.read_region(bounds);
    let mut surface = Surface::new(format);
    surface.write_region(Rect::from_xywh(0, 0, width, height), &samples);
    let (mode, depth) = session.active().map_or((ColorMode::Rgb, format.sample), |state| (state.doc.mode, state.doc.depth));
    let mut doc = Document::new("Clipboard", Size::new(width, height), mode, depth);
    if let Some(state) = session.active() {
        doc.icc_profile = state.doc.icc_profile.clone();
        doc.resolution_dpi = state.doc.resolution_dpi;
    }
    doc.layers.push(Layer::new("Layer 1", LayerContent::Raster(surface)));
    Some(doc)
}

/// The answer of `file.exportImage`.
pub fn answer(exported: &Exported, mime: &str, base64: impl Fn(&[u8]) -> String) -> Value {
    json!({ "data": base64(&exported.bytes), "mime": mime, "bytes": exported.bytes.len(), "width": exported.width, "height": exported.height })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn options_keep_the_aspect_ratio_of_one_side() {
        let o = options(&json!({"format": "jpg", "width": 400}), 1200, 800).unwrap();
        assert_eq!((o.format, o.mime, o.size, o.transparency, o.quality), ("jpg", "image/jpeg", Some((400, 267)), false, 90));
        let o = options(&json!({"format": "webp", "height": 80, "quality": 55}), 1200, 800).unwrap();
        assert_eq!((o.size, o.quality, o.transparency), (Some((120, 80)), 55, true));
        assert_eq!(options(&json!({"width": 1200, "height": 800}), 1200, 800).unwrap().size, None);
    }

    #[test]
    fn options_refuse_what_they_cannot_do() {
        for bad in [
            json!({"format": "gif"}),
            json!({"quality": 0}),
            json!({"quality": "high"}),
            json!({"width": 0}),
            json!({"width": 40_000}),
            json!({"height": -5}),
            json!({"matte": "red"}),
            json!({"matte": "#12345"}),
            json!({"matte": 3}),
        ] {
            assert!(options(&bad, 10, 10).is_err(), "{bad}");
        }
        assert_eq!(options(&json!({"matte": "#ff8000"}), 10, 10).unwrap().matte, [1.0, 128.0 / 255.0, 0.0]);
    }
}
