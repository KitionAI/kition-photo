//! A character no loaded font can draw (#4) is reported by `check` as `photo.font-substituted`,
//! naming the character and the layer, instead of silently drawing a box.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

fn substituted(doc: &PhotoDocument) -> Vec<(String, Option<String>)> {
    doc.check().into_iter().filter(|d| d.code == "photo.font-substituted").map(|d| (d.message, d.target.path)).collect()
}

#[test]
fn a_character_in_no_font_is_reported_with_its_code_point() {
    let mut doc = PhotoDocument::create("menu.psd", 200, 100).unwrap();
    doc.execute("text.add", json!({ "x": 10, "y": 30, "text": "Menu", "font": "Inter", "size": 12 })).unwrap();
    assert!(substituted(&doc).is_empty(), "{:?}", substituted(&doc));
    // U+0378 is unassigned: no font on any machine has a glyph for it.
    let made = doc.execute("text.add", json!({ "x": 10, "y": 60, "text": "Open \u{378}\u{378} now", "font": "Inter", "size": 12 })).unwrap();
    let found = substituted(&doc);
    assert_eq!(found.len(), 1, "{found:?}");
    let (message, target) = &found[0];
    assert!(message.contains("U+0378"), "{message}");
    assert_eq!(target.as_deref(), Some(format!("layer:{}", made["layer"]).as_str()));
    // Spaces, line breaks and invisible format characters are never "missing".
    let mut clean = PhotoDocument::create("menu.psd", 200, 100).unwrap();
    clean.execute("text.add", json!({ "x": 10, "y": 30, "text": "A\u{200D}B\u{FE0F}\tC\rD\u{00AD}E", "font": "Inter", "size": 12 })).unwrap();
    assert!(substituted(&clean).is_empty(), "{:?}", substituted(&clean));
}
