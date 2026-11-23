//! `Document` → replica: a full write (new document, PSD/PSB import) and an incremental diff
//! (the engine command bridge, Task 3) that writes only what changed.

use std::collections::HashMap;

use kition_kit::kdm::{KdmError, NodeRef, ObjectId, Tx};
use kition_kit::session::{BlobSink, IdMap};
use kition_photo_doc::{Document, Layer, LayerContent, LayerId, MAX_GROUP_DEPTH};
use serde_json::{Map, Value, json};

use crate::lists::{self, Items};
use crate::props::{self, LayerExtra, StyleSettings, SurfaceMeta};
use crate::tiles::{Sink, TileCache};
use crate::{doc_state, guides, image_id, kdm_err, resources, text_body, tiles};

type KdmResult<T> = Result<T, KdmError>;
/// A layer's current tree node, so a later diff can move or recurse into it without being able
/// to read the tree back from a `Tx` (see [`reconcile_children`]).
pub type Nodes = HashMap<ObjectId, NodeRef>;

fn fields(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn mode_str(mode: kition_photo_doc::ColorMode) -> KdmResult<String> {
    serde_json::to_value(mode)
        .map_err(kdm_err)
        .and_then(|v| v.as_str().map(str::to_owned).ok_or_else(|| KdmError::Engine("mode did not serialize to a string".into())))
}

fn depth_str(depth: kition_photo_doc::SampleType) -> KdmResult<String> {
    serde_json::to_value(depth)
        .map_err(kdm_err)
        .and_then(|v| v.as_str().map(str::to_owned).ok_or_else(|| KdmError::Engine("depth did not serialize to a string".into())))
}

/// Writes a whole document into a fresh replica's creating transaction (import, or any other
/// bulk build). Returns the `Image` object's id (always [`image_id`], but handed back so callers
/// do not need to know that).
pub fn write_document(tx: &mut Tx<'_>, document: &Document, ids: &mut IdMap<LayerId>, nodes: &mut Nodes, blobs: &mut dyn BlobSink) -> KdmResult<ObjectId> {
    let mut cache = TileCache::default();
    write_document_with(tx, document, ids, nodes, &mut Sink::new(blobs, &mut cache))
}

/// [`write_document`] through a [`Sink`], whose tile cache later writes reuse.
pub fn write_document_with(tx: &mut Tx<'_>, document: &Document, ids: &mut IdMap<LayerId>, nodes: &mut Nodes, blobs: &mut Sink<'_>) -> KdmResult<ObjectId> {
    let icc = document.icc_profile.as_ref().map(|data| props::embed(blobs, data));
    let image_fields = fields(json!({
        "width": document.size.width,
        "height": document.size.height,
        "dpi": document.resolution_dpi,
        "mode": mode_str(document.mode)?,
        "depth": depth_str(document.depth)?,
        "icc": icc,
    }));
    let image = tx.create_with_id("Image", &image_id(), image_fields)?;
    guides::write_all(tx, &image, &document.guides)?;
    write_layers(tx, &image, &document.layers, None, ids, nodes, blobs, 0)?;
    for channel in &document.channels {
        write_channel(tx, &image, channel, blobs)?;
    }
    tx.set(&image, "docState", doc_state::to_value(document, blobs)?)?;
    // Needs `ids` fully built first: a layer comp's states name layers by their KDM object id.
    resources::write_all(tx, document, ids, blobs)?;
    Ok(image)
}

#[allow(clippy::too_many_arguments)]
fn write_layers(
    tx: &mut Tx<'_>,
    image: &ObjectId,
    layers: &[Layer],
    parent: Option<&NodeRef>,
    ids: &mut IdMap<LayerId>,
    nodes: &mut Nodes,
    blobs: &mut Sink<'_>,
    depth: usize,
) -> KdmResult<()> {
    if depth > MAX_GROUP_DEPTH {
        return Err(KdmError::Engine(format!("layer groups nested deeper than {MAX_GROUP_DEPTH}")));
    }
    for layer in layers {
        let object = write_layer(tx, layer, blobs)?;
        ids.insert(object.clone(), layer.id);
        let node = tx.tree_create(image, "layers", parent, &object)?;
        nodes.insert(object.clone(), node.clone());
        if let LayerContent::Group(group) = &layer.content {
            write_layers(tx, image, &group.children, Some(&node), ids, nodes, blobs, depth + 1)?;
        }
    }
    Ok(())
}

fn write_layer(tx: &mut Tx<'_>, layer: &Layer, blobs: &mut Sink<'_>) -> KdmResult<ObjectId> {
    let mask = match &layer.mask {
        None => None,
        Some(mask) => Some(write_surface(tx, &SurfaceMeta::for_mask(mask), &mask.surface, blobs)?),
    };
    let (kind, data) = props::content_data(&layer.content, blobs)?;
    let text = match props::text_body(&layer.content) {
        Some(body) => Some(text_body::create(tx, &body)?),
        None => None,
    };
    let (width, height) = canvas_size(tx)?;
    let extra = serde_json::to_value(LayerExtra::from_layer(layer, width, height, blobs)?).map_err(kdm_err)?;
    let settings = serde_json::to_value(StyleSettings::of(&layer.effects, blobs)).map_err(kdm_err)?;
    let layer_fields = fields(json!({
        "name": layer.name,
        "visible": layer.visible,
        "locks": serde_json::to_value(layer.locks).map_err(kdm_err)?,
        "blend": serde_json::to_value(layer.blend).map_err(kdm_err)?,
        "opacity": layer.opacity,
        "fillOpacity": layer.fill_opacity,
        "clipped": layer.clipped,
        "kind": kind,
        "data": data,
        "extra": extra,
        "text": text.as_ref().map(ObjectId::as_str),
        "styleSettings": settings,
        "mask": mask.as_ref().map(ObjectId::as_str),
        // The layer's engine id, so every read gives it the same one (KHP A46, A57; see
        // `crate::read`). Read-only to the object model.
        "number": layer.id.0,
    }));
    let object = tx.create("Layer", layer_fields)?;
    if let LayerContent::Raster(surface) = &layer.content {
        tiles::write_all(tx, &object, surface, blobs)?;
    }
    write_styles(tx, &object, &[], &layer.effects.items, blobs)?;
    write_frames(tx, &object, &[], frames_of(layer), blobs)?;
    Ok(object)
}

/// A layer's video frames (none for a layer that is no video layer).
fn frames_of(layer: &Layer) -> &[kition_photo_doc::Surface] {
    layer.video.as_ref().map_or(&[], |video| video.frames.as_slice())
}

/// `Layer.styles`: one `Style` object per effect, matched by position.
struct Styles;

impl Items for Styles {
    type Item = kition_photo_doc::Effect;

    fn create(&mut self, tx: &mut Tx<'_>, effect: &Self::Item) -> KdmResult<ObjectId> {
        tx.create("Style", fields(json!({ "effect": serde_json::to_value(effect).map_err(kdm_err)? })))
    }

    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &Self::Item, after: &Self::Item) -> KdmResult<bool> {
        if before == after {
            return Ok(false);
        }
        tx.set(object, "effect", serde_json::to_value(after).map_err(kdm_err)?)?;
        Ok(true)
    }
}

fn positions(count: usize) -> Vec<String> {
    (0..count).map(|index| lists::identity(None, index)).collect()
}

fn write_styles(
    tx: &mut Tx<'_>,
    layer: &ObjectId,
    before: &[kition_photo_doc::Effect],
    after: &[kition_photo_doc::Effect],
    _blobs: &mut Sink<'_>,
) -> KdmResult<bool> {
    if before == after {
        return Ok(false);
    }
    lists::sync(tx, layer, "styles", None, before, &positions(before.len()), after, &positions(after.len()), &mut Styles)
}

/// `Layer.frames`: one `Surface` per video frame, matched by position, each frame's tiles its own
/// map entries.
struct Frames<'a, 'b> {
    blobs: &'a mut Sink<'b>,
}

impl Items for Frames<'_, '_> {
    type Item = kition_photo_doc::Surface;

    fn create(&mut self, tx: &mut Tx<'_>, frame: &Self::Item) -> KdmResult<ObjectId> {
        write_surface(tx, &SurfaceMeta::for_frame(frame), frame, self.blobs)
    }

    fn update(&mut self, tx: &mut Tx<'_>, object: &ObjectId, before: &Self::Item, after: &Self::Item) -> KdmResult<bool> {
        let mut wrote = false;
        let (old, new) = (SurfaceMeta::for_frame(before), SurfaceMeta::for_frame(after));
        if serde_json::to_value(&old).map_err(kdm_err)? != serde_json::to_value(&new).map_err(kdm_err)? {
            tx.set(object, "props", serde_json::to_value(new).map_err(kdm_err)?)?;
            wrote = true;
        }
        wrote |= tiles::write_diff(tx, object, before, after, self.blobs)?;
        Ok(wrote)
    }
}

fn write_frames(
    tx: &mut Tx<'_>,
    layer: &ObjectId,
    before: &[kition_photo_doc::Surface],
    after: &[kition_photo_doc::Surface],
    blobs: &mut Sink<'_>,
) -> KdmResult<bool> {
    if before.is_empty() && after.is_empty() {
        return Ok(false);
    }
    lists::sync(tx, layer, "frames", None, before, &positions(before.len()), after, &positions(after.len()), &mut Frames { blobs })
}

fn write_surface(tx: &mut Tx<'_>, meta: &SurfaceMeta, surface: &kition_photo_doc::Surface, blobs: &mut Sink<'_>) -> KdmResult<ObjectId> {
    let id = tx.create("Surface", fields(json!({ "props": serde_json::to_value(meta).map_err(kdm_err)? })))?;
    tiles::write_all(tx, &id, surface, blobs)?;
    Ok(id)
}

fn write_channel(tx: &mut Tx<'_>, image: &ObjectId, channel: &kition_photo_doc::AlphaChannel, blobs: &mut Sink<'_>) -> KdmResult<()> {
    let surface = write_surface(tx, &SurfaceMeta::for_channel(channel), &channel.surface, blobs)?;
    let object = tx.create("Channel", fields(json!({ "name": channel.name, "surface": surface.as_str() })))?;
    let len = tx.list_items(image, "channels")?.len();
    tx.list_insert(image, "channels", len, &object)?;
    Ok(())
}

/// Stores the numbers a read gave layers that do not store them ([`crate::Read::unstored`]), for
/// layers that still exist. Answers whether it wrote anything.
pub(crate) fn store_numbers(tx: &mut Tx<'_>, numbers: &[(ObjectId, u64)]) -> KdmResult<bool> {
    let mut wrote = false;
    for (object, number) in numbers {
        if tx.exists(object) {
            tx.set(object, "number", json!(number))?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// Writes only what differs between `before` and `after`: layer field changes, inserts, deletes
/// and tree moves (computed from the two `Document` trees — a `Tx` cannot read the replica's own
/// tree back), and tiles whose `Arc<Tile>` differs (pointer identity first). Channels are diffed
/// by position (the engine model gives them no stable id); any change rewrites the whole channel
/// list, which is rare enough not to need finer tracking yet. Answers whether it wrote anything.
pub fn write_changes(
    tx: &mut Tx<'_>,
    before: &Document,
    after: &Document,
    ids: &mut IdMap<LayerId>,
    nodes: &mut Nodes,
    blobs: &mut dyn BlobSink,
) -> KdmResult<bool> {
    let mut cache = TileCache::default();
    write_changes_with(tx, before, after, ids, nodes, &mut Sink::new(blobs, &mut cache))
}

/// [`write_changes`] through a [`Sink`]: tiles its cache has already encoded are not encoded
/// again (a duplicated layer, an unchanged video frame).
pub fn write_changes_with(
    tx: &mut Tx<'_>,
    before: &Document,
    after: &Document,
    ids: &mut IdMap<LayerId>,
    nodes: &mut Nodes,
    blobs: &mut Sink<'_>,
) -> KdmResult<bool> {
    let image = image_id();
    let mut wrote = false;
    wrote |= set_if_changed(tx, &image, "width", json!(before.size.width), json!(after.size.width))?;
    wrote |= set_if_changed(tx, &image, "height", json!(before.size.height), json!(after.size.height))?;
    wrote |= set_if_changed(tx, &image, "dpi", json!(before.resolution_dpi), json!(after.resolution_dpi))?;
    wrote |= set_if_changed(tx, &image, "mode", json!(mode_str(before.mode)?), json!(mode_str(after.mode)?))?;
    wrote |= set_if_changed(tx, &image, "depth", json!(depth_str(before.depth)?), json!(depth_str(after.depth)?))?;
    if before.guides != after.guides {
        wrote |= guides::write_diff(tx, &image, &after.guides)?;
    }

    let before_layers = index_layers(&before.layers);
    let after_layers = index_layers(&after.layers);

    // Field and tile changes of layers whose identity persists, independent of where they sit.
    for (id, after_layer) in &after_layers {
        if let Some(before_layer) = before_layers.get(id)
            && let Some(object) = ids.object(*id).cloned()
        {
            wrote |= diff_layer(tx, &object, before_layer, after_layer, blobs)?;
        }
    }

    // Structure: new layers, moves and reorders.
    wrote |= reconcile_children(tx, &image, None, &before.layers, &after.layers, &before_layers, ids, nodes, blobs, 0)?;

    // Deletes, after what a deleted group held and kept was moved out of it, children before their
    // group: kition-kdm refuses to delete an object whose tree node has live children.
    for id in children_first(&before.layers).into_iter().filter(|id| !after_layers.contains_key(id)) {
        if let Some(object) = ids.object(id).cloned() {
            // What the layer holds is its own: it goes with it.
            delete_parts(tx, &object)?;
            tx.delete(&object)?;
            ids.remove(&object);
            nodes.remove(&object);
            wrote = true;
        }
    }

    if let Some(value) = doc_state::diff(before, after, blobs)? {
        tx.set(&image, "docState", value)?;
        wrote = true;
    }
    // Layer comps name layers by object id, so the resources go after every layer change.
    wrote |= resources::write_changes(tx, before, after, ids, blobs)?;

    if before.channels != after.channels {
        wrote |= rewrite_channels(tx, &image, &after.channels, blobs)?;
    }

    Ok(wrote)
}

/// Deletes what a layer object holds: its mask, its text body, its styles and video frames.
fn delete_parts(tx: &mut Tx<'_>, object: &ObjectId) -> KdmResult<()> {
    for field in ["mask", "text"] {
        if let Ok(Value::String(part)) = tx.get(object, field) {
            let part = ObjectId::from_string(part);
            if tx.exists(&part) {
                tx.delete(&part)?;
            }
        }
    }
    for field in ["styles", "frames"] {
        for part in tx.list_items(object, field)? {
            if tx.exists(&part) {
                tx.delete(&part)?;
            }
        }
    }
    Ok(())
}

fn set_if_changed(tx: &mut Tx<'_>, object: &ObjectId, field: &str, before: Value, after: Value) -> KdmResult<bool> {
    if before == after {
        return Ok(false);
    }
    tx.set(object, field, after)?;
    Ok(true)
}

/// Every layer of a tree, each group's children (at any depth) before the group.
fn children_first(layers: &[Layer]) -> Vec<LayerId> {
    fn walk(layers: &[Layer], out: &mut Vec<LayerId>) {
        for layer in layers {
            if let LayerContent::Group(group) = &layer.content {
                walk(&group.children, out);
            }
            out.push(layer.id);
        }
    }
    let mut out = Vec::new();
    walk(layers, &mut out);
    out
}

/// Flattens a layer tree into `LayerId -> &Layer`, ignoring position (used to tell which layers
/// are new, gone, or possibly edited).
fn index_layers(layers: &[Layer]) -> HashMap<LayerId, &Layer> {
    fn walk<'a>(layers: &'a [Layer], out: &mut HashMap<LayerId, &'a Layer>) {
        for layer in layers {
            out.insert(layer.id, layer);
            if let LayerContent::Group(group) = &layer.content {
                walk(&group.children, out);
            }
        }
    }
    let mut out = HashMap::new();
    walk(layers, &mut out);
    out
}

/// A persisting layer's own children before this diff (empty for a layer that is new, or is not
/// a group, or was not a group before).
fn before_children<'a>(before_layers: &HashMap<LayerId, &'a Layer>, id: LayerId) -> &'a [Layer] {
    match before_layers.get(&id) {
        Some(Layer { content: LayerContent::Group(group), .. }) => &group.children,
        _ => &[],
    }
}

/// Rebuilds one parent's children to exactly `after_children`, given they were `before_children`
/// before this diff (empty if the parent is itself new). `tree_create` and `tree_move` only ever
/// place a node last among its siblings, so the plan is: find the longest prefix of
/// `after_children` that is already in the right relative order (each next one's `before_pos` is
/// higher than the last kept one's — the only shape "append to the end" can leave alone), leave
/// that prefix untouched, and replay everything after it — a `tree_create` for a genuinely new
/// layer, a `tree_move` for one that existed (anywhere) before — in `after_children`'s order, so
/// each lands, appended, in exactly its target position. This is not always the globally minimal
/// move count (picking a different, non-prefix subsequence to keep can sometimes do better), but
/// it is simple, always exactly correct, and optimal for the common case (one layer moved, or
/// one new layer inserted, with the rest of the parent's children untouched).
#[allow(clippy::too_many_arguments)]
fn reconcile_children(
    tx: &mut Tx<'_>,
    image: &ObjectId,
    parent_node: Option<&NodeRef>,
    before_siblings: &[Layer],
    after_children: &[Layer],
    before_layers: &HashMap<LayerId, &Layer>,
    ids: &mut IdMap<LayerId>,
    nodes: &mut Nodes,
    blobs: &mut Sink<'_>,
    depth: usize,
) -> KdmResult<bool> {
    // As deep as `write_document` and `read_document` go: a deeper tree could be written but
    // never read back.
    if depth > MAX_GROUP_DEPTH {
        return Err(KdmError::Engine(format!("layer groups nested deeper than {MAX_GROUP_DEPTH}")));
    }
    let mut wrote = false;
    let before_pos: HashMap<LayerId, usize> = before_siblings.iter().enumerate().map(|(index, layer)| (layer.id, index)).collect();

    let mut prefix_len = 0usize;
    let mut previous_pos: Option<usize> = None;
    for layer in after_children {
        let Some(&pos) = before_pos.get(&layer.id) else { break };
        if previous_pos.is_some_and(|previous| pos <= previous) {
            break;
        }
        previous_pos = Some(pos);
        prefix_len += 1;
    }

    for layer in &after_children[..prefix_len] {
        // Already exactly where it belongs; only its own children (if any) might still differ.
        if let LayerContent::Group(group) = &layer.content {
            let own_before = before_children(before_layers, layer.id);
            let node = ids.object(layer.id).and_then(|object| nodes.get(object)).cloned();
            wrote |= reconcile_children(tx, image, node.as_ref(), own_before, &group.children, before_layers, ids, nodes, blobs, depth + 1)?;
        }
    }

    for layer in &after_children[prefix_len..] {
        let object = match ids.object(layer.id).cloned() {
            Some(object) => {
                let node = nodes.get(&object).cloned().ok_or_else(|| KdmError::Engine(format!("no tree node tracked for layer {object}")))?;
                tx.tree_move(image, "layers", &node, parent_node)?;
                wrote = true;
                object
            }
            None => {
                let object = write_layer(tx, layer, blobs)?;
                ids.insert(object.clone(), layer.id);
                let node = tx.tree_create(image, "layers", parent_node, &object)?;
                nodes.insert(object.clone(), node);
                wrote = true;
                object
            }
        };
        if let LayerContent::Group(group) = &layer.content {
            let own_before = before_children(before_layers, layer.id);
            let node = nodes.get(&object).cloned();
            wrote |= reconcile_children(tx, image, node.as_ref(), own_before, &group.children, before_layers, ids, nodes, blobs, depth + 1)?;
        }
    }
    Ok(wrote)
}

/// Diffs one unchanged-identity layer's fields and (for raster content) tiles. Tree position is
/// [`reconcile_children`]'s job, not this one's.
fn diff_layer(tx: &mut Tx<'_>, object: &ObjectId, before: &Layer, after: &Layer, blobs: &mut Sink<'_>) -> KdmResult<bool> {
    let mut wrote = false;
    wrote |= set_if_changed(tx, object, "name", json!(before.name), json!(after.name))?;
    wrote |= set_if_changed(tx, object, "visible", json!(before.visible), json!(after.visible))?;
    if before.locks != after.locks {
        tx.set(object, "locks", serde_json::to_value(after.locks).map_err(kdm_err)?)?;
        wrote = true;
    }
    if before.blend != after.blend {
        tx.set(object, "blend", serde_json::to_value(after.blend).map_err(kdm_err)?)?;
        wrote = true;
    }
    wrote |= set_if_changed(tx, object, "opacity", json!(before.opacity), json!(after.opacity))?;
    wrote |= set_if_changed(tx, object, "fillOpacity", json!(before.fill_opacity), json!(after.fill_opacity))?;
    wrote |= set_if_changed(tx, object, "clipped", json!(before.clipped), json!(after.clipped))?;

    if !content_fields_equal(before, after) {
        let (old_kind, old_data) = props::content_data(&before.content, blobs)?;
        let (kind, data) = props::content_data(&after.content, blobs)?;
        if old_kind != kind {
            tx.set(object, "kind", json!(kind))?;
            wrote = true;
        }
        if old_data != data || old_kind != kind {
            tx.set(object, "data", data)?;
            wrote = true;
        }
        wrote |= diff_text(tx, object, before, after)?;
    }
    if !extra_fields_equal(before, after) {
        let (width, height) = canvas_size(tx)?;
        let old = serde_json::to_value(LayerExtra::from_layer(before, width, height, blobs)?).map_err(kdm_err)?;
        let new = serde_json::to_value(LayerExtra::from_layer(after, width, height, blobs)?).map_err(kdm_err)?;
        if old != new {
            tx.set(object, "extra", new)?;
            wrote = true;
        }
    }
    wrote |= write_frames(tx, object, frames_of(before), frames_of(after), blobs)?;

    if before.effects != after.effects {
        let old = serde_json::to_value(StyleSettings::of(&before.effects, blobs)).map_err(kdm_err)?;
        let new = serde_json::to_value(StyleSettings::of(&after.effects, blobs)).map_err(kdm_err)?;
        if old != new {
            tx.set(object, "styleSettings", new)?;
            wrote = true;
        }
        wrote |= write_styles(tx, object, &before.effects.items, &after.effects.items, blobs)?;
    }

    wrote |= diff_mask(tx, object, &before.mask, &after.mask, blobs)?;

    if let LayerContent::Raster(after_surface) = &after.content {
        if let LayerContent::Raster(before_surface) = &before.content {
            wrote |= tiles::write_diff(tx, object, before_surface, after_surface, blobs)?;
        } else {
            tiles::write_all(tx, object, after_surface, blobs)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

/// A type layer's text body: edited in place when the layer stays a type layer, made or deleted
/// when it becomes or stops being one.
fn diff_text(tx: &mut Tx<'_>, object: &ObjectId, before: &Layer, after: &Layer) -> KdmResult<bool> {
    let stored = match tx.get(object, "text")? {
        Value::String(id) => Some(ObjectId::from_string(id)).filter(|id| tx.exists(id)),
        _ => None,
    };
    match (props::text_body(&before.content), props::text_body(&after.content), stored) {
        (Some(old), Some(new), Some(body)) => text_body::update(tx, &body, &old, &new),
        (_, Some(new), stored) => {
            if let Some(body) = stored {
                tx.delete(&body)?;
            }
            let body = text_body::create(tx, &new)?;
            tx.set(object, "text", json!(body.as_str()))?;
            Ok(true)
        }
        (_, None, Some(body)) => {
            tx.delete(&body)?;
            tx.set(object, "text", Value::Null)?;
            Ok(true)
        }
        (_, None, None) => Ok(false),
    }
}

fn canvas_size(tx: &Tx<'_>) -> KdmResult<(u32, u32)> {
    let side = |field| {
        tx.get(&crate::image_id(), field)?.as_u64().and_then(|value| u32::try_from(value).ok()).ok_or_else(|| kdm_err(format!("invalid canvas {field}")))
    };
    Ok((side("width")?, side("height")?))
}

/// Whether the layer state `Layer.extra` holds ([`LayerExtra`]) is the same.
fn extra_fields_equal(before: &Layer, after: &Layer) -> bool {
    before.vector_mask == after.vector_mask
        && before.label == after.label
        && before.link_group == after.link_group
        && before.excluded_channels == after.excluded_channels
        && before.blend_if == after.blend_if
        && before.advanced == after.advanced
        && before.psd_blocks == after.psd_blocks
        && before.video.as_ref().map(|v| (&v.source, v.fps, v.show_altered)) == after.video.as_ref().map(|v| (&v.source, v.fps, v.show_altered))
}

/// Whether the two layers' content needs no `kind`, `data` or text body write: same kind, and (for
/// raster, the common case of a stroke) only the default pixel — pixel tiles are diffed
/// separately. A group's expanded state is not stored.
fn content_fields_equal(before: &Layer, after: &Layer) -> bool {
    match (&before.content, &after.content) {
        (LayerContent::Raster(a), LayerContent::Raster(b)) => a.default_pixel() == b.default_pixel(),
        (LayerContent::Group(a), LayerContent::Group(b)) => a.artboard == b.artboard,
        (LayerContent::Adjustment(a), LayerContent::Adjustment(b)) => a == b,
        (LayerContent::Fill(a), LayerContent::Fill(b)) => a == b,
        (LayerContent::Text(a), LayerContent::Text(b)) => props::same_but_cache(&before.content, &after.content) || a == b,
        (LayerContent::Shape(a), LayerContent::Shape(b)) => a == b,
        (LayerContent::Smart(a), LayerContent::Smart(b)) => a == b,
        _ => false,
    }
}

fn diff_mask(
    tx: &mut Tx<'_>,
    object: &ObjectId,
    before: &Option<kition_photo_doc::LayerMask>,
    after: &Option<kition_photo_doc::LayerMask>,
    blobs: &mut Sink<'_>,
) -> KdmResult<bool> {
    match (before, after) {
        (None, None) => Ok(false),
        (Some(_), None) => {
            if let Value::String(surface) = tx.get(object, "mask")? {
                tx.delete(&ObjectId::from_string(surface))?;
            }
            tx.set(object, "mask", Value::Null)?;
            Ok(true)
        }
        (None, Some(mask)) => {
            let surface = write_surface(tx, &SurfaceMeta::for_mask(mask), &mask.surface, blobs)?;
            tx.set(object, "mask", json!(surface.as_str()))?;
            Ok(true)
        }
        (Some(before_mask), Some(after_mask)) => {
            let mut wrote = false;
            let surface_value = tx.get(object, "mask")?;
            let Value::String(surface_id) = surface_value else {
                // The mask ref is dangling (its surface was deleted elsewhere): rebuild it.
                let surface = write_surface(tx, &SurfaceMeta::for_mask(after_mask), &after_mask.surface, blobs)?;
                tx.set(object, "mask", json!(surface.as_str()))?;
                return Ok(true);
            };
            let surface = ObjectId::from_string(surface_id);
            if before_mask.enabled != after_mask.enabled
                || before_mask.linked != after_mask.linked
                || before_mask.density != after_mask.density
                || before_mask.feather != after_mask.feather
            {
                tx.set(&surface, "props", serde_json::to_value(SurfaceMeta::for_mask(after_mask)).map_err(kdm_err)?)?;
                wrote = true;
            }
            wrote |= tiles::write_diff(tx, &surface, &before_mask.surface, &after_mask.surface, blobs)?;
            Ok(wrote)
        }
    }
}

fn rewrite_channels(tx: &mut Tx<'_>, image: &ObjectId, channels: &[kition_photo_doc::AlphaChannel], blobs: &mut Sink<'_>) -> KdmResult<bool> {
    let existing = tx.list_items(image, "channels")?;
    for (index, object) in existing.iter().enumerate().rev() {
        if let Value::String(surface) = tx.get(object, "surface")? {
            tx.delete(&ObjectId::from_string(surface))?;
        }
        tx.delete(object)?;
        tx.list_remove(image, "channels", index)?;
    }
    for channel in channels {
        write_channel(tx, image, channel, blobs)?;
    }
    Ok(true)
}
