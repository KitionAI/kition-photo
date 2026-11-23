//! Two-replica merges, through `KitSession<PhotoModel>`: concurrent edits that touch different
//! state both survive; edits to the same register resolve to one winner, and both replicas agree
//! on which.
//!
//! Blobs live outside the replica (D6: a workspace asset store), so these tests share one blob
//! store between a session and its peers (`SharedBlobs`) rather than giving each its own, empty
//! one — closer to how two copies of a real document would see the same tile bytes.

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

fn peer(of: &Session, value: u64, blobs: &SharedBlobs) -> Session {
    let snapshot = of.snapshot().unwrap_or_else(|error| panic!("snapshot: {error}"));
    KitSession::open_snapshot(&snapshot, Box::new(blobs.clone()), actor(value)).unwrap_or_else(|error| panic!("open_snapshot: {error}"))
}

fn sync(a: &mut Session, b: &mut Session) {
    for _ in 0..8 {
        a.import_changes(&b.export_changes(&a.version()).unwrap_or_else(|error| panic!("export: {error}"))).unwrap_or_else(|error| panic!("import: {error}"));
        b.import_changes(&a.export_changes(&b.version()).unwrap_or_else(|error| panic!("export: {error}"))).unwrap_or_else(|error| panic!("import: {error}"));
        if a.version() == b.version() {
            return;
        }
    }
    panic!("the replicas did not converge");
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
fn concurrent_strokes_in_different_tiles_both_survive() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let mut b = peer(&a, 2, &blobs);
    let layer = layer_named(&a, "Layer 1");

    let (key_a, bytes_a) = tile_blob(10);
    paint(&mut a, &layer, &person("a"), &key_a, &bytes_a);
    let (key_b, bytes_b) = ("5,5".to_owned(), tile_blob(20).1);
    paint(&mut b, &layer, &person("b"), &key_b, &bytes_b);

    sync(&mut a, &mut b);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert_eq!(a.replica().map_entries(&layer, "tiles").unwrap().len(), 2, "both tiles kept");
    assert_eq!(a.model(), b.model());
}

#[test]
fn concurrent_strokes_in_the_same_tile_pick_one_winner_on_both_replicas() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let mut b = peer(&a, 2, &blobs);
    let layer = layer_named(&a, "Layer 1");

    let (key, bytes_a) = tile_blob(10);
    let (_, bytes_b) = tile_blob(20);
    paint(&mut a, &layer, &person("a"), &key, &bytes_a);
    paint(&mut b, &layer, &person("b"), &key, &bytes_b);

    sync(&mut a, &mut b);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    let tiles = a.replica().map_entries(&layer, "tiles").unwrap();
    assert_eq!(tiles.len(), 1, "one register, one winner");
    assert_eq!(tiles, b.replica().map_entries(&layer, "tiles").unwrap(), "both replicas agree on the winner");
}

#[test]
fn concurrent_property_edits_of_one_layer_both_survive() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let mut b = peer(&a, 2, &blobs);
    let layer = layer_named(&a, "Layer 1");

    let for_a = layer.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.set(&for_a, "opacity", serde_json::json!(0.5))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));
    let for_b = layer.clone();
    b.execute(&person("b"), move |_, tx| -> KResult<()> {
        tx.set(&for_b, "name", serde_json::json!("Renamed"))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    sync(&mut a, &mut b);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert_eq!(a.replica().get(&layer, "opacity").unwrap().as_f64(), Some(0.5), "a's edit kept");
    assert_eq!(a.replica().get(&layer, "name").unwrap(), serde_json::json!("Renamed"), "b's edit kept");
}

#[test]
fn concurrent_reorder_and_delete_both_survive() {
    let blobs = SharedBlobs::default();
    let mut a = first(&blobs);
    let mut b = peer(&a, 2, &blobs);
    let layer1 = layer_named(&a, "Layer 1");
    let layer2 = layer_named(&a, "Layer 2");
    let image = image_object(&a);

    let entries = a.replica().tree(&image, "layers").unwrap_or_else(|error| panic!("tree: {error}"));
    let node2 = entries.iter().find(|entry| entry.object == layer2).unwrap_or_else(|| panic!("node for layer 2")).node.clone();
    let for_a_image = image.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.tree_move(&for_a_image, "layers", &node2, None)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));

    let for_b = layer1.clone();
    b.execute(&person("b"), move |_, tx| -> KResult<()> {
        tx.delete(&for_b)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    sync(&mut a, &mut b);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert!(!a.replica().exists(&layer1), "the delete survives");
    let names: Vec<String> = a.model().document.layers.iter().map(|layer| layer.name.clone()).collect();
    assert_eq!(names, vec!["Layer 2".to_owned()], "the reorder survives and the deleted layer is gone, once each");
    assert_eq!(a.model(), b.model());
}

/// `group`'s current tree node, as `session` itself sees it.
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
fn concurrent_moves_of_different_layers_into_the_same_group_converge() {
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

    // `b` forks only now, so both replicas start from the same committed tree and each can read
    // its own `NodeRef`s for the group and for the layer it is about to move.
    let mut b = peer(&a, 2, &blobs);

    let (image_a, group_node_a, layer1_node_a) = (image.clone(), node_of(&a, &image, &group), node_of(&a, &image, &layer1));
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.tree_move(&image_a, "layers", &layer1_node_a, Some(&group_node_a))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));

    let (image_b, group_node_b, layer2_node_b) = (image.clone(), node_of(&b, &image, &group), node_of(&b, &image, &layer2));
    b.execute(&person("b"), move |_, tx| -> KResult<()> {
        tx.tree_move(&image_b, "layers", &layer2_node_b, Some(&group_node_b))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    sync(&mut a, &mut b);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert_eq!(a.model(), b.model());

    let group_layer = a.model().document.layers.iter().find(|layer| layer.name == "Group").unwrap_or_else(|| panic!("group layer"));
    let children: std::collections::BTreeSet<&str> = match &group_layer.content {
        kition_photo_doc::LayerContent::Group(group) => group.children.iter().map(|layer| layer.name.as_str()).collect(),
        _ => Default::default(),
    };
    assert_eq!(children, std::collections::BTreeSet::from(["Layer 1", "Layer 2"]), "both concurrent moves landed in the group");
}

#[test]
fn deleting_a_layer_whose_mask_a_peer_edits_merges_without_crashing() {
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

    let mut b = peer(&a, 2, &blobs);

    let for_a = layer1.clone();
    a.execute(&person("a"), move |_, tx| -> KResult<()> {
        tx.delete(&for_a)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("a: {error}"));

    let (key, bytes) = mask_tile_blob(42);
    let blob = b.blobs_mut().put(&bytes, "application/x-kition-photo-tile+zstd");
    let for_b = surface.clone();
    b.execute(&person("b"), move |_, tx| -> KResult<()> {
        tx.map_set(&for_b, "tiles", &key, serde_json::to_value(&blob).map_err(|error| kition_kit::session::KitError::new(error.to_string()))?)?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("b: {error}"));

    sync(&mut a, &mut b);
    assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
    assert!(!a.replica().exists(&layer1), "the layer stays deleted");
    assert!(a.model().document.layers.iter().all(|layer| layer.name != "Layer 1"), "the view has no trace of it");
    // Loading fresh from the merged replica is just as clean: the orphaned mask surface (nothing
    // references it any more) never gets read, so it is harmless, not a dangling reference error.
    let fresh = PhotoModel::load(a.replica(), a.blobs()).unwrap_or_else(|error| panic!("fresh load: {error}"));
    assert_eq!(a.model(), &fresh);
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
