//! Paragraph text (#3): hyphenation breaks long words in a justified column, a `box` that is not
//! `[x, y, w, h]` is refused rather than ignored, and `text.info` answers one entry per paragraph.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const WORDS: &str = "Internationalization considerations notwithstanding, extraordinarily comprehensive documentation characteristically accompanies institutional responsibilities.";

fn image() -> PhotoDocument {
    PhotoDocument::create("column.psd", 400, 400).unwrap()
}

fn column(doc: &mut PhotoDocument, hyphenate: bool) -> Value {
    let made = doc.execute("text.add", json!({ "box": [10, 10, 130, 380], "text": WORDS, "size": 12, "align": "justify", "hyphenate": hyphenate })).unwrap();
    doc.execute("text.info", json!({ "layer": made["layer"] })).unwrap()
}

/// Lines (as char ranges) that end inside a word.
fn split_words(info: &Value) -> Vec<String> {
    let chars: Vec<char> = info["text"].as_str().unwrap().chars().collect();
    info["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|line| {
            let end = line["end"].as_u64().unwrap() as usize;
            let (Some(before), Some(after)) = (end.checked_sub(1).and_then(|i| chars.get(i)), chars.get(end)) else { return None };
            (before.is_alphabetic() && after.is_alphabetic()).then(|| chars[line["start"].as_u64().unwrap() as usize..end].iter().collect())
        })
        .collect()
}

#[test]
fn hyphenation_breaks_long_words_in_a_justified_column() {
    let mut doc = image();
    let plain = column(&mut doc, false);
    assert!(split_words(&plain).is_empty(), "without hyphenation lines end between words: {plain}");
    let hyphenated = column(&mut doc, true);
    assert!(!split_words(&hyphenated).is_empty(), "no line ends inside a word: {}", hyphenated["lines"]);
    // The text itself is unchanged: hyphens are drawn, not inserted.
    assert_eq!(hyphenated["text"], WORDS);
    let ends: Vec<u64> = hyphenated["lines"].as_array().unwrap().iter().map(|line| line["end"].as_u64().unwrap()).collect();
    assert_eq!(ends.last(), Some(&(WORDS.chars().count() as u64)), "every character is laid out");
}

#[test]
fn a_box_that_is_not_x_y_w_h_is_refused() {
    let mut doc = image();
    for bad in [json!([200, 100]), json!([0, 0, 200]), json!([0, 0, "wide", 100]), json!("200x100")] {
        let error = doc.execute("text.add", json!({ "box": bad, "text": "Hello" })).unwrap_err();
        assert!(error.contains("box"), "{bad}: {error}");
    }
    let made = doc.execute("text.add", json!({ "x": 10, "y": 40, "text": "Hello" })).unwrap();
    let error = doc.execute("text.edit", json!({ "layer": made["layer"], "box": [200, 100] })).unwrap_err();
    assert!(error.contains("box"), "{error}");
}

#[test]
fn type_info_answers_each_paragraph() {
    let mut doc = image();
    let made = doc.execute("text.add", json!({ "box": [10, 10, 300, 300], "text": "First paragraph\rSecond one\rThird", "size": 12 })).unwrap();
    let info = doc.execute("text.info", json!({ "layer": made["layer"] })).unwrap();
    let paragraphs: Vec<(u64, u64)> =
        info["paragraphs"].as_array().unwrap().iter().map(|p| (p["start"].as_u64().unwrap(), p["end"].as_u64().unwrap())).collect();
    assert_eq!(paragraphs, [(0, 16), (16, 27), (27, 32)], "{}", info["paragraphs"]);
    assert!(info["paragraphs"].as_array().unwrap().iter().all(|p| p["style"].is_object()));
}
