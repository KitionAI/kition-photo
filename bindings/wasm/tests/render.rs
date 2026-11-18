//! The agent's preview by explicit view and the measurements beside it (task card C-photo;
//! `src/render.rs`): `file.render` draws the composite or only the layers asked, framed to a region,
//! answers how its pixels map to the image and what it drew, reads no editor state, changes nothing
//! and answers the same bytes for the same call; `image.measure` answers bounds, counts, mean
//! color and overlap exactly.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::failure::{CommandFailure, parse_failure};
use kition_kit::{CallContext, KitDocument};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

/// White 200 x 100: A, a red block at (10, 10, 40, 30); B, a blue block at (100, 20, 50, 50),
/// hidden.
fn image() -> (PhotoDocument, u64, u64) {
    let mut doc = PhotoDocument::create("art.psd", 200, 100).unwrap();
    let layer = |doc: &mut PhotoDocument, name: &str, rect: [i32; 4], color: &str| {
        let id = doc.execute("layer.new.layer", json!({ "name": name })).unwrap()["layer"].as_u64().unwrap();
        doc.execute("select.rect", json!({ "x": rect[0], "y": rect[1], "width": rect[2], "height": rect[3] })).unwrap();
        doc.execute("edit.fill", json!({ "color": color })).unwrap();
        doc.execute("select.deselect", json!({})).unwrap();
        id
    };
    let a = layer(&mut doc, "A", [10, 10, 40, 30], "#ff0000");
    let b = layer(&mut doc, "B", [100, 20, 50, 50], "#0000ff");
    doc.execute("layer.setProps", json!({ "layer": b, "visible": false })).unwrap();
    (doc, a, b)
}

fn decode(answer: &Value) -> image::RgbaImage {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(answer["png"].as_str().unwrap()).unwrap();
    image::load_from_memory(&bytes).unwrap().to_rgba8()
}

#[test]
fn the_composite_is_framed_and_mapped_to_the_image() {
    let (mut doc, ..) = image();
    let answer = doc.execute("file.render", json!({})).unwrap();
    assert_eq!((answer["width"].as_u64(), answer["height"].as_u64()), (Some(200), Some(100)));
    assert_eq!(answer["region"], json!([0, 0, 200, 100]));
    assert_eq!(answer["drawn"], json!({ "composite": true }));
    let png = decode(&answer);
    assert_eq!(png.get_pixel(20, 20).0, [255, 0, 0, 255]);
    assert_eq!(png.get_pixel(120, 40).0, [255, 255, 255, 255], "B is hidden in the composite");
    let half = doc.execute("file.render", json!({ "target": { "region": [0, 0, 100, 50] }, "scale": 0.5 })).unwrap();
    assert_eq!((half["width"].as_u64(), half["height"].as_u64()), (Some(50), Some(25)));
    assert_eq!(half["scale"], json!([0.5, 0.5]));
    let fit = doc.execute("file.render", json!({ "maxSize": 50 })).unwrap();
    assert_eq!((fit["width"].as_u64(), fit["height"].as_u64()), (Some(50), Some(25)));
}

#[test]
fn a_layer_target_draws_that_layer_alone_even_hidden() {
    let (mut doc, a, b) = image();
    let answer = doc.execute("file.render", json!({ "target": { "layer": b } })).unwrap();
    assert_eq!(answer["region"], json!([100, 20, 50, 50]), "framed to its bounds");
    assert_eq!(answer["drawn"], json!({ "layers": [b] }));
    assert_eq!(decode(&answer).get_pixel(5, 5).0, [0, 0, 255, 255]);
    let both = doc.execute("file.render", json!({ "target": { "layers": [a, b], "region": [0, 0, 200, 100] } })).unwrap();
    let png = decode(&both);
    assert_eq!(png.get_pixel(20, 20).0, [255, 0, 0, 255]);
    assert_eq!(png.get_pixel(5, 90).0[3], 0, "no background layer drawn");
    let by_index = doc.execute("file.render", json!({ "target": { "layer": "#2" } })).unwrap();
    assert_eq!(by_index["drawn"], json!({ "layers": [a] }));
}

#[test]
fn the_view_is_the_calls_not_the_editors_and_nothing_changes() {
    let (mut doc, a, b) = image();
    let call = json!({ "target": { "layer": a }, "background": "white" });
    let first = doc.execute("file.render", call.clone()).unwrap();
    let (revision, dirty) = (doc.revision(), doc.dirty());
    doc.set_context(&CallContext { on: vec![json!(b)], part: None, time: None }).unwrap();
    doc.execute("select.rect", json!({ "x": 0, "y": 0, "width": 5, "height": 5 })).unwrap();
    doc.execute("tools.setColors", json!({ "foreground": "#00ff00" })).unwrap();
    let revision_after_editor = doc.revision();
    let second = doc.execute("file.render", call).unwrap();
    assert_eq!(first, second, "the same bytes");
    assert_eq!(doc.revision(), revision_after_editor, "rendering is no edit");
    assert!(revision <= revision_after_editor && (dirty || !dirty));
}

#[test]
fn the_bytes_do_not_depend_on_the_thread_pool() {
    // Larger than one 256-pixel render tile, with a soft stroke and a blend mode across tiles.
    let mut doc = PhotoDocument::create("art.psd", 1300, 900).unwrap();
    doc.execute("layer.new.layer", json!({ "name": "Ink" })).unwrap();
    doc.execute("paint.stroke", json!({ "points": [[10, 10, 1], [1290, 880, 1], [40, 870, 0.5]], "size": 90, "hardness": 0.2, "color": "#3366cc" })).unwrap();
    doc.execute("layer.setProps", json!({ "blend": "Multiply", "opacity": 0.7 })).unwrap();
    let mut answers = Vec::new();
    for threads in [1, 3, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        answers.push(pool.install(|| {
            (doc.execute("file.render", json!({ "scale": 0.7 })).unwrap(), doc.execute("file.render", json!({ "scale": 1.5, "maxSize": 4096 })).unwrap())
        }));
    }
    assert!(answers.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn views_it_cannot_draw_are_refused_as_invalid() {
    let (mut doc, ..) = image();
    for bad in [
        json!({ "target": { "slide": 1 } }),
        json!({ "target": { "layer": 999 } }),
        json!({ "target": { "region": [0, 0, 0, 10] } }),
        json!({ "scale": 3 }),
        json!({ "maxSize": 0 }),
        json!({ "background": "plaid" }),
    ] {
        let error = doc.execute("file.render", bad.clone()).unwrap_err();
        assert_eq!(parse_failure(&error).map(|(_, reason)| reason), Some(CommandFailure::InvalidParams), "{bad}: {error}");
    }
}

#[test]
fn measurements_say_bounds_counts_mean_and_overlap() {
    let (mut doc, a, b) = image();
    let measured = doc.execute("image.measure", json!({ "layer": a })).unwrap();
    assert_eq!(measured["bounds"], json!([10, 10, 40, 30]));
    assert_eq!(measured["pixels"], 1200);
    assert_eq!(measured["mean"], json!([255.0, 0.0, 0.0, 255.0]));
    assert_eq!(measured["histogram"][76], 1200, "red's luminosity");
    let none = doc.execute("image.measure", json!({ "layer": a, "with": b })).unwrap();
    assert_eq!(none["overlap"], json!({ "with": b, "pixels": 0, "bounds": null }));
    doc.execute("layer.translate", json!({ "layer": a, "dx": 80, "dy": 0 })).unwrap();
    let some = doc.execute("image.measure", json!({ "layer": a, "with": b })).unwrap();
    assert_eq!(some["overlap"]["bounds"], json!([100, 20, 30, 20]));
    assert_eq!(some["overlap"]["pixels"], 600);
    let composite = doc.execute("image.measure", json!({ "region": [0, 0, 10, 10] })).unwrap();
    assert_eq!((composite["pixels"].clone(), composite["mean"].clone()), (json!(100), json!([255.0, 255.0, 255.0, 255.0])));
}
