//! JSON mirrors of `kition-photo-doc` types the schema does not name as fields of their own.
//!
//! A layer stores two of them: `Layer.data`, the kind's own parameters (adjustment settings,
//! fill, a type layer's font and frame, shape outline, object layer source), and `Layer.extra`,
//! the rest of [`Layer`] the named schema fields do not cover: PSD bookkeeping (vector mask,
//! label, link group, excluded channels, Blend If, Advanced Blending, and raw
//! additional-layer-info blocks) a lossless PSD round trip still needs, and a video layer's
//! frame rate and source (its frames are `Layer.frames`, a list of surfaces). A type layer's
//! characters and style runs are its `TextBody` ([`crate::text_body`]); its layer style is
//! `Layer.styles` (one `Style` object per effect) and `Layer.styleSettings`. A group's
//! expanded state is each person's own view and is not stored. Rendered-pixel caches (`cache`,
//! `fill_cache`) are not modelled; the view keeps or rebuilds them.
//!
//! Large binary data (ICC-sized blobs, raw PSD blocks, embedded object layer files) goes through
//! the session's [`BlobSink`], like tiles: never inlined as base64 in a register, so the replica
//! stays small.

use std::sync::Arc;

use kition_kit::kdm::KdmError;
use kition_kit::session::{BlobRef, BlobSink, BlobSource};
use kition_photo_color::PixelFormat;
use kition_photo_doc::text::{AntiAlias, Orientation, TextShape, TextWarp};
use kition_photo_doc::{
    Adjustment, AdvancedBlending, Affine, AlphaChannel, Artboard, BlendIf, Color, ColorIndicates, EditableFilter, Effect, Effects, Fill, Group, LabelColor,
    Layer, LayerContent, LayerMask, LiveShape, ObjectLayer, Path, ShapeLayer, ShapeStroke, SmartSource, StackMode, TextLayer, VectorMask, VideoSource,
};
use kition_photo_geom::warp::Warp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kdm_err;
use crate::text_body::Body;
use crate::tiles::Sink;

type KdmResult<T> = Result<T, KdmError>;

/// Stores `bytes` through `blobs`, tagged as opaque binary data (never a tile).
pub(crate) fn embed(blobs: &mut dyn BlobSink, bytes: &[u8]) -> BlobRef {
    blobs.put(bytes, "application/octet-stream")
}

/// The bytes behind a [`BlobRef`] written by [`embed`], or [`KdmError`] if the blob store no
/// longer has them (a corrupt or incomplete snapshot).
pub(crate) fn fetch(blobs: &dyn BlobSource, blob: &BlobRef) -> KdmResult<Arc<[u8]>> {
    blobs.get(&blob.hash).ok_or_else(|| KdmError::Engine(format!("missing blob {}", blob.hash)))
}

fn hex4(key: [u8; 4]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex4(s: &str) -> KdmResult<[u8; 4]> {
    if s.len() != 8 {
        return Err(KdmError::Engine(format!("bad 4-byte key `{s}`")));
    }
    let mut out = [0u8; 4];
    for (i, chunk) in out.iter_mut().enumerate() {
        *chunk = u8::from_str_radix(s.get(i * 2..i * 2 + 2).unwrap_or(""), 16).map_err(kdm_err)?;
    }
    Ok(out)
}

/// An `Option<u64>` as decimal text (numbers past `i64::MAX` do not survive a replica register);
/// reads plain numbers too.
mod u64_text {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(v) => serializer.serialize_some(&v.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Text(String),
            Number(u64),
        }
        match Option::<Stored>::deserialize(deserializer)? {
            None => Ok(None),
            Some(Stored::Number(v)) => Ok(Some(v)),
            Some(Stored::Text(t)) => t.parse().map(Some).map_err(serde::de::Error::custom),
        }
    }
}

/// `Layer.extra`: PSD bookkeeping the schema's named fields do not carry.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct LayerExtra {
    pub vector_mask: Option<VectorMask>,
    pub label: LabelColor,
    /// As a string: a replica's numbers are signed 64-bit, and a stored group id can be any u64.
    #[serde(with = "u64_text")]
    pub link_group: Option<u64>,
    pub excluded_channels: u32,
    pub blend_if: BlendIf,
    pub advanced: AdvancedBlending,
    /// Preserved PSD additional-layer-info blocks: (hex-encoded 4-byte key, the block's bytes).
    pub psd_blocks: Vec<(String, BlobRef)>,
    /// A video layer's (Layer › Video Layers) source and frame rate; its frames are the layer's
    /// `frames` list.
    pub video: Option<VideoMeta>,
}

/// [`kition_photo_doc::VideoData`] as `Layer.extra` stores it: all but the frames.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct VideoMeta {
    pub source: VideoSource,
    pub fps: f32,
    pub show_altered: bool,
}

impl LayerExtra {
    pub fn from_layer(layer: &Layer, width: u32, height: u32, blobs: &mut Sink<'_>) -> KdmResult<Self> {
        let video = layer.video.as_ref().map(|video| VideoMeta { source: video.source.clone(), fps: video.fps, show_altered: video.show_altered });
        // A newly written vector mask has no source block. Its canonical block adds no
        // information on reread; keep only source bytes the writer cannot regenerate exactly.
        let generated_mask = layer
            .vector_mask
            .as_ref()
            .filter(|_| !matches!(layer.content, LayerContent::Shape(_)))
            .map(|mask| kition_photo_io::vector_map::vector_mask_bytes(mask, width, height));
        Ok(Self {
            vector_mask: layer.vector_mask.clone(),
            label: layer.label,
            link_group: layer.link_group,
            excluded_channels: layer.excluded_channels,
            blend_if: layer.blend_if.clone(),
            advanced: layer.advanced,
            psd_blocks: layer
                .psd_blocks
                .iter()
                .filter(|(key, data)| *key != *b"vmsk" || generated_mask.as_ref().is_none_or(|generated| data.as_ref() != generated))
                .map(|(key, data)| (hex4(*key), embed(blobs, data)))
                .collect(),
            video,
        })
    }

    /// Applies the stored state to `layer`; a video layer's frames (`frames`, read from the
    /// layer's `frames` list) go with it.
    pub fn apply_to(self, layer: &mut Layer, frames: Vec<kition_photo_raster::Surface>, blobs: &dyn BlobSource) -> KdmResult<()> {
        layer.vector_mask = self.vector_mask;
        layer.label = self.label;
        layer.link_group = self.link_group;
        layer.excluded_channels = self.excluded_channels;
        layer.blend_if = self.blend_if;
        layer.advanced = self.advanced;
        layer.psd_blocks =
            self.psd_blocks.into_iter().map(|(key, blob)| Ok((unhex4(&key)?, Arc::new(fetch(blobs, &blob)?.to_vec())))).collect::<KdmResult<Vec<_>>>()?;
        layer.video = self.video.map(|video| kition_photo_doc::VideoData { frames, source: video.source, fps: video.fps, show_altered: video.show_altered });
        Ok(())
    }
}

/// Whether two contents are equal apart from their rendered caches (which are not replicated):
/// if so, a reloaded layer can keep the cache the view already rendered.
pub(crate) fn same_but_cache(a: &LayerContent, b: &LayerContent) -> bool {
    match (a, b) {
        (LayerContent::Text(a), LayerContent::Text(b)) => TextLayer { cache: None, ..a.clone() } == TextLayer { cache: None, ..b.clone() },
        (LayerContent::Shape(a), LayerContent::Shape(b)) => ShapeLayer { cache: None, ..a.clone() } == ShapeLayer { cache: None, ..b.clone() },
        (LayerContent::Smart(a), LayerContent::Smart(b)) => ObjectLayer { cache: None, ..a.clone() } == ObjectLayer { cache: None, ..b.clone() },
        _ => false,
    }
}

/// Puts what the view holds of a layer but the replica does not on `to`, read again from the
/// replica: a fill layer's Photoshop-rendered pixels (which
/// name the fill they were rendered for, so a changed fill ignores them), and the rendered
/// caches of type, shape and object layer content that is otherwise the same.
pub(crate) fn carry_caches(from: &Layer, to: &mut Layer) {
    to.fill_cache.clone_from(&from.fill_cache);
    if !same_but_cache(&from.content, &to.content) {
        return;
    }
    match (&from.content, &mut to.content) {
        (LayerContent::Text(a), LayerContent::Text(b)) => b.cache.clone_from(&a.cache),
        (LayerContent::Shape(a), LayerContent::Shape(b)) => b.cache.clone_from(&a.cache),
        (LayerContent::Smart(a), LayerContent::Smart(b)) => b.cache.clone_from(&a.cache),
        _ => {}
    }
}

/// A raster layer's default pixel as [`content_data`] stores it (transparent black when absent).
pub(crate) fn raster_default(data: &Value, channels: usize) -> Vec<f32> {
    let stored: Option<Vec<f32>> = data.get("defaultPixel").and_then(|v| serde_json::from_value(v.clone()).ok());
    match stored {
        Some(px) if px.len() == channels => px,
        _ => vec![0.0; channels],
    }
}

/// A type layer's `Layer.data`: all but its characters and runs (its `TextBody`).
#[derive(Serialize, Deserialize)]
struct TextData {
    font_family: String,
    size_pt: f32,
    color: Color,
    transform: Affine,
    shape: TextShape,
    orientation: Orientation,
    antialias: AntiAlias,
    #[serde(default)]
    warp: Option<TextWarp>,
    #[serde(default)]
    psd_raw: Option<BlobRef>,
}

#[derive(Serialize, Deserialize)]
struct ShapeData {
    path: Path,
    #[serde(default)]
    fill: Option<Fill>,
    #[serde(default)]
    stroke: Option<ShapeStroke>,
    #[serde(default)]
    live: Option<LiveShape>,
    #[serde(default)]
    psd_raw: Option<BlobRef>,
}

#[derive(Serialize, Deserialize)]
enum SmartSourceData {
    Embedded { file_name: String, blob: BlobRef },
    Linked { path: String },
}

#[derive(Serialize, Deserialize)]
struct SmartData {
    source: SmartSourceData,
    transform: Affine,
    #[serde(default)]
    editable_filters: Vec<EditableFilter>,
    #[serde(default)]
    psd_raw: Option<BlobRef>,
    filters_enabled: bool,
    #[serde(default)]
    warp: Option<Warp>,
    #[serde(default)]
    stack_mode: Option<StackMode>,
    #[serde(default)]
    perspective: Option<[f64; 9]>,
    #[serde(default)]
    filter_mask: Option<FilterMaskData>,
}

/// An object layer's filter mask: its surface embedded whole (one blob per tile) with the mask's
/// own settings.
#[derive(Serialize, Deserialize)]
struct FilterMaskData {
    surface: crate::doc_state::EmbeddedSurface,
    enabled: bool,
    linked: bool,
    density: f32,
    feather: f32,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct GroupData {
    artboard: Option<Artboard>,
}

/// The characters and runs of a type layer's content (its `TextBody`), `None` for other kinds.
pub(crate) fn text_body(content: &LayerContent) -> Option<Body> {
    match content {
        LayerContent::Text(text) => Some(Body { text: text.text.clone(), runs: text.runs.clone(), paragraphs: text.paragraphs.clone() }),
        _ => None,
    }
}

/// `kind` (the schema's enum) and `Layer.data` for one layer's content.
pub(crate) fn content_data(content: &LayerContent, blobs: &mut Sink<'_>) -> KdmResult<(&'static str, Value)> {
    match content {
        // Pixels are the `tiles` map; only a non-transparent default pixel (an adjusted empty
        // layer, a solid background built sparse) needs storing beside them.
        LayerContent::Raster(surface) => {
            let default_pixel = surface.default_pixel();
            Ok(("raster", if default_pixel.iter().all(|v| *v == 0.0) { Value::Null } else { serde_json::json!({ "defaultPixel": default_pixel }) }))
        }
        LayerContent::Group(Group { artboard, .. }) => serde_json::to_value(GroupData { artboard: artboard.clone() }).map(|v| ("group", v)).map_err(kdm_err),
        LayerContent::Adjustment(adjustment) => serde_json::to_value(adjustment).map(|v| ("adjustment", v)).map_err(kdm_err),
        LayerContent::Fill(fill) => serde_json::to_value(fill).map(|v| ("fill", v)).map_err(kdm_err),
        LayerContent::Text(text) => text_data(text, blobs).and_then(|v| serde_json::to_value(v).map_err(kdm_err)).map(|v| ("text", v)),
        LayerContent::Shape(shape) => shape_data(shape, blobs).and_then(|v| serde_json::to_value(v).map_err(kdm_err)).map(|v| ("shape", v)),
        LayerContent::Smart(smart) => smart_data(smart, blobs).and_then(|v| serde_json::to_value(v).map_err(kdm_err)).map(|v| ("smart", v)),
    }
}

fn text_data(text: &TextLayer, blobs: &mut dyn BlobSink) -> KdmResult<TextData> {
    Ok(TextData {
        font_family: text.font_family.clone(),
        size_pt: text.size_pt,
        color: text.color,
        transform: text.transform,
        shape: text.shape,
        orientation: text.orientation,
        antialias: text.antialias,
        warp: text.warp.clone(),
        psd_raw: text.psd_raw.as_ref().map(|data| embed(blobs, data)),
    })
}

fn shape_data(shape: &ShapeLayer, blobs: &mut dyn BlobSink) -> KdmResult<ShapeData> {
    Ok(ShapeData {
        path: shape.path.clone(),
        fill: shape.fill.clone(),
        stroke: shape.stroke.clone(),
        live: shape.live.clone(),
        psd_raw: shape.psd_raw.as_ref().map(|data| embed(blobs, data)),
    })
}

fn smart_data(smart: &ObjectLayer, blobs: &mut Sink<'_>) -> KdmResult<SmartData> {
    let source = match &smart.source {
        SmartSource::Embedded { file_name, bytes } => SmartSourceData::Embedded { file_name: file_name.clone(), blob: embed(blobs, bytes) },
        SmartSource::Linked { path } => SmartSourceData::Linked { path: path.clone() },
    };
    Ok(SmartData {
        source,
        transform: smart.transform,
        editable_filters: smart.editable_filters.clone(),
        psd_raw: smart.psd_raw.as_ref().map(|data| embed(blobs, data)),
        filters_enabled: smart.filters_enabled,
        warp: smart.warp.clone(),
        stack_mode: smart.stack_mode,
        perspective: smart.perspective,
        filter_mask: match &smart.filter_mask {
            Some(mask) => Some(FilterMaskData {
                surface: crate::doc_state::embed_surface(&mask.surface, blobs)?,
                enabled: mask.enabled,
                linked: mask.linked,
                density: mask.density,
                feather: mask.feather,
            }),
            None => None,
        },
    })
}

/// Rebuilds a layer's [`LayerContent`] from its `kind` string, `Layer.data` and (a type layer's)
/// `TextBody`, for every kind except `raster`: the caller builds that one directly from the layer
/// object's own `tiles` map (the document's raster pixel format, not anything in `data`, decides
/// how to decode them). A group reads expanded: whether it is is each person's own view.
pub(crate) fn content_from(kind: &str, data: Value, body: Option<Body>, blobs: &dyn BlobSource) -> KdmResult<LayerContent> {
    match kind {
        "group" => {
            let group: GroupData = if data.is_null() { GroupData::default() } else { serde_json::from_value(data).map_err(kdm_err)? };
            Ok(LayerContent::Group(Group { children: Vec::new(), expanded: true, artboard: group.artboard }))
        }
        "adjustment" => {
            let adjustment: Adjustment = serde_json::from_value(data).map_err(kdm_err)?;
            Ok(LayerContent::Adjustment(adjustment))
        }
        "fill" => {
            let fill: Fill = serde_json::from_value(data).map_err(kdm_err)?;
            Ok(LayerContent::Fill(fill))
        }
        "text" => {
            let text: TextData = serde_json::from_value(data).map_err(kdm_err)?;
            let body = body.unwrap_or_default();
            Ok(LayerContent::Text(TextLayer {
                text: body.text,
                font_family: text.font_family,
                size_pt: text.size_pt,
                color: text.color,
                transform: text.transform,
                cache: None,
                psd_raw: text.psd_raw.map(|blob| fetch(blobs, &blob).map(|bytes| Arc::new(bytes.to_vec()))).transpose()?,
                runs: body.runs,
                paragraphs: body.paragraphs,
                shape: text.shape,
                orientation: text.orientation,
                antialias: text.antialias,
                warp: text.warp,
            }))
        }
        "shape" => {
            let shape: ShapeData = serde_json::from_value(data).map_err(kdm_err)?;
            Ok(LayerContent::Shape(ShapeLayer {
                path: shape.path,
                fill: shape.fill,
                stroke: shape.stroke,
                live: shape.live,
                cache: None,
                psd_raw: shape.psd_raw.map(|blob| fetch(blobs, &blob).map(|bytes| Arc::new(bytes.to_vec()))).transpose()?,
            }))
        }
        "smart" => {
            let smart: SmartData = serde_json::from_value(data).map_err(kdm_err)?;
            let source = match smart.source {
                SmartSourceData::Embedded { file_name, blob } => SmartSource::Embedded { file_name, bytes: Arc::new(fetch(blobs, &blob)?.to_vec()) },
                SmartSourceData::Linked { path } => SmartSource::Linked { path },
            };
            Ok(LayerContent::Smart(ObjectLayer {
                source,
                transform: smart.transform,
                editable_filters: smart.editable_filters,
                cache: None,
                psd_raw: smart.psd_raw.map(|blob| fetch(blobs, &blob).map(|bytes| Arc::new(bytes.to_vec()))).transpose()?,
                filters_enabled: smart.filters_enabled,
                filter_mask: smart
                    .filter_mask
                    .map(|mask| -> KdmResult<LayerMask> {
                        Ok(LayerMask {
                            surface: crate::doc_state::extract_surface(&mask.surface, blobs)?,
                            enabled: mask.enabled,
                            linked: mask.linked,
                            density: mask.density,
                            feather: mask.feather,
                        })
                    })
                    .transpose()?,
                warp: smart.warp,
                stack_mode: smart.stack_mode,
                perspective: smart.perspective,
            }))
        }
        other => Err(KdmError::Engine(format!("unknown layer kind `{other}`"))),
    }
}

/// `Surface.props`: everything a mask or channel surface needs beyond its tiles.
#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub(crate) struct SurfaceMeta {
    pub key: Option<String>,
    /// A video frame's pixel format (a mask's and a channel's follow from what holds them).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<PixelFormat>,
    pub default_pixel: Vec<f32>,
    pub enabled: bool,
    pub linked: bool,
    pub density: f32,
    pub feather: f32,
    pub color: Option<Color>,
    pub opacity: f32,
    pub spot: Option<(Color, f32)>,
    pub indicates: Option<ColorIndicates>,
}

impl SurfaceMeta {
    pub fn for_mask(mask: &LayerMask) -> Self {
        Self {
            default_pixel: mask.surface.default_pixel(),
            enabled: mask.enabled,
            linked: mask.linked,
            density: mask.density,
            feather: mask.feather,
            ..Default::default()
        }
    }

    pub fn for_channel(channel: &AlphaChannel) -> Self {
        Self {
            key: channel.key.clone(),
            default_pixel: channel.surface.default_pixel(),
            color: Some(channel.color),
            opacity: channel.opacity,
            spot: channel.spot,
            indicates: Some(channel.indicates),
            ..Default::default()
        }
    }

    pub fn for_frame(frame: &kition_photo_raster::Surface) -> Self {
        Self { format: Some(frame.format()), default_pixel: frame.default_pixel(), ..Default::default() }
    }

    /// Applies the mask-relevant fields to a freshly built [`LayerMask`] (its surface is filled
    /// separately, from the same `Surface` object's `tiles`).
    pub fn apply_to_mask(&self, mask: &mut LayerMask) {
        mask.enabled = self.enabled;
        mask.linked = self.linked;
        mask.density = self.density;
        mask.feather = self.feather;
    }

    /// Applies the channel-relevant fields to a freshly built [`AlphaChannel`].
    pub fn apply_to_channel(&self, channel: &mut AlphaChannel) {
        channel.key = self.key.clone();
        if let Some(color) = self.color {
            channel.color = color;
        }
        channel.opacity = self.opacity;
        channel.spot = self.spot;
        if let Some(indicates) = self.indicates {
            channel.indicates = indicates;
        }
    }
}

/// [`Effects`] as one JSON value, its PSD bytes a blob: how a layer comp records a layer's
/// style. A layer stores its own style as `Layer.styles` and `Layer.styleSettings` instead.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct EffectsData {
    pub enabled: bool,
    pub items: Vec<Effect>,
    pub psd_raw: Option<BlobRef>,
    pub reference: Option<(f64, f64)>,
}

impl EffectsData {
    pub fn from_effects(effects: &Effects, blobs: &mut dyn BlobSink) -> Self {
        Self {
            enabled: effects.enabled,
            items: effects.items.clone(),
            psd_raw: effects.psd_raw.as_ref().map(|data| embed(blobs, data)),
            reference: effects.reference,
        }
    }

    pub fn into_effects(self, blobs: &dyn BlobSource) -> KdmResult<Effects> {
        Ok(Effects {
            enabled: self.enabled,
            items: self.items,
            psd_raw: self.psd_raw.map(|blob| fetch(blobs, &blob).map(|bytes| Arc::new(bytes.to_vec()))).transpose()?,
            reference: self.reference,
        })
    }
}

/// `Layer.styleSettings`: all of a layer's [`Effects`] but the effects themselves (each a
/// `Style` object of `Layer.styles`).
#[derive(Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct StyleSettings {
    pub enabled: bool,
    pub psd_raw: Option<BlobRef>,
    pub reference: Option<(f64, f64)>,
}

impl StyleSettings {
    pub fn of(effects: &Effects, blobs: &mut dyn BlobSink) -> Self {
        Self { enabled: effects.enabled, psd_raw: effects.psd_raw.as_ref().map(|data| embed(blobs, data)), reference: effects.reference }
    }

    pub fn into_effects(self, items: Vec<Effect>, blobs: &dyn BlobSource) -> KdmResult<Effects> {
        Ok(Effects {
            enabled: self.enabled,
            items,
            psd_raw: self.psd_raw.map(|blob| fetch(blobs, &blob).map(|bytes| Arc::new(bytes.to_vec()))).transpose()?,
            reference: self.reference,
        })
    }
}
