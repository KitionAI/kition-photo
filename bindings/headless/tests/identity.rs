//! `document.identity` and `document.fork` (Kit Process Protocol section 5.6, KitModel M7a)
//! through the real `kition-photo` process. A `.kphoto` bundle embeds its replica (`kdm.bin` and
//! the replica's blobs under `blobs/kdm/`), and the kit opens the document from it, so a byte copy
//! of a bundle is the same document. A host reads the identity a file carries without opening it,
//! and forks a file into a new document: a new docId, the same layers, ids and pixels, and every
//! other entry of the bundle as it was. Neither file holds history (deleted content, blobs only an
//! undo step kept): a `.kphoto` holds its replica's content snapshot. A PSD or PSB embeds no
//! replica: it carries no identity and forks unchanged.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use kition_kit::KitDocument;
use kition_kit_host::{ClientError, HeadlessKit, LaunchOptions};
use kition_photo_format::zip::{ZipReader, ZipWriter};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const LIMIT: Duration = Duration::from_secs(60);

/// Name of a layer deleted before the fork: in the source session's history, in neither file.
const SECRET: &str = "SECRET-DELETED-BEFORE-THE-FORK";

fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-photo-identity-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

async fn launch(dir: &Path) -> HeadlessKit {
    let mut options = LaunchOptions::new(env!("CARGO_BIN_EXE_kition-photo"));
    options.workspace = Some(dir.to_path_buf());
    HeadlessKit::launch(options).await.unwrap()
}

fn filled(doc: &mut PhotoDocument, name: &str, color: &str, at: i32) {
    doc.execute("layer.new.layer", json!({ "name": name })).unwrap();
    doc.execute("select.rect", json!({ "x": at, "y": at, "width": 64, "height": 64 })).unwrap();
    doc.execute("edit.fill", json!({ "color": color })).unwrap();
    doc.execute("select.deselect", json!({})).unwrap();
}

/// Every entry of a bundle, by name.
fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let zip = ZipReader::new(bytes).unwrap();
    zip.entries.iter().map(|entry| (entry.name.clone(), zip.read(entry, usize::MAX).unwrap())).collect()
}

/// The replica's blobs a bundle holds (`blobs/kdm/<sha256>.zst`).
fn replica_blobs(bytes: &[u8]) -> BTreeSet<String> {
    entries(bytes).into_iter().filter(|(name, _)| name.starts_with("blobs/kdm/")).map(|(name, _)| name).collect()
}

/// A picture with a replica: two filled layers, then a third (named [`SECRET`]) filled and
/// deleted, saved as a `.kphoto`, with an entry another program wrote.
fn picture() -> Vec<u8> {
    let mut doc = PhotoDocument::create("Plan.kphoto", 256, 256).unwrap();
    doc.version().unwrap();
    filled(&mut doc, "Kept", "#d02020", 0);
    filled(&mut doc, "Also kept", "#2020d0", 96);
    filled(&mut doc, SECRET, "#20d020", 160);
    doc.execute("layer.delete", json!({})).unwrap();
    assert!(holds(&doc.snapshot().unwrap(), SECRET), "the control: the session's history holds the deleted layer");
    doc.prepare_save().unwrap();
    let saved = doc.save(Some("Plan.kphoto")).unwrap();
    assert!(kition_photo_format::read_kdm(&saved).unwrap().is_some(), "the bundle embeds the replica");
    let mut out = ZipWriter::new();
    for (name, data) in entries(&saved) {
        out.add(&name, &data).unwrap();
    }
    out.add("notes/kept.txt", b"an entry another program wrote").unwrap();
    out.finish().unwrap()
}

/// Every object of the picture with its fields, by id: what must be equal in a fork.
async fn objects(kit: &HeadlessKit, path: &str) -> Value {
    let state = kit.open(path, false, LIMIT).await.unwrap();
    let document = state["documentId"].clone();
    let batch = |request: Value| {
        let mut request = request;
        request["documentId"] = document.clone();
        kit.request("model.batch", request, LIMIT)
    };
    let inventory = batch(json!({ "ops": [], "objects": true })).await.unwrap();
    let ids: Vec<Value> = inventory["objects"].as_array().unwrap().iter().map(|object| object["id"].clone()).collect();
    assert!(ids.len() >= 3, "the image and two layers: {inventory}");
    let load: Vec<Value> = ids.iter().map(|id| json!({ "target": id })).collect();
    let loaded = batch(json!({ "ops": [], "load": load })).await.unwrap();
    kit.request("document.close", json!({ "documentId": document }), LIMIT).await.unwrap();
    json!({ "objects": inventory["objects"], "loaded": loaded["loaded"] })
}

fn holds(bytes: &[u8], needle: &str) -> bool {
    bytes.windows(needle.len()).any(|window| window == needle.as_bytes())
}

/// What the kit said about a refusal (`error.data.detail`).
fn detail(error: &ClientError) -> String {
    error.data().and_then(|data| data["detail"].as_str()).unwrap_or_default().to_owned()
}

#[tokio::test]
async fn a_kphoto_tells_its_identity_and_forks_into_a_new_document_without_history() {
    let dir = workspace("embedded");
    std::fs::write(dir.join("Plan.kphoto"), picture()).unwrap();
    let kit = launch(&dir).await;
    assert!(kit.serves("document.identity"), "{}", kit.capabilities());

    // The identity the file carries, read without opening it, and the same for a byte copy.
    let source_id = kit.identity("Plan.kphoto", LIMIT).await.unwrap().doc_id.expect("a .kphoto with a replica has an identity");
    std::fs::copy(dir.join("Plan.kphoto"), dir.join("Copy.kphoto")).unwrap();
    assert_eq!(kit.identity("Copy.kphoto", LIMIT).await.unwrap().doc_id.as_deref(), Some(source_id.as_str()), "a byte copy is the same document");

    // A fork is a new document with the same objects, ids and values.
    let fork = kit.fork("Plan.kphoto", "Fork.kphoto", LIMIT).await.unwrap();
    let fork_id = fork.doc_id.clone().expect("the fork's identity");
    assert_ne!(fork_id, source_id);
    assert_eq!(fork.name, "Fork.kphoto");
    assert!(!fork.version.is_empty());
    assert_eq!(kit.identity("Fork.kphoto", LIMIT).await.unwrap().doc_id.as_deref(), Some(fork_id.as_str()), "the forked file carries the new id");
    assert_eq!(kit.identity("Plan.kphoto", LIMIT).await.unwrap().doc_id.as_deref(), Some(source_id.as_str()), "the source is unchanged");
    assert_eq!(objects(&kit, "Fork.kphoto").await, objects(&kit, "Plan.kphoto").await, "the same objects, ids and values");

    // The fork's replica travels as a whole (an agent's draft of it: `model.export`, then
    // `model.open`), though it starts at the fork and has no earlier changes.
    let state = kit.open("Fork.kphoto", true, LIMIT).await.unwrap();
    let exported = kit.request("model.export", json!({ "documentId": state["documentId"] }), LIMIT).await.unwrap();
    let mut open = json!({ "name": "Fork.kphoto", "snapshot": exported["changes"] });
    if let Some(blobs) = exported.get("blobs") {
        open["blobs"] = blobs.clone();
    }
    let draft = kit.request("model.open", open, LIMIT).await.unwrap();
    assert_ne!(draft["documentId"], state["documentId"], "{draft}");
    kit.request("document.close", json!({ "documentId": state["documentId"] }), LIMIT).await.unwrap();

    // The fork's file: the replica's blobs its state references and no others; every other entry
    // (manifest, tiles, previews, the unknown entry) as it was.
    let (source_bytes, fork_bytes) = (std::fs::read(dir.join("Plan.kphoto")).unwrap(), std::fs::read(dir.join("Fork.kphoto")).unwrap());
    let (source_blobs, fork_blobs) = (replica_blobs(&source_bytes), replica_blobs(&fork_bytes));
    assert!(fork_blobs.is_subset(&source_blobs), "the fork's blobs are the source's state's: {source_blobs:?} {fork_blobs:?}");
    let others = |bytes: &[u8]| -> Vec<(String, Vec<u8>)> {
        entries(bytes).into_iter().filter(|(name, _)| name != "kdm.bin" && !name.starts_with("blobs/kdm/")).collect()
    };
    assert_eq!(others(&fork_bytes), others(&source_bytes), "everything but the replica is kept");

    // The fork edits, saves and opens again as any picture, under its own id.
    let mut opened = PhotoDocument::open("Fork.kphoto", &fork_bytes).unwrap();
    filled(&mut opened, "New", "#808080", 32);
    std::fs::write(dir.join("Edited.kphoto"), opened.save(Some("Edited.kphoto")).unwrap()).unwrap();
    assert_eq!(kit.identity("Edited.kphoto", LIMIT).await.unwrap().doc_id.as_deref(), Some(fork_id.as_str()), "an edit of the fork is the fork");

    // No history: the deleted layer's name is in neither file (a `.kphoto` holds content only).
    assert!(!holds(&source_bytes, SECRET), "the source holds its content, not its history");
    assert!(!holds(&fork_bytes, SECRET), "deleted content does not travel");

    // A fork of a fork is a third document with the same content.
    let again = kit.fork("Fork.kphoto", "Again.kphoto", LIMIT).await.unwrap().doc_id.expect("the second fork's identity");
    assert!(again != source_id && again != fork_id, "{again}");
    assert_eq!(objects(&kit, "Again.kphoto").await, objects(&kit, "Plan.kphoto").await);

    // A fork never replaces a file.
    let taken = kit.fork("Plan.kphoto", "Fork.kphoto", LIMIT).await.unwrap_err();
    assert_eq!(taken.reason(), Some("file_conflict"), "{taken}");
    assert_eq!(std::fs::read(dir.join("Fork.kphoto")).unwrap(), fork_bytes, "the first fork is kept");
    kit.shutdown(Duration::from_secs(5)).await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn files_without_a_replica_have_no_identity_and_bytes_of_no_picture_are_refused() {
    let dir = workspace("plain");
    let mut doc = PhotoDocument::create("Plain.psd", 128, 128).unwrap();
    doc.version().unwrap();
    filled(&mut doc, "Ink", "#d02020", 0);
    let psd = doc.save(Some("Plain.psd")).unwrap();
    // A bundle written without a replica, as another tool (or an export) writes one.
    let picture = kition_photo_io::import("Plain.psd", &psd).unwrap().document;
    let bundle = kition_photo_io::export(&picture, "Plain.kphoto", &kition_photo_io::ExportOptions::default()).unwrap().bytes;
    assert!(kition_photo_format::read_kdm(&bundle).unwrap().is_none());
    std::fs::write(dir.join("Plain.psd"), &psd).unwrap();
    std::fs::write(dir.join("Plain.kphoto"), &bundle).unwrap();
    // An empty file (one made for the kit before it saved a picture there).
    std::fs::write(dir.join("New.psd"), b"").unwrap();
    let kit = launch(&dir).await;

    for (name, bytes) in [("Plain.psd", psd.as_slice()), ("Plain.kphoto", bundle.as_slice()), ("New.psd", b"".as_slice())] {
        assert_eq!(kit.identity(name, LIMIT).await.unwrap().doc_id, None, "{name} holds no replica");
        let target = format!("Copy of {name}");
        let fork = kit.fork(name, &target, LIMIT).await.unwrap();
        assert_eq!(fork.doc_id, None, "{name}");
        assert_eq!(std::fs::read(dir.join(&target)).unwrap(), bytes, "{name}: a file without a replica forks unchanged");
    }

    // Bytes that are no picture of this kit are refused, never answered with no identity.
    std::fs::write(dir.join("Text.kphoto"), b"words a person wrote elsewhere").unwrap();
    for refused in [kit.identity("Text.kphoto", LIMIT).await.unwrap_err(), kit.fork("Text.kphoto", "TextCopy.kphoto", LIMIT).await.unwrap_err()] {
        assert_eq!(refused.reason(), Some("unsupported_format"), "{refused}: {}", detail(&refused));
    }
    assert!(!dir.join("TextCopy.kphoto").exists());
    kit.shutdown(Duration::from_secs(5)).await;
    let _ = std::fs::remove_dir_all(dir);
}
