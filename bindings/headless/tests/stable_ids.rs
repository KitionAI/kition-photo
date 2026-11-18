//! What `photo_inspect`, the commands and the diagnostics call a layer stays its address (KHP A46,
//! A53, A57): after the rollback with which the kits server undoes a failed run (it closes the
//! document and opens its file again), after a replica-only open (`model.open`) and after
//! `model.export` opened elsewhere, inspect answers the same ids; and with several documents open
//! in one kit, what one holds never changes the ids of another.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit::KitDocument;
use kition_kit_server::protocol::Origin;
use kition_kit_server::{KitBackend, OpenRequest};
use kition_photo_kit::PhotoDocument;
use serde_json::{Map, Value, json};

const MANIFEST: &str = include_str!("../../../kit.json");

fn kit() -> Headless<PhotoDocument> {
    kition_photo_kit::gate_commands(kition_photo_automation::workspace::authorize_engine_step);
    Headless::with_replica_port(KitInfo::from_manifest(MANIFEST, kition_photo_kit::SANDBOXED, kition_photo_kit::EDITOR_STATE).unwrap())
}

fn backend(root: &Path) -> (Headless<PhotoDocument>, HostFolder) {
    let mut backend = kit();
    let folder = HostFolder::new(root).unwrap();
    backend.set_host(folder.host());
    (backend, folder)
}

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-photo-ids-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn params(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// The layers as `photo_inspect` lists them: `(name, id)`, top to bottom, groups before their
/// contents.
fn layer_ids(state: &Map<String, Value>) -> Vec<(String, u64)> {
    fn walk(layers: &Value, out: &mut Vec<(String, u64)>) {
        for layer in layers.as_array().into_iter().flatten() {
            out.push((layer["name"].as_str().unwrap().to_owned(), layer["id"].as_u64().unwrap()));
            walk(&layer["children"], out);
        }
    }
    let mut out = Vec::new();
    walk(&state["layers"], &mut out);
    out
}

/// A layered image made with the kit's own commands, as PSD bytes.
fn layered_psd() -> Vec<u8> {
    let mut doc = PhotoDocument::create("art.psd", 64, 48).unwrap();
    for (command, params) in [
        ("layer.new.layer", json!({ "name": "Ink" })),
        ("paint.stroke", json!({ "points": [[4, 4, 1], [40, 30, 1]], "size": 6, "color": "#ff0000" })),
        ("layer.new.group", json!({ "name": "Group" })),
        ("layer.new.layer", json!({ "name": "Inside" })),
        ("layer.newAdjustmentLayer.levels", json!({ "gamma": 1.4 })),
        ("type.create", json!({ "text": "Hi", "x": 4, "y": 30, "size": 12, "color": "#000000", "name": "Title" })),
    ] {
        doc.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"));
    }
    doc.save(Some("art.psd")).unwrap()
}

/// The kits server's rollback of a failed run: it closes the document and opens its file again.
#[test]
fn a_failed_run_rolled_back_leaves_inspect_answering_the_same_ids() {
    let root = temp_root("rollback");
    std::fs::write(root.join("art.psd"), layered_psd()).unwrap();
    let (mut b, folder) = backend(&root);
    let opened = b.open(OpenRequest { file: &folder.file("art.psd").unwrap(), read_only: false, create: None }).unwrap();
    let before = b.describe(&opened.id).unwrap().state;
    let ids = layer_ids(&before);
    let ink = ids.iter().find(|(name, _)| name == "Ink").unwrap().1;
    b.execute(&opened.id, "layer.new.layer", params(json!({ "name": "Draft" })), Origin::Agent).unwrap();
    b.execute(&opened.id, "layer.setProps", params(json!({ "layer": ink, "opacity": 0.5 })), Origin::Agent).unwrap();
    assert!(b.execute(&opened.id, "layer.setProps", params(json!({ "layer": 987_654, "opacity": 0.5 })), Origin::Agent).is_err());
    b.close(&opened.id, true).unwrap();
    let reopened = b.open(OpenRequest { file: &folder.file("art.psd").unwrap(), read_only: false, create: None }).unwrap();
    let after = b.describe(&reopened.id).unwrap().state;
    assert_eq!(layer_ids(&after), ids, "inspect names every layer as before the run");
    assert_eq!(after, before, "and answers as before the run");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ids_of_one_open_document_do_not_depend_on_another() {
    let root = temp_root("two");
    let bytes = layered_psd();
    std::fs::write(root.join("a.psd"), &bytes).unwrap();
    std::fs::write(root.join("b.psd"), &bytes).unwrap();
    let (mut b, folder) = backend(&root);
    let alone = b.open(OpenRequest { file: &folder.file("a.psd").unwrap(), read_only: false, create: None }).unwrap();
    let want = layer_ids(&b.describe(&alone.id).unwrap().state);
    b.execute(&alone.id, "layer.new.layer", params(json!({ "name": "More" })), Origin::Agent).unwrap();
    let second = b.open(OpenRequest { file: &folder.file("b.psd").unwrap(), read_only: false, create: None }).unwrap();
    assert_eq!(layer_ids(&b.describe(&second.id).unwrap().state), want, "the second document's ids are its file's");
    // A layer made in each takes the next id of its own document.
    b.execute(&alone.id, "layer.new.layer", params(json!({ "name": "Next" })), Origin::Agent).unwrap();
    b.execute(&second.id, "layer.new.layer", params(json!({ "name": "Next" })), Origin::Agent).unwrap();
    let next = |state: &Map<String, Value>| layer_ids(state).into_iter().find(|(name, _)| name == "Next").unwrap().1;
    let in_b = next(&b.describe(&second.id).unwrap().state);
    let in_a = next(&b.describe(&alone.id).unwrap().state);
    assert_eq!(in_b, want.iter().map(|(_, id)| id).max().unwrap() + 1, "b's next id follows b's own layers");
    assert_eq!(in_a, in_b + 1, "a made one more layer before");
    let _ = std::fs::remove_dir_all(root);
}

mod base64 {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
                out.push(if i <= chunk.len() { char::from(ALPHABET[((n >> shift) & 63) as usize]) } else { '=' });
            }
        }
        out
    }
}

/// A sync room's state, as `model.open` takes it.
fn room(name: &str, snapshot: &[u8], blobs: &[u8]) -> Value {
    json!({ "name": name, "snapshot": base64::encode(snapshot), "blobs": base64::encode(blobs) })
}

#[test]
fn a_replica_only_open_and_an_exported_replica_answer_the_same_ids() {
    let mut peer = PhotoDocument::open("art.psd", &layered_psd()).unwrap();
    peer.execute("layer.new.layer", json!({ "name": "Peer" })).unwrap();
    let want = layer_ids(peer.inspect().unwrap().as_object().unwrap());
    let mut first = kit();
    let opened = first.model("model.open", room("art.psd", &peer.snapshot().unwrap(), &peer.blobs().unwrap())).unwrap();
    let id = opened["documentId"].as_str().unwrap().to_owned();
    assert_eq!(layer_ids(&first.describe(&id).unwrap().state), want, "model.open of the peer's replica");

    // What the kit exports opens elsewhere with the same ids.
    let exported = first.model("model.export", json!({ "documentId": id })).unwrap();
    let mut other = kit();
    let state = json!({ "name": "art.psd", "snapshot": exported["changes"], "blobs": exported.get("blobs").cloned().unwrap_or(json!("")) });
    let again = other.model("model.open", state).unwrap()["documentId"].as_str().unwrap().to_owned();
    assert_eq!(layer_ids(&other.describe(&again).unwrap().state), want, "model.export, then model.open");
}
