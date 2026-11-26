//! The headless `kition-photo` saves on the direct path (feature `photo.file.content-only`): the
//! `.kphoto` an agent's batch writes holds the image's content (`content.json`: the state alone,
//! no version and no change) and none of its history.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use serde_json::{Value, json};

/// Written once, overwritten at once: in no file.
const OVERWRITTEN: &str = "Ug5Le1Fo0Dw4";

fn content(file: &[u8]) -> Vec<u8> {
    kition_photo_format::read_kdm_content(file).unwrap().expect("the file holds the replica's content")
}

fn manifest(file: &[u8]) -> Vec<u8> {
    kition_photo_format::zip::ZipReader::new(file).unwrap().read_by_name("manifest.json", 64 * 1024 * 1024).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agents_batches_save_content_only() {
    let f = Fixture::new("Content.kphoto", 2);
    let (seen, _) = f.inspect().await;
    let id = ids_by_name(&seen)["L0"];
    f.ok("batch", json!({"ops":[{"op":"set","target":id.to_string(),"field":"name","value":OVERWRITTEN}]})).await;
    f.ok("batch", json!({"ops":[{"op":"set","target":id.to_string(),"field":"name","value":"Kept"}]})).await;
    let file = std::fs::read(f.file()).unwrap();
    let entry: Value = serde_json::from_slice(&content(&file)).unwrap();
    assert!(entry.get("version").is_none() && entry.get("changes").is_none(), "the state alone: no version, no change");
    assert!(entry["objects"].is_object());
    for held in [content(&file), manifest(&file)] {
        assert!(!held.windows(OVERWRITTEN.len()).any(|window| window == OVERWRITTEN.as_bytes()), "the overwritten name is in no file");
    }
    assert_eq!(layer_on_disk(&f.file(), id).unwrap()["name"], "Kept");
    f.shutdown().await;
}
