//! A `.kphoto` holds the image's content, not its history (kition-sdk data model 7.4; feature
//! `photo.file.content-only`): its replica entry (`content.json`) is the state alone, with no
//! version, origin or author, and none of the deleted or overwritten content; an unchanged image
//! saves to the same bytes, also once opened again; a file opened without a store is the same
//! document with the same object ids and edits; undo after a save takes back the last edit; and a
//! file opens on top of a stored replica.
//!
//! The needles below are written once each and share no run of four characters with anything else
//! in the image, so each literal is in the bytes that hold it: the test checks the session's
//! changes hold them (the control) before it checks the file does not.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use kition_kit::kdm::{ActorId, Replica, Version};
use kition_kit::session::KitModel;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Poster.kphoto";
const TYPED_THEN_DELETED: &str = "Jx4Hb8Ty3Nc6";
const DELETED_LAYER: &str = "Qv7Zp9Wm2Rk5";
const OVERWRITTEN: &str = "Ug5Le1Fo0Dw4";
const NEEDLES: [&str; 3] = [TYPED_THEN_DELETED, DELETED_LAYER, OVERWRITTEN];

fn contains(bytes: &[u8], needle: &str) -> bool {
    let utf16: Vec<u8> = needle.encode_utf16().flat_map(u16::to_le_bytes).collect();
    bytes.windows(needle.len()).any(|window| window == needle.as_bytes()) || bytes.windows(utf16.len()).any(|window| window == utf16.as_slice())
}

/// The replica entry of a `.kphoto`.
fn content(file: &[u8]) -> Vec<u8> {
    kition_photo_format::read_kdm_content(file).unwrap().expect("the file holds the replica's content")
}

/// What a `.kphoto` holds, decompressed: its replica entry and its manifest.
fn held(file: &[u8]) -> Vec<u8> {
    let zip = kition_photo_format::zip::ZipReader::new(file).unwrap();
    let mut out = content(file);
    out.extend(zip.read_by_name("manifest.json", 64 * 1024 * 1024).unwrap());
    out
}

fn version(doc: &mut PhotoDocument) -> Version {
    Version::decode(&doc.version().unwrap()).unwrap()
}

/// The file as a host saves it.
fn host_save(doc: &PhotoDocument) -> Vec<u8> {
    doc.save(Some(FILE)).unwrap()
}

fn layer(answer: &Value) -> u64 {
    answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer in {answer}"))
}

/// An image whose history holds typed-then-deleted text, a deleted layer and an overwritten layer
/// name; answers it, its type layer and the version its replica began at.
fn image_with_history() -> (PhotoDocument, u64, Vec<u8>) {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    let began = doc.version().unwrap();
    let title = layer(&doc.execute("text.add", json!({ "x": 4, "y": 20, "text": "Kept", "size": 12 })).unwrap());
    doc.execute("text.edit", json!({ "layer": title, "text": format!("Kept{TYPED_THEN_DELETED}") })).unwrap();
    doc.execute("text.edit", json!({ "layer": title, "text": "Kept" })).unwrap();
    let gone = layer(&doc.execute("layer.new.layer", json!({ "name": DELETED_LAYER })).unwrap());
    doc.execute("layer.delete", json!({ "layer": gone })).unwrap();
    let renamed = layer(&doc.execute("layer.new.layer", json!({ "name": OVERWRITTEN })).unwrap());
    doc.execute("layer.setProps", json!({ "layer": renamed, "name": "Paint" })).unwrap();
    (doc, title, began)
}

fn state(doc: &mut PhotoDocument) -> Value {
    let snapshot = doc.snapshot().unwrap();
    let mut state = Replica::load(kition_photo_engine::kdm_model::PhotoModel::schema(), &snapshot, ActorId::random()).unwrap().state_json().unwrap();
    state.as_object_mut().unwrap().remove("version");
    state["objects"].clone()
}

fn names(doc: &PhotoDocument) -> Vec<String> {
    doc.inspect().unwrap()["layers"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn the_file_holds_the_content_and_none_of_the_history() {
    let (mut doc, _, began) = image_with_history();
    let changes = doc.export_changes(&began).unwrap();
    for needle in NEEDLES {
        assert!(contains(&changes, needle), "the control: the session's changes hold {needle}");
    }
    let bytes = host_save(&doc);
    let inside = held(&bytes);
    for needle in NEEDLES {
        assert!(!contains(&inside, needle), "the file holds {needle}");
    }
    let entry: Value = serde_json::from_slice(&content(&bytes)).unwrap();
    let mut keys: Vec<&str> = entry.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["docId", "kdm", "kit", "objects", "schemaVersion"], "the state alone: no version, no line of changes");
    assert!(!contains(&content(&bytes), "\"lineage\""), "the content names no line of changes");
    assert_eq!(entry["docId"], json!(doc.doc_id().unwrap()));
    let mut back = PhotoDocument::open(FILE, &bytes).unwrap();
    assert_eq!(state(&mut back), state(&mut doc), "the state is all there");
}

#[test]
fn an_unchanged_image_saves_to_the_same_bytes() {
    let (mut doc, _, _) = image_with_history();
    let first = host_save(&doc);
    let at = version(&mut doc);
    assert_eq!(host_save(&doc), first, "two saves with nothing between");
    assert_eq!(version(&mut doc), at, "a save writes no change");
    doc.mark_saved();
    assert_eq!(host_save(&doc), first, "and after the host stored the first");
    // Opened again, the file saves to its own bytes, and the layers keep their object ids.
    let opened = PhotoDocument::open(FILE, &first).unwrap();
    assert_eq!(content(&host_save(&opened)), content(&first), "the replica entry, from the file opened again");
    assert_eq!(host_save(&opened), first, "the whole file, from the file opened again");
    assert_eq!(opened.layer_object_ids(), doc.layer_object_ids(), "the same object ids");
}

#[test]
fn a_file_opened_without_a_store_is_the_same_document_and_edits() {
    let (mut doc, title, _) = image_with_history();
    let bytes = host_save(&doc);
    doc.mark_saved();
    let mut back = PhotoDocument::open(FILE, &bytes).unwrap();
    assert!(back.has_replica());
    assert_eq!(back.doc_id(), doc.doc_id());
    assert_eq!(back.layer_object_ids(), doc.layer_object_ids(), "the same object ids");
    assert_eq!(state(&mut back), state(&mut doc), "the same state");
    back.execute("text.edit", json!({ "layer": title, "text": "Edited" })).unwrap();
    back.execute("layer.new.layer", json!({ "name": "New" })).unwrap();
    let saved = host_save(&back);
    let again = PhotoDocument::open(FILE, &saved).unwrap();
    assert_eq!(again.doc_id(), doc.doc_id());
    assert!(names(&again).contains(&"New".to_owned()), "{:?}", names(&again));
    assert_eq!(again.inspect().unwrap()["layers"].as_array().unwrap().iter().find(|l| l["id"] == title).unwrap()["text"]["text"], "Edited");
}

#[test]
fn undo_after_a_save_takes_back_the_last_edit() {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    doc.execute("layer.new.layer", json!({ "name": "A" })).unwrap();
    let b = layer(&doc.execute("layer.new.layer", json!({ "name": "B" })).unwrap());
    host_save(&doc);
    doc.mark_saved();
    assert!(doc.inspect().unwrap()["canUndo"].as_bool().unwrap(), "a save is no undo step and clears none");
    doc.execute("edit.undo", json!({})).unwrap();
    assert!(!names(&doc).contains(&"B".to_owned()), "the undo took back the last layer: {:?}", names(&doc));
    assert!(doc.inspect().unwrap()["layers"].as_array().unwrap().iter().all(|l| l["id"] != b));
    doc.execute("edit.undo", json!({})).unwrap();
    assert!(!names(&doc).contains(&"A".to_owned()), "then the one before");
    doc.execute("edit.redo", json!({})).unwrap();
    assert!(names(&doc).contains(&"A".to_owned()));
}

#[test]
fn a_file_opens_on_the_stored_replica_that_holds_its_version() {
    let (mut doc, _, _) = image_with_history();
    let bytes = host_save(&doc);
    doc.mark_saved();
    let stored = doc.snapshot().unwrap();
    let blobs = doc.blobs().unwrap();
    let mut opened = PhotoDocument::open_with_replica(FILE, &bytes, &stored, &blobs).unwrap();
    assert_eq!(version(&mut opened), version(&mut doc), "the stored replica, as it was: the file holds its state");
    assert_eq!(host_save(&opened), bytes, "and saves the document it opened, to the same bytes");
    assert_eq!(version(&mut opened), version(&mut doc), "writing nothing");
}

#[test]
fn a_file_saved_elsewhere_opens_on_the_stored_replica_by_its_state() {
    let (mut doc, title, _) = image_with_history();
    let stored = doc.snapshot().unwrap();
    let stored_at = version(&mut doc);
    let blobs = doc.blobs().unwrap();
    let bytes = host_save(&doc);
    doc.mark_saved();
    // Elsewhere, from the file alone: an edit, a new layer, a save.
    let mut elsewhere = PhotoDocument::open(FILE, &bytes).unwrap();
    elsewhere.execute("text.edit", json!({ "layer": title, "text": "Elsewhere" })).unwrap();
    elsewhere.execute("layer.new.layer", json!({ "name": "New" })).unwrap();
    let edited = host_save(&elsewhere);
    let mut opened = PhotoDocument::open_with_replica(FILE, &edited, &stored, &blobs).unwrap();
    assert_eq!(opened.doc_id(), doc.doc_id());
    // The file's state, by object id: the whole canonical state, the deleted layer's tree node
    // included once (the stored replica keeps it and the state adds none of its own).
    assert_eq!(state(&mut opened), state(&mut elsewhere), "the file's state, by object id");
    assert_eq!(opened.layer_object_ids(), elsewhere.layer_object_ids(), "the same layers, by object id");
    assert_eq!(opened.inspect().unwrap()["layers"], elsewhere.inspect().unwrap()["layers"], "the file's state");
    let at = version(&mut opened);
    assert!(at.includes(&stored_at) && at != stored_at, "on top of the stored replica, its line of changes going on");
}

/// Every hash a manifest's JSON names (any string of 64 lowercase hex digits).
fn manifest_hashes(value: &Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::String(text) if text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) => {
            out.insert(text.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| manifest_hashes(item, out)),
        Value::Object(members) => members.values().for_each(|member| manifest_hashes(member, out)),
        _ => {}
    }
}

/// Pixels are stored once (case `photo.file.content-only/pixels-stored-once`): an image of N
/// distinct tiles is a file of N tile entries, and the manifest and the replica both name those
/// same entries; the file holds no entry neither names, and none twice.
#[test]
fn a_file_with_n_distinct_tiles_holds_n_pixel_entries_that_the_manifest_and_the_replica_share() {
    use std::collections::BTreeSet;
    // 3 × 2 tiles of clouds, each tile its own pixels.
    let mut doc = PhotoDocument::create(FILE, 768, 512).unwrap();
    doc.execute("filter.render.clouds", json!({ "seed": 5 })).unwrap();
    let bytes = host_save(&doc);
    let zip = kition_photo_format::zip::ZipReader::new(&bytes).unwrap();
    let names: Vec<String> = zip.entries.iter().map(|entry| entry.name.clone()).collect();
    let tiles: BTreeSet<String> = names.iter().filter_map(|name| name.strip_prefix("tiles/")).map(str::to_owned).collect();
    assert_eq!(tiles.len(), 6, "N distinct tiles, N tile entries: {names:?}");
    let blobs: BTreeSet<String> =
        names.iter().filter_map(|name| name.strip_prefix("blobs/").and_then(|rest| rest.strip_suffix(".zst"))).map(str::to_owned).collect();
    for name in &names {
        assert!(name == "manifest.json" || name == "content.json" || name.starts_with("tiles/") || name.starts_with("blobs/"), "{name}");
    }

    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 64 << 20).unwrap()).unwrap();
    let mut by_manifest = BTreeSet::new();
    manifest_hashes(&manifest, &mut by_manifest);
    let replica = Replica::load(kition_photo_engine::kdm_model::PhotoModel::schema(), &content(&bytes), ActorId::random()).unwrap();
    let by_replica = kition_photo_engine::kdm_model::state_blobs(&replica);
    assert!(tiles.is_subset(&by_manifest), "the manifest names every tile entry");
    assert!(tiles.is_subset(&by_replica), "the replica names the same tile entries: {tiles:?} {by_replica:?}");
    let stored: BTreeSet<String> = tiles.union(&blobs).cloned().collect();
    assert_eq!(stored.len(), tiles.len() + blobs.len(), "no hash is stored twice");
    let named: BTreeSet<String> = by_manifest.union(&by_replica).cloned().collect();
    assert_eq!(stored, named, "the file holds what the manifest or the replica names, and nothing else");

    // A copy of a layer adds no pixels: its tiles are the same entries.
    doc.execute("layer.duplicate", json!({})).unwrap();
    let copied = host_save(&doc);
    let copied_tiles = kition_photo_format::zip::ZipReader::new(&copied).unwrap().entries.iter().filter(|entry| entry.name.starts_with("tiles/")).count();
    assert_eq!(copied_tiles, 6, "a duplicated layer shares its tiles");
}
