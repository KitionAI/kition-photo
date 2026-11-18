//! Shape layers built from several shapes: `shape.create` adds to an
//! existing shape layer through `addTo` (an id with `op`, or `{layer, op}`), an even-odd fill rule
//! cuts holes, and the bounds are exactly the drawn pixels.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn run(image: &mut PhotoDocument, command: &str, params: Value) -> Value {
    image.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn layer_count(image: &PhotoDocument) -> usize {
    image.inspect().unwrap()["layers"].as_array().unwrap().len()
}

/// The pixel at (x, y) of the image (RGBA).
fn pixel(image: &PhotoDocument, x: u32, y: u32) -> [u8; 4] {
    let rgba = image.render(f64::from(x), f64::from(y), 1.0, 1, 1).unwrap();
    [rgba[0], rgba[1], rgba[2], rgba[3]]
}

#[test]
fn add_to_takes_the_object_form() {
    for (add_to, op) in [(json!({ "op": "subtract" }), None), (Value::Null, Some("subtract"))] {
        let mut image = PhotoDocument::create("shapes.psd", 64, 64).unwrap();
        let made = run(&mut image, "shape.create", json!({ "kind": "rect", "rect": [0, 0, 40, 40], "fill": "#ff0000" }));
        let id = made["layer"].clone();
        let layers = layer_count(&image);
        let mut params = json!({ "kind": "rect", "rect": [10, 10, 10, 10] });
        if let Some(op) = op {
            params["addTo"] = id.clone();
            params["op"] = json!(op);
        } else {
            let mut object = add_to.clone();
            object["layer"] = id.clone();
            params["addTo"] = object;
        }
        let added = run(&mut image, "shape.create", params);
        assert_eq!(added["layer"], id, "added to the same layer: {added}");
        assert_eq!(layer_count(&image), layers, "no new layer");
        assert_eq!(added["path"]["subpaths"].as_array().unwrap().len(), 2, "{added}");
        assert_eq!(added["path"]["subpaths"][1]["op"], "subtract", "{added}");
        assert_eq!(pixel(&image, 15, 15), [255, 255, 255, 255], "the subtracted square is a hole");
        assert_eq!(pixel(&image, 5, 5), [255, 0, 0, 255]);
    }
}

#[test]
fn even_odd_cuts_holes() {
    let mut image = PhotoDocument::create("donut.psd", 64, 64).unwrap();
    let path = json!({ "fillRule": "evenodd", "subpaths": [
        { "knots": [[0, 0], [40, 0], [40, 40], [0, 40]] },
        { "knots": [[10, 10], [30, 10], [30, 30], [10, 30]] },
    ] });
    let made = run(&mut image, "shape.create", json!({ "kind": "path", "path": path, "fill": "#0000ff" }));
    assert_eq!(made["path"]["fillRule"], "evenodd");
    assert_eq!(pixel(&image, 20, 20), [255, 255, 255, 255], "even-odd: the inner square is a hole");
    assert_eq!(pixel(&image, 5, 5), [0, 0, 255, 255]);

    // shape.edit switches the rule of an existing shape.
    let plain = json!({ "subpaths": [
        { "knots": [[0, 0], [40, 0], [40, 40], [0, 40]] },
        { "knots": [[10, 10], [30, 10], [30, 30], [10, 30]] },
    ] });
    let mut image = PhotoDocument::create("donut.psd", 64, 64).unwrap();
    let id = run(&mut image, "shape.create", json!({ "kind": "path", "path": plain, "fill": "#0000ff" }))["layer"].clone();
    assert_eq!(pixel(&image, 20, 20), [0, 0, 255, 255], "non-zero: filled");
    run(&mut image, "shape.edit", json!({ "layer": id, "fillRule": "evenodd" }));
    assert_eq!(pixel(&image, 20, 20), [255, 255, 255, 255], "even-odd after shape.edit: a hole");
}

#[test]
fn bounds_are_the_drawn_pixels() {
    let mut image = PhotoDocument::create("bounds.psd", 64, 64).unwrap();
    let made = run(&mut image, "shape.create", json!({ "kind": "rect", "rect": [4, 6, 20, 10], "fill": "#000000" }));
    assert_eq!(made["bounds"], json!([4, 6, 20, 10]), "{made}");
    // Subtracting the right half leaves the left half.
    let cut = run(&mut image, "shape.create", json!({ "kind": "rect", "rect": [14, 0, 40, 40], "addTo": { "layer": made["layer"], "op": "subtract" } }));
    assert_eq!(cut["bounds"], json!([4, 6, 10, 10]), "{cut}");
    let inspect = image.inspect().unwrap();
    assert_eq!(inspect["layers"][0]["bounds"], json!([4, 6, 10, 10]));
}
