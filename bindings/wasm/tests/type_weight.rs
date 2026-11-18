//! Regression: inspect gives a type layer's weight, and a weight no
//! font here has (Inter Bold: the built-in Inter is Regular, Medium and SemiBold) is reported,
//! with the weight it is drawn at, until the host adds a font that has it. A test binary of its
//! own: fonts are the process's.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn run(image: &mut PhotoDocument, command: &str, params: Value) -> Value {
    image.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

fn weight_reports(image: &PhotoDocument) -> Vec<String> {
    image.check().into_iter().filter(|d| d.code == "photo.font-substituted" && d.message.contains("weight")).map(|d| d.message).collect()
}

#[test]
fn a_weight_no_font_has_is_reported_until_one_is_added() {
    let mut image = PhotoDocument::create("label.psd", 400, 120).unwrap();
    let made = run(&mut image, "type.create", json!({ "x": 10, "y": 80, "text": "Budget", "font": "Inter", "size": 40, "weight": 700 }));
    let layers = image.inspect().unwrap()["layers"].clone();
    let layer = layers.as_array().unwrap().iter().find(|l| l["id"] == made["layer"]).cloned().unwrap();
    assert_eq!(layer["text"]["weight"], json!(700), "{layer}");
    let reports = weight_reports(&image);
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].contains("700") && reports[0].contains("600"), "names the weight asked for and the one drawn: {reports:?}");

    // The host adds Inter's variable font: every weight draws, nothing to report.
    let dir = std::env::var_os("KITION_FONTS_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../kition-fonts")));
    let inter = std::fs::read(dir.join("fonts/inter/Inter-Variable.ttf")).expect("needs the kition-fonts checkout");
    kition_photo_kit::add_font(&inter);
    assert!(weight_reports(&image).is_empty(), "{:?}", weight_reports(&image));
}
