//! Bridges the Photo engine's `Document` ([`kition_photo_doc`]) to a `kit.photo` KDM replica.
//!
//! [`PhotoModel`] implements [`KitModel`]: it is the engine's materialised view of a replica, the
//! same `Document` the rest of the engine already knows. Layers, surfaces (masks, video frames)
//! and channels become KDM objects; a layer's or surface's pixels become one content-addressed
//! blob per 256×256 tile ([`tiles`]), keyed `"x,y"` in its `tiles` map, so concurrent edits to
//! different tiles both survive and edits to the same tile resolve last-writer-wins (each map key
//! is its own register). A type layer's characters are a text ([`text_body`]), its style runs
//! marks on them; a layer's effects, the image's notes, layer comps, saved paths and patterns are
//! object lists ([`lists`], [`resources`]); the guides are a map keyed by guide ([`guides`]).
//! [`write_document`] and [`write_changes`] are the engine command bridge's entry points:
//! writing a whole document in one creating transaction, or only what a command changed.
//!
//! `Document.selection` and `Document.quick_mask` (the selection, edited as a channel) are
//! per-person state and are never written to the replica; nor are `Document::id` and
//! `Document::name` (this view's own identity and file title), nor each person's view
//! ([`private`]: a group's expanded state, an open note, the timeline's playhead, the last applied
//! comp). The rest of the document-level state (metadata, measurement, slices, the timeline, …)
//! round-trips through one `Image.docState` register ([`doc_state`]), last-writer-wins as a
//! whole. Rendered-pixel caches (`Layer::fill_cache`, every kind's `cache`) are dropped; the
//! compositor rebuilds them on demand.
//!
//! **Canvas size.** The image is at most 25,600 pixels a side (the schema's `width` and
//! `height`), so a layer's `tiles` map holds at most 100 × 100 = 10,000 keys over the canvas.
//!
//! **Layer ids.** Every layer stores its engine id in its read-only `number` field, so a reopen,
//! a replica-only open and a peer all give a layer the same id; two layers that store one number
//! are told apart the same way on every replica (see [`read`]).
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod blob_gc;
mod doc_state;
mod guides;
mod lists;
mod private;
mod props;
mod read;
mod resources;
mod session;
mod settle;
mod text_body;
mod tiles;
mod write;

use std::collections::BTreeSet;
use std::sync::Arc;

use kition_kit::kdm::{ActorId, KdmError, ObjectId, Replica, Schema, Tx};
use kition_kit::session::{BlobSink, BlobSource, Damage, IdMap, KitError, KitModel};
use kition_photo_doc::{Document, Layer, LayerContent, LayerId};
use kition_photo_geom::TileCoord;
use kition_photo_raster::Surface;

pub use kition_kit;
pub use read::{Read, Reuse, read_document, read_document_reusing};
pub use session::{ImportError, PhotoSession};
pub use settle::settle as settle_batch;
pub use tiles::{Sink, TileCache};
pub use write::{Nodes, write_changes, write_changes_with, write_document, write_document_with};

type Result<T> = kition_kit::session::Result<T>;

const SCHEMA: &str = include_str!("../model/schema.json");

/// The largest canvas side the schema takes, in pixels: a layer's `tiles` map then holds at most
/// 10,000 keys over the canvas.
pub const MAX_CANVAS_SIDE: u32 = 25_600;

/// Whether `replica` was written with this schema version: made under it, and no change of it
/// under another. A replica of another version is not read (no reader for an older shape is
/// kept): a `.kphoto` holding one opens from its manifest instead, as a new replica.
pub fn same_schema(replica: &Replica) -> bool {
    let version = replica.schema().schema_version;
    replica.min_schema_version() == version && replica.author_schema_version().is_none_or(|author| author == version)
}

/// [`same_schema`] for a snapshot's bytes; `false` for bytes that do not load.
pub fn replica_matches(snapshot: &[u8]) -> bool {
    Replica::load(<PhotoModel as KitModel>::schema(), snapshot, ActorId::random()).is_ok_and(|replica| same_schema(&replica))
}

/// The blobs `replica`'s current state references (tiles, ICC profiles, effects, the document
/// state), and none that only its history or an undo step does: what a file holding the replica
/// as a new document keeps (`document.fork`).
pub fn state_blobs(replica: &Replica) -> BTreeSet<String> {
    blob_gc::BlobIndex::build(replica).referenced().into_iter().collect()
}

/// The `Image` object's id: one per document, minted by [`write_document`] with
/// [`kition_kit::kdm::Tx::create_with_id`], so every reader finds it without an index.
fn image_id() -> ObjectId {
    ObjectId::from_string("image")
}

/// Wraps a [`KdmError`] (or anything else `Display`s) as a [`KitError`].
fn kit_err(error: impl std::fmt::Display) -> KitError {
    KitError::new(error.to_string())
}

/// Wraps anything `Display`s (a `serde_json::Error`, a `ParseIntError`, …) as a [`KdmError`],
/// for the helper modules that write and read through a `Tx`/`Replica` and so return
/// [`KdmError`] directly (this crate's `KitModel` methods then convert it once with [`kit_err`]).
pub(crate) fn kdm_err(error: impl std::fmt::Display) -> KdmError {
    KdmError::Engine(error.to_string())
}

/// The engine's view of a `kit.photo` replica: the same [`Document`] the rest of the engine
/// knows, plus the id map and tree node map [`write_changes`] needs to find and place each
/// layer's object again.
pub struct PhotoModel {
    pub document: Document,
    layers: IdMap<LayerId>,
    nodes: Nodes,
    /// The layers the last read gave another id than the number they store, with that id: the
    /// next write stores it ([`Read::unstored`]).
    unstored: Vec<(ObjectId, u64)>,
}

impl PhotoModel {
    /// A view built by the caller itself: `document` as written by [`write_document`] or
    /// [`write_changes`], with the maps they filled.
    pub fn from_parts(document: Document, layers: IdMap<LayerId>, nodes: Nodes) -> Self {
        Self { document, layers, nodes, unstored: Vec::new() }
    }

    fn from_read(read: Read) -> Self {
        Self { document: read.document, layers: read.ids, nodes: read.nodes, unstored: read.unstored }
    }

    /// Replaces the view after the caller wrote `document` to the replica itself (a diff bridge's
    /// [`write_changes`]), with the maps that write left.
    pub fn adopt(&mut self, document: Document, layers: IdMap<LayerId>, nodes: Nodes) {
        self.document = document;
        self.layers = layers;
        self.nodes = nodes;
    }

    /// The layers whose id the replica does not store yet, with that id; the caller's next write
    /// stores them ([`write::store_numbers`]) and then clears them ([`PhotoModel::stored`]).
    pub fn unstored(&self) -> &[(ObjectId, u64)] {
        &self.unstored
    }

    /// The numbers [`PhotoModel::unstored`] listed are stored now.
    pub fn stored(&mut self) {
        self.unstored.clear();
    }

    /// Reloads the whole view from `replica`, keeping the state the replica does not hold (this
    /// view's identity, title, selection and quick mask).
    pub fn reload(&mut self, replica: &Replica, blobs: &dyn BlobSource) -> Result<()> {
        self.reload_changed(replica, blobs, None)
    }

    /// [`PhotoModel::reload`], keeping the view's layers whose objects are not in `changed`
    /// (`None`: read every layer again).
    fn reload_changed(&mut self, replica: &Replica, blobs: &dyn BlobSource, changed: Option<&BTreeSet<ObjectId>>) -> Result<()> {
        let reuse = changed.map(|changed| read::Reuse { view: &self.document, ids: &self.layers, changed });
        let read = read::read_document_reusing(replica, blobs, reuse).map_err(kit_err)?;
        let mut document = read.document;
        let previous = &mut self.document;
        private::carry(previous, &mut document);
        document.id = previous.id;
        document.name = std::mem::take(&mut previous.name);
        document.selection = previous.selection.take();
        document.quick_mask = previous.quick_mask.take();
        self.document = document;
        self.layers = read.ids;
        self.nodes = read.nodes;
        self.unstored = read.unstored;
        Ok(())
    }

    /// The `LayerId -> ObjectId` map, current as of the last `load`/`apply`. `KitSession::execute`
    /// hands the bridge only `&PhotoModel`, so it clones this (and [`PhotoModel::nodes`]) into its
    /// own working copy to mutate while building a [`write_changes`] transaction; `apply` rebuilds
    /// both from the replica after the transaction commits, so nothing needs writing back.
    pub fn ids(&self) -> &IdMap<LayerId> {
        &self.layers
    }

    /// The `ObjectId -> NodeRef` map [`write_changes`] needs to move or recurse into an existing
    /// layer's tree node, current as of the last `load`/`apply`. See [`PhotoModel::ids`].
    pub fn nodes(&self) -> &Nodes {
        &self.nodes
    }
}

impl PartialEq for PhotoModel {
    /// Compares documents as content, not as engine session state: `Document::id` is this
    /// process's own handle, each person's view ([`private`]) is their own, and layer ids are
    /// compared apart from the content (two views of converged replicas give every layer the same
    /// id, see [`read`]).
    fn eq(&self, other: &Self) -> bool {
        fn zero_layer_ids(layers: &[Layer]) -> Vec<Layer> {
            layers
                .iter()
                .map(|layer| {
                    let mut layer = layer.clone();
                    layer.id = LayerId(0);
                    if let LayerContent::Group(group) = &mut layer.content {
                        group.children = zero_layer_ids(&group.children);
                    }
                    layer
                })
                .collect()
        }
        let mut a = private::shared(&self.document);
        let mut b = private::shared(&other.document);
        a.id = b.id;
        a.layers = zero_layer_ids(&a.layers);
        b.layers = zero_layer_ids(&b.layers);
        a == b
    }
}

impl std::fmt::Debug for PhotoModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhotoModel").field("document", &self.document).finish()
    }
}

impl KitModel for PhotoModel {
    fn schema() -> Arc<Schema> {
        #[allow(clippy::expect_used)]
        Arc::new(Schema::parse(SCHEMA).expect("model/schema.json is a fixed, tested file"))
    }

    fn load(replica: &Replica, blobs: &dyn BlobSource) -> Result<Self> {
        Ok(Self::from_read(read_document(replica, blobs).map_err(kit_err)?))
    }

    fn apply(&mut self, replica: &Replica, changed: &BTreeSet<ObjectId>, blobs: &dyn BlobSource) -> Result<Damage> {
        let image = image_id();
        if changed.contains(&image) {
            self.reload_changed(replica, blobs, Some(changed))?;
            return Ok(Damage::Full);
        }
        let mut damage = Damage::None;
        for id in changed {
            let is_tracked_layer = self.layers.local(id).is_some() && replica.object_type(id).ok().as_deref() == Some("Layer");
            if !is_tracked_layer || !replica.exists(id) {
                // A surface (mask/channel), a channel, an object this view never placed (a
                // concurrent insert, or an undone delete returning), or a deleted-but-still-
                // tracked layer: reload rather than guess at its position in the tree.
                self.reload_changed(replica, blobs, Some(changed))?;
                return Ok(Damage::Full);
            }
            // A layer storing another number than its id here (two layers made with one number on
            // two replicas): read the whole tree again, which tells them apart.
            let Some(layer_id) = self.layers.local(id).filter(|layer_id| replica.get(id, "number").ok().and_then(|n| n.as_u64()) == Some(layer_id.0)) else {
                self.reload_changed(replica, blobs, Some(changed))?;
                return Ok(Damage::Full);
            };
            let mut patched = read::read_layer(replica, id, self.document.mode, self.document.depth, blobs).map_err(kit_err)?;
            patched.id = layer_id;
            let Some(before) = find_layer(&self.document.layers, layer_id) else {
                self.reload_changed(replica, blobs, Some(changed))?;
                return Ok(Damage::Full);
            };
            // A group's children are the tree's structure, not this object's own fields (and
            // `read_layer` never reads them): any change to them touches `image`'s own `layers`
            // container, which the check above already reloads for. Patching only this object's
            // fields must keep the children the view already has.
            if let (LayerContent::Group(before_group), LayerContent::Group(patched_group)) = (&before.content, &mut patched.content) {
                patched_group.children = before_group.children.clone();
                // Expanded or not is this view's own.
                patched_group.expanded = before_group.expanded;
            }
            // Rendered-appearance caches are not stored in the replica (`read_layer` always
            // starts them `None`); keep what the view already rendered when only other fields
            // changed (a stale cache for changed content would show the old appearance).
            props::carry_caches(before, &mut patched);
            let step = layer_damage(before, &patched);
            let mut slot = Some(patched);
            if !replace_layer(&mut self.document.layers, layer_id, &mut slot) {
                self.reload_changed(replica, blobs, Some(changed))?;
                return Ok(Damage::Full);
            }
            damage = damage.merge(step);
        }
        Ok(damage)
    }

    fn validate(tx: &mut Tx<'_>, _changed: &BTreeSet<ObjectId>) -> Result<bool> {
        let mut wrote = false;
        for layer in tx.objects_of_type("Layer") {
            if let serde_json::Value::String(target) = tx.get(&layer, "mask").map_err(kit_err)?
                && !tx.exists(&ObjectId::from_string(target))
            {
                tx.set(&layer, "mask", serde_json::Value::Null).map_err(kit_err)?;
                wrote = true;
            }
        }
        let image = image_id();
        let channels = tx.list_items(&image, "channels").map_err(kit_err)?;
        for (index, channel) in channels.iter().enumerate().rev() {
            if !tx.exists(channel) {
                tx.list_remove(&image, "channels", index).map_err(kit_err)?;
                wrote = true;
                continue;
            }
            if let serde_json::Value::String(target) = tx.get(channel, "surface").map_err(kit_err)?
                && !tx.exists(&ObjectId::from_string(target))
            {
                tx.delete(channel).map_err(kit_err)?;
                tx.list_remove(&image, "channels", index).map_err(kit_err)?;
                wrote = true;
            }
        }
        // Tree nodes naming a deleted layer need no write here: `Replica::tree` (and this crate's
        // own `read_document`) already skip a node whose object no longer exists, together with
        // its subtree, so they are already absent from every loaded view.
        Ok(wrote)
    }

    fn import(name: &str, bytes: &[u8], blobs: &mut dyn BlobSink, actor: ActorId) -> Result<Replica> {
        let imported = kition_photo_io::import(name, bytes).map_err(kit_err)?;
        let mut ids = IdMap::default();
        let mut nodes = Nodes::default();
        let (replica, ()) = Replica::create_with(Self::schema(), kition_kit::kdm::DocId::new(), actor, &kition_kit::kdm::Origin::system("import"), |tx| {
            write_document(tx, &imported.document, &mut ids, &mut nodes, blobs)?;
            Ok(())
        })
        .map_err(kit_err)?;
        Ok(replica)
    }

    fn export(&self, _replica: &Replica, name: &str, _blobs: &dyn BlobSource) -> Result<Vec<u8>> {
        let result = kition_photo_io::export(&self.document, name, &kition_photo_io::ExportOptions::default()).map_err(kit_err)?;
        Ok(result.bytes)
    }
}

/// Finds the layer named `id`, at any depth.
fn find_layer(layers: &[Layer], id: LayerId) -> Option<&Layer> {
    for layer in layers {
        if layer.id == id {
            return Some(layer);
        }
        if let LayerContent::Group(group) = &layer.content
            && let Some(found) = find_layer(&group.children, id)
        {
            return Some(found);
        }
    }
    None
}

/// Replaces the layer named `id` with `*slot` (keeping `id`), at any depth. `slot` is emptied on
/// success; answers whether it found the layer (and so emptied `slot`).
fn replace_layer(layers: &mut [Layer], id: LayerId, slot: &mut Option<Layer>) -> bool {
    for layer in layers.iter_mut() {
        if layer.id == id {
            let Some(mut patched) = slot.take() else { return false };
            patched.id = id;
            *layer = patched;
            return true;
        }
        if let LayerContent::Group(group) = &mut layer.content
            && replace_layer(&mut group.children, id, slot)
        {
            return true;
        }
    }
    false
}

/// Whether two layers differ only in their raster pixels (so [`layer_damage`] can report exact
/// tile rects instead of [`Damage::Full`]).
fn same_except_pixels(a: &Layer, b: &Layer) -> bool {
    a.name == b.name
        && a.visible == b.visible
        && a.locks == b.locks
        && a.blend == b.blend
        && a.opacity == b.opacity
        && a.fill_opacity == b.fill_opacity
        && a.clipped == b.clipped
        && a.mask.is_none() == b.mask.is_none()
        && a.effects == b.effects
}

fn layer_damage(before: &Layer, after: &Layer) -> Damage {
    if !same_except_pixels(before, after) {
        return Damage::Full;
    }
    match (&before.content, &after.content) {
        (LayerContent::Raster(a), LayerContent::Raster(b)) => surface_damage(a, b),
        (a, b) if a == b => Damage::None,
        _ => Damage::Full,
    }
}

fn surface_damage(before: &Surface, after: &Surface) -> Damage {
    let mut rects = Vec::new();
    let mut seen = BTreeSet::new();
    for (coord, tile) in after.tiles() {
        seen.insert(*coord);
        let unchanged = before.tile(*coord).is_some_and(|prior| Arc::ptr_eq(prior, tile) || prior == tile);
        if !unchanged {
            rects.push(tile_rect(*coord));
        }
    }
    for (coord, _) in before.tiles() {
        if !seen.contains(coord) {
            rects.push(tile_rect(*coord));
        }
    }
    if rects.is_empty() { Damage::None } else { Damage::Rects(rects) }
}

fn tile_rect(coord: TileCoord) -> [i32; 4] {
    let rect = coord.rect();
    [rect.x0, rect.y0, rect.x1, rect.y1]
}
