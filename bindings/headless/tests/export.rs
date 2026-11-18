//! `kition-photo` exports the image flattened to PNG, JPEG, TIFF and WebP through the host's
//! `document.export` (KHP A41): each as a new file, the image unchanged, an existing file kept, and
//! only the manifest's formats.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_headless::{Headless, HostFolder, KitInfo};
use kition_kit::KitDocument;
use kition_kit_server::{KitBackend, OpenRequest};
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[test]
fn the_host_exports_the_image_as_new_flat_files() {
    let root = std::env::temp_dir().join(format!("kition-photo-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    let file = root.join("poster.psd");
    let mut doc = PhotoDocument::create("poster.psd", 64, 40).unwrap();
    doc.execute("select.rect", json!({ "x": 8, "y": 8, "width": 30, "height": 20 })).unwrap();
    doc.execute("edit.fill", json!({ "color": "#1f4e8c" })).unwrap();
    std::fs::write(&file, doc.save(None).unwrap()).unwrap();

    let manifest = include_str!("../../../kit.json");
    let mut kit = Headless::<PhotoDocument>::new(KitInfo::from_manifest(manifest, kition_photo_kit::SANDBOXED, kition_photo_kit::EDITOR_STATE).unwrap());
    assert_eq!(kit.capabilities()["formats"]["export"], json!(["png", "jpg", "tiff", "webp"]), "the host is told what the kit exports");
    let folder = HostFolder::new(&root).unwrap();
    kit.set_host(folder.host());
    let id = kit.open(OpenRequest { file: &folder.file("poster.psd").unwrap(), read_only: false, create: None }).unwrap().id;
    let magic: [(&str, &[u8]); 4] = [("png", b"\x89PNG"), ("jpg", b"\xff\xd8\xff"), ("tiff", b"II*\0"), ("webp", b"RIFF")];
    for (format, start) in magic {
        let name = format!("poster.{format}");
        let handle = folder.export(&name).unwrap();
        kit.export(&id, &handle, format, None).unwrap_or_else(|error| panic!("{format}: {error:?}"));
        let bytes = std::fs::read(root.join(&name)).unwrap();
        assert!(bytes.starts_with(start) || (format == "tiff" && bytes.starts_with(b"MM\0*")), "{format}: {:?}", &bytes[..8.min(bytes.len())]);
        assert!(kit.export(&id, &handle, format, None).is_err(), "{format}: an export never replaces a file");
        assert!(folder.export(&name).is_err(), "{format}: the host mints no export handle on an existing file");
    }
    // The agent's `<prefix>_export` runs `file.exportImage` (kit.json `agent.export`): the bytes in
    // the answer, as the agent, without touching the image.
    let answer = kit
        .execute(&id, "file.exportImage", serde_json::Map::from_iter([("format".into(), json!("png"))]), kition_kit_server::protocol::Origin::Agent)
        .unwrap();
    let result = answer.result.unwrap();
    assert_eq!(result["mime"], "image/png");
    assert!(result["data"].as_str().is_some_and(|data| data.starts_with("iVBORw0KGgo")), "base64 PNG");
    assert!(!kit.document(&id).unwrap().dirty, "exporting leaves the image as it is");
    assert!(kit.export(&id, &folder.export("poster.gif").unwrap(), "gif", None).is_err(), "only the manifest's exports");
    assert!(!root.join("poster.gif").exists());
    let _ = std::fs::remove_dir_all(root);
}

/// Every `exports` entry names the command that writes it, the one `agent.export` names (platform
/// K21), and that command, run as the agent with the entry's extension as `format`, answers the
/// entry's file. Before, the entries named `file.export.quickExport`, which takes a path and is
/// not offered to agents.
#[test]
fn every_export_entry_names_the_agents_export_command() {
    let manifest: serde_json::Value = serde_json::from_str(include_str!("../../../kit.json")).unwrap();
    let command = manifest["agent"]["export"]["command"].as_str().unwrap();
    let mut kit = Headless::<PhotoDocument>::new(
        KitInfo::from_manifest(include_str!("../../../kit.json"), kition_photo_kit::SANDBOXED, kition_photo_kit::EDITOR_STATE).unwrap(),
    );
    let root = std::env::temp_dir().join(format!("kition-photo-export-entries-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::write(root.join("poster.psd"), PhotoDocument::create("poster.psd", 32, 20).unwrap().save(None).unwrap()).unwrap();
    let folder = HostFolder::new(&root).unwrap();
    kit.set_host(folder.host());
    let id = kit.open(OpenRequest { file: &folder.file("poster.psd").unwrap(), read_only: false, create: None }).unwrap().id;
    let entries = manifest["exports"].as_array().unwrap();
    assert_eq!(entries.len(), 4);
    for entry in entries {
        let extension = entry["extension"].as_str().unwrap();
        assert_eq!(entry["command"], command, "{extension}");
        let params = serde_json::Map::from_iter([("format".into(), json!(extension))]);
        let answer = kit.execute(&id, command, params, kition_kit_server::protocol::Origin::Agent).unwrap_or_else(|error| panic!("{extension}: {error:?}"));
        let result = answer.result.unwrap();
        assert_eq!(result["mime"], entry["mime"], "{extension}");
        assert!(result["data"].as_str().is_some_and(|data| !data.is_empty()), "{extension}");
    }
    assert!(!kit.document(&id).unwrap().dirty);
    let _ = std::fs::remove_dir_all(root);
}
