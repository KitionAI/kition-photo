//! The editor reads the document's guides to draw, move and snap to them (plan 2026-11-24, stage 7):
//! `guides()` answers each orientation's positions in the order `view.moveGuide` and
//! `view.deleteGuide` count them, follows undo, and changes nothing.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[test]
fn guides_answer_each_orientation_in_command_order() {
    let mut doc = PhotoDocument::create("guides.psd", 200, 100).unwrap();
    assert_eq!(doc.guides().unwrap(), json!({ "horizontal": [], "vertical": [] }));
    doc.execute("view.newGuide", json!({ "orientation": "vertical", "position": 40 })).unwrap();
    doc.execute("view.newGuide", json!({ "orientation": "vertical", "position": 120.5 })).unwrap();
    doc.execute("view.newGuide", json!({ "orientation": "horizontal", "position": 25 })).unwrap();
    assert_eq!(doc.guides().unwrap(), json!({ "horizontal": [25.0], "vertical": [40.0, 120.5] }));
    doc.execute("view.moveGuide", json!({ "orientation": "vertical", "index": 1, "position": 150 })).unwrap();
    doc.execute("view.deleteGuide", json!({ "orientation": "vertical", "index": 0 })).unwrap();
    assert_eq!(doc.guides().unwrap(), json!({ "horizontal": [25.0], "vertical": [150.0] }));
    // Reading is not an edit, and the guides follow the history.
    let revision = doc.revision();
    doc.guides().unwrap();
    assert_eq!(doc.revision(), revision);
    doc.undo().unwrap();
    assert_eq!(doc.guides().unwrap(), json!({ "horizontal": [25.0], "vertical": [40.0, 150.0] }));
}
