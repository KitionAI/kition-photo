//! Layers go where the Layers panel puts them: a new layer made while a
//! group is the active layer goes into that group, at its top; a layer mask from the selection
//! keeps the selection whether it reveals or hides it; Duplicate Layer takes the copy's `name`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn run(image: &mut PhotoDocument, command: &str, params: Value) -> Value {
    image.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn id(answer: &Value) -> u64 {
    answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer in {answer}"))
}

/// The names of `layers` (an inspect `layers` or `children` array), top first.
fn names(layers: &Value) -> Vec<String> {
    layers.as_array().unwrap().iter().map(|layer| layer["name"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn new_layers_made_with_a_group_active_go_into_the_group() {
    let mut image = PhotoDocument::create("groups.psd", 64, 64).unwrap();
    run(&mut image, "layer.new.group", json!({ "name": "Cards" }));
    run(&mut image, "layer.new.layer", json!({ "name": "Back" }));
    // The new layer is active now; the next one goes above it, inside the group.
    run(&mut image, "layer.new.layer", json!({ "name": "Front" }));
    let inspect = image.inspect().unwrap();
    assert_eq!(names(&inspect["layers"]), ["Cards", "Background"], "{inspect}");
    assert_eq!(names(&inspect["layers"][0]["children"]), ["Front", "Back"]);

    // A group made inside a group nests.
    run(&mut image, "layer.new.group", json!({ "name": "Inner" }));
    let inspect = image.inspect().unwrap();
    assert_eq!(names(&inspect["layers"][0]["children"]), ["Inner", "Front", "Back"]);
}

#[test]
fn every_kind_of_new_layer_goes_into_the_active_group() {
    let mut image = PhotoDocument::create("kinds.psd", 64, 64).unwrap();
    let group = id(&run(&mut image, "layer.new.group", json!({ "name": "Kinds" })));
    run(&mut image, "shape.create", json!({ "kind": "rect", "rect": [4, 4, 10, 10], "name": "Shape" }));
    run(&mut image, "layer.select", json!({ "layer": group }));
    run(&mut image, "type.create", json!({ "x": 4, "y": 30, "text": "Hi", "name": "Type" }));
    run(&mut image, "layer.select", json!({ "layer": group }));
    run(&mut image, "layer.newFillLayer.solidColor", json!({ "color": "#336699" }));
    run(&mut image, "layer.select", json!({ "layer": group }));
    run(&mut image, "layer.newAdjustmentLayer.invert", json!({}));
    let inspect = image.inspect().unwrap();
    let kinds = inspect["layers"].as_array().unwrap().iter().find(|layer| layer["name"] == "Kinds").unwrap();
    let mut inside: Vec<&str> = kinds["children"].as_array().unwrap().iter().map(|layer| layer["kind"].as_str().unwrap()).collect();
    inside.sort_unstable();
    assert_eq!(inside, ["Adjustment", "Fill", "Shape", "Type"], "{kinds}");

    // A duplicated group still goes above the group, not into it.
    let copy = id(&run(&mut image, "layer.duplicate", json!({ "layer": group })));
    let inspect = image.inspect().unwrap();
    let top: Vec<u64> = inspect["layers"].as_array().unwrap().iter().map(|layer| layer["id"].as_u64().unwrap()).collect();
    assert_eq!(&top[..2], [copy, group], "{inspect}");
}

#[test]
fn a_mask_from_the_selection_keeps_the_selection() {
    for command in ["layer.layerMask.revealSelection", "layer.layerMask.hideSelection"] {
        let mut image = PhotoDocument::create("masks.psd", 64, 64).unwrap();
        run(&mut image, "layer.new.layer", json!({ "name": "Ink" }));
        run(&mut image, "select.rect", json!({ "x": 8, "y": 8, "width": 20, "height": 20 }));
        run(&mut image, command, json!({}));
        let inspect = image.inspect().unwrap();
        assert_eq!(inspect["hasSelection"], true, "{command} dropped the selection");
        assert_eq!(inspect["selectionBounds"], json!([8, 8, 20, 20]), "{command}");
        assert_eq!(inspect["layers"][0]["hasMask"], true, "{command}");
    }
}

#[test]
fn duplicate_names_the_copy() {
    let mut image = PhotoDocument::create("copies.psd", 64, 64).unwrap();
    let ink = id(&run(&mut image, "layer.new.layer", json!({ "name": "Ink" })));
    let named = id(&run(&mut image, "layer.duplicate", json!({ "layer": ink, "name": "Ink shadow" })));
    let unnamed = id(&run(&mut image, "layer.duplicate", json!({ "layer": ink })));
    let inspect = image.inspect().unwrap();
    let name = |wanted: u64| inspect["layers"].as_array().unwrap().iter().find(|layer| layer["id"].as_u64() == Some(wanted)).unwrap()["name"].clone();
    assert_eq!(name(named), "Ink shadow");
    assert_eq!(name(unnamed), "Ink copy");
    let error = image.execute("layer.duplicate", json!({ "layer": ink, "name": 7 })).unwrap_err();
    assert!(error.contains("name"), "{error}");
}
