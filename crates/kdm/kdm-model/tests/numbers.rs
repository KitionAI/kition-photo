//! Layer ids survive reading the replica (KHP A46, A57): every layer stores its engine id in its
//! read-only `number` field, so a reopen, a replica-only open and an export opened elsewhere give
//! the ids the session had. A draft and the document that each mint one id end, once the draft
//! is merged, with distinct ids by a rule every read applies alike (the layer with the smaller
//! object id keeps it, the other takes the next free one), and the merge stores no number of its
//! own; the replica's next edit stores the new number. A layer made through the object model,
//! which stores no number, gets the next free one, stored by the repair (inside the merge's step
//! when a draft made it).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;

use kdm_model::{PhotoModel, PhotoSession};
use kition_kit::kdm::{ActorId, ObjectId, Origin, Replica, UndoScope, Version};
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
    let content = PhotoSession::open_snapshot(&session.content().unwrap().content, session.blobs().clone()).unwrap();
    assert_eq!(ids(&content), want, "opened from the content a file holds");
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

/// An agent's draft and the person each add a layer; both mint id 5. The person accepts the
/// draft.
fn both_mint_five() -> (PhotoSession, ObjectId, ObjectId) {
    let mut a = PhotoSession::create(&base()).unwrap();
    let mut b = reopened(&a);
    let since = b.version();
    let mut doc = b.document().clone();
    doc.layers.push(layer("From B", 5));
    b.record(&Origin::agent("agent:b"), &doc).unwrap();
    let mut doc = a.document().clone();
    doc.layers.push(layer("From A", 5));
    a.record(&person("a"), &doc).unwrap();
    let a_layer = a.model().ids().object(LayerId(5)).unwrap().clone();
    let b_layer = b.model().ids().object(LayerId(5)).unwrap().clone();
    a.add_blobs(&b.blobs().clone());
    let changes = b.export_changes(&since).unwrap();
    assert_eq!(a.merge_changes(&changes, &Origin::agent("agent:b")).unwrap().0, Some(UndoScope::Person));
    (a, a_layer, b_layer)
}

#[test]
fn a_draft_and_the_document_minting_one_id_end_with_distinct_ids() {
    let (mut a, a_layer, b_layer) = both_mint_five();
    let (keeper, other) = if a_layer < b_layer { (&a_layer, &b_layer) } else { (&b_layer, &a_layer) };
    assert_eq!(a.replica().get(other, "number").unwrap(), json!(5), "the merge stores no number of its own");
    let local = |s: &PhotoSession, object: &ObjectId| s.model().ids().local(object).unwrap().0;
    assert_eq!(local(&a, keeper), 5, "the smaller object id keeps the number");
    let next = local(&a, other);
    assert_eq!(next, 6, "the other takes the next free one");
    let all: Vec<u64> = ids(&a).into_values().collect();
    let unique: std::collections::BTreeSet<u64> = all.iter().copied().collect();
    assert_eq!(unique.len(), all.len(), "ids are unique: {:?}", ids(&a));
    assert_eq!(ids(&reopened(&a)), ids(&a), "a reopen agrees");

    // The next edit stores the new number; a reopen agrees.
    let mut doc = a.document().clone();
    doc.layers.push(layer("Later", 7));
    a.record(&person("a"), &doc).unwrap();
    assert_eq!(a.replica().get(other, "number").unwrap(), json!(next), "the edit stores the new number");
    assert_eq!(local(&reopened(&a), other), next);
}

/// A layer the agent's draft made through the object model stores no number: the repair inside
/// the merge's step stores the next free one, so one undo of the person takes back the layer and
/// its number together, exactly.
#[test]
fn a_layer_a_draft_made_without_a_number_is_numbered_inside_the_merge() {
    let mut session = PhotoSession::create(&base()).unwrap();
    let mut raw = Replica::load(PhotoModel::schema(), &session.snapshot().unwrap(), ActorId::from_u64(78).unwrap()).unwrap();
    let since = raw.version();
    let image = ObjectId::from_string("image");
    let made = raw
        .transact(&Origin::agent("agent:script"), |tx| {
            let fields = json!({
                "name": "Drafted", "visible": true, "blend": "Normal", "opacity": 1.0, "fillOpacity": 1.0,
                "clipped": false, "kind": "raster",
            });
            let object = tx.create("Layer", fields.as_object().unwrap().clone())?;
            tx.tree_create(&image, "layers", None, &object)?;
            Ok(object)
        })
        .unwrap();
    let changes = raw.export_changes(&since).unwrap();
    let before = session.state_hash().unwrap();
    let preview = session.merged_state(&changes).unwrap();
    assert_eq!(session.merge_changes(&changes, &Origin::agent("agent:script")).unwrap().0, Some(UndoScope::Person));
    assert_eq!(ids(&session)["Drafted"], 5, "the next free id");
    assert_eq!(session.replica().get(&made, "number").unwrap(), json!(5), "stored inside the merge");
    assert_eq!(preview, session.replica().state_json().unwrap(), "the draft's preview holds the number too");
    let after = session.state_hash().unwrap();
    assert!(session.undo_in(UndoScope::Person).unwrap().0);
    assert_eq!(session.state_hash().unwrap(), before, "one undo takes back the layer and its number");
    assert!(session.redo_in(UndoScope::Person).unwrap().0);
    assert_eq!(session.state_hash().unwrap(), after);
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
