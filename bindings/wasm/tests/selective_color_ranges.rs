//! Selective Color takes every colour range its description lists:
//! `reds`, `yellows`, `greens`, `cyans`, `blues`, `magentas`, `whites`, `neutrals` and `blacks`,
//! each as [cyan, magenta, yellow, black] in percent, on the adjustment layer and on pixels.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const RANGES: [&str; 9] = ["reds", "yellows", "greens", "cyans", "blues", "magentas", "whites", "neutrals", "blacks"];

fn ranges() -> Value {
    let mut params = json!({ "method": "absolute" });
    for (i, range) in RANGES.iter().enumerate() {
        let i = i as i64;
        params[*range] = json!([i, -i, 2 * i, -2 * i]);
    }
    params
}

#[test]
fn the_adjustment_layer_takes_every_range() {
    let mut image = PhotoDocument::create("ranges.psd", 32, 32).unwrap();
    let made = image.execute("layer.newAdjustmentLayer.selectiveColor", ranges()).unwrap_or_else(|error| panic!("{error}"));
    let id = made["layer"].as_u64().unwrap_or_else(|| panic!("answered {made}"));
    let inspect = image.inspect().unwrap();
    let layer = inspect["layers"].as_array().unwrap().iter().find(|layer| layer["id"].as_u64() == Some(id)).unwrap();
    let params = &layer["adjustmentParams"];
    for (i, range) in RANGES.iter().enumerate() {
        let i = i as f64;
        assert_eq!(params[*range], json!([i, -i, 2.0 * i, -2.0 * i]), "{range} in {params}");
    }
    assert_eq!(params["method"], "absolute", "{params}");
}

#[test]
fn the_destructive_adjustment_takes_every_range() {
    let mut image = PhotoDocument::create("ranges.psd", 32, 32).unwrap();
    image.execute("layer.new.layer", json!({})).unwrap();
    image.execute("edit.fill", json!({ "color": "#3366cc" })).unwrap();
    image.execute("image.adjustments.selectiveColor", ranges()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn an_unknown_range_is_still_refused() {
    let mut image = PhotoDocument::create("ranges.psd", 32, 32).unwrap();
    let error = image.execute("layer.newAdjustmentLayer.selectiveColor", json!({ "purples": [0, 0, 0, 0] })).unwrap_err();
    assert!(error.contains("purples"), "{error}");
}
