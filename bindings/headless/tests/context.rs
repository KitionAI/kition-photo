//! An agent's run leaves its editor state out of the file (task card C-photo): a `.kphoto` stores
//! the pixel selection of the session that saved it, and an agent that selects and fills in a run,
//! by `on` and by its own `select.rect`, saves the file with the selection it was opened with, not
//! its own: the saved bytes' manifest says so.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn an_agents_selection_and_layer_are_not_saved_into_the_file() {
    let mut doc = image_doc("Image.kphoto", 3);
    doc.execute("select.rect", json!({ "x": 5, "y": 6, "width": 7, "height": 8 })).unwrap();
    let bytes = doc.save(Some("Image.kphoto")).unwrap();
    let opened = PhotoDocument::open("Image.kphoto", &bytes).unwrap().inspect().unwrap()["selectionBounds"].clone();
    println!("the file opens with selection {opened}");
    let f = Fixture::from_bytes("Image.kphoto", &bytes, CallOptions::default());
    let (details, _) = f.inspect().await;
    let l1 = ids_by_name(&details)["L1"];
    let ran = f
        .ok(
            "run",
            json!({ "steps": [
                { "command": "select.rect", "params": { "x": 300, "y": 60, "width": 50, "height": 50 } },
                { "command": "edit.fill", "on": [l1], "params": { "color": "#ff00ff" } },
            ] }),
        )
        .await;
    assert_eq!(ran["saved"], true, "{ran}");
    let again = PhotoDocument::open("Image.kphoto", &std::fs::read(f.file()).unwrap()).unwrap();
    let inspected = again.inspect().unwrap();
    assert_eq!(inspected["selectionBounds"], opened, "the file's selection, not the agent's");
    let stored =
        |bytes: &[u8]| kition_photo_format::load_from_bytes(bytes).unwrap().selection.map(|mask| kition_photo_engine::inspect::xywh(mask.content_bounds()));
    let saved = stored(&std::fs::read(f.file()).unwrap());
    println!("stored selection: in the file before {:?}, after the agent's run {saved:?}", stored(&bytes));
    assert_ne!(saved, Some([300, 60, 50, 50]), "the agent's selection is not in the saved bytes");
    let filled = layers_of(&inspected).into_iter().find(|layer| layer["id"] == l1).unwrap();
    assert_eq!(filled["bounds"], json!([300, 60, 136, 120]), "the agent's fill is there: {filled}");
    f.shutdown().await;
}
