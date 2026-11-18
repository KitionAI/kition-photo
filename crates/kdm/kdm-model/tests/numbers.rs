//! Layer ids survive reading the replica (KHP A46, A57): every layer stores its engine id in its
//! read-only `number` field, so a reopen, a replica-only open and an export opened elsewhere give
//! the ids the session had. Two replicas that mint one id concurrently end with distinct ids by a
//! rule every read applies alike (the layer with the smaller object id keeps it, the other takes
//! the next free one), whatever the merge order, and the merge writes nothing; the replica's next
//! edit stores the new number. A layer made through the object model, which stores no number,
//! gets the next free one, stored by the repair.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use kdm_model::{PhotoModel, PhotoSession};
use kition_kit::kdm::{ActorId, ObjectId, Origin, Replica, Version};
use kition_kit::session::KitModel;
use kition_photo_doc::{ColorMode, Document, Layer, LayerId, PixelFormat, SampleType, Size};
use serde_json::json;

fn person(name: &str) -> Origin {
    Origin::person(format!("user:{name}"))
}

fn layer(name: &str, id: u64) -> Layer {
    let mut layer = Layer::raster(name, PixelFormat::RGBA8);
    layer.id = LayerId(id);
    layer
}

/// A document whose layers carry the ids a document numbers them with.
fn base() -> Document {
    let mut doc = Document::new("t", Size { width: 64, height: 64 }, ColorMode::Rgb, SampleType::U8);
    doc.layers.push(layer("Background", 1));
    let mut group = Layer::group("Group", vec![layer("Inside", 3)]);
    group.id = LayerId(2);
    doc.layers.push(group);
    doc.layers.push(layer("Top", 4));
    doc
}

/// Every layer's id by name.
fn ids(session: &PhotoSession) -> BTreeMap<String, u64> {
    session.document().walk().into_iter().map(|(_, _, l)| (l.name.clone(), l.id.0)).collect()
}

fn reopened(session: &PhotoSession) -> PhotoSession {
    PhotoSession::open_snapshot(&session.snapshot().unwrap(), session.blobs().clone()).unwrap()
}

#[test]
fn every_read_of_the_replica_gives_the_ids_the_session_had() {
    let session = PhotoSession::create(&base()).unwrap();
    let want: BTreeMap<String, u64> = base().walk().into_iter().map(|(_, _, l)| (l.name.clone(), l.id.0)).collect();
    assert_eq!(ids(&session), want);
    assert_eq!(ids(&reopened(&session)), want, "opened from its snapshot");
    assert_eq!(ids(&reopened(&reopened(&session))), want, "twice");
    let exported = PhotoSession::open_snapshot(&session.export_changes(&Version::empty()).unwrap(), session.blobs().clone()).unwrap();
    assert_eq!(ids(&exported), want, "opened from every change it exported");
}

#[test]
fn layers_made_in_a_session_keep_their_ids_through_undo_and_reopen() {
    let mut session = PhotoSession::create(&base()).unwrap();
    let mut after = session.document().clone();
    after.layers.push(layer("New", 9));
    session.record(&person("a"), &after).unwrap();
    assert_eq!(ids(&session)["New"], 9);
    assert_eq!(ids(&reopened(&session))["New"], 9);
    let mut gone = session.document().clone();
    gone.layers.retain(|l| l.name != "Top");
    session.record(&person("a"), &gone).unwrap();
    session.undo_in(kition_kit::kdm::UndoScope::Person).unwrap();
    assert_eq!(ids(&session)["Top"], 4, "an undone delete brings the layer back under its id");
    assert_eq!(ids(&reopened(&session))["Top"], 4);
}

/// Two replicas of one base each add a layer offline; both mint id 5.
fn concurrent(a_first: bool) -> (PhotoSession, PhotoSession, ObjectId, ObjectId) {
    let start = PhotoSession::create(&base()).unwrap();
    let mut a = reopened(&start);
    let mut b = reopened(&start);
    let mut doc = a.document().clone();
    doc.layers.push(layer("From A", 5));
    a.record(&person("a"), &doc).unwrap();
    let mut doc = b.document().clone();
    doc.layers.push(layer("From B", 5));
    b.record(&person("b"), &doc).unwrap();
    let a_layer = a.model().ids().object(LayerId(5)).unwrap().clone();
    let b_layer = b.model().ids().object(LayerId(5)).unwrap().clone();
    let (va, vb) = (a.version(), b.version());
    let (from_a, from_b) = (a.export_changes(&vb).unwrap(), b.export_changes(&va).unwrap());
    let blobs = (a.blobs().clone(), b.blobs().clone());
    a.add_blobs(&blobs.1);
    b.add_blobs(&blobs.0);
    if a_first {
        b.import_changes(&from_a).unwrap();
        a.import_changes(&from_b).unwrap();
    } else {
        a.import_changes(&from_b).unwrap();
        b.import_changes(&from_a).unwrap();
    }
    (a, b, a_layer, b_layer)
}

#[test]
fn concurrent_layers_with_one_id_end_with_distinct_ids_on_both_replicas() {
    for a_first in [true, false] {
        let (mut a, mut b, a_layer, b_layer) = concurrent(a_first);
        assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap(), "the replicas converge");
        assert!(a.version().includes(&b.version()) && b.version().includes(&a.version()), "the merge itself writes nothing");
        assert_eq!(ids(&a), ids(&b), "and give every layer the same id (a first: {a_first})");
        let (keeper, other) = if a_layer < b_layer { (&a_layer, &b_layer) } else { (&b_layer, &a_layer) };
        let local = |s: &PhotoSession, object: &ObjectId| s.model().ids().local(object).unwrap().0;
        assert_eq!(local(&a, keeper), 5, "the smaller object id keeps the number");
        assert_eq!(local(&b, keeper), 5);
        let next = local(&a, other);
        assert_eq!(next, 6, "the other takes the next free one");
        assert_eq!(local(&b, other), next);
        let all: Vec<u64> = ids(&a).into_values().collect();
        let unique: std::collections::BTreeSet<u64> = all.iter().copied().collect();
        assert_eq!(unique.len(), all.len(), "ids are unique: {:?}", ids(&a));
        assert_eq!(ids(&reopened(&a)), ids(&a), "a reopen agrees");

        // The next edit stores the new number; the peer that takes it agrees.
        let mut doc = a.document().clone();
        doc.layers.push(layer("Later", 7));
        a.record(&person("a"), &doc).unwrap();
        assert_eq!(a.replica().get(other, "number").unwrap(), json!(next), "the edit stores the new number");
        b.add_blobs(&a.blobs().clone());
        b.import_changes(&a.export_changes(&b.version()).unwrap()).unwrap();
        assert_eq!(ids(&b), ids(&a));
        assert_eq!(local(&b, other), next);
    }
}

/// A layer made through the object model (`model.batch`) stores no number: every replica gives it
/// the next free one, and the repair stores it.
#[test]
fn a_layer_made_without_a_number_gets_the_next_free_one_stored() {
    let mut session = PhotoSession::create(&base()).unwrap();
    let mut raw = Replica::load(PhotoModel::schema(), &session.snapshot().unwrap(), ActorId::from_u64(77).unwrap()).unwrap();
    let since = raw.version();
    let image = ObjectId::from_string("image");
    let made = raw
        .transact(&person("script"), |tx| {
            let fields = json!({
                "name": "Scripted", "visible": true, "blend": "Normal", "opacity": 1.0, "fillOpacity": 1.0,
                "clipped": false, "kind": "raster",
            });
            let object = tx.create("Layer", fields.as_object().unwrap().clone())?;
            tx.tree_create(&image, "layers", None, &object)?;
            Ok(object)
        })
        .unwrap();
    session.import_changes(&raw.export_changes(&since).unwrap()).unwrap();
    assert_eq!(ids(&session)["Scripted"], 5, "the next free id");
    assert_eq!(session.replica().get(&made, "number").unwrap(), json!(5), "stored by the repair");
    assert_eq!(ids(&reopened(&session))["Scripted"], 5);
}
