//! `document.text` (KitText 1.0): the words of an image's visible type layers in the Layers panel's
//! order, a part per layer, within the limit, with parts inside the text, changing nothing
//! (`kition_kit::testing::check_text`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use kition_kit::KitDocument;
use kition_kit::testing::check_text;
use kition_kit::text::{DEFAULT_LIMIT, DocumentText, TextPart};
use kition_photo_kit::PhotoDocument;

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn open(path: &str) -> PhotoDocument {
    let bytes = std::fs::read(repo().join(path)).unwrap_or_else(|error| panic!("{path}: {error}"));
    PhotoDocument::open(path, &bytes).unwrap()
}

fn part(label: &str, start: usize, end: usize) -> TextPart {
    TextPart { label: label.into(), start, end }
}

fn type_layer(image: &mut PhotoDocument, text: &str, name: &str) -> u64 {
    let made = image.execute("type.create", serde_json::json!({ "x": 10, "y": 40, "text": text, "name": name, "size": 12 })).unwrap();
    made["layer"].as_u64().unwrap_or_else(|| panic!("type.create answered {made}"))
}

/// Heading (bottom), a group holding Body, and a hidden Draft on top.
fn image() -> PhotoDocument {
    let mut image = PhotoDocument::create("untitled.psd", 200, 100).unwrap();
    type_layer(&mut image, "Grand Opening", "Heading");
    type_layer(&mut image, "Doors at 9\rFree entry", "Body");
    image.execute("layer.groupLayers", serde_json::json!({ "name": "Copy" })).unwrap();
    let draft = type_layer(&mut image, "not yet", "Draft");
    image.execute("layer.hideLayers", serde_json::json!({ "layer": draft })).unwrap();
    image
}

const WORDS: &str = "Doors at 9\nFree entry\nGrand Opening";

#[test]
fn visible_type_layers_are_read_top_to_bottom_a_part_each() {
    let image = image();
    let text = check_text(&image).unwrap();
    assert_eq!(text, DocumentText { text: WORDS.into(), truncated: false, parts: vec![part("Layer: Body", 0, 21), part("Layer: Heading", 22, 35)] });
}

#[test]
fn a_saved_type_layer_answers_its_text() {
    let bytes = image().save(Some("words.psd")).unwrap();
    let reopened = PhotoDocument::open("words.psd", &bytes).unwrap();
    assert_eq!(check_text(&reopened).unwrap().text, WORDS);
    let substituted = check_text(&open("conformance/check-font-substituted.kphoto")).unwrap();
    assert_eq!(substituted, DocumentText { text: "Title".into(), truncated: false, parts: vec![part("Layer: Title", 0, 5)] });
}

#[test]
fn the_limit_truncates_by_characters() {
    let image = image();
    let cut = image.text(15).unwrap();
    assert_eq!(cut, DocumentText { text: "Doors at 9\nFree".into(), truncated: true, parts: vec![part("Layer: Body", 0, 15)] });
    let exact = image.text(WORDS.chars().count()).unwrap();
    assert_eq!((exact.text.as_str(), exact.truncated), (WORDS, false));
}

#[test]
fn every_fixture_passes_the_text_check() {
    for entry in std::fs::read_dir(repo().join("conformance")).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        if !(name.ends_with(".psd") || name.ends_with(".kphoto")) {
            continue;
        }
        let bytes = std::fs::read(repo().join("conformance").join(&name)).unwrap();
        let Ok(image) = PhotoDocument::open(&name, &bytes) else { continue };
        check_text(&image).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn reading_changes_nothing() {
    let image = image();
    let (revision, dirty, bytes) = (image.revision(), image.dirty(), image.save(None).unwrap());
    let state = image.inspect().unwrap();
    image.text(DEFAULT_LIMIT).unwrap();
    image.text(3).unwrap();
    assert_eq!((image.revision(), image.dirty()), (revision, dirty));
    assert_eq!(image.save(None).unwrap(), bytes);
    assert_eq!(image.inspect().unwrap(), state);
}

#[test]
fn an_image_without_type_has_no_text() {
    for image in [PhotoDocument::create("untitled.psd", 64, 64).unwrap(), open("conformance/basic.psd")] {
        assert_eq!(check_text(&image).unwrap(), DocumentText::default());
    }
}
