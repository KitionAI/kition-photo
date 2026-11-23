//! `Image.docState`: what in [`Document`] neither the named schema fields nor the object lists
//! ([`crate::resources`], [`crate::guides`]) cover: metadata, global light, the work and clipping
//! paths, the colour table and duotone inks, measurement, text styles, slices, variables and the
//! timeline. Not per-person state (`selection`, the quick mask, the timeline's playhead, the last
//! applied comp and the Last Document State), nor rendered-pixel caches. One JSON register,
//! last-writer-wins as a whole (a concurrent edit of two of these rarely used features picks one
//! side, same as any other register).

use kition_kit::kdm::KdmError;
use kition_kit::session::{BlobRef, BlobSource};
use kition_photo_color::PixelFormat;
use kition_photo_doc::{ClippingPath, ColorTable, Document, Duotone, GlobalLight, Measurement, Metadata, Path, Slices, TextStyles, Timeline, Variables};
use kition_photo_geom::TileCoord;
use kition_photo_raster::Surface;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kdm_err;
use crate::props;
use crate::tiles::Sink;

type KdmResult<T> = Result<T, KdmError>;

/// A surface embedded whole in a JSON register (not a KDM map): every tile is still a blob (so
/// pixel bytes never sit in the replica itself), but the set of tiles is this one register's
/// value, not individually mergeable. Fine for surfaces edited whole (a pattern's pixels, an
/// object layer's filter mask); layer, mask and video frame surfaces keep their own `tiles` map.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct EmbeddedSurface {
    format: PixelFormat,
    default_pixel: Vec<f32>,
    tiles: Vec<(String, BlobRef)>,
}

pub(crate) fn embed_surface(surface: &Surface, blobs: &mut Sink<'_>) -> KdmResult<EmbeddedSurface> {
    let format = surface.format();
    let mut tiles = Vec::with_capacity(surface.tile_count());
    for (coord, tile) in surface.tiles() {
        tiles.push((format!("{},{}", coord.tx, coord.ty), blobs.cache.blob(format, tile, blobs.blobs)));
    }
    Ok(EmbeddedSurface { format, default_pixel: surface.default_pixel(), tiles })
}

pub(crate) fn extract_surface(embedded: &EmbeddedSurface, blobs: &dyn BlobSource) -> KdmResult<Surface> {
    let mut surface = Surface::with_default(embedded.format, &embedded.default_pixel);
    for (key, blob) in &embedded.tiles {
        let (tx, ty) = key.split_once(',').ok_or_else(|| KdmError::Engine(format!("bad embedded tile key `{key}`")))?;
        let coord = TileCoord { tx: tx.parse().map_err(kdm_err)?, ty: ty.parse().map_err(kdm_err)? };
        let bytes = blobs.get(&blob.hash).ok_or_else(|| KdmError::Engine(format!("missing embedded tile blob {}", blob.hash)))?;
        let decoded = kition_photo_format::decode_tile(embedded.format, &bytes).map_err(kdm_err)?;
        surface.tile_mut(coord).bytes_mut().copy_from_slice(&decoded);
    }
    Ok(surface)
}

/// What `docState` holds (see the module docs).
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct DocState {
    xmp: Option<String>,
    exif: Option<BlobRef>,
    text: Vec<(String, String)>,
    psd_resources: Vec<(u16, String, BlobRef)>,
    /// `(hex signature, hex key, data)`.
    psd_global_blocks: Vec<(String, String, BlobRef)>,
    global_light: GlobalLight,
    work_path: Option<Path>,
    clipping_path: Option<ClippingPath>,
    color_table: Option<ColorTable>,
    duotone: Option<Duotone>,
    measurement: Measurement,
    text_styles: TextStyles,
    slices: Slices,
    variables: Variables,
    timeline: Option<Timeline>,
}

fn hex4(key: [u8; 4]) -> String {
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex4(text: &str) -> KdmResult<[u8; 4]> {
    if text.len() != 8 {
        return Err(KdmError::Engine(format!("bad 4-byte key `{text}`")));
    }
    let mut out = [0u8; 4];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(text.get(index * 2..index * 2 + 2).unwrap_or(""), 16).map_err(kdm_err)?;
    }
    Ok(out)
}

/// The timeline without its playhead, which is each person's own.
fn shared_timeline(timeline: &Option<Timeline>) -> Option<Timeline> {
    timeline.clone().map(|timeline| Timeline { current: 0, ..timeline })
}

fn build(document: &Document, blobs: &mut Sink<'_>) -> DocState {
    let meta = &document.metadata;
    DocState {
        xmp: meta.xmp.clone(),
        exif: meta.exif.as_ref().map(|data| props::embed(blobs, data)),
        text: meta.text.clone(),
        psd_resources: meta.psd_resources.iter().map(|(id, name, data)| (*id, name.clone(), props::embed(blobs, data))).collect(),
        psd_global_blocks: meta.psd_global_blocks.iter().map(|(sig, key, data)| (hex4(*sig), hex4(*key), props::embed(blobs, data))).collect(),
        global_light: document.global_light,
        work_path: document.work_path.clone(),
        clipping_path: document.clipping_path.clone(),
        color_table: document.color_table.clone(),
        duotone: document.duotone.clone(),
        measurement: document.measurement.clone(),
        text_styles: document.text_styles.clone(),
        slices: document.slices.clone(),
        variables: document.variables.clone(),
        timeline: shared_timeline(&document.timeline),
    }
}

/// `Image.docState`'s value for `document`, for a full write or to compare against the stored
/// value in [`diff`].
pub(crate) fn to_value(document: &Document, blobs: &mut Sink<'_>) -> KdmResult<Value> {
    serde_json::to_value(build(document, blobs)).map_err(kdm_err)
}

/// Whether `before` and `after` would write different `docState` values; if so, `after`'s value.
/// The whole register is last-writer-wins, so there is nothing finer to diff.
pub(crate) fn diff(before: &Document, after: &Document, blobs: &mut Sink<'_>) -> KdmResult<Option<Value>> {
    if same_fields(before, after) {
        return Ok(None);
    }
    let before_value = to_value(before, blobs)?;
    let after_value = to_value(after, blobs)?;
    Ok((before_value != after_value).then_some(after_value))
}

/// Whether every field [`build`] reads is equal.
fn same_fields(a: &Document, b: &Document) -> bool {
    a.metadata == b.metadata
        && a.global_light == b.global_light
        && a.work_path == b.work_path
        && a.clipping_path == b.clipping_path
        && a.color_table == b.color_table
        && a.duotone == b.duotone
        && a.measurement == b.measurement
        && a.text_styles == b.text_styles
        && a.slices == b.slices
        && a.variables == b.variables
        && shared_timeline(&a.timeline) == shared_timeline(&b.timeline)
}

/// Applies a stored `docState` value onto `document` (freshly built: its own state for every
/// field here is still the default).
pub(crate) fn apply(document: &mut Document, value: Value, blobs: &dyn BlobSource) -> KdmResult<()> {
    if value.is_null() {
        return Ok(());
    }
    let state: DocState = serde_json::from_value(value).map_err(kdm_err)?;
    document.metadata = Metadata {
        xmp: state.xmp,
        exif: state.exif.map(|blob| props::fetch(blobs, &blob).map(|bytes| std::sync::Arc::new(bytes.to_vec()))).transpose()?,
        text: state.text,
        psd_resources: state
            .psd_resources
            .into_iter()
            .map(|(id, name, blob)| Ok((id, name, std::sync::Arc::new(props::fetch(blobs, &blob)?.to_vec()))))
            .collect::<KdmResult<Vec<_>>>()?,
        psd_global_blocks: state
            .psd_global_blocks
            .into_iter()
            .map(|(sig, key, blob)| Ok((unhex4(&sig)?, unhex4(&key)?, std::sync::Arc::new(props::fetch(blobs, &blob)?.to_vec()))))
            .collect::<KdmResult<Vec<_>>>()?,
    };
    document.global_light = state.global_light;
    document.work_path = state.work_path;
    document.clipping_path = state.clipping_path;
    document.color_table = state.color_table;
    document.duotone = state.duotone;
    document.measurement = state.measurement;
    document.text_styles = state.text_styles;
    document.slices = state.slices;
    document.variables = state.variables;
    document.timeline = state.timeline;
    Ok(())
}
