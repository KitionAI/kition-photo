//! A `.kphoto` holds the image's content, not its history (kition-sdk data model 7.3, rule M16b;
//! feature `photo.file.content-only`): its replica entry (`kdm.bin`) holds one change (the seal)
//! and none of the deleted or overwritten content; an unchanged image saves to the same bytes; a
//! file opened without a store is the same document and edits; a replica from the file and one
//! with the whole history converge; undo after a save takes back the last edit; and a file opens
//! on top of a stored replica. Hosts seal before saving (`prepare_save`); the one fallback, a save
//! nobody sealed for (the kit editor before its release that calls `prepareSave`), seals a copy and
//! `mark_saved` takes the seal in.
//!
//! The needles below are written once each and share no run of four characters with anything else
//! in the image, so the engine's compression keeps each literal in the bytes that hold it: every
//! test checks the session's full snapshot holds them (the control) before it checks the file does
//! not.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use kition_kit::kdm::{ActorId, Origin, Replica, Version};
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
    kition_photo_format::read_kdm_snapshot(file).unwrap().expect("the file holds the replica's content")
}

/// What a `.kphoto` holds, decompressed: its replica entry and its manifest.
fn held(file: &[u8]) -> Vec<u8> {
    let zip = kition_photo_format::zip::ZipReader::new(file).unwrap();
    let mut out = content(file);
    out.extend(zip.read_by_name("manifest.json", 64 * 1024 * 1024).unwrap());
    out
}

fn version_of(file: &[u8]) -> Version {
    Replica::load(kition_photo_engine::kdm_model::PhotoModel::schema(), &content(file), ActorId::random()).unwrap().version()
}

fn version(doc: &mut PhotoDocument) -> Version {
    Version::decode(&doc.version().unwrap()).unwrap()
}

/// The file as a host saves it: sealed first (`prepare_save`), then the bytes.
fn host_save(doc: &mut PhotoDocument) -> Vec<u8> {
    doc.prepare_save().unwrap();
    doc.save(Some(FILE)).unwrap()
}

fn layer(answer: &Value) -> u64 {
    answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer in {answer}"))
}

/// An image whose history holds typed-then-deleted text, a deleted layer and an overwritten layer
/// name; answers it and its type layer.
fn image_with_history() -> (PhotoDocument, u64) {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    let title = layer(&doc.execute("type.create", json!({ "x": 4, "y": 20, "text": "Kept", "size": 12 })).unwrap());
    doc.execute("type.edit", json!({ "layer": title, "text": format!("Kept{TYPED_THEN_DELETED}") })).unwrap();
    doc.execute("type.edit", json!({ "layer": title, "text": "Kept" })).unwrap();
    let gone = layer(&doc.execute("layer.new.layer", json!({ "name": DELETED_LAYER })).unwrap());
    doc.execute("layer.delete", json!({ "layer": gone })).unwrap();
    let renamed = layer(&doc.execute("layer.new.layer", json!({ "name": OVERWRITTEN })).unwrap());
    doc.execute("layer.setProps", json!({ "layer": renamed, "name": "Paint" })).unwrap();
    (doc, title)
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
fn the_file_holds_the_content_and_of_the_history_only_the_seal() {
    let (mut doc, _) = image_with_history();
    let full = doc.snapshot().unwrap();
    for needle in NEEDLES {
        assert!(contains(&full, needle), "the control: the session's history holds {needle}");
    }
    assert!(Replica::snapshot_origins(&full).unwrap().len() > 1, "the control: the history holds every change");
    let bytes = host_save(&mut doc);
    let inside = held(&bytes);
    for needle in NEEDLES {
        assert!(!contains(&inside, needle), "the file holds {needle}");
    }
    assert_eq!(Replica::snapshot_origins(&content(&bytes)).unwrap(), vec![Some(Origin::system("content-seal"))], "of the history, one change: the seal");
    // The direct path: the seal is the session's own.
    assert_eq!(version(&mut doc), version_of(&bytes));
    let mut back = PhotoDocument::open(FILE, &bytes).unwrap();
    assert_eq!(state(&mut back), state(&mut doc), "the state is all there");
}

#[test]
fn an_unchanged_image_saves_to_the_same_bytes() {
    let (mut doc, _) = image_with_history();
    let first = host_save(&mut doc);
    let at = version(&mut doc);
    assert_eq!(host_save(&mut doc), first, "two saves with nothing between");
    assert_eq!(version(&mut doc), at, "a sealed image seals nothing more");
    doc.mark_saved();
    assert_eq!(host_save(&mut doc), first, "and after the host stored the first");
    // Opened again, the replica entry is the same bytes (the manifest names the session's own
    // document handle, `document.id`, which a new session numbers anew).
    let mut opened = PhotoDocument::open(FILE, &first).unwrap();
    assert_eq!(content(&host_save(&mut opened)), content(&first), "and from the file opened again");
}

#[test]
fn a_file_opened_without_a_store_is_the_same_document_and_edits() {
    let (mut doc, title) = image_with_history();
    let bytes = host_save(&mut doc);
    doc.mark_saved();
    let mut back = PhotoDocument::open(FILE, &bytes).unwrap();
    assert!(back.has_replica());
    assert_eq!(back.doc_id(), doc.doc_id());
    assert_eq!(back.layer_object_ids(), doc.layer_object_ids(), "the same object ids");
    assert_eq!(version(&mut back), version(&mut doc), "the same version: the seal is the session's");
    back.execute("type.edit", json!({ "layer": title, "text": "Edited" })).unwrap();
    back.execute("layer.new.layer", json!({ "name": "New" })).unwrap();
    let saved = host_save(&mut back);
    let again = PhotoDocument::open(FILE, &saved).unwrap();
    assert_eq!(again.doc_id(), doc.doc_id());
    assert!(names(&again).contains(&"New".to_owned()), "{:?}", names(&again));
    assert_eq!(again.inspect().unwrap()["layers"].as_array().unwrap().iter().find(|l| l["id"] == title).unwrap()["text"]["text"], "Edited");
}

#[test]
fn a_replica_from_the_file_and_one_with_the_whole_history_converge() {
    let (mut full, title) = image_with_history();
    let bytes = host_save(&mut full);
    full.mark_saved();
    let mut file = PhotoDocument::open(FILE, &bytes).unwrap();
    full.execute("layer.new.layer", json!({ "name": "Full" })).unwrap();
    file.execute("type.edit", json!({ "layer": title, "text": "From the file" })).unwrap();
    for _ in 0..4 {
        let (to_file, to_full) = (full.export_changes(&file.version().unwrap()).unwrap(), file.export_changes(&full.version().unwrap()).unwrap());
        let (blobs_full, blobs_file) = (full.blobs().unwrap(), file.blobs().unwrap());
        full.add_blobs(&blobs_file).unwrap();
        file.add_blobs(&blobs_full).unwrap();
        full.import_changes(&to_full).unwrap();
        file.import_changes(&to_file).unwrap();
        if full.version().unwrap() == file.version().unwrap() {
            break;
        }
    }
    assert_eq!(full.version().unwrap(), file.version().unwrap());
    assert_eq!(state(&mut full), state(&mut file), "both replicas hold the same document");
    assert!(names(&full).contains(&"Full".to_owned()));
    let text = full.inspect().unwrap()["layers"].as_array().unwrap().iter().find(|l| l["id"] == title).unwrap()["text"]["text"].clone();
    assert_eq!(text, "From the file", "both edits survive");
}

#[test]
fn undo_after_a_save_takes_back_the_last_edit() {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    doc.execute("layer.new.layer", json!({ "name": "A" })).unwrap();
    let b = layer(&doc.execute("layer.new.layer", json!({ "name": "B" })).unwrap());
    host_save(&mut doc);
    doc.mark_saved();
    assert!(doc.inspect().unwrap()["canUndo"].as_bool().unwrap(), "the seal is no undo step and clears none");
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
    let (mut doc, _) = image_with_history();
    let bytes = host_save(&mut doc);
    doc.mark_saved();
    let stored = doc.snapshot().unwrap();
    let blobs = doc.blobs().unwrap();
    let mut opened = PhotoDocument::open_with_replica(FILE, &bytes, &stored, &blobs).unwrap();
    assert_eq!(version(&mut opened), version(&mut doc), "the stored replica, as it was");
    let history = opened.snapshot().unwrap();
    for needle in NEEDLES {
        assert!(contains(&history, needle), "the history keeps {needle}");
    }
    assert_eq!(version_of(&host_save(&mut opened)), version_of(&bytes), "and saves the document it opened, sealing nothing new");
}

#[test]
fn a_file_saved_elsewhere_opens_on_the_stored_replica_by_its_state() {
    let (mut doc, title) = image_with_history();
    let stored = doc.snapshot().unwrap();
    let blobs = doc.blobs().unwrap();
    let bytes = host_save(&mut doc);
    doc.mark_saved();
    // Elsewhere, from the file alone: an edit, a new layer, a save.
    let mut elsewhere = PhotoDocument::open(FILE, &bytes).unwrap();
    elsewhere.execute("type.edit", json!({ "layer": title, "text": "Elsewhere" })).unwrap();
    elsewhere.execute("layer.new.layer", json!({ "name": "New" })).unwrap();
    let edited = host_save(&mut elsewhere);
    let mut opened = PhotoDocument::open_with_replica(FILE, &edited, &stored, &blobs).unwrap();
    assert_eq!(opened.doc_id(), doc.doc_id());
    assert_eq!(state(&mut opened), state(&mut elsewhere), "the file's state, by object id");
    assert!(contains(&opened.snapshot().unwrap(), DELETED_LAYER), "on top of the stored history, which it keeps");
}

/// The fallback (delete with it once a kit-editor release that calls prepareSave is in
/// web/pnpm-lock.yaml): a save nobody sealed for writes the content of a sealed copy, the same
/// bytes for the same version, and `mark_saved` takes the seal into the session.
#[test]
fn a_save_nobody_sealed_for_seals_a_copy_and_mark_saved_takes_the_seal_in() {
    let (mut doc, _) = image_with_history();
    let at = version(&mut doc);
    let first = doc.save(Some(FILE)).unwrap();
    assert_eq!(version(&mut doc), at, "the session is not sealed by a save");
    assert_eq!(doc.save(Some(FILE)).unwrap(), first, "the same version seals the same way");
    let inside = held(&first);
    for needle in NEEDLES {
        assert!(!contains(&inside, needle), "the file holds {needle}");
    }
    assert_eq!(Replica::snapshot_origins(&content(&first)).unwrap(), vec![Some(Origin::system("content-seal"))]);
    doc.mark_saved();
    assert_eq!(version(&mut doc), version_of(&first), "mark_saved took the seal in");
    let mut file = PhotoDocument::open(FILE, &first).unwrap();
    doc.execute("layer.new.layer", json!({ "name": "Session" })).unwrap();
    file.execute("layer.new.layer", json!({ "name": "File" })).unwrap();
    let (to_file, to_doc) = (doc.export_changes(&file.version().unwrap()).unwrap(), file.export_changes(&doc.version().unwrap()).unwrap());
    doc.import_changes(&to_doc).unwrap();
    file.import_changes(&to_file).unwrap();
    assert_eq!(state(&mut doc), state(&mut file));
}
