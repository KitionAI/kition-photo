//! `Image.docState`: everything in [`Document`] the named schema fields do not cover, besides
//! per-person state (`selection`, and the quick mask, which is the selection being edited as a
//! channel) and rendered-pixel caches, so none of it is silently dropped by
//! a round trip through the replica. One JSON register, last-writer-wins as a whole (a
//! concurrent edit of two of these rarely used features picks one side, same as any other
//! register) — unlike layers, surfaces and tiles, which are each their own object or map entry.
//!
//! A few of these fields hold pixels (patterns) or reference a layer by its
//! engine-local [`LayerId`] (layer comps): pixels go through [`BlobSink`]/[`BlobSource`] like
//! tiles (just not through a KDM map, since nothing needs per-tile merging here), and a layer
//! reference is stored as the layer's KDM [`ObjectId`] string and resolved back through the same
//! [`IdMap`] [`crate::read_document`] builds, so it still names the right layer after a reload
//! mints that layer a new local id.

use kition_kit::kdm::{KdmError, ObjectId};
use kition_kit::session::{BlobRef, BlobSink, BlobSource, IdMap};
use kition_photo_color::PixelFormat;
use kition_photo_doc::{
    ClippingPath, ColorTable, CompAppearance, CompLayerState, Document, Duotone, GlobalLight, LayerComp, LayerId, Measurement, Metadata, NamedPath, Note, Path,
    Pattern, Slices, TextStyles, Timeline, Variables,
};
use kition_photo_geom::TileCoord;
use kition_photo_raster::Surface;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kdm_err;
use crate::props::{self, EffectsData};
use crate::tiles::Sink;

type KdmResult<T> = Result<T, KdmError>;

/// A surface embedded whole in a JSON register (not a KDM map): every tile is still a blob (so
/// pixel bytes never sit in the replica itself), but the set of tiles is this one register's
/// value, not individually mergeable. Fine for the rarely edited, document-level surfaces here
/// (patterns); layer and mask surfaces keep their own `tiles` map.
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

#[derive(Serialize, Deserialize)]
struct DocPattern {
    id: String,
    name: String,
    width: u32,
    height: u32,
    surface: EmbeddedSurface,
}

#[derive(Serialize, Deserialize)]
struct DocNamedPath {
    #[serde(default)]
    key: Option<String>,
    name: String,
    path: Path,
    #[serde(default)]
    psd_raw: Option<BlobRef>,
}

#[derive(Serialize, Deserialize)]
struct DocCompAppearance {
    blend: kition_photo_doc::BlendMode,
    opacity: f32,
    fill_opacity: f32,
    effects: EffectsData,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct DocCompLayerState {
    /// The layer's KDM object id, or `""` if it could not be resolved when this was written.
    layer: String,
    visible: Option<bool>,
    position: Option<(i32, i32)>,
    appearance: Option<DocCompAppearance>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct DocLayerComp {
    id: u32,
    name: String,
    comment: String,
    apply_visibility: bool,
    apply_position: bool,
    apply_appearance: bool,
    states: Vec<DocCompLayerState>,
}

fn comp_to_doc(comp: &LayerComp, ids: &IdMap<LayerId>, blobs: &mut dyn BlobSink) -> DocLayerComp {
    let mut states = Vec::with_capacity(comp.states.len());
    for state in &comp.states {
        let appearance = state.appearance.as_ref().map(|appearance| DocCompAppearance {
            blend: appearance.blend,
            opacity: appearance.opacity,
            fill_opacity: appearance.fill_opacity,
            effects: EffectsData::from_effects(&appearance.effects, blobs),
        });
        states.push(DocCompLayerState {
            layer: ids.object(state.layer).map(ObjectId::as_str).unwrap_or_default().to_owned(),
            visible: state.visible,
            position: state.position,
            appearance,
        });
    }
    DocLayerComp {
        id: comp.id,
        name: comp.name.clone(),
        comment: comp.comment.clone(),
        apply_visibility: comp.apply_visibility,
        apply_position: comp.apply_position,
        apply_appearance: comp.apply_appearance,
        states,
    }
}

fn doc_to_comp(doc: DocLayerComp, ids: &IdMap<LayerId>, blobs: &dyn BlobSource) -> KdmResult<LayerComp> {
    let mut states = Vec::with_capacity(doc.states.len());
    for state in doc.states {
        // A layer this comp recorded that no longer exists (deleted since): drop it, like
        // `LayerComp::missing_layers` already treats a comp whose layer disappeared.
        let Some(layer) = ids.local(&ObjectId::from_string(state.layer)) else { continue };
        let appearance = match state.appearance {
            Some(appearance) => Some(CompAppearance {
                blend: appearance.blend,
                opacity: appearance.opacity,
                fill_opacity: appearance.fill_opacity,
                effects: appearance.effects.into_effects(blobs)?,
            }),
            None => None,
        };
        states.push(CompLayerState { layer, visible: state.visible, position: state.position, appearance });
    }
    Ok(LayerComp {
        id: doc.id,
        name: doc.name,
        comment: doc.comment,
        apply_visibility: doc.apply_visibility,
        apply_position: doc.apply_position,
        apply_appearance: doc.apply_appearance,
        states,
    })
}

/// Everything in [`Document`] the named schema fields do not cover (see the module docs).
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
    paths: Vec<DocNamedPath>,
    work_path: Option<Path>,
    clipping_path: Option<ClippingPath>,
    patterns: Vec<DocPattern>,
    color_table: Option<ColorTable>,
    duotone: Option<Duotone>,
    layer_comps: Vec<DocLayerComp>,
    last_applied_comp: Option<u32>,
    last_document_state: Option<DocLayerComp>,
    measurement: Measurement,
    notes: Vec<Note>,
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

fn build(document: &Document, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<DocState> {
    let meta = &document.metadata;
    let mut patterns = Vec::with_capacity(document.patterns.len());
    for pattern in &document.patterns {
        patterns.push(DocPattern {
            id: pattern.id.clone(),
            name: pattern.name.clone(),
            width: pattern.width,
            height: pattern.height,
            surface: embed_surface(&pattern.surface, blobs)?,
        });
    }
    Ok(DocState {
        xmp: meta.xmp.clone(),
        exif: meta.exif.as_ref().map(|data| props::embed(blobs, data)),
        text: meta.text.clone(),
        psd_resources: meta.psd_resources.iter().map(|(id, name, data)| (*id, name.clone(), props::embed(blobs, data))).collect(),
        psd_global_blocks: meta.psd_global_blocks.iter().map(|(sig, key, data)| (hex4(*sig), hex4(*key), props::embed(blobs, data))).collect(),
        global_light: document.global_light,
        paths: document
            .paths
            .iter()
            .map(|named| DocNamedPath {
                key: named.key.clone(),
                name: named.name.clone(),
                path: named.path.clone(),
                psd_raw: named.psd_raw.as_ref().map(|data| props::embed(blobs, data)),
            })
            .collect(),
        work_path: document.work_path.clone(),
        clipping_path: document.clipping_path.clone(),
        patterns,
        color_table: document.color_table.clone(),
        duotone: document.duotone.clone(),
        layer_comps: document.layer_comps.iter().map(|comp| comp_to_doc(comp, ids, blobs)).collect(),
        last_applied_comp: document.last_applied_comp,
        last_document_state: document.last_document_state.as_ref().map(|comp| comp_to_doc(comp, ids, blobs)),
        measurement: document.measurement.clone(),
        notes: document.notes.clone(),
        text_styles: document.text_styles.clone(),
        slices: document.slices.clone(),
        variables: document.variables.clone(),
        timeline: document.timeline.clone(),
    })
}

/// `Image.docState`'s value for `document`, for a full write or to compare against the stored
/// value in [`diff`].
pub(crate) fn to_value(document: &Document, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<Value> {
    serde_json::to_value(build(document, ids, blobs)?).map_err(kdm_err)
}

/// Whether `before` and `after` would write different `docState` values; if so, `after`'s value.
/// The whole register is last-writer-wins, so there is nothing finer to diff.
pub(crate) fn diff(before: &Document, after: &Document, ids: &IdMap<LayerId>, blobs: &mut Sink<'_>) -> KdmResult<Option<Value>> {
    // Building the value encodes every pattern tile: skip it for the common edit that touches
    // none of these fields. Layer comps name layers through `ids`, which the same edit can change
    // (a new layer, a delete), so they always take the full comparison when there are any.
    if before.layer_comps.is_empty()
        && after.layer_comps.is_empty()
        && before.last_document_state.is_none()
        && after.last_document_state.is_none()
        && same_fields(before, after)
    {
        return Ok(None);
    }
    let before_value = to_value(before, ids, blobs)?;
    let after_value = to_value(after, ids, blobs)?;
    Ok((before_value != after_value).then_some(after_value))
}

/// Whether every field [`build`] reads, except layer comps, is equal.
fn same_fields(a: &Document, b: &Document) -> bool {
    a.metadata == b.metadata
        && a.global_light == b.global_light
        && a.paths == b.paths
        && a.work_path == b.work_path
        && a.clipping_path == b.clipping_path
        && a.patterns == b.patterns
        && a.color_table == b.color_table
        && a.duotone == b.duotone
        && a.last_applied_comp == b.last_applied_comp
        && a.measurement == b.measurement
        && a.notes == b.notes
        && a.text_styles == b.text_styles
        && a.slices == b.slices
        && a.variables == b.variables
        && a.timeline == b.timeline
}

/// Applies a stored `docState` value onto `document` (freshly built: its own state for every
/// field here is still the default). Layer comp states whose layer no longer resolves through
/// `ids` are dropped, like an already-missing layer in a comp Photoshop never recorded this way.
pub(crate) fn apply(document: &mut Document, value: Value, ids: &IdMap<LayerId>, blobs: &dyn BlobSource) -> KdmResult<()> {
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
    document.paths = state
        .paths
        .into_iter()
        .map(|named| -> KdmResult<NamedPath> {
            Ok(NamedPath {
                key: named.key,
                name: named.name,
                path: named.path,
                psd_raw: named.psd_raw.map(|blob| props::fetch(blobs, &blob).map(|bytes| std::sync::Arc::new(bytes.to_vec()))).transpose()?,
            })
        })
        .collect::<KdmResult<Vec<_>>>()?;
    document.work_path = state.work_path;
    document.clipping_path = state.clipping_path;
    document.patterns = state
        .patterns
        .into_iter()
        .map(|pattern| -> KdmResult<Pattern> {
            Ok(Pattern { id: pattern.id, name: pattern.name, width: pattern.width, height: pattern.height, surface: extract_surface(&pattern.surface, blobs)? })
        })
        .collect::<KdmResult<Vec<_>>>()?;
    document.color_table = state.color_table;
    document.duotone = state.duotone;
    document.layer_comps = state.layer_comps.into_iter().map(|comp| doc_to_comp(comp, ids, blobs)).collect::<KdmResult<Vec<_>>>()?;
    document.last_applied_comp = state.last_applied_comp;
    document.last_document_state = state.last_document_state.map(|comp| doc_to_comp(comp, ids, blobs)).transpose()?;
    document.measurement = state.measurement;
    document.notes = state.notes;
    document.text_styles = state.text_styles;
    document.slices = state.slices;
    document.variables = state.variables;
    document.timeline = state.timeline;
    Ok(())
}
