//! A draft's merge (kition-sdk data model 6.6, rules D1 to D9) through `KitSession<PhotoModel>`:
//! an agent works on a draft made at version v while the person edits the document; accepting the
//! draft keeps both where they touch different state (two tiles of one layer, two fields of one
//! layer, a move and a delete, two moves into one group), and refuses it where both changed the
//! same state (one tile painted on both sides), naming the layer and the tile. The merge is one
//! undo step of the person: one undo gives back the exact state before it, one redo the merge.
//!
//! Blobs live outside the replica (D6: a workspace asset store), so these tests share one blob
//! store between the document and its draft (`SharedBlobs`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kdm_model::{Nodes, PhotoModel, write_document};
use kition_kit::kdm::{ActorId, DocId, ObjectId, Origin, Replica, UndoScope};
use kition_kit::session::{BlobRef, BlobSink, BlobSource, Damage, IdMap, KitModel, KitSession, sha256_hex};
use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{Document, Layer, Size};
use kition_photo_geom::TileCoord;
use kition_photo_raster::Surface;

type KResult<T> = kition_kit::session::Result<T>;
type Session = KitSession<PhotoModel>;

/// `Arc<Mutex<_>>`, not `Rc<RefCell<_>>`: a `KitSession`'s blob store must be `Send` (engines run
/// sessions on worker threads).
#[derive(Clone, Default)]
struct SharedBlobs(Arc<Mutex<HashMap<String, Arc<[u8]>>>>);

impl BlobSource for SharedBlobs {
    fn get(&self, hash: &str) -> Option<Arc<[u8]>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).get(hash).cloned()
    }
}

impl BlobSink for SharedBlobs {
    fn put(&mut self, bytes: &[u8], mime: &str) -> BlobRef {
        let hash = sha256_hex(bytes);
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry(hash.clone()).or_insert_with(|| Arc::from(bytes));
        BlobRef { hash, mime: mime.to_owned(), size: bytes.len() as u64 }
    }
}

fn actor(value: u64) -> ActorId {
    ActorId::from_u64(value).unwrap_or_else(|_| panic!("actor {value}"))
}

fn person(name: &str) -> Origin {
    Origin::person(format!("user:{name}"))
}

fn two_layer_document() -> Document {
    let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
    doc.layers.push(Layer::raster("Layer 1", PixelFormat::RGBA8));
    doc.layers.push(Layer::raster("Layer 2", PixelFormat::RGBA8));
    doc
}

fn first(blobs: &SharedBlobs) -> Session {
    let document = two_layer_document();
    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    let mut sink = blobs.clone();
    let (replica, ()) = Replica::create_with(PhotoModel::schema(), DocId::new(), actor(1), &Origin::system("create"), |tx| {
        write_document(tx, &document, &mut ids, &mut nodes, &mut sink).map(|_| ())
    })
    .unwrap_or_else(|error| panic!("create: {error}"));
    KitSession::from_replica(replica, Box::new(blobs.clone())).unwrap_or_else(|error| panic!("attach: {error}"))
}

/// The agent's draft of `of` as it is now: a session on a copy of its replica (`KitSession::fork`).
fn draft(of: &Session, value: u64, blobs: &SharedBlobs) -> Session {
    of.fork(actor(value), Box::new(blobs.clone())).unwrap_or_else(|error| panic!("fork: {error}"))
}

fn agent() -> Origin {
    Origin::agent("agent:test")
}

/// The draft's changes since it was made, taken into `document` as the person accepts them (the
/// host passes the agent's origin): answers the merge's undo scope.
fn merge(document: &mut Session, draft: &Session, since: &kition_kit::kdm::Version) -> KResult<Option<UndoScope>> {
    let changes = draft.export_changes(since).unwrap_or_else(|error| panic!("export: {error}"));
    document.merge_changes(&changes, &agent()).map(|(scope, _)| scope)
}

/// One undo of the person gives back `before` exactly, one redo `after`; the agent's undo has
/// nothing to take (the merge is the person's step, KitModel M12a).
fn one_step(document: &mut Session, before: &str, after: &str) {
    assert!(!document.can_undo_in(UndoScope::Agent), "the agent's undo does not take the merge");
    assert!(document.undo_in(UndoScope::Person).unwrap_or_else(|error| panic!("undo: {error}")).0);
    assert_eq!(document.state_hash().unwrap(), before, "one undo gives back the exact state before the merge");
    assert!(document.redo_in(UndoScope::Person).unwrap_or_else(|error| panic!("redo: {error}")).0);
    assert_eq!(document.state_hash().unwrap(), after, "one redo gives back the merge");
}

fn image_object(session: &Session) -> ObjectId {
    let mut images = session.replica().objects_of_type("Image");
    assert_eq!(images.len(), 1, "exactly one Image object");
    images.remove(0)
}

fn layer_named(session: &Session, name: &str) -> ObjectId {
    session
        .replica()
        .objects_of_type("Layer")
        .into_iter()
        .find(|id| session.replica().get(id, "name").unwrap_or(serde_json::Value::Null) == serde_json::json!(name))
        .unwrap_or_else(|| panic!("no layer named {name}"))
}

fn tile_blob(byte: u8) -> (String, Vec<u8>) {
    encoded_tile(PixelFormat::RGBA8, byte)
}

/// Like [`tile_blob`], but encoded as a mask surface decodes (always 8-bit grayscale,
/// `kdm_model::read`'s `mask_format`, regardless of the document's own depth).
fn mask_tile_blob(byte: u8) -> (String, Vec<u8>) {
    encoded_tile(PixelFormat::GRAY8, byte)
}

fn encoded_tile(format: PixelFormat, byte: u8) -> (String, Vec<u8>) {
    let mut surface = Surface::new(format);
    surface.tile_mut(TileCoord { tx: 0, ty: 0 }).bytes_mut()[0] = byte;
    let tile = surface.tile(TileCoord { tx: 0, ty: 0 }).cloned().unwrap_or_else(|| panic!("tile just written"));
    let encoded = kition_photo_format::encode_tile(format, &tile);
    ("0,0".to_owned(), encoded)
}

fn paint(session: &mut Session, layer: &ObjectId, origin: &Origin, key: &str, bytes: &[u8]) {
    let blob = session.blobs_mut().put(bytes, "application/x-kition-photo-tile+zstd");
    let layer = layer.clone();
    let key = key.to_owned();
    session
        .execute(origin, move |_, tx| -> KResult<()> {
            tx.map_set(&layer, "tiles", &key, serde_json::to_value(&blob).map_err(|error| kition_kit::session::KitError::new(error.to_string()))?)?;
            Ok(())
        })
        .unwrap_or_else(|error| panic!("paint: {error}"));
}

#[test]
fn a_draft_and_the_person_painting_different_tiles_of_one_layer_both_stay() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let since = a.version();
    let mut b = draft(&a, 2, &blobs);
    let layer = layer_named(&a, "Layer 1");

    let (key_b, bytes_b) = ("5,5".to_owned(), tile_blob(20).1);
    paint(&mut b, &layer, &agent(), &key_b, &bytes_b);
    let (key_a, bytes_a) = tile_blob(10);
    paint(&mut a, &layer, &person("a"), &key_a, &bytes_a);
    let before = a.state_hash().unwrap();

    assert_eq!(merge(&mut a, &b, &since).unwrap_or_else(|error| panic!("merge: {error}")), Some(UndoScope::Person));
    let tiles: Vec<String> = a.replica().map_entries(&layer, "tiles").unwrap().into_iter().map(|(key, _)| key).collect();
    assert_eq!(tiles, vec!["0,0".to_owned(), "5,5".to_owned()], "both tiles kept");
    assert_eq!(a.replica().map_get(&layer, "tiles", "0,0").unwrap()["hash"], serde_json::json!(sha256_hex(&bytes_a)), "the person's tile");
    assert_eq!(a.replica().map_get(&layer, "tiles", "5,5").unwrap()["hash"], serde_json::json!(sha256_hex(&bytes_b)), "the draft's tile");
    let fresh = PhotoModel::load(a.replica(), a.blobs()).unwrap_or_else(|error| panic!("fresh load: {error}"));
    assert_eq!(a.model(), &fresh, "the view follows the merge");
    let after = a.state_hash().unwrap();
    one_step(&mut a, &before, &after);
}

#[test]
fn the_same_tile_painted_in_the_draft_and_by_the_person_refuses_the_merge_naming_the_layer_and_tile() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let since = a.version();
    let mut b = draft(&a, 2, &blobs);
    let layer = layer_named(&a, "Layer 1");

    let (key, bytes_a) = tile_blob(10);
    let (_, bytes_b) = tile_blob(20);
    paint(&mut b, &layer, &agent(), &key, &bytes_b);
    paint(&mut a, &layer, &person("a"), &key, &bytes_a);
    let (before, version) = (a.state_hash().unwrap(), a.version());

    let refused = merge(&mut a, &b, &since).expect_err("the same tile changed on both sides").to_string();
    assert!(refused.contains(&format!("{layer}.tiles[\"0,0\"]")), "the conflict names the layer and the tile: {refused}");
    assert_eq!(a.state_hash().unwrap(), before, "a refused merge changes nothing");
    assert_eq!(a.version(), version);
    assert_eq!(a.replica().map_get(&layer, "tiles", "0,0").unwrap()["hash"], serde_json::json!(sha256_hex(&bytes_a)), "the person's tile stays");
}

#[test]
fn a_draft_and_the_person_editing_different_fields_of_one_layer_both_stay() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let since = a.version();
    let mut b = draft(&a, 2, &blobs);
    let layer = layer_named(&a, "Layer 1");

    let for_a = layer.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.set(&for_a, "opacity", serde_json::json!(0.5))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));
    let for_b = layer.clone();
    b.execute(&agent(), move |_, tx| -> KResult<()> {
        tx.set(&for_b, "name", serde_json::json!("Renamed"))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));
    let before = a.state_hash().unwrap();

    merge(&mut a, &b, &since).unwrap_or_else(|error| panic!("merge: {error}"));
    assert_eq!(a.replica().get(&layer, "opacity").unwrap().as_f64(), Some(0.5), "the person's edit kept");
    assert_eq!(a.replica().get(&layer, "name").unwrap(), serde_json::json!("Renamed"), "the draft's edit kept");
    assert!(a.model().document.layers.iter().any(|layer| layer.name == "Renamed" && layer.opacity == 0.5), "the view holds both");
    let after = a.state_hash().unwrap();
    one_step(&mut a, &before, &after);
}

#[test]
fn a_draft_reordering_layers_while_the_person_deletes_one_keeps_both() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let since = a.version();
    let mut b = draft(&a, 2, &blobs);
    let layer1 = layer_named(&a, "Layer 1");
    let layer2 = layer_named(&a, "Layer 2");
    let image = image_object(&a);

    let node2 = node_of(&b, &image, &layer2);
    let for_b_image = image.clone();
    b.execute(&agent(), move |_, tx| -> KResult<()> {
        tx.tree_move(&for_b_image, "layers", &node2, None)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    let for_a = layer1.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.delete(&for_a)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));

    merge(&mut a, &b, &since).unwrap_or_else(|error| panic!("merge: {error}"));
    assert!(!a.replica().exists(&layer1), "the delete stays");
    let names: Vec<String> = a.model().document.layers.iter().map(|layer| layer.name.clone()).collect();
    assert_eq!(names, vec!["Layer 2".to_owned()], "the reorder is taken and the deleted layer is gone, once each");
    let fresh = PhotoModel::load(a.replica(), a.blobs()).unwrap_or_else(|error| panic!("fresh load: {error}"));
    assert_eq!(a.model(), &fresh);
}

/// `object`'s current tree node, as `session` itself sees it.
fn node_of(session: &Session, image: &ObjectId, object: &ObjectId) -> kition_kit::kdm::NodeRef {
    session
        .replica()
        .tree(image, "layers")
        .unwrap_or_else(|error| panic!("tree: {error}"))
        .into_iter()
        .find(|entry| &entry.object == object)
        .unwrap_or_else(|| panic!("no tree node for {object}"))
        .node
}

#[test]
fn a_draft_and_the_person_moving_different_layers_into_one_group_both_land() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let image = image_object(&a);
    let layer1 = layer_named(&a, "Layer 1");
    let layer2 = layer_named(&a, "Layer 2");

    let for_setup = image.clone();
    let group = a
        .execute(&person("setup"), move |_, tx| -> KResult<ObjectId> {
            let fields = serde_json::json!({
                "name": "Group", "visible": true, "blend": "PassThrough", "opacity": 1.0, "fillOpacity": 1.0, "clipped": false, "kind": "group",
                "data": { "artboard": null }, "extra": {},
            });
            let group = tx.create("Layer", fields.as_object().cloned().unwrap_or_default())?;
            tx.tree_create(&for_setup, "layers", None, &group)?;
            Ok(group)
        })
        .unwrap_or_else(|error| panic!("setup: {error}"))
        .0;

    // The draft is made only now, so both start from the same tree.
    let since = a.version();
    let mut b = draft(&a, 2, &blobs);

    let (image_b, group_node_b, layer2_node_b) = (image.clone(), node_of(&b, &image, &group), node_of(&b, &image, &layer2));
    b.execute(&agent(), move |_, tx| -> KResult<()> {
        tx.tree_move(&image_b, "layers", &layer2_node_b, Some(&group_node_b))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    let (image_a, group_node_a, layer1_node_a) = (image.clone(), node_of(&a, &image, &group), node_of(&a, &image, &layer1));
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.tree_move(&image_a, "layers", &layer1_node_a, Some(&group_node_a))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));
    let before = a.state_hash().unwrap();

    merge(&mut a, &b, &since).unwrap_or_else(|error| panic!("merge: {error}"));
    let group_layer = a.model().document.layers.iter().find(|layer| layer.name == "Group").unwrap_or_else(|| panic!("group layer"));
    let children: std::collections::BTreeSet<&str> = match &group_layer.content {
        kition_photo_doc::LayerContent::Group(group) => group.children.iter().map(|layer| layer.name.as_str()).collect(),
        _ => Default::default(),
    };
    assert_eq!(children, std::collections::BTreeSet::from(["Layer 1", "Layer 2"]), "both moves landed in the group");
    let fresh = PhotoModel::load(a.replica(), a.blobs()).unwrap_or_else(|error| panic!("fresh load: {error}"));
    assert_eq!(a.model(), &fresh);
    let after = a.state_hash().unwrap();
    one_step(&mut a, &before, &after);
}

/// The person deletes a layer while the agent's draft paints its mask: the draft's tiles land on
/// a surface nothing references any more, which a fresh read never reads, so the merge is clean.
#[test]
fn deleting_a_layer_whose_mask_a_draft_paints_merges_without_crashing() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let layer1 = layer_named(&a, "Layer 1");

    let for_setup = layer1.clone();
    let (surface, _) = a
        .execute(&person("setup"), move |_, tx| -> KResult<ObjectId> {
            let surface = tx.create("Surface", serde_json::Map::new())?;
            tx.set(&for_setup, "mask", serde_json::json!(surface.as_str()))?;
            Ok(surface)
        })
        .unwrap_or_else(|error| panic!("setup: {error}"));
    assert!(a.model().document.layers[0].mask.is_some());

    let since = a.version();
    let mut b = draft(&a, 2, &blobs);

    let for_a = layer1.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.delete(&for_a)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));

    let (key, bytes) = mask_tile_blob(42);
    let blob = b.blobs_mut().put(&bytes, "application/x-kition-photo-tile+zstd");
    let for_b = surface.clone();
    b.execute(&agent(), move |_, tx| -> KResult<()> {
        tx.map_set(&for_b, "tiles", &key, serde_json::to_value(&blob).map_err(|error| kition_kit::session::KitError::new(error.to_string()))?)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    merge(&mut a, &b, &since).unwrap_or_else(|error| panic!("merge: {error}"));
    assert!(!a.replica().exists(&layer1), "the layer stays deleted");
    assert!(a.model().document.layers.iter().all(|layer| layer.name != "Layer 1"), "the view has no trace of it");
    let fresh = PhotoModel::load(a.replica(), a.blobs()).unwrap_or_else(|error| panic!("fresh load: {error}"));
    assert_eq!(a.model(), &fresh);
}

/// The draft deletes a mask's surface outright (the object model allows it) and leaves the layer
/// naming it: the repair inside the merge's step clears the layer's `mask`, so the view reads
/// a layer without a mask, the draft's preview is that state, and one undo gives back the exact
/// state before the merge.
#[test]
fn a_draft_that_deletes_a_masks_surface_is_repaired_inside_the_merge() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let layer1 = layer_named(&a, "Layer 1");
    let for_setup = layer1.clone();
    let (surface, _) = a
        .execute(&person("setup"), move |_, tx| -> KResult<ObjectId> {
            let surface = tx.create("Surface", serde_json::Map::new())?;
            tx.set(&for_setup, "mask", serde_json::json!(surface.as_str()))?;
            Ok(surface)
        })
        .unwrap_or_else(|error| panic!("setup: {error}"));
    let since = a.version();
    let mut b = draft(&a, 2, &blobs);
    // Written without the draft's view (`record`): its view cannot read a mask whose surface is gone.
    let for_b = surface.clone();
    b.record(&agent(), move |tx| -> KResult<()> {
        tx.delete(&for_b)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));
    let changes = b.export_changes(&since).unwrap();
    let before = a.state_hash().unwrap();
    let preview = a.merged_state(&changes).unwrap_or_else(|error| panic!("preview: {error}"));

    a.merge_changes(&changes, &agent()).unwrap_or_else(|error| panic!("merge: {error}"));
    assert!(!a.replica().exists(&surface));
    assert_eq!(a.replica().get(&layer1, "mask").unwrap(), serde_json::Value::Null, "the repair cleared the mask");
    assert!(a.model().document.layers.iter().all(|layer| layer.mask.is_none()));
    assert_eq!(preview, a.replica().state_json().unwrap(), "the preview is what accepting the draft gives");
    assert_eq!(a.replica().undo_depth(UndoScope::Person), 2, "the setup and the merge with its repair: one step each");
    let after = a.state_hash().unwrap();
    one_step(&mut a, &before, &after);
    assert!(a.model().document.layers[0].mask.is_none());
}

#[test]
fn undo_of_a_layer_delete_restores_it_in_place() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let layer1 = layer_named(&a, "Layer 1");

    let for_a = layer1.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.delete(&for_a)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("delete: {error}"));
    assert!(a.model().document.layers.iter().all(|layer| layer.name != "Layer 1"));

    let (done, damage) = a.undo_in(UndoScope::Person).unwrap_or_else(|error| panic!("undo: {error}"));
    assert!(done);
    assert_ne!(damage, Damage::None);
    assert!(a.model().document.layers.iter().any(|layer| layer.name == "Layer 1"), "restored in place");
    assert_eq!(a.model().document.layers.len(), 2);
}
