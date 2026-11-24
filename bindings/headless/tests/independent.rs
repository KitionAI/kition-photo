//! The wrong-write classes the independent reading of every write command found (task card
//! C8-photo-read; `bindings/wasm/tests/declarations_independent.rs` holds the readings). Each test
//! is an agent's write with the `basis` of its inspect after an independent writer (a colleague's
//! own engine instance) rewrote the file: one change the write rests on, which must refuse it,
//! and one it does not rest on, which must not.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use serde_json::{Value, json};

const FILE: &str = "Read.kphoto";
const LAYERS: usize = 3;

fn step(command: &str, params: Value) -> Value {
    json!({ "command": command, "params": params })
}

/// Runs `command` with `basis` after the colleague's `change`; `Ok` with the answer, or the error.
async fn after(
    change: &[(&str, Value)],
    command: &str,
    params: impl Fn(&std::collections::BTreeMap<String, u64>) -> Value,
) -> (Result<Value, Value>, Fixture, std::collections::BTreeMap<String, u64>) {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let ids = ids_by_name(&details);
    f.colleague(change);
    let result = f.call("run", json!({ "basis": basis, "steps": [step(command, params(&ids))] })).await;
    (result, f, ids)
}

fn conflict(result: &Result<Value, Value>, what: &str) {
    match result {
        Err(error) if error["error"]["code"] == "conflict" => {}
        other => panic!("{what}: a change the write rests on must refuse it, got {other:?}"),
    }
}

fn applied(result: &Result<Value, Value>, what: &str) -> Value {
    match result {
        Ok(answer) => answer.clone(),
        Err(error) => panic!("{what}: a change the write does not rest on must not refuse it: {error}"),
    }
}

/// The ids of the image's layers in a fresh fixture (they are the same in every one).
fn names() -> std::collections::BTreeMap<String, u64> {
    ids_by_name(&image_doc(FILE, LAYERS).inspect().unwrap())
}

/// Class "a read that is not held": a command acting on the active layer (no `layer`) outside the
/// pixel families held nothing of it, so another writer's change of what it reads let it land.
#[tokio::test(flavor = "multi_thread")]
async fn the_active_layer_a_command_acts_on_is_held() {
    let ids = names();
    let top = ids["L2"];
    // The colleague gives the active layer (L2, the top) an effect: the drop shadow rests on its effects.
    let (result, f, _) =
        after(&[("layer.layerStyle.colorOverlay", json!({ "layer": top, "color": "#ff0000" }))], "layer.layerStyle.dropShadow", |_| json!({})).await;
    conflict(&result, "an effect added to the active layer");
    f.shutdown().await;
    let (result, f, _) = after(&[("layer.setProps", json!({ "layer": ids["L0"], "opacity": 0.5 }))], "layer.layerStyle.dropShadow", |_| json!({})).await;
    applied(&result, "another layer's opacity");
    f.shutdown().await;
}

/// Class "a read that is not held": a crop by a rectangle rests on the canvas its coordinates are
/// in; it held nothing.
#[tokio::test(flavor = "multi_thread")]
async fn a_crop_rests_on_the_canvas() {
    let ids = names();
    let (result, f, _) = after(
        &[("image.canvasSize", json!({ "width": 400, "height": 256, "anchor": "center" }))],
        "image.crop",
        |_| json!({ "x": 0, "y": 0, "width": 200, "height": 200 }),
    )
    .await;
    conflict(&result, "the canvas changed");
    f.shutdown().await;
    let (result, f, _) =
        after(&[("layer.setProps", json!({ "layer": ids["L0"], "opacity": 0.5 }))], "image.crop", |_| json!({ "x": 0, "y": 0, "width": 200, "height": 200 }))
            .await;
    applied(&result, "a layer's opacity");
    f.shutdown().await;
}

/// Class "a read that is not held": a note named by its place in the list; another writer's note
/// before it names another note there.
#[tokio::test(flavor = "multi_thread")]
async fn a_note_by_index_rests_on_the_notes() {
    let mut doc = image_doc(FILE, LAYERS);
    doc.execute("notes.add", json!({ "x": 10, "y": 10, "text": "first" })).unwrap();
    let bytes = doc.save_sealed(Some(FILE)).unwrap();
    for relevant in [true, false] {
        let f = Fixture::from_bytes(FILE, &bytes, kition_kits::tools::CallOptions::default());
        let (details, basis) = f.inspect().await;
        let ids = ids_by_name(&details);
        if relevant {
            // Their note 0 goes and a note of theirs comes: index 0 names their note now.
            f.colleague(&[("notes.delete", json!({ "index": 0 })), ("notes.add", json!({ "x": 90, "y": 90, "text": "theirs" }))]);
        } else {
            f.colleague(&[("layer.setProps", json!({ "layer": ids["L0"], "opacity": 0.5 }))]);
        }
        let result = f.call("run", json!({ "basis": basis, "steps": [step("notes.set", json!({ "index": 0, "text": "edited" }))] })).await;
        if relevant {
            conflict(&result, "the note at index 0 is another note now");
        } else {
            applied(&result, "a layer's opacity");
        }
        f.shutdown().await;
    }
}

/// Class "a read that is not held": a command on the selected layers (no `on`) held none of them.
#[tokio::test(flavor = "multi_thread")]
async fn the_selected_layers_a_command_acts_on_are_held() {
    let ids = names();
    let top = ids["L2"];
    let paint = vec![
        ("layer.select", json!({ "layer": top })),
        ("select.rect", json!({ "x": 0, "y": 0, "width": 4, "height": 4 })),
        ("edit.fill", json!({ "color": "#000000" })),
        ("select.deselect", json!({})),
    ];
    let (result, f, _) = after(&paint, "layer.duplicate", |_| json!({})).await;
    conflict(&result, "paint on the selected layer it duplicates");
    f.shutdown().await;
    let (result, f, _) = after(&[("layer.setProps", json!({ "layer": ids["L0"], "opacity": 0.5 }))], "layer.duplicate", |_| json!({})).await;
    applied(&result, "another layer's opacity");
    f.shutdown().await;
}
