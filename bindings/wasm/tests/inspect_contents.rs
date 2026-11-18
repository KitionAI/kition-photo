//! `inspect` shows what fill and shape layers draw and where, and
//! every command answers bounds in the same [x, y, width, height] form `inspect` uses.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn run(image: &mut PhotoDocument, command: &str, params: Value) -> Value {
    image.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn layer(image: &PhotoDocument, id: &Value) -> Value {
    fn find(layers: &Value, id: &Value) -> Option<Value> {
        layers.as_array()?.iter().find_map(|layer| if &layer["id"] == id { Some(layer.clone()) } else { find(&layer["children"], id) })
    }
    let inspect = image.inspect().unwrap();
    find(&inspect["layers"], id).unwrap_or_else(|| panic!("no layer {id} in {inspect}"))
}

#[test]
fn fill_layers_show_their_fill_and_bounds() {
    let mut image = PhotoDocument::create("fills.psd", 64, 48).unwrap();
    let solid = run(&mut image, "layer.newFillLayer.solidColor", json!({ "color": "#336699" }))["layer"].clone();
    let shown = layer(&image, &solid);
    assert_eq!(shown["fillContent"], "#336699", "{shown}");
    assert_eq!(shown["bounds"], json!([0, 0, 64, 48]), "a fill layer covers the canvas: {shown}");

    let gradient =
        run(&mut image, "layer.newFillLayer.gradient", json!({ "from": "#000000", "to": "#ffffff", "angle": 0, "style": "radial" }))["layer"].clone();
    let shown = layer(&image, &gradient);
    let fill = &shown["fillContent"]["gradient"];
    assert_eq!(fill["stops"], json!([[0.0, "#000000"], [1.0, "#ffffff"]]), "{shown}");
    assert_eq!(fill["style"], "radial", "{shown}");

    // Masked by the selection, or by a path as its vector mask: the bounds are the mask's.
    run(&mut image, "select.rect", json!({ "x": 8, "y": 4, "width": 20, "height": 10 }));
    let masked = run(&mut image, "layer.newFillLayer.solidColor", json!({ "color": "#ff0000" }))["layer"].clone();
    assert_eq!(layer(&image, &masked)["bounds"], json!([8, 4, 20, 10]));
    run(&mut image, "select.deselect", json!({}));
    let path = json!({ "subpaths": [{ "knots": [[10, 20], [40, 20], [40, 30], [10, 30]] }] });
    let shaped = run(&mut image, "layer.newFillLayer.solidColor", json!({ "color": "#00ff00", "path": path }))["layer"].clone();
    assert_eq!(layer(&image, &shaped)["bounds"], json!([10, 20, 30, 10]));
}

#[test]
fn shape_layers_show_their_fill_and_stroke() {
    let mut image = PhotoDocument::create("shapes.psd", 64, 48).unwrap();
    let made =
        run(&mut image, "shape.create", json!({ "kind": "ellipse", "rect": [4, 4, 30, 20], "fill": "#ff0000", "stroke": { "width": 2, "color": "#0000ff" } }));
    let shown = layer(&image, &made["layer"]);
    assert_eq!(shown["shape"]["fill"], "#ff0000", "{shown}");
    assert_eq!(shown["shape"]["stroke"]["fill"], "#0000ff", "{shown}");
    assert_eq!(shown["shape"]["kind"], "ellipse", "{shown}");
    assert_eq!(shown["bounds"], made["bounds"], "shape.create and inspect agree");
}

#[test]
fn commands_answer_bounds_as_inspect_does() {
    let mut image = PhotoDocument::create("bounds.psd", 200, 160).unwrap();
    // A render filter on a new layer.
    let drawn = run(&mut image, "filter.render.pictureFrame", json!({ "frame": "dots", "newLayer": true }));
    assert_eq!(drawn["bounds"], layer(&image, &drawn["layer"])["bounds"], "{drawn}");
    // Type: type.create and type.info.
    let typed = run(&mut image, "type.create", json!({ "x": 20, "y": 60, "text": "Bounds", "size": 24 }));
    let shown = layer(&image, &typed["layer"]);
    assert_eq!(typed["bounds"], shown["bounds"], "type.create {typed} vs inspect {shown}");
    let info = run(&mut image, "type.info", json!({ "layer": typed["layer"] }));
    assert_eq!(info["bounds"], shown["bounds"], "type.info vs inspect");
}
