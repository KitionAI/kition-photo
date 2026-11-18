//! Object layers and editable filters: Layer › Object Layers, Layer › Editable Filter and
//! Filter › Convert for Editable Filters.
//!
//! An object layer keeps its **source** (embedded file bytes, or a linked path) plus a
//! `transform` and a stack of editable filters; its `cache` is the rendered appearance the
//! compositors draw. The engine owns re-rendering (compose stays a pure function of the
//! document):
//!
//! ```text
//! source bytes ──decode + composite (cached by content hash)──► source image (source pixels)
//!              ──transform (exact shift, or bicubic warp)──────► placed (document pixels)
//!              ──each visible editable filter, blended (blend/opacity)──► filtered
//!              ──filter mask mix (unfiltered ↔ filtered)───────► cache
//! ```
//!
//! Converting a layer embeds it as an in-memory `.kphoto` bundle of a nested document (exactly
//! lossless, layered, any depth/model). PSD placed layers keep their source in the preserved
//! global `lnk2` block (found by uuid), and keep Photoshop's rendering until something changes,
//! so an unedited PSD still round-trips byte for byte. PSD export (`kition_photo_io::object_layer_map`)
//! writes every object layer back as one: its source embedded in `lnk2` (a `.kphoto` source as a
//! PSB), its editable filters in `filterFX` and its filter mask in `FEid`.

use std::sync::{Arc, Mutex};

use kition_photo_algo::resample::translate_surface;
use kition_photo_algo::transform::Homography;
use kition_photo_color::{BlendMode, PixelFormat};
use kition_photo_doc::{DocId, Document, Layer, LayerContent, LayerId, LayerMask, Metadata, ObjectLayer, SmartSource};
use kition_photo_geom::{Affine, Rect, Size};
use kition_photo_raster::Surface;
use serde_json::{Value, json};

use crate::commands::{CommandSpec, blend_from_str, layer_param};
use crate::{EngineError, Result, Session};

/// An open Edit Contents document and the object layer it updates when saved or closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmartLink {
    pub child: DocId,
    pub parent: DocId,
    pub layer: LayerId,
}

/// PSD placed-layer keys that describe the object layer's source; stale once we change it.
const PLACED_KEYS: [&[u8; 4]; 3] = [b"SoLd", b"PlLd", b"SoLE"];

/// Command id of a Photoshop editable filter Kition Photo does not implement (kept verbatim from PSD).
pub use kition_photo_io::object_layer_map::UNSUPPORTED_FILTER;

fn other(msg: impl Into<String>) -> EngineError {
    EngineError::Other(msg.into())
}

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

// ---------- source encoding / decoding ----------

/// Encodes a document as embedded object-layer contents: an in-memory `.kphoto` bundle.
pub fn encode_source(doc: &Document) -> Result<Vec<u8>> {
    kition_photo_format::save_to_bytes(doc, &Default::default()).map_err(|e| other(format!("can't encode object layer contents: {e}")))
}

/// Decodes object-layer contents (a `.kphoto` bundle, PSD/PSB, or any flat image format).
pub fn decode_source(file_name: &str, bytes: &[u8]) -> Result<Document> {
    crate::file_cmds::import(file_name, bytes).map_err(|e| other(format!("can't read object layer contents \"{file_name}\": {e}")))
}

fn base_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// A linked source's bytes. While the host owns every file (a kit's documents, KHP A29, A42) only
/// the host's lender is asked, and never for a link that leaves the workspace: the link stays as
/// data and the check reports the source missing ([`kition_photo_format::host_files::read_linked`]).
fn read_file(path: &str) -> Option<Vec<u8>> {
    kition_photo_format::host_files::read_linked(path)
}

/// The source file of an object layer: embedded bytes, the PSD's embedded linked-layer data (PSD
/// import keeps the placed layer's uuid as the path), or a linked file on disk.
pub fn source_bytes(meta: &Metadata, src: &SmartSource) -> Option<(String, Arc<Vec<u8>>)> {
    embedded_source_bytes(meta, src).or_else(|| match src {
        SmartSource::Linked { path } => read_file(path).map(|b| (base_name(path), Arc::new(b))),
        SmartSource::Embedded { .. } => None,
    })
}

/// Memory-only resolution also covers Photoshop's embedded files addressed by a `Linked` UUID.
fn embedded_source_bytes(meta: &Metadata, src: &SmartSource) -> Option<(String, Arc<Vec<u8>>)> {
    match src {
        SmartSource::Embedded { file_name, bytes } => Some((file_name.clone(), bytes.clone())),
        SmartSource::Linked { path } => kition_photo_io::linked::find_linked_file(meta, path).map(|f| (f.file_name, Arc::new(f.bytes))),
    }
}

/// The actual path is stored in the document, not in the command's params. Recheck it before
/// reading, reusing embedded PSD bytes rather than parsing their global blocks a second time.
fn authorized_source_bytes(s: &Session, cmd: &str, meta: &Metadata, src: &SmartSource) -> Result<Option<(String, Arc<Vec<u8>>)>> {
    if let Some(source) = embedded_source_bytes(meta, src) {
        return Ok(Some(source));
    }
    let SmartSource::Linked { path } = src else { return Ok(None) };
    if let Some(gate) = s.authorize {
        gate(cmd, &json!({"path": path}))?;
    }
    Ok(read_file(path).map(|b| (base_name(path), Arc::new(b))))
}

/// Recheck a source's actual path before an untrusted command reads it: the path lives in the
/// document, not in the command's params. Embedded PSD UUIDs never reach the filesystem.
fn authorize_source(s: &Session, cmd: &str, doc: &Document, src: &SmartSource) -> Result<()> {
    if let (Some(gate), SmartSource::Linked { path }) = (s.authorize, src)
        && embedded_source_bytes(&doc.metadata, src).is_none()
    {
        gate(cmd, &json!({"path": path}))?;
    }
    Ok(())
}

/// A decoded, composited source in its own pixel space (`bounds` starts at the origin).
#[derive(Clone, Debug)]
pub struct SourceImage {
    pub surface: Arc<Surface>,
    pub bounds: Rect,
}

type CacheKey = ([u8; 32], PixelFormat);

struct CacheEntry {
    key: CacheKey,
    image: SourceImage,
    decodes: u32,
}

/// Decoded source composites by content hash, most recent last. Re-filtering or re-transforming
/// an object layer never decodes its contents again.
static SOURCE_CACHE: Mutex<Vec<CacheEntry>> = Mutex::new(Vec::new());
const CACHE_ENTRIES: usize = 32;
const CACHE_BYTES: usize = 1 << 30;

fn cache_key(bytes: &[u8], fmt: PixelFormat) -> CacheKey {
    (*blake3::hash(bytes).as_bytes(), fmt)
}

fn cache_get(key: &CacheKey) -> Option<SourceImage> {
    let mut c = SOURCE_CACHE.lock().ok()?;
    let i = c.iter().position(|e| e.key == *key)?;
    let e = c.remove(i);
    let img = e.image.clone();
    c.push(e);
    Some(img)
}

fn cache_put(key: CacheKey, image: SourceImage) {
    let Ok(mut c) = SOURCE_CACHE.lock() else { return };
    let decodes = match c.iter().position(|e| e.key == key) {
        Some(i) => c.remove(i).decodes + 1,
        None => 1,
    };
    c.push(CacheEntry { key, image, decodes });
    let size = |e: &CacheEntry| e.image.bounds.width() as usize * e.image.bounds.height() as usize * e.key.1.channels() * 4;
    while c.len() > CACHE_ENTRIES || (c.len() > 1 && c.iter().map(size).sum::<usize>() > CACHE_BYTES) {
        c.remove(0);
    }
}

/// How many times these contents were decoded at this pixel format (0 = never / evicted).
pub fn source_decode_count(bytes: &[u8], fmt: PixelFormat) -> u32 {
    let key = cache_key(bytes, fmt);
    SOURCE_CACHE.lock().ok().and_then(|c| c.iter().find(|e| e.key == key).map(|e| e.decodes)).unwrap_or(0)
}

fn buffer_to_surface(buf: &kition_photo_compose::Buffer, fmt: PixelFormat) -> Surface {
    let n = fmt.channels();
    let mut data = vec![0.0f32; buf.px.len() * n];
    for (p, out) in buf.px.iter().zip(data.chunks_exact_mut(n)) {
        kition_photo_raster::from_rgba_into(&fmt, *p, out);
    }
    let mut s = Surface::new(fmt);
    if !buf.rect.is_empty() {
        s.write_region(buf.rect, &data);
    }
    s.prune();
    s
}

/// An SVG source rendered at `scale` times its own size (cached per scale), so a vector smart
/// object stays sharp however it is placed.
pub fn svg_source_image(bytes: &[u8], fmt: PixelFormat, scale: f64) -> Result<SourceImage> {
    let mut h = blake3::Hasher::new();
    h.update(bytes);
    h.update(&scale.to_bits().to_le_bytes());
    let key = (*h.finalize().as_bytes(), fmt);
    if let Some(img) = cache_get(&key) {
        return Ok(img);
    }
    let buf = kition_photo_io::svg::rasterize(bytes, scale as f32).map_err(|e| other(format!("can't render the SVG object layer: {e}")))?;
    let img = SourceImage { surface: Arc::new(buffer_to_surface(&buf, fmt)), bounds: buf.rect };
    cache_put(key, img.clone());
    Ok(img)
}

/// The scale a vector source is rasterised at for its placement: the larger axis scale of the
/// transform in 1/16 steps (so nudges reuse the cache), between 1/16 and 64.
fn vector_scale(sm: &ObjectLayer) -> f64 {
    let [a, b, c, d, ..] = sm.transform.m;
    let k = (a * a + b * b).sqrt().max((c * c + d * d).sqrt());
    if !k.is_finite() {
        return 1.0;
    }
    ((k * 16.0).ceil() / 16.0).clamp(1.0 / 16.0, 64.0)
}

/// The composited source image of `bytes` in pixel format `fmt` (decoded once, then cached).
pub fn source_image(file_name: &str, bytes: &[u8], fmt: PixelFormat) -> Result<SourceImage> {
    let key = cache_key(bytes, fmt);
    if let Some(img) = cache_get(&key) {
        return Ok(img);
    }
    let doc = decode_source(file_name, bytes)?;
    let buf = kition_photo_compose::flatten(&doc);
    let img = SourceImage { surface: Arc::new(buffer_to_surface(&buf, fmt)), bounds: doc.bounds() };
    cache_put(key, img.clone());
    Ok(img)
}

/// Layer › Object Layers › Stack Mode: the source's layers (or, when it holds a single group, the
/// group's layers) combined per pixel with `mode`. Cached like [`source_image`], keyed by mode.
pub fn stack_image(file_name: &str, bytes: &[u8], fmt: PixelFormat, mode: kition_photo_doc::StackMode) -> Result<SourceImage> {
    use kition_photo_algo::stack::Stat;
    let mut key = cache_key(bytes, fmt);
    key.0 = *blake3::hash(&[&key.0[..], b"stack:", mode.id().as_bytes()].concat()).as_bytes();
    if let Some(img) = cache_get(&key) {
        return Ok(img);
    }
    let doc = decode_source(file_name, bytes)?;
    let layers: &[Layer] = match doc.layers.as_slice() {
        [only] if only.is_group() => only.children().unwrap_or_default(),
        all => all,
    };
    let bounds = doc.bounds();
    let frames: Vec<Vec<[f32; 4]>> = layers
        .iter()
        .filter(|l| l.visible)
        .map(|l| {
            let mut one = l.clone();
            one.blend = BlendMode::Normal;
            one.clipped = false;
            kition_photo_compose::render_layer(&one, bounds).px
        })
        .collect();
    if frames.is_empty() {
        return Err(EngineError::Other("the object layer's contents have no visible layers".into()));
    }
    let stat = match mode {
        kition_photo_doc::StackMode::Entropy => Stat::Entropy,
        kition_photo_doc::StackMode::Kurtosis => Stat::Kurtosis,
        kition_photo_doc::StackMode::Maximum => Stat::Maximum,
        kition_photo_doc::StackMode::Mean => Stat::Mean,
        kition_photo_doc::StackMode::Median => Stat::Median,
        kition_photo_doc::StackMode::Minimum => Stat::Minimum,
        kition_photo_doc::StackMode::Range => Stat::Range,
        kition_photo_doc::StackMode::Skewness => Stat::Skewness,
        kition_photo_doc::StackMode::StandardDeviation => Stat::StandardDeviation,
        kition_photo_doc::StackMode::Summation => Stat::Summation,
        kition_photo_doc::StackMode::Variance => Stat::Variance,
    };
    let buf = kition_photo_compose::Buffer { rect: bounds, px: kition_photo_algo::stack::combine(&frames, stat) };
    let img = SourceImage { surface: Arc::new(buffer_to_surface(&buf, fmt)), bounds };
    cache_put(key, img.clone());
    Ok(img)
}

// ---------- rendering ----------

/// `top` blended over `base` with `mode` at `opacity` (an editable filter's blending options).
fn blend_surfaces(base: &Surface, top: &Surface, mode: BlendMode, opacity: f32) -> Surface {
    let fmt = base.format();
    let area = base.content_bounds().union(&top.content_bounds());
    let mut out = base.clone();
    if area.is_empty() {
        return out;
    }
    let n = fmt.channels();
    let (b, t) = (base.read_region(area), top.read_region(area));
    let mut o = vec![0.0f32; b.len()];
    for ((bp, tp), op) in b.chunks_exact(n).zip(t.chunks_exact(n)).zip(o.chunks_exact_mut(n)) {
        let rgba = kition_photo_color::blend::composite(mode, kition_photo_raster::to_rgba(&fmt, bp), kition_photo_raster::to_rgba(&fmt, tp), opacity);
        kition_photo_raster::from_rgba_into(&fmt, rgba, op);
    }
    out.write_region(area, &o);
    out.prune();
    out
}

/// Mixes `filtered` over `base` through the filter mask (premultiplied, any channel layout).
fn mask_mix(base: &Surface, filtered: &Surface, mask: &LayerMask) -> Surface {
    let fmt = base.format();
    let area = base.content_bounds().union(&filtered.content_bounds());
    let mut out = filtered.clone();
    if area.is_empty() {
        return out;
    }
    let n = fmt.channels();
    let (b, f) = (base.read_region(area), filtered.read_region(area));
    let mut k = Vec::new();
    mask.values_into(area, &mut k);
    let mut o = vec![0.0f32; b.len()];
    for (i, ((bp, fp), op)) in b.chunks_exact(n).zip(f.chunks_exact(n)).zip(o.chunks_exact_mut(n)).enumerate() {
        let m = k.get(i).copied().unwrap_or(1.0).clamp(0.0, 1.0);
        if !fmt.alpha {
            for c in 0..n {
                op[c] = bp[c] + (fp[c] - bp[c]) * m;
            }
            continue;
        }
        let a = n - 1;
        let oa = bp[a] + (fp[a] - bp[a]) * m;
        op[a] = oa;
        for c in 0..a {
            let pm = bp[c] * bp[a] + (fp[c] * fp[a] - bp[c] * bp[a]) * m;
            op[c] = if oa > 0.0 { (pm / oa).clamp(0.0, 1.0) } else { 0.0 };
        }
    }
    out.write_region(area, &o);
    out.prune();
    out
}

/// Runs the editable filter stack on the placed (unfiltered) content.
pub fn apply_editable_filters(placed: &Surface, sm: &ObjectLayer, canvas: Rect) -> Surface {
    if !sm.filters_enabled || !sm.editable_filters.iter().any(|f| f.visible) {
        return placed.clone();
    }
    let mut cur = placed.clone();
    // Edge pixels repeat at the canvas ∪ placed-content edge, as for layer filters.
    let extent = canvas.union(&placed.content_bounds());
    for f in sm.editable_filters.iter().filter(|f| f.visible) {
        // Unknown ids (e.g. Photoshop filters we don't implement) leave the pixels alone.
        let Some(out) = crate::filters::apply_filter_to_surface(&f.command, &f.params, &cur, canvas, None, extent) else { continue };
        cur = if f.blend == BlendMode::Normal && f.opacity >= 1.0 { out } else { blend_surfaces(&cur, &out, f.blend, f.opacity.clamp(0.0, 1.0)) };
    }
    match sm.filter_mask.as_ref().filter(|m| m.enabled) {
        Some(m) => mask_mix(placed, &cur, m),
        None => cur,
    }
}

/// Renders an object layer from its source. `Ok(None)` when the source is unavailable (a missing
/// linked file, or a PSD placed layer without embedded data): callers keep the existing cache.
pub fn render(doc: &Document, sm: &ObjectLayer) -> Result<Option<Surface>> {
    let Some((name, bytes)) = source_bytes(&doc.metadata, &sm.source) else { return Ok(None) };
    // A vector source is rendered at the placement's scale, and the placement is divided by
    // that scale so the rendered pixels land where the source units would.
    if sm.stack_mode.is_none() && sm.warp.is_none() && kition_photo_io::svg::is_svg(&bytes) {
        let k = vector_scale(sm);
        let img = svg_source_image(&bytes, doc.pixel_format(), k)?;
        let placed = match &sm.perspective {
            Some(p) => {
                let [h0, h1, h2, h3, h4, h5, h6, h7, h8] = *p;
                let h = Homography([h0 / k, h1 / k, h2, h3 / k, h4 / k, h5, h6 / k, h7 / k, h8]);
                kition_photo_algo::warp::place_source_projective(&img.surface, img.bounds, &h, None)
            }
            None => {
                let [a, b, c, d, e, f] = sm.transform.m;
                let t = Affine { m: [a / k, b / k, c / k, d / k, e, f] };
                kition_photo_algo::warp::place_source(&img.surface, img.bounds, &t, None)
            }
        };
        return Ok(Some(apply_editable_filters(&placed, sm, doc.bounds())));
    }
    let img = match sm.stack_mode {
        Some(mode) => stack_image(&name, &bytes, doc.pixel_format(), mode)?,
        None => source_image(&name, &bytes, doc.pixel_format())?,
    };
    // Through the warp (source space) and the transform in one pass; whole-pixel moves are exact.
    let placed = match &sm.perspective {
        Some(p) => kition_photo_algo::warp::place_source_projective(&img.surface, img.bounds, &Homography(*p), sm.warp.as_ref()),
        None => kition_photo_algo::warp::place_source(&img.surface, img.bounds, &sm.transform, sm.warp.as_ref()),
    };
    Ok(Some(apply_editable_filters(&placed, sm, doc.bounds())))
}

/// The object layer's pixels below editable filter `index` (the input that filter edits): the placed
/// source and the filters under it, without the filter mask. `Ok(None)` when the source is
/// unavailable.
pub fn render_below_filter(doc: &Document, sm: &ObjectLayer, index: usize) -> Result<Option<Surface>> {
    let mut below = sm.clone();
    below.editable_filters.truncate(index);
    below.filter_mask = None;
    render(doc, &below)
}

/// Re-renders smart layer `l` (which lives in `doc`) in place. Returns false if its source is
/// unavailable (the cache is then left alone).
pub fn refresh_layer(doc: &Document, l: &mut Layer) -> Result<bool> {
    let LayerContent::Smart(sm) = &mut l.content else { return Ok(false) };
    match render(doc, sm)? {
        Some(px) => {
            sm.cache = Some(px);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Re-renders the object layer `id` of `doc`.
pub fn refresh(doc: &mut Document, id: LayerId) -> Result<bool> {
    let mut l = doc.layer(id).cloned().ok_or(EngineError::NoLayer(id))?;
    if !matches!(l.content, LayerContent::Smart(_)) {
        return Err(other("the layer is not an object layer"));
    }
    let ok = refresh_layer(doc, &mut l)?;
    if ok && let Some(dst) = doc.layer_mut(id) {
        dst.content = l.content;
    }
    Ok(ok)
}

fn refresh_or_fail(doc: &mut Document, id: LayerId) -> Result<()> {
    if refresh(doc, id)? { Ok(()) } else { Err(other("the object layer's contents are unavailable (missing linked file?), so it can't be re-rendered")) }
}

fn smart(doc: &Document, id: LayerId) -> Result<&ObjectLayer> {
    match &doc.layer(id).ok_or(EngineError::NoLayer(id))?.content {
        LayerContent::Smart(sm) => Ok(sm),
        _ => Err(other("the layer is not an object layer")),
    }
}

fn smart_mut(doc: &mut Document, id: LayerId) -> Result<&mut ObjectLayer> {
    match &mut doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?.content {
        LayerContent::Smart(sm) => Ok(sm),
        _ => Err(other("the layer is not an object layer")),
    }
}

fn mask_from_selection(sel: &Surface) -> LayerMask {
    LayerMask { surface: sel.clone(), enabled: true, linked: true, density: 1.0, feather: 0.0 }
}

/// Appends an editable filter (from a `filter.*` command) and re-renders. A selection becomes the
/// filter mask, as in Photoshop. Without a renderable source the filter is applied to the cached
/// pixels (and still recorded).
pub(crate) fn add_editable_filter(doc: &mut Document, id: LayerId, sf: kition_photo_doc::EditableFilter, selection: Option<&Surface>) -> Result<()> {
    let canvas = doc.bounds();
    let renderable = source_bytes(&doc.metadata, &smart(doc, id)?.source).is_some();
    let sm = smart_mut(doc, id)?;
    if renderable {
        if let Some(sel) = selection
            && sm.filter_mask.is_none()
        {
            sm.filter_mask = Some(mask_from_selection(sel));
        }
        sm.editable_filters.push(sf);
        return refresh_or_fail(doc, id);
    }
    let bounds = selection.map(Surface::content_bounds).filter(|b| !b.is_empty()).unwrap_or(canvas);
    let cache = sm.cache.as_ref().ok_or_else(|| other("object layer has no pixels"))?;
    let out = crate::filters::apply_filter_to_surface(&sf.command, &sf.params, cache, bounds, selection, canvas)
        .ok_or_else(|| other(format!("unknown filter {}", sf.command)))?;
    sm.cache = Some(out);
    sm.editable_filters.push(sf);
    Ok(())
}

/// Rounds float noise off a composed transform (scale 0.1 then 10 should be exactly 1), so
/// repeated transforms land back on exact, lossless placements.
pub(crate) fn snap_affine(a: Affine) -> Affine {
    Affine { m: a.m.map(|v| if (v - v.round()).abs() < 1e-9 { v.round() } else { v }) }
}

/// Where the object layer's source pixels land in the document: its projective map (Distort,
/// Perspective) or its affine transform.
pub fn placement(sm: &ObjectLayer) -> Homography {
    sm.perspective.map(Homography).unwrap_or_else(|| affine_homography(&sm.transform))
}

pub(crate) fn affine_homography(a: &Affine) -> Homography {
    let [a, b, c, d, e, f] = a.m;
    Homography([a, c, e, b, d, f, 0.0, 0.0, 1.0])
}

/// Sets where the source lands. An affine map is stored exactly in `transform` (no
/// `perspective`); a projective one in `perspective`, with `transform` its affine approximation at
/// the source origin (for code that only needs scale and position).
pub(crate) fn set_placement(sm: &mut ObjectLayer, h: Homography) {
    let m = h.0;
    let n = if m[8].abs() > 1e-12 { m.map(|v| v / m[8]) } else { m };
    if n[6].abs() < 1e-12 && n[7].abs() < 1e-12 {
        sm.transform = snap_affine(Affine { m: [n[0], n[3], n[1], n[4], n[2], n[5]] });
        sm.perspective = None;
        return;
    }
    let h = Homography(n);
    let (o, x, y) = (h.apply(0.0, 0.0), h.apply(1.0, 0.0), h.apply(0.0, 1.0));
    sm.transform = Affine { m: [x.0 - o.0, x.1 - o.1, y.0 - o.0, y.1 - o.1, o.0, o.1] };
    sm.perspective = Some(n);
}

/// Composes `a` (document → document) onto the object layer's placement.
pub(crate) fn transform_placement(sm: &mut ObjectLayer, a: &Affine) {
    match sm.perspective {
        None => sm.transform = snap_affine(a.mul(&sm.transform)),
        Some(_) => set_placement(sm, affine_homography(a).mul(&placement(sm))),
    }
}

/// Moves an object layer by whole pixels without re-rendering.
pub(crate) fn shift_smart(sm: &mut ObjectLayer, dx: i32, dy: i32) {
    transform_placement(sm, &Affine::translate(dx as f64, dy as f64));
    if let Some(c) = &mut sm.cache {
        *c = crate::layer_multi_cmds::shift_surface(c, dx, dy);
    }
    if let Some(m) = &mut sm.filter_mask {
        m.surface = crate::layer_multi_cmds::shift_surface(&m.surface, dx, dy);
    }
}

/// Forget PSD placed-layer data that no longer describes the object layer (its source changed):
/// PSD export then writes fresh placed-layer blocks and embeds the new source.
fn detach_psd(l: &mut Layer) {
    if let LayerContent::Smart(sm) = &mut l.content {
        sm.psd_raw = None;
    }
    l.psd_blocks.retain(|(k, _)| !PLACED_KEYS.contains(&k));
}

// ---------- conversion ----------

/// Moves a layer (any kind) by whole pixels without re-rendering anything.
fn shift_layer(l: &mut Layer, dx: i32, dy: i32) {
    if dx == 0 && dy == 0 {
        return;
    }
    let a = Affine::translate(dx as f64, dy as f64);
    let mv = |s: &Surface| translate_surface(s, dx, dy);
    if let Some(m) = &mut l.mask {
        m.surface = mv(&m.surface);
    }
    if let Some(vm) = &mut l.vector_mask {
        vm.path = vm.path.transform(&a);
    }
    if let Some(fc) = &mut l.fill_cache {
        fc.surface = mv(&fc.surface);
    }
    match &mut l.content {
        LayerContent::Raster(s) => *s = mv(s),
        LayerContent::Text(t) => {
            t.transform = a.mul(&t.transform);
            if let Some(c) = &mut t.cache {
                *c = mv(c);
            }
        }
        LayerContent::Shape(sh) => {
            crate::vector_cmds::transform_shape(sh, &a);
            if let Some(c) = &mut sh.cache {
                *c = mv(c);
            }
        }
        LayerContent::Smart(sm) => shift_smart(sm, dx, dy),
        LayerContent::Group(g) => g.children.iter_mut().for_each(|c| shift_layer(c, dx, dy)),
        LayerContent::Adjustment(_) | LayerContent::Fill(_) => {}
    }
}

fn subtree_bounds(l: &Layer, canvas: Rect) -> Rect {
    match &l.content {
        LayerContent::Group(g) => g.children.iter().map(|c| subtree_bounds(c, canvas)).fold(Rect::EMPTY, |a, b| a.union(&b)),
        LayerContent::Adjustment(_) | LayerContent::Fill(_) => canvas,
        _ => l.surface().map_or(Rect::EMPTY, Surface::content_bounds),
    }
}

fn any_layer(l: &Layer, f: &dyn Fn(&Layer) -> bool) -> bool {
    f(l) || l.children().is_some_and(|c| c.iter().any(|c| any_layer(c, f)))
}

/// Builds the object layer replacing `l` in `doc`: the layer (with its mask, effects and
/// fill opacity) becomes a nested document cropped to its rendered bounds; visibility, opacity,
/// blend mode, clipping and label stay on the smart layer. The cache is the exact composite, so
/// the document looks the same before and after.
pub fn layer_to_smart(doc: &Document, l: &Layer) -> Result<Layer> {
    let canvas = doc.bounds();
    let background = l.name == "Background" && l.locks.position;
    let mut inner = l.clone();
    inner.visible = true;
    inner.opacity = 1.0;
    inner.clipped = false;
    inner.locks = Default::default();
    if !(inner.is_group() && inner.blend == BlendMode::PassThrough) {
        inner.blend = BlendMode::Normal;
    }
    if background {
        inner.name = "Layer 0".into();
    }
    let mut sub = Document::new(format!("{}.kphoto", l.name), doc.size, doc.mode, doc.depth);
    sub.resolution_dpi = doc.resolution_dpi;
    sub.icc_profile = doc.icc_profile.clone();
    sub.global_light = doc.global_light;
    // Pattern fills and pattern layer effects reference document-level patterns by ID.
    // Preserve those resources in the embedded document before rendering or saving it.
    sub.patterns = doc.patterns.clone();
    if any_layer(l, &|x| matches!(x.content, LayerContent::Smart(_))) {
        // Nested PSD placed layers find their embedded files here.
        sub.metadata.psd_global_blocks = doc.metadata.psd_global_blocks.clone();
    }
    sub.layers = vec![inner];

    let has_fx = any_layer(l, &|x| x.effects.enabled && !x.effects.items.is_empty());
    let region = subtree_bounds(l, canvas).union(&canvas).inflate(if has_fx { 256 } else { 0 });
    let buf = kition_photo_compose::render(&sub, region);
    let w = region.width() as usize;
    let mut b = Rect::EMPTY;
    for (i, p) in buf.px.iter().enumerate() {
        if p[3] > 0.0 {
            let (x, y) = (region.x0 + (i % w) as i32, region.y0 + (i / w) as i32);
            b = b.union(&Rect::new(x, y, x + 1, y + 1));
        }
    }
    if b.is_empty() {
        return Err(other("the layer is empty, so there is nothing to convert"));
    }
    let fmt = doc.pixel_format();
    let mut cache = buffer_to_surface(&buf, fmt);
    cache = kition_photo_algo::resample::crop_surface(&cache, b);
    cache.prune();

    shift_layer(&mut sub.layers[0], -b.x0, -b.y0);
    sub.size = Size::new(b.width(), b.height());
    let bytes = encode_source(&sub)?;
    // Seed the decode cache with the composite we already have.
    cache_put(cache_key(&bytes, fmt), SourceImage { surface: Arc::new(translate_surface(&cache, -b.x0, -b.y0)), bounds: sub.bounds() });

    let source = SmartSource::Embedded { file_name: sub.name.clone(), bytes: Arc::new(bytes) };
    let mut out = Layer::new(
        if background { "Layer 0".to_string() } else { l.name.clone() },
        LayerContent::Smart(ObjectLayer::new(source, Affine::translate(b.x0 as f64, b.y0 as f64), Some(cache))),
    );
    out.visible = l.visible;
    out.opacity = l.opacity;
    out.blend = if l.blend == BlendMode::PassThrough { BlendMode::Normal } else { l.blend };
    out.clipped = l.clipped;
    out.label = l.label;
    if !background {
        out.locks = l.locks;
    }
    Ok(out)
}

fn convert(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = match p.get("layer").and_then(Value::as_u64) {
        Some(id) => vec![LayerId(id)],
        None => crate::layer_multi_cmds::selected(s),
    };
    let new_id = s.edit("Convert to Object Layer", |doc, active| {
        let ids = crate::layer_multi_cmds::top_level(doc, &ids);
        let top = *ids.last().ok_or_else(|| other("no layer selected"))?;
        let l = doc.layer(top).ok_or(EngineError::NoLayer(top))?;
        let so = if ids.len() == 1 {
            layer_to_smart(doc, l)?
        } else {
            let children = ids.iter().map(|id| doc.layer(*id).cloned().ok_or(EngineError::NoLayer(*id))).collect::<Result<Vec<_>>>()?;
            layer_to_smart(doc, &Layer::group(l.name.clone(), children))?
        };
        let new_id = so.id;
        // As with Group Layers, place the result in the top selected root's parent. Insert
        // before removal so paths shifting in other parents cannot discard unselected content.
        doc.insert_above(Some(top), so);
        for id in ids {
            doc.remove(id).ok_or(EngineError::NoLayer(id))?;
        }
        *active = Some(new_id);
        Ok(new_id)
    })?;
    crate::layer_multi_cmds::reselect(s, vec![new_id], Some(new_id));
    Ok(json!({"layer": new_id.0}))
}

// ---------- contents ----------

fn via_copy(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, p)?;
    s.edit("New Object Layer via Copy", |doc, active| {
        let src = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
        let mut copy = src.duplicate();
        copy.name = doc.copy_name(&src.name);
        if let LayerContent::Smart(sm) = &mut copy.content {
            // Independent contents: resolve to embedded bytes and drop the shared PSD uuid.
            if let Some((file_name, bytes)) = source_bytes(&doc.metadata, &sm.source) {
                sm.source = SmartSource::Embedded { file_name, bytes };
            }
        } else {
            return Err(other("the layer is not an object layer"));
        }
        detach_psd(&mut copy);
        let new_id = doc.insert_above(Some(id), copy);
        *active = Some(new_id);
        Ok(json!({"layer": new_id.0}))
    })
}

fn path_param<'a>(cmd: &str, p: &'a Value) -> Result<&'a str> {
    p.get("path").and_then(Value::as_str).filter(|s| !s.is_empty()).ok_or_else(|| bad(cmd, "pass `path`"))
}

/// Swaps an object layer's source, keeping its transform and filters, and re-renders.
fn set_source(s: &mut Session, p: &Value, label: &str, keep_psd: bool, make: impl FnOnce(&Metadata, &SmartSource) -> Result<SmartSource>) -> Result<Value> {
    let id = layer_param(s, p)?;
    s.edit(label, |doc, _| {
        let new = make(&doc.metadata, &smart(doc, id)?.source)?;
        smart_mut(doc, id)?.source = new;
        if !keep_psd {
            detach_psd(doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?);
        }
        refresh_or_fail(doc, id)?;
        Ok(json!({"layer": id.0}))
    })
}

fn replace_contents(s: &mut Session, p: &Value) -> Result<Value> {
    let path = path_param("layer.objectLayers.replaceContents", p)?.to_string();
    let bytes = kition_photo_format::read_file(std::path::Path::new(&path)).map_err(|e| other(format!("can't read {path}: {e}")))?;
    let name = base_name(&path);
    decode_source(&name, &bytes)?; // fail before touching the document
    set_source(s, p, "Replace Contents", false, |_, _| Ok(SmartSource::Embedded { file_name: name, bytes: Arc::new(bytes) }))
}

fn export_contents(s: &mut Session, p: &Value) -> Result<Value> {
    let path = path_param("layer.objectLayers.exportContents", p)?;
    let id = layer_param(s, p)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let (name, bytes) = source_bytes(&st.doc.metadata, &smart(&st.doc, id)?.source).ok_or_else(|| other("the object layer's contents are unavailable"))?;
    crate::file_cmds::write_file(path, &bytes)?;
    Ok(json!({"path": path, "fileName": name, "bytes": bytes.len()}))
}

fn edit_contents(s: &mut Session, p: &Value) -> Result<Value> {
    let id = layer_param(s, p)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let parent = st.doc.id;
    // Already open for editing: switch to that document rather than opening another copy.
    if let Some(index) = s.smart_links.iter().find(|l| l.parent == parent && l.layer == id).and_then(|l| s.documents().iter().position(|d| d.doc.id == l.child))
    {
        s.set_active(index);
        return Ok(json!({"document": index, "parentLayer": id.0}));
    }
    let source = &smart(&st.doc, id)?.source;
    let (name, bytes) = authorized_source_bytes(s, "layer.objectLayers.editContents", &st.doc.metadata, source)?
        .ok_or_else(|| other("the object layer's contents are unavailable"))?;
    let mut child = decode_source(&name, &bytes)?;
    // Bundles keep their document id; each open copy needs its own.
    child.id = DocId::fresh();
    child.name = name;
    let index = s.add_document(child, None);
    // Admission may replace an ID already owned by another open document.
    let child_id = s.documents().get(index).ok_or(EngineError::NoDocument)?.doc.id;
    s.smart_links.retain(|l| l.child != child_id);
    s.smart_links.push(SmartLink { child: child_id, parent, layer: id });
    Ok(json!({"document": index, "parentLayer": id.0}))
}

/// Writes an Edit Contents document back into its parent object layer (one undoable step in the
/// parent) and marks it saved. Returns false if `index` isn't an Edit Contents document.
pub fn commit_child(s: &mut Session, index: usize) -> Result<bool> {
    let Some(st) = s.docs.get(index) else { return Ok(false) };
    let child = st.doc.clone();
    let Some(link) = s.smart_links.iter().find(|l| l.child == child.id).copied() else { return Ok(false) };
    let parent = s.docs.iter().position(|d| d.doc.id == link.parent).ok_or_else(|| other("the object layer's document was closed"))?;
    let bytes = Arc::new(encode_source(&child)?);
    let prev = s.active;
    s.active = Some(parent);
    let r = s.edit("Edit Contents", |doc, _| {
        let sm = smart_mut(doc, link.layer)?;
        let stem = match &sm.source {
            SmartSource::Embedded { file_name, .. } => file_name.rsplit_once('.').map_or(file_name.as_str(), |(a, _)| a).to_string(),
            SmartSource::Linked { .. } => child.name.rsplit_once('.').map_or(child.name.as_str(), |(a, _)| a).to_string(),
        };
        sm.source = SmartSource::Embedded { file_name: format!("{stem}.kphoto"), bytes: bytes.clone() };
        detach_psd(doc.layer_mut(link.layer).ok_or(EngineError::NoLayer(link.layer))?);
        refresh_or_fail(doc, link.layer)
    });
    s.active = prev;
    r?;
    if let Some(st) = s.docs.get_mut(index) {
        st.saved_revision = st.revision;
    }
    Ok(true)
}

/// Called before a document closes: an edited Edit Contents document updates its parent.
pub(crate) fn on_close(s: &mut Session, index: usize) {
    let Some(st) = s.docs.get(index) else { return };
    let id = st.doc.id;
    if st.is_dirty() && s.smart_links.iter().any(|l| l.child == id) {
        let _ = commit_child(s, index);
    }
    s.smart_links.retain(|l| l.child != id && l.parent != id);
}

fn is_smart_child(s: &Session) -> std::result::Result<(), String> {
    let d = s.active().ok_or("no document open")?;
    if s.smart_links.iter().any(|l| l.child == d.doc.id) { Ok(()) } else { Err("this document isn't object layer contents".into()) }
}

fn update_all(s: &mut Session) -> Result<Value> {
    let ids: Vec<LayerId> = s
        .active()
        .ok_or(EngineError::NoDocument)?
        .doc
        .walk()
        .into_iter()
        .filter(|(_, _, l)| matches!(&l.content, LayerContent::Smart(sm) if matches!(sm.source, SmartSource::Linked { .. })))
        .map(|(_, _, l)| l.id)
        .collect();
    let mut updated = Vec::new();
    s.edit("Update All Modified Content", |doc, _| {
        for id in &ids {
            if refresh(doc, *id)? {
                updated.push(id.0);
            }
        }
        Ok(())
    })?;
    Ok(json!({"updated": updated}))
}

// ---------- editable filter editing ----------

fn edit_filters(s: &mut Session, p: &Value, label: &str, f: impl FnOnce(&mut ObjectLayer) -> Result<()>) -> Result<Value> {
    let id = layer_param(s, p)?;
    s.edit(label, |doc, _| {
        f(smart_mut(doc, id)?)?;
        refresh_or_fail(doc, id)?;
        Ok(json!({"layer": id.0}))
    })
}

fn filter_index(cmd: &str, p: &Value, sm: &ObjectLayer) -> Result<usize> {
    let n = sm.editable_filters.len();
    if n == 0 {
        return Err(other("the object layer has no editable filters"));
    }
    match p.get("index") {
        None => Ok(n - 1),
        Some(v) => v.as_u64().map(|i| i as usize).filter(|i| *i < n).ok_or_else(|| bad(cmd, format!("`index` must be 0..{n} (0 = bottom filter)"))),
    }
}

fn set_filter_visible(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.editableFilter.setVisible";
    edit_filters(s, p, "Editable Filter Visibility", |sm| {
        let i = filter_index(CMD, p, sm)?;
        let f = &mut sm.editable_filters[i];
        f.visible = p.get("visible").and_then(Value::as_bool).unwrap_or(!f.visible);
        Ok(())
    })
}

fn set_filter_params(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.editableFilter.setParams";
    let new = p.get("params").cloned().ok_or_else(|| bad(CMD, "pass `params` (merged into the filter's parameters)"))?;
    edit_filters(s, p, "Edit Editable Filter", |sm| {
        let i = filter_index(CMD, p, sm)?;
        let f = &mut sm.editable_filters[i];
        if f.command == kition_photo_io::object_layer_map::UNSUPPORTED_FILTER {
            return Err(bad(CMD, "this editable filter isn't implemented in Kition Photo: it is kept as is (it can be hidden, moved or deleted)"));
        }
        match (&mut f.params, new) {
            (Value::Object(old), Value::Object(n)) => old.extend(n),
            (slot, n) => *slot = n,
        }
        Ok(())
    })
}

fn blending_options(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.editableFilter.blendingOptions";
    let blend = match p.get("blend").and_then(Value::as_str) {
        Some(b) => Some(blend_from_str(b).filter(|m| *m != BlendMode::PassThrough).ok_or_else(|| bad(CMD, format!("unknown blend mode `{b}`")))?),
        None => None,
    };
    let opacity = p.get("opacity").and_then(Value::as_f64).map(|o| if o > 1.0 { o / 100.0 } else { o } as f32);
    edit_filters(s, p, "Editable Filter Blending Options", |sm| {
        let i = filter_index(CMD, p, sm)?;
        let f = &mut sm.editable_filters[i];
        if let Some(b) = blend {
            f.blend = b;
        }
        if let Some(o) = opacity {
            f.opacity = o.clamp(0.0, 1.0);
        }
        Ok(())
    })
}

fn delete_filter(s: &mut Session, p: &Value) -> Result<Value> {
    edit_filters(s, p, "Delete Editable Filter", |sm| {
        let i = filter_index("layer.editableFilter.delete", p, sm)?;
        sm.editable_filters.remove(i);
        Ok(())
    })
}

fn move_filter(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.editableFilter.move";
    edit_filters(s, p, "Move Editable Filter", |sm| {
        let i = filter_index(CMD, p, sm)?;
        let n = sm.editable_filters.len();
        let to = p.get("to").and_then(Value::as_u64).map(|t| t as usize).filter(|t| *t < n).ok_or_else(|| bad(CMD, format!("`to` must be 0..{n}")))?;
        let f = sm.editable_filters.remove(i);
        sm.editable_filters.insert(to, f);
        Ok(())
    })
}

// ---------- enablement ----------

fn active_smart(s: &Session) -> std::result::Result<&ObjectLayer, String> {
    let d = s.active().ok_or("no document open")?;
    let id = d.active_layer.ok_or("no active layer")?;
    match &d.doc.layer(id).ok_or("no active layer")?.content {
        LayerContent::Smart(sm) => Ok(sm),
        other => Err(format!("the active layer is {} {} layer, not an object layer", other.article(), other.kind_name())),
    }
}
fn has_smart(s: &Session) -> std::result::Result<(), String> {
    active_smart(s).map(|_| ())
}
fn has_linked(s: &Session) -> std::result::Result<(), String> {
    match active_smart(s)?.source {
        SmartSource::Linked { .. } => Ok(()),
        _ => Err("the object layer is embedded".into()),
    }
}
fn has_editable_filters(s: &Session) -> std::result::Result<(), String> {
    if active_smart(s)?.editable_filters.is_empty() { Err("the object layer has no editable filters".into()) } else { Ok(()) }
}
fn has_filter_mask(s: &Session) -> std::result::Result<(), String> {
    active_smart(s)?.filter_mask.as_ref().map(|_| ()).ok_or_else(|| "the object layer has no filter mask".into())
}
fn convertible(s: &Session) -> std::result::Result<(), String> {
    let d = s.active().ok_or("no document open")?;
    let id = d.active_layer.ok_or("no active layer")?;
    d.doc.layer(id).map(|_| ()).ok_or_else(|| "no active layer".into())
}
fn not_smart(s: &Session) -> std::result::Result<(), String> {
    convertible(s)?;
    if active_smart(s).is_ok() { Err("the layer is already an object layer".into()) } else { Ok(()) }
}

macro_rules! spec {
    ($id:literal, $label:literal, $menu:expr, $params:literal, $en:expr, $run:expr) => {
        CommandSpec { id: $id, label: $label, menu: $menu, shortcut: None, params: $params, enabled: $en, run: $run, journal: true }
    };
}

const SO: &[&str] = &["Layer", "Object Layers"];
const SF: &[&str] = &["Layer", "Editable Filter"];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        spec!(
            "layer.objectLayers.convertToObjectLayer",
            "Convert to Object Layer",
            SO,
            r##"{"layer":id?} (default: combine selected layers)"##,
            convertible,
            convert
        ),
        spec!(
            "filter.convertForEditableFilters",
            "Convert for Editable Filters",
            &["Filter"],
            r##"{"layer":id?} (default: combine selected layers)"##,
            not_smart,
            convert
        ),
        spec!("layer.objectLayers.newObjectLayerViaCopy", "New Object Layer via Copy", SO, r##"{"layer":id?}"##, has_smart, via_copy),
        spec!("layer.objectLayers.rasterize", "Rasterize", SO, r##"{"layer":id?}"##, has_smart, |s, p| s.execute("layer.rasterize.objectLayer", p.clone())),
        spec!(
            "layer.objectLayers.editContents",
            "Edit Contents",
            SO,
            r##"{"layer":id?} → opens the contents as a new document; saving (layer.objectLayers.saveContents) or closing it updates the object layer"##,
            has_smart,
            edit_contents
        ),
        spec!("layer.objectLayers.saveContents", "Save Contents", &[], "{} (in an Edit Contents document)", is_smart_child, |s, _| {
            let i = s.active_index().ok_or(EngineError::NoDocument)?;
            Ok(json!({"updated": commit_child(s, i)?}))
        }),
        spec!("layer.objectLayers.replaceContents", "Replace Contents…", SO, r##"{"layer":id?,"path":str}"##, has_smart, replace_contents),
        spec!("layer.objectLayers.exportContents", "Export Contents…", SO, r##"{"layer":id?,"path":str}"##, has_smart, export_contents),
        spec!("layer.objectLayers.relinkToFile", "Relink to File…", SO, r##"{"layer":id?,"path":str}"##, has_smart, |s, p| {
            let path = path_param("layer.objectLayers.relinkToFile", p)?.to_string();
            set_source(s, p, "Relink to File", false, |_, _| Ok(SmartSource::Linked { path }))
        }),
        spec!("layer.objectLayers.updateModifiedContent", "Update Modified Content", SO, r##"{"layer":id?}"##, has_linked, |s, p| {
            let id = layer_param(s, p)?;
            s.edit("Update Modified Content", |doc, _| refresh_or_fail(doc, id))?;
            Ok(json!({"layer": id.0}))
        }),
        spec!(
            "layer.objectLayers.updateAllModifiedContent",
            "Update All Modified Content",
            SO,
            "{}",
            |s| s.active().map(|_| ()).ok_or_else(|| "no document open".into()),
            |s, _| update_all(s)
        ),
        spec!(
            "layer.objectLayers.convertToLayers",
            "Convert to Layers",
            SO,
            r##"{"layer":id?} (contents unpacked at the placement: one layer, or a group named after the object layer; editable filters are discarded)"##,
            has_smart,
            unpack::convert_to_layers
        ),
        spec!("layer.objectLayers.convertToEmbedded", "Convert to Embedded", SO, r##"{"layer":id?}"##, has_linked, |s, p| {
            set_source(s, p, "Convert to Embedded", true, |meta, src| {
                let (file_name, bytes) = source_bytes(meta, src).ok_or_else(|| other("the linked file can't be read"))?;
                Ok(SmartSource::Embedded { file_name, bytes })
            })
        }),
        spec!(
            "layer.objectLayers.convertToLinked",
            "Convert to Linked…",
            SO,
            r##"{"layer":id?,"path":str} (writes the contents there)"##,
            has_smart,
            |s, p| {
                let path = path_param("layer.objectLayers.convertToLinked", p)?.to_string();
                set_source(s, p, "Convert to Linked", false, |meta, src| {
                    let (_, bytes) = source_bytes(meta, src).ok_or_else(|| other("the object layer's contents are unavailable"))?;
                    crate::file_cmds::write_file(&path, &bytes)?;
                    Ok(SmartSource::Linked { path })
                })
            }
        ),
        // Editable filters
        spec!(
            "layer.editableFilter.disableEditableFilters",
            "Disable Editable Filters",
            SF,
            r##"{"layer":id?,"enabled":bool? (default: toggle)}"##,
            has_editable_filters,
            |s, p| {
                edit_filters(s, p, "Disable Editable Filters", |sm| {
                    sm.filters_enabled = p.get("enabled").and_then(Value::as_bool).unwrap_or(!sm.filters_enabled);
                    Ok(())
                })
            }
        ),
        spec!("layer.editableFilter.deleteFilterMask", "Delete Filter Mask", SF, r##"{"layer":id?}"##, has_filter_mask, |s, p| {
            edit_filters(s, p, "Delete Filter Mask", |sm| {
                sm.filter_mask = None;
                Ok(())
            })
        }),
        spec!(
            "layer.editableFilter.disableFilterMask",
            "Disable Filter Mask",
            SF,
            r##"{"layer":id?,"enabled":bool? (default: toggle)}"##,
            has_filter_mask,
            |s, p| {
                edit_filters(s, p, "Disable Filter Mask", |sm| {
                    if let Some(m) = &mut sm.filter_mask {
                        m.enabled = p.get("enabled").and_then(Value::as_bool).unwrap_or(!m.enabled);
                    }
                    Ok(())
                })
            }
        ),
        spec!(
            "layer.editableFilter.blendingOptions",
            "Blending Options…",
            SF,
            r##"{"layer":id?,"index":u32? (0 = bottom; default top),"blend":str?,"opacity":0..1?}"##,
            has_editable_filters,
            blending_options
        ),
        spec!("layer.editableFilter.clearEditableFilters", "Clear Editable Filters", SF, r##"{"layer":id?}"##, has_editable_filters, |s, p| {
            edit_filters(s, p, "Clear Editable Filters", |sm| {
                sm.editable_filters.clear();
                sm.filter_mask = None;
                Ok(())
            })
        }),
        spec!(
            "layer.editableFilter.setVisible",
            "Show/Hide Editable Filter",
            &[],
            r##"{"layer":id?,"index":u32?,"visible":bool? (default: toggle)}"##,
            has_editable_filters,
            set_filter_visible
        ),
        spec!(
            "layer.editableFilter.setParams",
            "Edit Editable Filter",
            &[],
            r##"{"layer":id?,"index":u32?,"params":{…} (merged)}"##,
            has_editable_filters,
            set_filter_params
        ),
        spec!("layer.editableFilter.delete", "Delete Editable Filter", &[], r##"{"layer":id?,"index":u32?}"##, has_editable_filters, delete_filter),
        spec!("layer.editableFilter.move", "Move Editable Filter", &[], r##"{"layer":id?,"index":u32?,"to":u32}"##, has_editable_filters, move_filter),
    ]
}

mod unpack;

#[cfg(test)]
mod tests;
