//! Command-level round trips: a new document is edited only with engine commands, each edit is
//! checked, then the document is saved as PSD, reopened, and compared whole: the inspected
//! document, every type layer's text and style (`type.info`), and the composite's pixels. A second
//! save of the reopened file must reopen the same again.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const W: u32 = 64;
const H: u32 = 48;

fn new_doc() -> PhotoDocument {
    PhotoDocument::create("art.psd", W, H).unwrap()
}

fn run(doc: &mut PhotoDocument, command: &str, params: Value) -> Value {
    doc.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn inspect(doc: &PhotoDocument) -> Value {
    doc.inspect().unwrap()
}

fn active(doc: &PhotoDocument) -> i64 {
    inspect(doc)["activeLayer"].as_i64().unwrap()
}

fn layer<'a>(state: &'a Value, name: &str) -> &'a Value {
    state["layers"].as_array().unwrap().iter().find(|layer| layer["name"] == name).unwrap_or_else(|| panic!("no layer {name}: {state}"))
}

fn composite(doc: &PhotoDocument) -> Vec<u8> {
    doc.render(0.0, 0.0, 1.0, W, H).unwrap()
}

/// What a saved file keeps: the undo history, the revision counter and the session's selection
/// state belong to the open session, not the file.
fn lasting(doc: &mut PhotoDocument) -> Value {
    let mut state = inspect(doc);
    let object = state.as_object_mut().unwrap();
    for key in ["history", "historyIndex", "historyRedo", "canUndo", "canRedo", "revision"] {
        object.remove(key);
    }
    let types: Vec<Value> = object["layers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|layer| layer["kind"] == "Type")
        .map(|layer| run(doc, "type.info", json!({ "layer": layer["id"] })))
        .collect();
    let mut types = Value::Array(types);
    engine_data_precision(&mut types);
    object.insert("typeLayers".into(), types);
    // A PSD placed layer is read back `linked` to the uuid of the file the PSD carries for it,
    // embedded or not (see `smartSource` in the engine's inspect.rs); the contents are compared
    // by `smart_objects_and_fill_layers_survive_save_and_reopen` instead.
    for layer in object["layers"].as_array_mut().into_iter().flatten() {
        layer.as_object_mut().map(|layer| layer.remove("smartSource"));
    }
    // Layer ids number a session's layers; a reopened file numbers them afresh. Each id is
    // replaced by its layer's position in the panel.
    let ids: Vec<i64> = object["layers"].as_array().unwrap().iter().map(|layer| layer["id"].as_i64().unwrap()).collect();
    let position = |id: &Value| id.as_i64().and_then(|id| ids.iter().position(|known| *known == id)).map_or(Value::Null, |at| json!(at));
    renumber(&mut state, &position);
    state
}

/// Type layers are stored in Photoshop's EngineData, which writes numbers with five decimals, and
/// whose font set names every run's font by its PostScript name: a run that named none gets the
/// one its family and weight give (both still compared).
fn engine_data_precision(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("postscript_name");
            object.values_mut().for_each(engine_data_precision);
        }
        Value::Array(items) => items.iter_mut().for_each(engine_data_precision),
        Value::Number(number) if number.is_f64() => *value = json!((number.as_f64().unwrap() * 1e5).round() / 1e5),
        _ => {}
    }
}

fn renumber(value: &mut Value, position: &dyn Fn(&Value) -> Value) {
    match value {
        Value::Object(object) => {
            for (key, field) in object.iter_mut() {
                match key.as_str() {
                    "id" | "activeLayer" | "layer" | "parent" => *field = position(field),
                    // PSD stores a layer's opacity and fill in one byte each (0 to 255).
                    "opacity" | "fill" if field.is_f64() => *field = json!((field.as_f64().unwrap() * 255.0).round() as i64),
                    "selectedLayers" | "layerIds" | "children" => {
                        for id in field.as_array_mut().into_iter().flatten() {
                            *id = position(id);
                        }
                    }
                    _ => renumber(field, position),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| renumber(item, position)),
        _ => {}
    }
}

fn differences(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|key| !x.contains_key(*key))) {
                differences(&format!("{path}/{key}"), x.get(key).unwrap_or(&Value::Null), y.get(key).unwrap_or(&Value::Null), out);
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (at, (x, y)) in x.iter().zip(y).enumerate() {
                differences(&format!("{path}/{at}"), x, y, out);
            }
        }
        _ if a != b => out.push(format!("{path}: {a} != {b}")),
        _ => {}
    }
}

fn reopen(doc: &PhotoDocument) -> PhotoDocument {
    let bytes = doc.save(Some("art.psd")).unwrap();
    PhotoDocument::open("art.psd", &bytes).unwrap()
}

fn assert_round_trip(doc: &mut PhotoDocument) {
    let mut reopened = reopen(doc);
    let (before, after) = (lasting(doc), lasting(&mut reopened));
    let mut changed = Vec::new();
    differences("", &before, &after, &mut changed);
    assert!(changed.is_empty(), "the document after saving and reopening:\n{}", changed.join("\n"));
    assert!(composite(&reopened) == composite(doc), "the composite after saving and reopening");
    let mut again = reopen(&reopened);
    assert_eq!(lasting(&mut again), before, "a second save of the reopened file");
}

#[test]
fn layers_survive_save_and_reopen() {
    let mut doc = new_doc();
    run(&mut doc, "layer.new.layer", json!({ "name": "Ink" }));
    let ink = active(&doc);
    run(&mut doc, "paint.stroke", json!({ "points": [[4, 4, 1], [40, 30, 1]], "size": 6, "color": "#ff0000" }));
    run(&mut doc, "layer.setProps", json!({ "layer": ink, "opacity": 0.6, "blend": "Multiply", "name": "Ink line" }));
    run(&mut doc, "layer.setLabelColor", json!({ "layer": ink, "color": "red" }));
    run(&mut doc, "layer.duplicate", json!({ "layer": ink }));
    let copy = active(&doc);
    run(&mut doc, "layer.setProps", json!({ "layer": copy, "visible": false, "name": "Hidden copy", "fill": 0.5 }));
    run(&mut doc, "layer.new.group", json!({ "name": "Group" }));
    let state = inspect(&doc);
    assert_eq!(layer(&state, "Ink line")["blend"], "Multiply");
    assert_eq!(layer(&state, "Ink line")["labelColor"], "red");
    assert_eq!(layer(&state, "Hidden copy")["visible"], false);
    assert!(state["layers"].as_array().unwrap().iter().any(|layer| layer["name"] == "Group"));
    assert_round_trip(&mut doc);
}

fn painted(doc: &mut PhotoDocument, name: &str, color: &str) -> i64 {
    run(doc, "layer.new.layer", json!({ "name": name }));
    let id = active(doc);
    run(doc, "paint.stroke", json!({ "points": [[6, 6, 1], [58, 40, 1]], "size": 10, "color": color }));
    id
}

#[test]
fn adjustment_layers_survive_save_and_reopen() {
    let mut doc = new_doc();
    painted(&mut doc, "Paint", "#3366cc");
    let before = composite(&doc);
    run(&mut doc, "layer.newAdjustmentLayer.levels", json!({ "gamma": 1.6, "inBlack": 20, "outWhite": 230 }));
    run(&mut doc, "layer.newAdjustmentLayer.hueSaturation", json!({ "hue": 40, "saturation": 25 }));
    let state = inspect(&doc);
    assert_eq!(state["layers"].as_array().unwrap().iter().filter(|layer| layer["kind"] == "Adjustment").count(), 2, "{state}");
    assert!(composite(&doc) != before, "the adjustments change the picture");
    assert_round_trip(&mut doc);
}

#[test]
fn masks_and_clipping_survive_save_and_reopen() {
    let mut doc = new_doc();
    let base = painted(&mut doc, "Base", "#22aa44");
    run(&mut doc, "select.rect", json!({ "x": 10, "y": 8, "width": 30, "height": 20 }));
    run(&mut doc, "layer.layerMask.revealSelection", json!({ "layer": base }));
    run(&mut doc, "select.deselect", json!({}));
    let clipped = painted(&mut doc, "Clipped", "#ffcc00");
    run(&mut doc, "layer.createClippingMask", json!({ "layer": clipped }));
    let state = inspect(&doc);
    assert_eq!(layer(&state, "Base")["hasMask"], true);
    assert_eq!(layer(&state, "Clipped")["clipped"], true);
    assert_round_trip(&mut doc);
}

#[test]
fn type_layers_survive_save_and_reopen() {
    let mut doc = new_doc();
    run(&mut doc, "type.create", json!({ "text": "Kition 中文 ✓", "x": 4, "y": 30, "size": 14, "color": "#aa2200", "name": "Title" }));
    let title = active(&doc);
    let info = run(&mut doc, "type.info", json!({ "layer": title }));
    assert!(info.to_string().contains("Kition 中文 ✓"), "{info}");
    assert_eq!(layer(&inspect(&doc), "Title")["kind"], "Type");
    assert_round_trip(&mut doc);
}

#[test]
fn layer_styles_survive_save_and_reopen() {
    let mut doc = new_doc();
    let shape = painted(&mut doc, "Shape", "#8844ff");
    let before = composite(&doc);
    run(&mut doc, "layer.layerStyle.dropShadow", json!({ "layer": shape, "distance": 4, "size": 3, "color": "#000000", "opacity": 60 }));
    run(&mut doc, "layer.layerStyle.stroke", json!({ "layer": shape, "size": 2, "color": "#ffffff", "position": "outside" }));
    assert!(composite(&doc) != before, "the styles change the picture");
    assert_round_trip(&mut doc);
}

#[test]
fn smart_objects_and_fill_layers_survive_save_and_reopen() {
    let mut doc = new_doc();
    let art = painted(&mut doc, "Art", "#cc0066");
    run(&mut doc, "layer.objectLayers.convertToObjectLayer", json!({ "layer": art }));
    run(&mut doc, "layer.newFillLayer.solidColor", json!({ "color": "#204060" }));
    let fill = active(&doc);
    run(&mut doc, "layer.setProps", json!({ "layer": fill, "opacity": 0.4 }));
    let state = inspect(&doc);
    assert_eq!(layer(&state, "Art")["kind"], "Object", "{state}");
    assert_round_trip(&mut doc);
    // The object layer's contents survive: converted back to layers, the reopened file gives the
    // same layers and picture as the document it was saved from.
    let mut reopened = reopen(&doc);
    let id = |doc: &PhotoDocument| layer(&inspect(doc), "Art")["id"].clone();
    let (original, saved) = (id(&doc), id(&reopened));
    run(&mut doc, "layer.objectLayers.convertToLayers", json!({ "layer": original }));
    run(&mut reopened, "layer.objectLayers.convertToLayers", json!({ "layer": saved }));
    let mut changed = Vec::new();
    differences("", &lasting(&mut doc), &lasting(&mut reopened), &mut changed);
    assert!(changed.is_empty(), "the object layer's contents:\n{}", changed.join("\n"));
    assert!(composite(&reopened) == composite(&doc), "the converted contents' picture");
}
