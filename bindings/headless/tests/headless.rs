//! `kition-photo` as `kition-headless` serves it (`run_manifest_model`, with the automation engine's
//! gate): documents in the workspace, the agent's view of the commands, previews, the object model
//! over the change exchange, and a sync room's replica methods with the blobs their changes need.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit::KitDocument;
use kition_kit_server::protocol::{Origin, ScreenshotScope};
use kition_kit_server::{DocumentInfo, ErrorReason, KitBackend, OpenRequest, RpcError, ScreenshotRequest};
use kition_photo_kit::PhotoDocument;
use serde_json::{Map, Value, json};

const MANIFEST: &str = include_str!("../../../kit.json");

fn kit() -> Headless<PhotoDocument> {
    kition_photo_kit::gate_commands(kition_photo_automation::workspace::authorize_engine_step);
    Headless::with_replica_port(KitInfo::from_manifest(MANIFEST, kition_photo_kit::SANDBOXED, kition_photo_kit::EDITOR_STATE).unwrap())
}

/// A kit served the files under `root` through an in-process host's `files.*`.
fn backend(root: &Path) -> (Headless<PhotoDocument>, HostFolder) {
    let mut backend = kit();
    let folder = HostFolder::new(root).unwrap();
    backend.set_host(folder.host());
    (backend, folder)
}

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kition-photo-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn create(backend: &mut Headless<PhotoDocument>, folder: &HostFolder, path: &str) -> DocumentInfo {
    let options = json!({ "width": 64, "height": 48 });
    backend.open(OpenRequest { file: &folder.file(path).unwrap(), read_only: false, create: Some(("photo", options.as_object())) }).unwrap()
}

fn params(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn layer_names(b: &mut Headless<PhotoDocument>, id: &str) -> Vec<Value> {
    b.describe(id).unwrap().state["layers"].as_array().unwrap().iter().map(|l| l["name"].clone()).collect()
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

    pub fn decode(text: &str) -> Vec<u8> {
        let (mut out, mut buffer, mut bits) = (Vec::new(), 0u32, 0u32);
        for byte in text.trim_end_matches('=').bytes() {
            buffer = (buffer << 6) | ALPHABET.iter().position(|known| *known == byte).expect("base64") as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push(((buffer >> bits) & 0xff) as u8);
            }
        }
        out
    }
}

#[test]
fn creates_edits_saves_and_reopens_a_psd() {
    let root = temp_root("roundtrip");
    let path = root.join("art.psd");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "art.psd");
    assert_eq!(created.format, "psd");

    let executed = b.execute(&created.id, "layer.new.layer", params(json!({ "name": "Ink" })), Origin::Agent).unwrap();
    assert!(executed.changed);
    assert!(b.document(&created.id).unwrap().dirty);
    let described = b.describe(&created.id).unwrap();
    assert!(layer_names(&mut b, &created.id).contains(&json!("Ink")), "{:?}", described.state);
    assert!(described.summary.contains("64 x 48 px"), "{}", described.summary);

    let (saved, written) = b.save(&created.id, None, None).unwrap();
    assert!(!saved.dirty);
    assert_eq!(written.name, "art.psd");
    assert!(path.exists());
    b.close(&created.id, false).unwrap();

    let reopened = b.open(OpenRequest { file: &folder.file("art.psd").unwrap(), read_only: false, create: None }).unwrap();
    assert!(!reopened.dirty);
    // Layer ids number a session's layers; everything else survives saving and reopening.
    let lasting = |state: &Map<String, Value>| {
        let mut state = state.clone();
        for layer in state.get_mut("layers").and_then(Value::as_array_mut).into_iter().flatten() {
            layer.as_object_mut().map(|layer| layer.remove("id"));
        }
        state
    };
    assert_eq!(lasting(&b.describe(&reopened.id).unwrap().state), lasting(&described.state), "what survives saving and reopening");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn files_outside_the_root_are_not_granted() {
    let root = temp_root("grant");
    let (mut b, folder) = backend(&root);
    assert!(folder.file("../x.psd").is_err(), "the host grants nothing outside its folder");
    let made_up = "fh_00000000000000000000000000000000";
    assert_eq!(b.open(OpenRequest { file: made_up, read_only: false, create: None }).unwrap_err().reason(), Some(ErrorReason::FileNotGranted));
    let mut none = kit();
    assert!(none.open(OpenRequest { file: &folder.file("a.psd").unwrap(), read_only: false, create: None }).is_err(), "no host, no files");
}

#[test]
fn dirty_documents_need_discard_to_close() {
    let root = temp_root("dirty");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "new.psd");
    assert_eq!(b.close(&created.id, false).unwrap_err().reason(), Some(ErrorReason::DocumentDirty));
    b.close(&created.id, true).unwrap();
    assert_eq!(b.document(&created.id).unwrap_err().reason(), Some(ErrorReason::DocumentNotFound));
}

#[test]
fn layered_formats_only_and_the_document_keeps_its_file() {
    let root = temp_root("save");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "art.psd");
    b.save(&created.id, None, None).unwrap();
    assert!(b.save(&created.id, Some(&folder.file("flat.png").unwrap()), None).is_err(), "a flat image is no save target");
    assert!(!root.join("flat.png").exists());
    assert_eq!(b.document(&created.id).unwrap().name, "art.psd");
}

/// The agent sees schemas for every command, never a command the gate always refuses (they reach
/// the file system on their own) or one whose parameters are typed files; a refused call names why.
#[test]
fn the_agent_sees_schemas_and_no_file_commands() {
    let root = temp_root("listed");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "a.psd");
    let commands = b.commands(Some(&created.id)).unwrap();
    assert!(commands.len() > 300, "{}", commands.len());
    assert!(commands.iter().any(|c| c.id == "layer.new.layer" && c.mutates));
    let without: Vec<&str> = commands.iter().filter(|c| c.params["type"] != "object").map(|c| c.id.as_str()).collect();
    assert!(without.is_empty(), "every command has a parameter schema: {without:?}");
    for hidden in ["file.open", "file.saveACopy", "file.placeEmbedded", "plugin.reload", "pattern.import"] {
        assert!(!commands.iter().any(|c| c.id == hidden), "{hidden} is not offered");
    }
    let error = b.execute(&created.id, "file.open", Map::new(), Origin::Agent).unwrap_err();
    assert_eq!(error.reason(), Some(ErrorReason::CommandNotFound));
    // A command the gate refuses only for some parameters runs with others and is refused with them.
    let refused = b.execute(&created.id, "edit.assignProfile", params(json!({ "profile": "/etc/x.icc" })), Origin::Agent).unwrap_err();
    assert!(!refused.message.is_empty());
    b.execute(&created.id, "edit.assignProfile", params(json!({ "profile": "srgb" })), Origin::Agent).unwrap();
}

#[test]
fn screenshots_are_bounded_pngs() {
    let root = temp_root("shot");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "a.psd");
    let shot = b.screenshot(&created.id, &ScreenshotRequest { scope: ScreenshotScope::Canvas, max_size: 32, scale: 1.0, target: None }).unwrap();
    assert_eq!(&shot.png[1..4], b"PNG");
    assert!(shot.width.max(shot.height) <= 32, "{}x{}", shot.width, shot.height);
}

/// One kit, one identity: the binary and the KHP manifest (`kit.json`) agree on id and version.
/// The kit's version is its own. Upstream is credited only in NOTICE, so the manifest has no
/// `upstream`.
#[test]
fn kit_json_matches_the_binary() {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let b = kit();
    assert_eq!(manifest["id"], b.kit_id());
    assert_eq!(manifest["version"], b.kit_version());
    assert!(manifest.get("upstream").is_none(), "kit.json must not name upstream (it is credited only in NOTICE)");
}

#[test]
fn serves_the_object_model_over_the_change_exchange() {
    let root = temp_root("model");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "model.psd");
    let described = b.model("model.describe", json!({ "documentId": created.id })).unwrap();
    assert_eq!(described["capability"], "KitObjectModel 1.0");
    assert_eq!(described["root"], "Image");
    assert!(described["commands"].as_array().unwrap().iter().any(|command| command["id"] == "layer.new.layer"));

    let batch = |b: &mut Headless<PhotoDocument>, request: Value| {
        let mut request = request;
        request["documentId"] = json!(created.id);
        b.model("model.batch", request)
    };
    let added = batch(
        &mut b,
        json!({ "ops": [{ "op": "command", "id": "layer.new.layer", "params": { "name": "Ink" } }], "load": [{ "target": "root", "fields": ["layers"] }] }),
    )
    .unwrap();
    let layers = added["loaded"][0]["fields"]["layers"].as_array().unwrap().clone();
    assert!(layer_names(&mut b, &created.id).contains(&json!("Ink")));
    let ink = layers
        .iter()
        .map(|entry| entry["object"].as_str().unwrap().to_owned())
        .find(|layer| {
            let loaded = batch(&mut b, json!({ "ops": [], "load": [{ "target": layer, "fields": ["name"] }] })).unwrap();
            loaded["loaded"][0]["fields"]["name"] == "Ink"
        })
        .unwrap();
    batch(&mut b, json!({ "ops": [{ "op": "set", "target": ink, "field": "name", "value": "Scripted" }] })).unwrap();
    assert!(layer_names(&mut b, &created.id).contains(&json!("Scripted")));
    b.save(&created.id, None, None).unwrap();

    let (mut viewer, viewed) = backend(&root);
    let opened = viewer.open(OpenRequest { file: &viewed.read_only("model.psd").unwrap(), read_only: true, create: None }).unwrap();
    let refused =
        viewer.model("model.batch", json!({ "documentId": opened.id, "ops": [{ "op": "set", "target": ink, "field": "name", "value": "No" }] })).unwrap_err();
    assert_eq!(refused.reason(), Some(ErrorReason::CommandDisabled));
    let _ = std::fs::remove_dir_all(root);
}

/// A batch the image refuses changes nothing (KHP A16, A18; KitObjectModel OM4): the refused value
/// is neither read back nor saved, and the next batch runs. Before, the replica kept a value its
/// view could not read (`root.depth = "khp check"`), so a query read it back and every later batch
/// failed on it.
#[test]
fn a_refused_batch_leaves_the_image_as_it_was() {
    let root = temp_root("refused-batch");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "art.kphoto");
    b.save(&created.id, None, None).unwrap();
    let batch = |b: &mut Headless<PhotoDocument>, id: &str, ops: Value| b.model("model.batch", json!({ "documentId": id, "ops": ops }));
    let field = |b: &mut Headless<PhotoDocument>, id: &str, name: &str| {
        let loaded = b.model("model.batch", json!({ "documentId": id, "ops": [], "load": [{ "target": "root", "fields": [name] }] })).unwrap();
        loaded["loaded"][0]["fields"][name].clone()
    };
    let (depth, guides) = (field(&mut b, &created.id, "depth"), field(&mut b, &created.id, "guides"));
    let state = b.describe(&created.id).unwrap().state;

    for (name, value) in [("depth", json!("khp check")), ("guides", json!("khp check"))] {
        let refused = batch(&mut b, &created.id, json!([{ "op": "set", "target": "root", "field": name, "value": value }])).unwrap_err();
        assert!(!refused.message.is_empty(), "{name}: {refused:?}");
        assert_eq!(field(&mut b, &created.id, "depth"), depth, "after the refused {name}");
        assert_eq!(field(&mut b, &created.id, "guides"), guides, "after the refused {name}");
        assert_eq!(b.describe(&created.id).unwrap().state, state, "after the refused {name}");
    }

    // The next batch runs, and only it reaches the file.
    batch(&mut b, &created.id, json!([{ "op": "set", "target": "root", "field": "dpi", "value": 144.0 }])).unwrap();
    assert_eq!(field(&mut b, &created.id, "dpi").as_f64(), Some(144.0));
    b.save(&created.id, None, None).unwrap();
    b.close(&created.id, false).unwrap();
    let reopened = b.open(OpenRequest { file: &folder.file("art.kphoto").unwrap(), read_only: false, create: None }).unwrap();
    assert_eq!((field(&mut b, &reopened.id, "depth"), field(&mut b, &reopened.id, "dpi").as_f64()), (depth, Some(144.0)));
    assert_eq!(field(&mut b, &reopened.id, "guides"), guides);
    let _ = std::fs::remove_dir_all(root);
}

/// A field that takes fixed values says which (KHP A48): `model.describe` lists the values of the
/// image's mode and depth and of a layer's blend mode, and a batch that sets another value is
/// refused at that operation, naming the field, before the image sees it.
#[test]
fn the_object_model_lists_the_values_a_field_takes() {
    let root = temp_root("enums");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "art.psd");
    let described = b.model("model.describe", json!({ "documentId": created.id })).unwrap();
    let field = |ty: &str, name: &str| described["types"][ty]["fields"][name].clone();
    assert_eq!(field("Image", "depth"), json!({ "kind": "enum", "values": ["U8", "U16", "F32"] }));
    assert_eq!(field("Image", "mode")["values"], json!(["Bitmap", "Grayscale", "Indexed", "Rgb", "Cmyk", "Lab", "Multichannel", "Duotone"]));
    let blends = field("Layer", "blend")["values"].clone();
    assert!(["PassThrough", "Normal", "Multiply", "Luminosity"].iter().all(|mode| blends.as_array().unwrap().contains(&json!(mode))), "{blends}");

    let refused = b
        .model("model.batch", json!({ "documentId": created.id, "ops": [{ "op": "set", "target": "root", "field": "depth", "value": "khp check" }] }))
        .unwrap_err();
    assert_eq!(refused.code, ErrorReason::InvalidParams.code(), "{refused:?}");
    let refusal = refused.data.clone().unwrap_or_default()["model"].clone();
    assert_eq!((refusal["code"].as_str(), refusal["index"].as_u64()), (Some("invalid-value"), Some(0)), "{refused:?}");
    assert!(refusal["message"].as_str().unwrap_or_default().contains("`depth` of `Image`"), "{refused:?}");
    // A value the field lists is taken.
    let loaded = b.model("model.batch", json!({ "documentId": created.id, "ops": [], "load": [{ "target": "root", "fields": ["layers"] }] })).unwrap();
    let layer = loaded["loaded"][0]["fields"]["layers"][0]["object"].clone();
    b.model("model.batch", json!({ "documentId": created.id, "ops": [{ "op": "set", "target": layer, "field": "blend", "value": "Multiply" }] })).unwrap();
    let state = b.describe(&created.id).unwrap().state;
    assert!(state["layers"].to_string().contains("Multiply"), "{state:?}");
    let _ = std::fs::remove_dir_all(root);
}

/// What `describe` reports as the image is what its file holds (KHP A53): the selection is the
/// person's own editing state, like the quick mask, never written to the file or the shared
/// replica, so it is not reported as content. A failed run's rollback reopens a clean document from
/// its file; before, `hasSelection` (and `selectionBounds`) then answered differently, so the
/// rollback looked incomplete. Layer ids are still renumbered on reopening, so they are left out.
#[test]
fn the_image_as_described_is_what_its_file_holds() {
    let root = temp_root("lasting");
    std::fs::copy(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/basic.psd")), root.join("basic.psd")).unwrap();
    let (mut b, folder) = backend(&root);
    let opened = b.open(OpenRequest { file: &folder.file("basic.psd").unwrap(), read_only: false, create: None }).unwrap();
    let edit: Value = serde_json::from_str(include_str!("../../../conformance/edit.json")).unwrap();
    for step in edit["steps"].as_array().unwrap() {
        b.execute(&opened.id, step["command"].as_str().unwrap(), params(step["params"].clone()), Origin::Agent).unwrap();
    }
    b.save(&opened.id, None, None).unwrap();
    let without_ids = |state: Map<String, Value>| {
        let mut state = Value::Object(state);
        for layer in state["layers"].as_array_mut().into_iter().flatten() {
            layer.as_object_mut().map(|layer| layer.remove("id"));
        }
        state
    };
    let edited = without_ids(b.describe(&opened.id).unwrap().state);
    b.close(&opened.id, false).unwrap();
    let reopened = b.open(OpenRequest { file: &folder.file("basic.psd").unwrap(), read_only: false, create: None }).unwrap();
    assert_eq!(without_ids(b.describe(&reopened.id).unwrap().state), edited);
    let _ = std::fs::remove_dir_all(root);
}

/// The room's image, as a collaborator holds it.
fn peer() -> PhotoDocument {
    let mut peer = PhotoDocument::create("art.psd", 64, 48).unwrap();
    peer.execute("layer.new.layer", json!({ "name": "Base" })).unwrap();
    peer
}

/// A sync room's state, as `model.open` takes it.
fn room(peer: &mut PhotoDocument) -> Value {
    json!({ "name": "art.psd", "snapshot": base64::encode(&peer.snapshot().unwrap()), "blobs": base64::encode(&peer.blobs().unwrap()) })
}

/// The composite colour at (`x`, `y`).
fn color_at(doc: &PhotoDocument, x: f64, y: f64) -> Vec<u8> {
    doc.render(x, y, 1.0, 1, 1).unwrap()
}

/// The colour at (`x`, `y`) of the kit's document, as its saved bytes show it.
fn kit_color(kit: &mut Headless<PhotoDocument>, id: &str, x: f64, y: f64) -> Vec<u8> {
    let saved = kit.model("model.save", json!({ "documentId": id })).unwrap();
    color_at(&PhotoDocument::open("art.psd", &base64::decode(saved["data"].as_str().unwrap())).unwrap(), x, y)
}

/// A server's headless kit pool opens a sync room's state with no file and no workspace:
/// `model.open`, `model.import`, `model.export`, `model.save`.
#[test]
fn opens_a_room_without_a_file_and_imports_exports_and_saves_it() {
    let mut peer = peer();
    let mut kit = kit();
    let features = kit.capabilities()["features"].clone();
    for feature in kition_headless::REPLICA_FEATURES {
        assert!(features.as_array().unwrap().iter().any(|listed| listed == feature), "{feature} in {features}");
    }
    let opened = kit.model("model.open", room(&mut peer)).unwrap();
    let id = opened["documentId"].as_str().unwrap().to_owned();
    assert_eq!((opened["format"].as_str(), opened["title"].as_str()), (Some("psd"), Some("art.psd")));
    assert!(layer_names(&mut kit, &id).contains(&json!("Base")));
    assert!(kit.check(&id).unwrap()["diagnostics"].is_array());
    assert_eq!(kit.model("model.open", json!({ "name": "../art.psd", "snapshot": "" })).unwrap_err().code, RpcError::INVALID_PARAMS);
    assert_eq!(kit.model("model.open", json!({ "name": "art.txt", "snapshot": "" })).unwrap_err().reason(), Some(ErrorReason::UnsupportedFormat));

    // A collaborator's change comes in.
    let since = peer.version().unwrap();
    peer.execute("layer.new.layer", json!({ "name": "Theirs" })).unwrap();
    let change = peer.export_changes(&since).unwrap();
    kit.model("model.import", json!({ "documentId": id, "changes": base64::encode(&change) })).unwrap();
    assert!(layer_names(&mut kit, &id).contains(&json!("Theirs")));

    // The kit edits on top of it, and the collaborator takes what it exports.
    kit.model("model.batch", json!({ "documentId": id, "ops": [{ "op": "command", "id": "layer.new.layer", "params": { "name": "Kit" } }] })).unwrap();
    let since = peer.version().unwrap();
    let exported = kit.model("model.export", json!({ "documentId": id, "since": base64::encode(&since) })).unwrap();
    peer.import_changes(&base64::decode(exported["changes"].as_str().unwrap())).unwrap();
    assert!(peer.inspect().unwrap()["layers"].to_string().contains("\"Kit\""));

    // Saved as bytes in its own format and in the other layered one, never as a file.
    let saved = kit.model("model.save", json!({ "documentId": id })).unwrap();
    assert_eq!((saved["name"].as_str(), saved["format"].as_str()), (Some("art.psd"), Some("psd")));
    let reopened = PhotoDocument::open("art.psd", &base64::decode(saved["data"].as_str().unwrap())).unwrap();
    let names = reopened.inspect().unwrap()["layers"].to_string();
    assert!(["Base", "Theirs", "Kit"].iter().all(|name| names.contains(name)), "{names}");
    assert_eq!(kit.model("model.save", json!({ "documentId": id, "format": "photo" })).unwrap()["name"], "art.kphoto");
    assert_eq!(kit.model("model.save", json!({ "documentId": id, "format": "png" })).unwrap_err().reason(), Some(ErrorReason::UnsupportedFormat));
}

/// Painted pixels are blobs beside the changes that reference them: `model.import` takes a
/// collaborator's (`blobs`) before its changes, and `model.export` answers the ones its changes
/// need, so each side shows the other's strokes.
#[test]
fn imports_and_exports_the_blobs_their_changes_need() {
    let mut peer = peer();
    let mut kit = kit();
    let state = room(&mut peer);
    let id = kit.model("model.open", state.clone()).unwrap()["documentId"].as_str().unwrap().to_owned();
    let blank = kit_color(&mut kit, &id, 30.0, 14.0);

    // The collaborator paints: its change references tiles the kit does not have.
    let since = peer.version().unwrap();
    peer.execute("paint.stroke", json!({ "points": [[8, 14], [56, 14]], "size": 8, "color": "#ff0000" })).unwrap();
    let change = base64::encode(&peer.export_changes(&since).unwrap());
    let needed = peer.blobs_since(&since).unwrap();
    assert!(!needed.is_empty(), "the stroke's tiles are blobs");
    let painted = color_at(&peer, 30.0, 14.0);
    assert_ne!(painted, blank);

    // Damaged blobs are refused before anything is imported, and the document stays open.
    let error = kit.model("model.import", json!({ "documentId": id, "changes": change, "blobs": base64::encode(&[1, 2, 3]) })).unwrap_err();
    assert_eq!(error.code, RpcError::INVALID_PARAMS, "{error:?}");
    assert_eq!(kit_color(&mut kit, &id, 30.0, 14.0), blank);

    kit.model("model.import", json!({ "documentId": id, "changes": change, "blobs": base64::encode(&needed) })).unwrap();
    assert_eq!(kit_color(&mut kit, &id, 30.0, 14.0), painted, "the collaborator's stroke shows");

    // The kit paints on top, and the collaborator takes its changes with the blobs they need.
    let since = peer.version().unwrap();
    let stroke = json!({ "documentId": id, "ops": [{ "op": "command", "id": "paint.stroke", "params": { "points": [[8, 34], [56, 34]], "size": 8, "color": "#0000ff" } }] });
    kit.model("model.batch", stroke).unwrap();
    let kits = kit_color(&mut kit, &id, 30.0, 34.0);
    assert_ne!(kits, blank);
    let exported = kit.model("model.export", json!({ "documentId": id, "since": base64::encode(&since) })).unwrap();
    let shipped = base64::decode(exported["blobs"].as_str().expect("the kit's stroke ships its tiles"));
    peer.add_blobs(&shipped).unwrap();
    peer.import_changes(&base64::decode(exported["changes"].as_str().unwrap())).unwrap();
    assert_eq!(color_at(&peer, 30.0, 34.0), kits, "the kit's stroke shows");

    // Every change with every blob the image references opens a new replica with its pixels.
    let everything = kit.model("model.export", json!({ "documentId": id })).unwrap();
    let mut joined = self::kit();
    let reopened = joined.model("model.open", json!({ "name": "art.psd", "snapshot": everything["changes"], "blobs": everything["blobs"] })).unwrap();
    let joined_id = reopened["documentId"].as_str().unwrap().to_owned();
    assert_eq!((kit_color(&mut joined, &joined_id, 30.0, 14.0), kit_color(&mut joined, &joined_id, 30.0, 34.0)), (painted, kits));
}

#[test]
fn never_opens_an_empty_image_for_a_replica_it_cannot_read() {
    let mut kit = kit();
    for snapshot in [&b"not a replica"[..], &b""[..]] {
        let error = kit.model("model.open", json!({ "name": "art.psd", "snapshot": base64::encode(snapshot) })).unwrap_err();
        assert_eq!(error.reason(), Some(ErrorReason::UnsupportedFormat), "{error:?}");
    }
    let mut peer = peer();
    let mut state = room(&mut peer);
    state["blobs"] = json!(base64::encode(&[1, 2, 3]));
    assert_eq!(kit.model("model.open", state).unwrap_err().reason(), Some(ErrorReason::UnsupportedFormat), "damaged blobs");
    assert!(kit.documents().is_empty(), "nothing was opened");
}

/// What the kit refuses is changes it cannot apply: here, from a newer build whose schema this
/// kit does not read. The document is closed.
#[test]
fn refuses_changes_it_cannot_take_and_closes_the_document() {
    use kition_kit::kdm::{ActorId, Replica, Schema};

    let mut peer = peer();
    let mut kit = kit();
    let snapshot = peer.snapshot().unwrap();
    let id = kit.model("model.open", room(&mut peer)).unwrap()["documentId"].as_str().unwrap().to_owned();

    let mut raw: Value = serde_json::from_str(include_str!("../../../crates/kdm/kdm-model/model/schema.json")).unwrap();
    let newer = raw["schemaVersion"].as_u64().unwrap() + 1;
    raw["schemaVersion"] = json!(newer);
    let mut newer_peer = Replica::load(std::sync::Arc::new(Schema::parse(&raw.to_string()).unwrap()), &snapshot, ActorId::from_u64(77).unwrap()).unwrap();
    let since = newer_peer.version();
    newer_peer.transact(&kition_photo_engine::Origin::system("migrate"), |tx| tx.raise_schema_version(u32::try_from(newer).unwrap()).map(|_| ())).unwrap();
    let refused = newer_peer.export_changes(&since).unwrap();

    let error = kit.model("model.import", json!({ "documentId": id, "changes": base64::encode(&refused) })).unwrap_err();
    assert_eq!(error.reason(), Some(ErrorReason::InvalidParams), "{error:?}");
    assert_eq!(error.data.as_ref().unwrap()["refused"], true);
    assert_eq!(kit.model("model.export", json!({ "documentId": id })).unwrap_err().reason(), Some(ErrorReason::DocumentNotFound));
    assert!(kit.documents().is_empty());
}

/// The agent's edit is the agent's (KHP A11): the person's undo leaves it (KitModel M12a).
#[test]
fn the_agents_edit_is_the_agents() {
    let root = temp_root("origin");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "a.psd");
    b.execute(&created.id, "layer.new.layer", params(json!({ "name": "Agent" })), Origin::Agent).unwrap();
    assert!(!b.document(&created.id).unwrap().can_undo, "nothing of the person's to undo");
    b.execute(&created.id, "layer.new.layer", params(json!({ "name": "Person" })), Origin::User).unwrap();
    assert!(b.document(&created.id).unwrap().can_undo);
    let _ = std::fs::remove_dir_all(root);
}

/// A parameter of the wrong JSON type is refused by the SDK's schema check (KHP A5, A18), for the
/// agent and the person alike, and the image stays as it was.
#[test]
fn a_parameter_of_the_wrong_type_is_refused() {
    let root = temp_root("wrong-type");
    let (mut b, folder) = backend(&root);
    let created = create(&mut b, &folder, "a.psd");
    let before = layer_names(&mut b, &created.id).len();
    for origin in [Origin::Agent, Origin::User] {
        let error = b.execute(&created.id, "layer.new.layer", params(json!({ "name": { "not": "a name" } })), origin).unwrap_err();
        assert_eq!(error.code, ErrorReason::InvalidParams.code());
        let detail = error.data.unwrap()["detail"].as_str().unwrap().to_owned();
        assert!(detail.contains("`name`: expected a string"), "{detail}");
        assert_eq!(layer_names(&mut b, &created.id).len(), before);
    }
    b.execute(&created.id, "layer.new.layer", params(json!({ "name": "Kept" })), Origin::Agent).unwrap();
    assert_eq!(layer_names(&mut b, &created.id).len(), before + 1);
}

/// A colour parameter takes a colour: `edit.fill` and `edit.stroke` accept `#rrggbb` and
/// `[r,g,b,a]` from the agent, under the SDK's parameter checks (their notation once named the two
/// forms as the only two allowed strings, so every real colour was refused).
#[test]
fn colour_parameters_take_hex_and_rgba() {
    let root = temp_root("colours");
    let (mut b, folder) = backend(&root);
    let info = create(&mut b, &folder, "colours.psd");
    let id = info.id;
    b.execute(&id, "select.all", Map::new(), Origin::Agent).unwrap();
    for color in [json!("#c0392b"), json!([0.2, 0.4, 0.6, 1.0])] {
        let filled = b.execute(&id, "edit.fill", params(json!({ "contents": "color", "color": color.clone() })), Origin::Agent);
        assert!(filled.is_ok(), "edit.fill {color}: {:?}", filled.err());
        let stroked = b.execute(&id, "edit.stroke", params(json!({ "width": 2, "color": color.clone() })), Origin::Agent);
        assert!(stroked.is_ok(), "edit.stroke {color}: {:?}", stroked.err());
    }
    let _ = std::fs::remove_dir_all(root);
}

/// `document.text` (KitText 1.0): the manifest's capability reaches `kit.initialize`, and the
/// executable answers the type layers' words without changing the document.
#[test]
fn answers_document_text_as_the_manifest_declares() {
    let mut b = kit();
    assert_eq!(b.capabilities()["capabilities"], json!([kition_kit::text::CAPABILITY]));
    let root = temp_root("text");
    let path = root.join("title.kphoto");
    std::fs::copy(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/check-font-substituted.kphoto")), &path).unwrap();
    let folder = HostFolder::new(&root).unwrap();
    b.set_host(folder.host());
    let opened = b.open(OpenRequest { file: &folder.file("title.kphoto").unwrap(), read_only: false, create: None }).unwrap();
    let text = b.text(&opened.id, kition_kit_server::protocol::DOCUMENT_TEXT_LIMIT).unwrap();
    assert_eq!(text, json!({ "text": "Title", "truncated": false, "parts": [{ "label": "Layer: Title", "start": 0, "end": 5 }] }));
    assert_eq!(b.text(&opened.id, 2).unwrap(), json!({ "text": "Ti", "truncated": true, "parts": [{ "label": "Layer: Title", "start": 0, "end": 2 }] }));
    assert!(!b.document(&opened.id).unwrap().dirty);
    assert_eq!(b.text("no-such-document", 10).unwrap_err().reason(), Some(ErrorReason::DocumentNotFound));
    let _ = std::fs::remove_dir_all(root);
}
