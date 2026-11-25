//! Regression: a shape layer's bounds are its drawn pixels, the same
//! width as asked for (`rect [200, 300, 600, 600]` is 600 wide, not 601), and a subtracted
//! sub-path outside the filled area does not widen them, on either side.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn run(image: &mut PhotoDocument, command: &str, params: Value) -> Value {
    image.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn bounds_of(image: &PhotoDocument, layer: &Value) -> Value {
    let layers = image.inspect().unwrap()["layers"].clone();
    layers.as_array().unwrap().iter().find(|l| l["id"] == *layer).unwrap_or_else(|| panic!("{layers}"))["bounds"].clone()
}

fn shape(kind: &str, colour: &str) -> (PhotoDocument, Value, Value) {
    let mut image = PhotoDocument::create("shapes.psd", 1400, 1000).unwrap();
    let mut params = json!({ "kind": kind, "rect": [200, 300, 600, 600], "fill": colour });
    if kind == "roundedRect" {
        params["radii"] = json!(40);
    }
    let made = run(&mut image, "shape.add", params);
    let layer = made["layer"].clone();
    (image, layer, made)
}

#[test]
fn shape_bounds_are_the_shape() {
    for kind in ["ellipse", "rect", "roundedRect"] {
        for colour in ["#e2466b", "#000000"] {
            let (image, layer, made) = shape(kind, colour);
            assert_eq!(made["bounds"], json!([200, 300, 600, 600]), "{kind} {colour}: {made}");
            assert_eq!(bounds_of(&image, &layer), json!([200, 300, 600, 600]), "{kind} {colour}");
        }
        // A subtracted circle sticking out on either side cuts, but adds no width.
        for rect in [json!([740, 500, 80, 80]), json!([180, 500, 80, 80])] {
            let (mut image, layer, _) = shape(kind, "#e2466b");
            let cut = run(&mut image, "shape.add", json!({ "kind": "ellipse", "rect": rect, "addTo": layer, "op": "subtract" }));
            assert_eq!(cut["bounds"], json!([200, 300, 600, 600]), "{kind} less {rect}: {}", cut["bounds"]);
            assert_eq!(bounds_of(&image, &layer), json!([200, 300, 600, 600]), "{kind} less {rect}");
        }
    }
}
