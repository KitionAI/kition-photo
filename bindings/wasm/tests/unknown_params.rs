//! A command with a parameter schema refuses a key the schema does not name, and says which
//! (platform rule E7a, kit requirement AX15): an agent that misspells a parameter is told instead
//! of the command running without it.
#![allow(clippy::unwrap_used, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

fn doc() -> PhotoDocument {
    PhotoDocument::create("Image.psd", 64, 48).unwrap()
}

#[test]
fn every_command_refuses_an_unknown_parameter() {
    if let Err(problems) = kition_kit::testing::check_unknown_params(doc, kition_photo_kit::params::OPEN) {
        panic!("{} problems:\n{}", problems.len(), problems.join("\n"));
    }
}

#[test]
fn a_misspelled_parameter_names_the_one_it_resembles() {
    let error = doc().execute("layer.new.layer", json!({ "nmae": "A" })).unwrap_err();
    assert!(error.contains("`nmae`") && error.contains("did you mean `name`"), "{error}");
}

#[test]
fn keys_the_session_and_commands_read_beyond_their_notation_are_taken() {
    let mut image = doc();
    // `coalesce` (every command), `target` (a routed command), a slice's options (named in its
    // notation only), and `layer` on a layer style effect.
    image.execute("image.adjustments.invert", json!({ "target": "pixels", "coalesce": "k" })).unwrap();
    image.execute("slice.new", json!({ "rect": [0, 0, 8, 8], "name": "a", "alt": "A", "url": "https://example.com" })).unwrap();
    let layer = image.execute("layer.new.layer", json!({ "name": "A" })).unwrap()["layer"].clone();
    image.execute("layer.layerStyle.stroke", json!({ "layer": layer, "size": 2 })).unwrap();
    // A new type layer takes the Type tool's options, which are character and paragraph keys.
    image.execute("text.add", json!({ "x": 4, "y": 20, "text": "A", "font": "Arial", "fontStyle": "Bold", "size": 12, "align": "left" })).unwrap();
}
