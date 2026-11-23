//! The headless `kition-photo` saves on the direct path (feature `photo.file.content-only`): the
//! host seals the document (`prepare_save`) before every save, so the `.kphoto` an agent's batch
//! writes holds the image's content and, of its history, only that seal.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::kdm::{Origin, Replica};
use serde_json::json;

/// Written once, overwritten at once: in no file.
const OVERWRITTEN: &str = "Ug5Le1Fo0Dw4";

fn origins(file: &[u8]) -> Vec<Option<Origin>> {
    Replica::snapshot_origins(&kition_photo_format::read_kdm_snapshot(file).unwrap().expect("the file holds the replica's content")).unwrap()
}

fn manifest(file: &[u8]) -> Vec<u8> {
    kition_photo_format::zip::ZipReader::new(file).unwrap().read_by_name("manifest.json", 64 * 1024 * 1024).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agents_batches_save_sealed_content() {
    let f = Fixture::new("Content.kphoto", 2);
    let (seen, _) = f.inspect().await;
    let id = ids_by_name(&seen)["L0"];
    f.ok("batch", json!({"ops":[{"op":"set","target":id.to_string(),"field":"name","value":OVERWRITTEN}]})).await;
    f.ok("batch", json!({"ops":[{"op":"set","target":id.to_string(),"field":"name","value":"Kept"}]})).await;
    let file = std::fs::read(f.file()).unwrap();
    assert_eq!(origins(&file), vec![Some(Origin::system("content-seal"))], "of the history, the file holds the seal");
    let content = kition_photo_format::read_kdm_snapshot(&file).unwrap().unwrap();
    for held in [content, manifest(&file)] {
        assert!(!held.windows(OVERWRITTEN.len()).any(|window| window == OVERWRITTEN.as_bytes()), "the overwritten name is in no file");
    }
    assert_eq!(layer_on_disk(&f.file(), id).unwrap()["name"], "Kept");
    f.shutdown().await;
}
