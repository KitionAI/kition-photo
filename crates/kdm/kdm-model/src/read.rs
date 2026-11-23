//! Replica → `Document`: rebuilds the engine's document model from a committed replica.
//!
//! **Layer ids.** A layer's engine id is what agents, commands and diagnostics call it (KHP A46,
//! A57), so every layer stores it in its read-only `number` field and every read gives it that id.
//! The writer stores it when it creates the layer. Two layers storing one number (made on two
//! replicas at once, each taking its document's next number) are told apart by a rule every read
//! applies alike: the one with the smaller object id keeps it, the other takes the next number
//! above every stored one, in reading order. A layer storing none (made through the object model,
//! or written before the numbers were stored) takes one the same way. The result depends only on
//! the replica's state, so every replica holding it agrees, whatever order the changes came in;
//! the replica's next edit stores the numbers a read gave ([`Read::unstored`]), and the repair
//! stores those of layers that store none ([`number_repairs`]).

use std::collections::{BTreeSet, HashMap, HashSet};

use kition_kit::kdm::{KdmError, ObjectId, Replica, TreeEntry};
use kition_kit::session::{BlobSource, IdMap};
use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{AlphaChannel, Document, Effect, Layer, LayerContent, LayerId, LayerMask, MAX_GROUP_DEPTH, Size};
use kition_photo_raster::Surface;
use serde_json::Value;

use crate::props::{LayerExtra, StyleSettings, SurfaceMeta};
use crate::write::Nodes;
use crate::{doc_state, guides, image_id, kdm_err, resources, text_body, tiles};

type KdmResult<T> = Result<T, KdmError>;

fn as_str(value: &Value, what: &str) -> KdmResult<String> {
    value.as_str().map(str::to_owned).ok_or_else(|| KdmError::Engine(format!("`{what}` is not a string")))
}

fn mode_of(text: &str) -> KdmResult<ColorMode> {
    serde_json::from_value(Value::String(text.to_owned())).map_err(kdm_err)
}

fn depth_of(text: &str) -> KdmResult<SampleType> {
    serde_json::from_value(Value::String(text.to_owned())).map_err(kdm_err)
}

/// The pixel format a raster layer's own tiles decode as: the document's mode and depth, with
/// alpha (every raster layer carries one).
fn raster_format(mode: ColorMode, depth: SampleType) -> PixelFormat {
    PixelFormat::new(mode, depth, true)
}

/// Masks are always 8-bit grayscale, regardless of the document's own depth (as the engine's
/// `LayerMask::reveal_all` builds them).
fn mask_format() -> PixelFormat {
    PixelFormat::GRAY8
}

/// An alpha/spot channel's surface: grayscale at the document's depth, no alpha of its own.
fn channel_format(depth: SampleType) -> PixelFormat {
    PixelFormat::new(ColorMode::Grayscale, depth, false)
}

/// What a read of the replica gives: the document, the `LayerId -> ObjectId` map and the
/// `ObjectId -> NodeRef` map a later [`crate::write_changes`] needs, and the layers whose id is
/// not the number they store.
pub struct Read {
    pub document: Document,
    pub ids: IdMap<LayerId>,
    pub nodes: Nodes,
    /// The layers this read gave another id than the number they store (two storing one) or that
    /// store none, with that id: the replica's next edit stores it, so it lasts.
    pub unstored: Vec<(ObjectId, u64)>,
}

/// Rebuilds the whole document from a committed replica.
pub fn read_document(replica: &Replica, blobs: &dyn BlobSource) -> KdmResult<Read> {
    read_document_reusing(replica, blobs, None)
}

/// A view to take unchanged layers from in [`read_document_reusing`]: `view`'s layers whose
/// objects (and mask surfaces) are not in `changed` are kept as they are, pixels, rendered caches
/// and all, instead of being read and decoded again. `ids` is the view's own id map.
pub struct Reuse<'a> {
    pub view: &'a Document,
    pub ids: &'a IdMap<LayerId>,
    pub changed: &'a BTreeSet<ObjectId>,
}

/// [`read_document`], taking unchanged layers from `reuse`'s view. A layer that is read again
/// keeps the view's rendered caches when only other state changed.
pub fn read_document_reusing(replica: &Replica, blobs: &dyn BlobSource, reuse: Option<Reuse<'_>>) -> KdmResult<Read> {
    // Layers made while reading take no numbers of a document being edited around this: every
    // id comes from the replica.
    let _detached = kition_photo_doc::detached();
    let image = image_id();
    let width = replica.get(&image, "width")?.as_u64().ok_or_else(|| KdmError::Engine("`width` is not an integer".into()))? as u32;
    let height = replica.get(&image, "height")?.as_u64().ok_or_else(|| KdmError::Engine("`height` is not an integer".into()))? as u32;
    let dpi = replica.get(&image, "dpi")?.as_f64().ok_or_else(|| KdmError::Engine("`dpi` is not a number".into()))? as f32;
    let mode = mode_of(&as_str(&replica.get(&image, "mode")?, "mode")?)?;
    let depth = depth_of(&as_str(&replica.get(&image, "depth")?, "depth")?)?;

    let mut document = Document::new("", Size { width, height }, mode, depth);
    document.resolution_dpi = dpi;

    let icc = replica.get(&image, "icc")?;
    if !icc.is_null() {
        let blob: kition_kit::session::BlobRef = serde_json::from_value(icc).map_err(kdm_err)?;
        let bytes = blobs.get(&blob.hash).ok_or_else(|| KdmError::Engine(format!("missing blob {}", blob.hash)))?;
        document.icc_profile = Some(std::sync::Arc::new(bytes.to_vec()));
    }

    document.guides = guides::read(replica, &image)?;

    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    // The view's layers by object, if its pixels can be reused: same mode and depth.
    let mut previous: HashMap<ObjectId, &Layer> = HashMap::new();
    let mut changed = None;
    if let Some(reuse) = &reuse
        && reuse.view.mode == mode
        && reuse.view.depth == depth
    {
        let mut by_id = HashMap::new();
        index(&reuse.view.layers, &mut by_id, 0);
        previous = by_id.into_iter().filter_map(|(id, layer)| Some((reuse.ids.object(id)?.clone(), layer))).collect();
        changed = Some(reuse.changed);
    }
    let old = Old { layers: previous, changed };
    let tree = Tree::read(replica, &image)?;
    let numbers = Numbers::mint(replica, &tree.order(replica))?;
    document.layers = read_layers(replica, &tree, mode, depth, blobs, &numbers.ids, &old, &mut ids, &mut nodes)?;

    for object in replica.list(&image, "channels")? {
        if !replica.exists(&object) {
            continue;
        }
        if let Some(channel) = read_channel(replica, &object, depth, blobs)? {
            document.channels.push(channel);
        }
    }

    doc_state::apply(&mut document, replica.get(&image, "docState")?, blobs)?;
    // After `ids`: a layer comp's states name layers by their KDM object id.
    resources::read(replica, &mut document, &ids, blobs)?;

    Ok(Read { document, ids, nodes, unstored: numbers.unstored })
}

/// The largest number a layer stores and is read with: the largest integer every replica holds
/// exactly (KitModel M4).
const MAX_NUMBER: u64 = 1 << 53;

/// The number `object` stores, if it is a layer id.
fn stored_number(replica: &Replica, object: &ObjectId) -> Option<u64> {
    replica.get(object, "number").ok()?.as_u64().filter(|n| (1..=MAX_NUMBER).contains(n))
}

/// The engine id of every layer of one read (see the module docs).
struct Numbers {
    ids: HashMap<ObjectId, LayerId>,
    unstored: Vec<(ObjectId, u64)>,
}

impl Numbers {
    /// Mints the ids of the layers `order` lists (in reading order): a layer keeps the number it
    /// stores unless another stores it too, in which case the one with the smaller object id
    /// keeps it; every other layer takes the next number above every stored one, in order.
    fn mint(replica: &Replica, order: &[ObjectId]) -> KdmResult<Self> {
        let stored: Vec<Option<u64>> = order.iter().map(|object| stored_number(replica, object)).collect();
        let mut holders: HashMap<u64, &ObjectId> = HashMap::new();
        for (object, number) in order.iter().zip(&stored) {
            if let Some(number) = number {
                holders.entry(*number).and_modify(|holder| *holder = (*holder).min(object)).or_insert(object);
            }
        }
        let mut taken: HashSet<u64> = holders.keys().copied().collect();
        let mut next = taken.iter().copied().max().unwrap_or(0);
        let mut ids = HashMap::with_capacity(order.len());
        let mut unstored = Vec::new();
        for (object, number) in order.iter().zip(stored) {
            let id = match number {
                Some(number) if holders.get(&number) == Some(&object) => number,
                _ => {
                    // Above every stored number; from 1 if a stored number is the largest a
                    // replica holds.
                    let free = |from: u64| (from..=MAX_NUMBER).find(|n| !taken.contains(n));
                    next = free(next.saturating_add(1)).or_else(|| free(1)).ok_or_else(|| KdmError::Engine("no layer number left".into()))?;
                    taken.insert(next);
                    unstored.push((object.clone(), next));
                    next
                }
            };
            ids.insert(object.clone(), LayerId(id));
        }
        Ok(Self { ids, unstored })
    }
}

/// The layers that store no number, with the number every read gives them: for the repair to
/// store (a layer made through the object model, or by a kit before the numbers were stored).
/// Layers that lost a number another layer also stores are left to the next edit, so a merge
/// writes nothing of its own.
pub(crate) fn number_repairs(replica: &Replica) -> KdmResult<Vec<(ObjectId, u64)>> {
    let tree = Tree::read(replica, &image_id())?;
    let numbers = Numbers::mint(replica, &tree.order(replica))?;
    Ok(numbers.unstored.into_iter().filter(|(object, _)| stored_number(replica, object).is_none()).collect())
}

/// The previous view's layers for [`read_document_reusing`].
struct Old<'a> {
    layers: HashMap<ObjectId, &'a Layer>,
    /// `None`: nothing is reused (no view, or its mode or depth differ).
    changed: Option<&'a BTreeSet<ObjectId>>,
}

fn index<'a>(layers: &'a [Layer], out: &mut HashMap<LayerId, &'a Layer>, depth: usize) {
    if depth > MAX_GROUP_DEPTH {
        return;
    }
    for layer in layers {
        out.insert(layer.id, layer);
        if let LayerContent::Group(group) = &layer.content {
            index(&group.children, out, depth + 1);
        }
    }
}

/// The `layers` tree of the image, with each node's children and the roots, by entry index.
struct Tree {
    entries: Vec<TreeEntry>,
    children_of: Vec<Vec<usize>>,
    roots: Vec<usize>,
}

impl Tree {
    fn read(replica: &Replica, image: &ObjectId) -> KdmResult<Self> {
        let entries = replica.tree(image, "layers")?;
        // `TreeEntry::node` has no `Hash` (see `kition_kdm::NodeRef`); matching parents by
        // equality is O(entries) per node, fine at the layer counts documents use.
        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); entries.len()];
        let mut roots: Vec<usize> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            match &entry.parent {
                None => roots.push(index),
                Some(parent) => match entries.iter().position(|candidate| &candidate.node == parent) {
                    Some(parent_index) => children_of[parent_index].push(index),
                    None => roots.push(index),
                },
            }
        }
        Ok(Self { entries, children_of, roots })
    }

    /// The layers in reading order (each before its children), as [`read_layers`] reads them.
    fn order(&self, replica: &Replica) -> Vec<ObjectId> {
        fn walk(tree: &Tree, index: usize, replica: &Replica, out: &mut Vec<ObjectId>, remaining_depth: usize) {
            let Some(entry) = tree.entries.get(index) else { return };
            if remaining_depth == 0 || !replica.exists(&entry.object) {
                return;
            }
            out.push(entry.object.clone());
            // Only a group's children are read (a node under another kind of layer is not).
            if replica.get(&entry.object, "kind").ok().as_ref().and_then(Value::as_str) != Some("group") {
                return;
            }
            for &child in tree.children_of.get(index).into_iter().flatten() {
                walk(tree, child, replica, out, remaining_depth - 1);
            }
        }
        let mut out = Vec::with_capacity(self.entries.len());
        for &root in &self.roots {
            walk(self, root, replica, &mut out, MAX_GROUP_DEPTH + 1);
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn read_layers(
    replica: &Replica,
    tree: &Tree,
    mode: ColorMode,
    depth: SampleType,
    blobs: &dyn BlobSource,
    numbers: &HashMap<ObjectId, LayerId>,
    old: &Old<'_>,
    ids: &mut IdMap<LayerId>,
    nodes: &mut Nodes,
) -> KdmResult<Vec<Layer>> {
    struct BuildCtx<'a> {
        tree: &'a Tree,
        replica: &'a Replica,
        mode: ColorMode,
        depth: SampleType,
        blobs: &'a dyn BlobSource,
        numbers: &'a HashMap<ObjectId, LayerId>,
        old: &'a Old<'a>,
    }

    fn build(index: usize, ctx: &BuildCtx<'_>, ids: &mut IdMap<LayerId>, nodes: &mut Nodes, remaining_depth: usize) -> KdmResult<Option<Layer>> {
        if remaining_depth == 0 {
            return Err(KdmError::Engine(format!("layer groups nested deeper than {MAX_GROUP_DEPTH}")));
        }
        let entry = &ctx.tree.entries[index];
        let object = &entry.object;
        if !ctx.replica.exists(object) {
            return Ok(None);
        }
        let id = ctx.numbers.get(object).copied().ok_or_else(|| KdmError::Engine(format!("layer {object} has no number")))?;
        let before = ctx.old.layers.get(object).copied();
        let unchanged = ctx.old.changed.is_some_and(|changed| {
            !changed.contains(object) && parts(ctx.replica, object).is_some_and(|parts| parts.iter().all(|part| !changed.contains(part)))
        });
        let mut layer = match before {
            Some(before) if unchanged => {
                let mut layer = before.clone();
                if let LayerContent::Group(group) = &mut layer.content {
                    group.children.clear();
                }
                layer
            }
            _ => {
                let mut layer = read_layer(ctx.replica, object, ctx.mode, ctx.depth, ctx.blobs)?;
                if let Some(before) = before {
                    crate::props::carry_caches(before, &mut layer);
                }
                layer
            }
        };
        layer.id = id;
        ids.insert(object.clone(), layer.id);
        nodes.insert(object.clone(), entry.node.clone());
        if let LayerContent::Group(group) = &mut layer.content {
            for &child_index in &ctx.tree.children_of[index] {
                if let Some(child) = build(child_index, ctx, ids, nodes, remaining_depth - 1)? {
                    group.children.push(child);
                }
            }
        }
        Ok(Some(layer))
    }
    let ctx = BuildCtx { tree, replica, mode, depth, blobs, numbers, old };

    let mut layers = Vec::new();
    for &root in &tree.roots {
        if let Some(layer) = build(root, &ctx, ids, nodes, MAX_GROUP_DEPTH + 1)? {
            layers.push(layer);
        }
    }
    Ok(layers)
}

/// The objects a layer holds (its mask, text body, styles and video frames), or `None` if they
/// cannot be read.
fn parts(replica: &Replica, object: &ObjectId) -> Option<Vec<ObjectId>> {
    let mut out = Vec::new();
    for field in ["mask", "text"] {
        if let Value::String(part) = replica.get(object, field).ok()? {
            out.push(ObjectId::from_string(part));
        }
    }
    for field in ["styles", "frames"] {
        out.extend(replica.list(object, field).ok()?);
    }
    Some(out)
}

pub(crate) fn read_layer(replica: &Replica, object: &ObjectId, mode: ColorMode, depth: SampleType, blobs: &dyn BlobSource) -> KdmResult<Layer> {
    let name = as_str(&replica.get(object, "name")?, "name")?;
    let visible = replica.get(object, "visible")?.as_bool().unwrap_or(true);
    let locks_value = replica.get(object, "locks")?;
    let locks = if locks_value.is_null() { Default::default() } else { serde_json::from_value(locks_value).map_err(kdm_err)? };
    let blend = serde_json::from_value(replica.get(object, "blend")?).map_err(kdm_err)?;
    let opacity = replica.get(object, "opacity")?.as_f64().unwrap_or(1.0) as f32;
    let fill_opacity = replica.get(object, "fillOpacity")?.as_f64().unwrap_or(1.0) as f32;
    let clipped = replica.get(object, "clipped")?.as_bool().unwrap_or(false);
    let kind = as_str(&replica.get(object, "kind")?, "kind")?;

    let data = replica.get(object, "data")?;
    let extra_value = replica.get(object, "extra")?;
    let extra: LayerExtra = if extra_value.is_null() { LayerExtra::default() } else { serde_json::from_value(extra_value).map_err(kdm_err)? };

    let content = if kind == "raster" {
        let format = raster_format(mode, depth);
        LayerContent::Raster(tiles::read_all(replica, object, format, &crate::props::raster_default(&data, format.channels()), blobs)?)
    } else {
        let body = match replica.get(object, "text")? {
            Value::String(id) if kind == "text" && replica.exists(&ObjectId::from_string(id.as_str())) => {
                Some(text_body::read(replica, &ObjectId::from_string(id))?)
            }
            _ => None,
        };
        crate::props::content_from(&kind, data, body, blobs)?
    };

    let mask = match replica.get(object, "mask")? {
        Value::String(surface_id) => Some(read_mask(replica, &ObjectId::from_string(surface_id), blobs)?),
        _ => None,
    };

    let settings_value = replica.get(object, "styleSettings")?;
    let settings: StyleSettings = if settings_value.is_null() { StyleSettings::default() } else { serde_json::from_value(settings_value).map_err(kdm_err)? };
    let mut items = Vec::new();
    for style in replica.list(object, "styles")? {
        if replica.exists(&style) {
            items.push(serde_json::from_value::<Effect>(replica.get(&style, "effect")?).map_err(kdm_err)?);
        }
    }
    let effects = settings.into_effects(items, blobs)?;

    let mut frames = Vec::new();
    if extra.video.is_some() {
        for frame in replica.list(object, "frames")? {
            if replica.exists(&frame) {
                frames.push(read_frame(replica, &frame, raster_format(mode, depth), blobs)?);
            }
        }
    }

    let mut layer = Layer::new(name, content);
    layer.visible = visible;
    layer.locks = locks;
    layer.blend = blend;
    layer.opacity = opacity;
    layer.fill_opacity = fill_opacity;
    layer.clipped = clipped;
    layer.mask = mask;
    layer.effects = effects;
    extra.apply_to(&mut layer, frames, blobs)?;
    Ok(layer)
}

/// A video frame: its own format (the layer's raster format if it stores none).
fn read_frame(replica: &Replica, surface_id: &ObjectId, format: PixelFormat, blobs: &dyn BlobSource) -> KdmResult<Surface> {
    let props_value = replica.get(surface_id, "props")?;
    let meta: SurfaceMeta = if props_value.is_null() { SurfaceMeta::default() } else { serde_json::from_value(props_value).map_err(kdm_err)? };
    let format = meta.format.unwrap_or(format);
    let default_pixel = if meta.default_pixel.len() == format.channels() { meta.default_pixel.clone() } else { vec![0.0; format.channels()] };
    tiles::read_all(replica, surface_id, format, &default_pixel, blobs)
}

fn read_mask(replica: &Replica, surface_id: &ObjectId, blobs: &dyn BlobSource) -> KdmResult<LayerMask> {
    let props_value = replica.get(surface_id, "props")?;
    let meta: SurfaceMeta = if props_value.is_null() { SurfaceMeta::default() } else { serde_json::from_value(props_value).map_err(kdm_err)? };
    let default_pixel = if meta.default_pixel.is_empty() { vec![1.0] } else { meta.default_pixel.clone() };
    let surface = tiles::read_all(replica, surface_id, mask_format(), &default_pixel, blobs)?;
    let mut mask = LayerMask { surface, enabled: true, linked: true, density: 1.0, feather: 0.0 };
    meta.apply_to_mask(&mut mask);
    Ok(mask)
}

fn read_channel(replica: &Replica, object: &ObjectId, depth: SampleType, blobs: &dyn BlobSource) -> KdmResult<Option<AlphaChannel>> {
    let name = as_str(&replica.get(object, "name")?, "name")?;
    let surface_id = match replica.get(object, "surface")? {
        Value::String(id) => ObjectId::from_string(id),
        _ => return Ok(None),
    };
    if !replica.exists(&surface_id) {
        return Ok(None);
    }
    let props_value = replica.get(&surface_id, "props")?;
    let meta: SurfaceMeta = if props_value.is_null() { SurfaceMeta::default() } else { serde_json::from_value(props_value).map_err(kdm_err)? };
    let default_pixel = if meta.default_pixel.is_empty() { vec![0.0] } else { meta.default_pixel.clone() };
    let surface = tiles::read_all(replica, &surface_id, channel_format(depth), &default_pixel, blobs)?;
    let mut channel = AlphaChannel::new(name, surface);
    meta.apply_to_channel(&mut channel);
    Ok(Some(channel))
}
