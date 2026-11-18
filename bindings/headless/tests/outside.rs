//! The premise guard and receipts when the image file itself changes (task card C-photo): a
//! colleague's program, an engine of this kit that shares nothing with the server under test,
//! opens the image, edits it and writes it back, so the server sees only a changed file. The agent
//! works through the kits' MCP server's tools on the `kition-photo` executable, holding the ids and
//! the `revision` of an inspect made before the colleague's edit.
//!
//! Photo's addresses: a layer by id (`layer: 7` or `on`), by its place in the top-level stack from
//! the top as inspect lists it (`layer: "#3"`), by its whole name (`layer: "L2"`), or by a point (`at: [x, y]`, the
//! topmost visible layer with a pixel there). Each write holds what its command reads or replaces
//! (`src/place.rs`): a property, the 256-pixel tiles of a region and the image's size, a name.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::{CallOptions, Guard};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Image.kphoto";
const LAYERS: usize = 8;

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    for field in ["id", "selection"] {
        doc.as_object_mut().unwrap().remove(field);
    }
    doc
}

fn opacity(layer: impl Into<Value>, value: f64) -> Value {
    json!({ "command": "layer.setProps", "params": { "layer": layer.into(), "opacity": value } })
}

fn opacity_at(point: [i32; 2], value: f64) -> Value {
    json!({ "command": "layer.setProps", "params": { "at": point, "opacity": value } })
}

fn fill(layer: impl Into<Value>, rect: [i32; 4], color: &str) -> Value {
    json!({ "command": "edit.fill", "params": { "layer": layer.into(), "color": color, "region": { "rect": rect } } })
}

/// The colleague paints a small block on layer `id` at `rect`.
fn paint(id: u64, rect: [i32; 4], color: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("layer.select", json!({ "layer": id })),
        ("select.rect", json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] })),
        ("edit.fill", json!({ "color": color })),
        ("select.deselect", json!({})),
    ]
}

fn opacity_on_disk(f: &Fixture, id: u64) -> f64 {
    layer_on_disk(&f.file(), id).unwrap()["opacity"].as_f64().unwrap()
}

/// The `"#n"` of top-level layer `id` (from the top, as inspect lists the top level) in an inspect.
fn index_of(details: &Value, id: u64) -> String {
    let top: Vec<u64> = details["layers"].as_array().unwrap().iter().map(|layer| layer["id"].as_u64().unwrap()).collect();
    format!("#{}", top.iter().position(|layer| *layer == id).unwrap() + 1)
}

// ---- 1. An unrelated layer ----

#[tokio::test(flavor = "multi_thread")]
async fn s1_a_colleagues_change_of_another_layer_applies_and_others_names_it() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let names = ids_by_name(&details);
    let (mine, theirs, painted) = (names["L2"], names["L5"], names["L6"]);
    f.colleague(&[("layer.setProps", json!({ "layer": theirs, "opacity": 0.5 }))]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    let others = &ran["others"];
    assert_eq!(ids(&others["changed"]), [theirs.to_string()], "{ran}");
    println!("s1: others for one unrelated opacity: {} bytes: {others}", others.to_string().len());
    f.colleague(&paint(painted, block(6), "#000000"));
    let ran = f.ok("run", json!({ "basis": revision(&ran), "steps": [fill(mine, region(2), "#00ff00")] })).await;
    assert_eq!(ids(&ran["others"]["changed"]), [painted.to_string()], "{ran}");
    println!("s1: others for one unrelated paint: {} bytes: {}", ran["others"].to_string().len(), ran["others"]);
    assert_eq!(opacity_on_disk(&f, mine), 0.25);
    f.shutdown().await;
}

// ---- 2. The same property of the target ----

#[tokio::test(flavor = "multi_thread")]
async fn s2_a_colleagues_change_of_the_same_property_is_refused_and_the_retry_applies() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    f.colleague(&[("layer.setProps", json!({ "layer": mine, "opacity": 0.5 }))]);
    let before = std::fs::read(f.file()).unwrap();
    let refused = f.refused("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    assert_conflict(&refused, &mine.to_string(), "changed");
    assert_eq!(std::fs::read(f.file()).unwrap(), before, "nothing written");
    println!("s2: refused changed: {} bytes: {refused}", refused.to_string().len());
    let (_, basis) = f.inspect().await;
    f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    assert_eq!(opacity_on_disk(&f, mine), 0.25);
    // Its name changed instead: outside an opacity write's premise.
    let (_, basis) = f.inspect().await;
    f.colleague(&[("layer.setProps", json!({ "layer": mine, "name": "Theirs" }))]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.75)] })).await;
    assert_eq!(ids(&ran["others"]["changed"]), [mine.to_string()], "{ran}");
    f.shutdown().await;
}

// ---- 3. The target deleted ----

#[tokio::test(flavor = "multi_thread")]
async fn s3_a_colleague_deleting_the_target_is_refused_as_deleted() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    f.colleague(&[("layer.delete", json!({ "layer": mine }))]);
    let before = std::fs::read(f.file()).unwrap();
    for step in [opacity(mine, 0.25), fill(mine, region(2), "#00ff00")] {
        let refused = f.refused("run", json!({ "basis": basis, "steps": [step] })).await;
        assert_conflict(&refused, &mine.to_string(), "deleted");
        println!("s3: refused deleted: {}", refused["conflicts"]);
    }
    assert_eq!(std::fs::read(f.file()).unwrap(), before, "nothing written");
    f.shutdown().await;
}

// ---- 4. A layer inserted in the stack ----
//
// `"#n"` counts the top level from the top, as inspect and the Layers panel list it
// (`index_reversed`); kition-kits reads the window among the image's top-level nodes in the
// replica's tree and follows the index from that end.

/// A layer inserted above the target: a write by id applies.
#[tokio::test(flavor = "multi_thread")]
async fn s4a_a_write_by_id_after_a_layer_inserted_above_applies() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    f.colleague(&[("layer.select", json!({ "layer": mine })), ("layer.new.layer", json!({ "name": "Above" }))]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    assert_eq!(opacity_on_disk(&f, mine), 0.25);
    println!("s4a: by id after an insert above: applied; others {}", ran["others"]);
    f.shutdown().await;
}

/// A layer inserted below the target leaves the target's `"#n"` (from the top) as it was: the
/// write by it applies to the target.
#[tokio::test(flavor = "multi_thread")]
async fn s4b_a_write_by_index_after_a_layer_inserted_below_applies() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let names = ids_by_name(&details);
    let mine = names["L2"];
    let index = index_of(&details, mine);
    f.colleague(&[("layer.select", json!({ "layer": names["L1"] })), ("layer.new.layer", json!({ "name": "Below" }))]);
    f.ok("run", json!({ "basis": basis, "steps": [opacity(index.clone(), 0.375)] })).await;
    assert_eq!(opacity_on_disk(&f, mine), 0.375);
    println!("s4b: by {index} after an insert below: applied to the target");
    f.shutdown().await;
}

/// A layer inserted above the target moves it down one from the top: the write by its old `"#n"`
/// is refused as `relocated`, naming the target and where it is now (`now.spelled`), and nothing
/// is written; the write by id applies.
#[tokio::test(flavor = "multi_thread")]
async fn s4c_a_write_by_index_after_a_layer_inserted_above_is_refused_as_relocated() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    let index = index_of(&details, mine);
    f.colleague(&[("layer.select", json!({ "layer": mine })), ("layer.new.layer", json!({ "name": "Above" }))]);
    let before = std::fs::read(f.file()).unwrap();
    let refused = f.refused("run", json!({ "basis": basis, "steps": [opacity(index.clone(), 0.625)] })).await;
    let conflict = assert_conflict(&refused, &mine.to_string(), "relocated");
    let n: usize = index.trim_start_matches('#').parse().unwrap();
    assert_eq!(conflict["now"]["spelled"], format!("#{}", n + 1), "{refused}");
    assert_eq!(std::fs::read(f.file()).unwrap(), before, "nothing written");
    println!("s4c: by {index} after an insert above: refused {}", refused["conflicts"]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.875)] })).await;
    assert_eq!(opacity_on_disk(&f, mine), 0.875);
    println!("s4c: by id after an insert above: applied; others {}", ran["others"]);
    f.shutdown().await;
}

// ---- 5. Paint on the target ----

/// The colleague paints on the target inside the region the agent fills: refused. Outside it in
/// another tile or the same tile: applies and preserves the complete saved merge.
#[tokio::test(flavor = "multi_thread")]
async fn s5_paint_on_the_target_inside_and_outside_the_region() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    let [x, y, ..] = region(2);
    f.colleague(&paint(mine, [x + 5, y + 5, 10, 10], "#000000"));
    let refused = f.refused("run", json!({ "basis": basis, "steps": [fill(mine, region(2), "#00ff00")] })).await;
    assert_conflict(&refused, &mine.to_string(), "changed");
    println!("s5: inside: refused {}", refused["conflicts"]);
    let (_, basis) = f.inspect().await;
    let far = block(7);
    f.colleague(&paint(mine, [far[0], far[1], 10, 10], "#000000"));
    let ran = f.ok("run", json!({ "basis": basis, "steps": [fill(mine, region(2), "#00ff00")] })).await;
    println!("s5: outside, another tile: applied; others {}", ran["others"]);
    let (_, basis) = f.inspect().await;
    let [bx, by, bw, bh] = block(2);
    f.colleague(&paint(mine, [bx + bw - 15, by + bh - 15, 10, 10], "#000000"));
    let before = std::fs::read(f.file()).unwrap();
    let ran = f.ok("run", json!({ "basis": basis, "steps": [fill(mine, region(2), "#00ff00")] })).await;
    let mut reference = PhotoDocument::open(FILE, &before).unwrap();
    let [x, y, width, height] = region(2);
    reference.execute("select.rect", json!({"x":x,"y":y,"width":width,"height":height})).unwrap();
    reference.execute("edit.fill", json!({"layer":mine,"color":"#00ff00"})).unwrap();
    assert_eq!(saved(&std::fs::read(f.file()).unwrap()), saved(&reference.save(None).unwrap()));
    println!("s5: outside, the same tile: applied; others {}", ran["others"]);
    f.shutdown().await;
}

// ---- 6. The target moved ----

/// The colleague moves the target's pixels 70 pixels right: a property write by id applies; a fill
/// by coordinates on it is refused (its pixels in the region changed); a pick at the point where
/// it was is refused (what was there moved).
#[tokio::test(flavor = "multi_thread")]
async fn s6_the_target_moved() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    f.colleague(&[("layer.translate", json!({ "layer": mine, "dx": 70, "dy": 0 }))]);
    let before = std::fs::read(f.file()).unwrap();
    let refused = f.refused("run", json!({ "basis": basis, "steps": [fill(mine, region(2), "#00ff00")] })).await;
    assert_conflict(&refused, &mine.to_string(), "changed");
    println!("s6: fill by coordinates on the moved layer: refused {}", refused["conflicts"]);
    let [x, y, ..] = block(2);
    let refused = f.refused("run", json!({ "basis": basis, "steps": [opacity_at([x + 20, y + 60], 0.3)] })).await;
    assert_eq!(refused["error"]["code"], "conflict", "{refused}");
    assert_eq!(std::fs::read(f.file()).unwrap(), before, "nothing written");
    println!("s6: pick where it was: refused {}", refused["conflicts"]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    println!("s6: opacity by id: applied; others {}", ran["others"]);
    f.shutdown().await;
}

// ---- 7. The canvas cropped ----

#[tokio::test(flavor = "multi_thread")]
async fn s7_the_canvas_resized() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    let width = details["width"].as_i64().unwrap();
    f.colleague(&[("image.canvasSize", json!({ "width": width - 100, "height": details["height"], "anchor": "center" }))]);
    let before = std::fs::read(f.file()).unwrap();
    let refused = f.refused("run", json!({ "basis": basis, "steps": [fill(mine, region(2), "#00ff00")] })).await;
    assert_eq!(refused["error"]["code"], "conflict", "{refused}");
    assert_eq!(std::fs::read(f.file()).unwrap(), before, "nothing written");
    // The image keeps its object id on every read, so its own fields' change names it (kition-sdk
    // C8-fix-10), not a count in `unnamed`.
    assert_conflict(&refused, "image", "changed");
    let image = refused["others"]["changed"].as_array().unwrap().iter().find(|change| change["id"] == "image").cloned();
    assert!(image.is_some_and(|change| change["fields"].as_array().unwrap().contains(&json!("width"))), "{refused}");
    println!("s7: fill by coordinates after a canvas resize: refused {}", refused["conflicts"]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    println!("s7: opacity by id: applied; others {} bytes", ran["others"].to_string().len());
    f.shutdown().await;
}

// ---- 8. Names swapped ----

#[tokio::test(flavor = "multi_thread")]
async fn s8_a_name_that_now_names_another_layer_is_refused() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let names = ids_by_name(&details);
    let (mine, other) = (names["L2"], names["L3"]);
    f.colleague(&[
        ("layer.setProps", json!({ "layer": mine, "name": "Swap" })),
        ("layer.setProps", json!({ "layer": other, "name": "L2" })),
        ("layer.setProps", json!({ "layer": mine, "name": "L3" })),
    ]);
    let before = std::fs::read(f.file()).unwrap();
    let refused = f.refused("run", json!({ "basis": basis, "steps": [opacity("L2", 0.25)] })).await;
    assert_conflict(&refused, &mine.to_string(), "relocated");
    assert_conflict(&refused, &other.to_string(), "relocated");
    assert_eq!(std::fs::read(f.file()).unwrap(), before, "nothing written");
    println!("s8: by name after a swap: refused {}", refused["conflicts"]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    println!("s8: by id after a swap: applied; others {}", ran["others"]);
    f.shutdown().await;
}

// ---- 9. Other guards ----

#[tokio::test(flavor = "multi_thread")]
async fn s9_the_epoch_guard_refuses_an_unrelated_change_and_off_overwrites() {
    let f = Fixture::with(FILE, LAYERS, CallOptions { guard: Guard::Epoch, ..CallOptions::default() });
    let (details, basis) = f.inspect().await;
    let names = ids_by_name(&details);
    f.colleague(&[("layer.setProps", json!({ "layer": names["L5"], "opacity": 0.5 }))]);
    let refused = f.refused("run", json!({ "basis": basis, "steps": [opacity(names["L2"], 0.25)] })).await;
    assert_eq!(refused["error"]["code"], "conflict", "{refused}");
    println!("s9: epoch: refused {}", refused["error"]);
    f.shutdown().await;
    let f = Fixture::with(FILE, LAYERS, CallOptions { guard: Guard::Off, ..CallOptions::default() });
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    f.colleague(&[("layer.setProps", json!({ "layer": mine, "opacity": 0.5 }))]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    assert_eq!(opacity_on_disk(&f, mine), 0.25, "overwritten");
    println!("s9: off: applied; others {}", ran["others"]);
    f.shutdown().await;
}

// ---- 10. Saved again unchanged ----

#[tokio::test(flavor = "multi_thread")]
async fn s10_a_file_saved_again_unchanged_is_no_change() {
    let f = Fixture::new(FILE, LAYERS);
    let (details, basis) = f.inspect().await;
    let mine = ids_by_name(&details)["L2"];
    let before = std::fs::read(f.file()).unwrap();
    f.colleague(&[]);
    println!("s10: the bytes {}", if std::fs::read(f.file()).unwrap() == before { "are the same" } else { "differ" });
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    assert!(ran.get("others").is_none_or(Value::is_null), "no notice: {ran}");
    f.shutdown().await;
}

// ---- 11. A PSD: what the kit holds is what the file stores ----

/// The agent sets an opacity a PSD stores in 8 bits (0.37 is 94/255), then a colleague changes
/// another layer of the `.psd`. The agent's next write on its own layer applies, and `others` names
/// only the colleague's layer: the kit held the opacity as the file stores it, so reading the file
/// again shows no change of it.
#[tokio::test(flavor = "multi_thread")]
async fn s11_a_psd_holds_an_opacity_as_the_file_stores_it() {
    let f = Fixture::new("Image.psd", LAYERS);
    let (details, basis) = f.inspect().await;
    let names = ids_by_name(&details);
    let mine = names["L2"];
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.37)] })).await;
    let stored = 94.0 / 255.0;
    assert!((opacity_on_disk(&f, mine) - stored).abs() < 1e-6);
    let receipt = &ran["changes"]["changed"][0]["edits"][0];
    assert!((receipt["after"].as_f64().unwrap() - stored).abs() < 1e-6, "the receipt tells the value the file stores: {ran}");
    let basis = revision(&ran);
    f.colleague(&[("layer.setProps", json!({ "layer": names["L5"], "opacity": 0.5 }))]);
    let ran = f.ok("run", json!({ "basis": basis, "steps": [opacity(mine, 0.25)] })).await;
    assert_eq!(ids(&ran["others"]["changed"]), [names["L5"].to_string()], "{ran}");
    println!("s11: psd: own opacity then a colleague's: applied; others {}", ran["others"]);
    f.shutdown().await;
}
