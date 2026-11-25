//! A `.kphoto` holds the image's content and nobody's view of it (feature
//! `photo.file.content-only`, case `same-bytes-across-sessions`; feature `photo.view.state`): two
//! sessions that open the same file and save it without editing it write the same bytes, the
//! whole file, whatever each one's selection, quick mask, collapsed groups, open notes and frame;
//! each person's view travels through the host's `view-state` store instead
//! (`PhotoDocument::view_state`, the editor's `photo.restoreViewState`), and putting it back is no
//! edit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use base64::Engine;
use kition_kit::KitDocument;
use kition_photo_kit::{PhotoDocument, RESTORE_VIEW_STATE};
use serde_json::{Value, json};

const FILE: &str = "Views.kphoto";

fn layer(answer: &Value) -> u64 {
    answer["layer"].as_u64().unwrap_or_else(|| panic!("no layer in {answer}"))
}

/// The file as a host saves it: sealed first (`prepare_save`), then the bytes.
fn host_save(doc: &mut PhotoDocument) -> Vec<u8> {
    doc.prepare_save().unwrap();
    doc.save(Some(FILE)).unwrap()
}

/// A picture with a group, a painted layer, a note and a timeline, saved by its maker with a view
/// of their own (the group collapsed); answers the file and the group's id.
fn picture() -> (Vec<u8>, u64) {
    let mut doc = PhotoDocument::create(FILE, 300, 200).unwrap();
    let group = layer(&doc.execute("layer.new.group", json!({ "name": "Group" })).unwrap());
    doc.execute("layer.new.layer", json!({ "name": "Paint" })).unwrap();
    doc.execute("select.rect", json!({ "x": 20, "y": 30, "width": 120, "height": 80 })).unwrap();
    doc.execute("edit.fill", json!({ "color": "#c04020" })).unwrap();
    doc.execute("edit.deselectAll", json!({})).unwrap();
    doc.execute("note.add", json!({ "x": 4, "y": 4, "text": "a note", "open": false })).unwrap();
    doc.execute("timeline.add", json!({ "duration": 12 })).unwrap();
    doc.execute("layer.setExpanded", json!({ "layer": group, "expanded": false })).unwrap();
    doc.execute("select.rect", json!({ "x": 1, "y": 1, "width": 9, "height": 9 })).unwrap();
    (host_save(&mut doc), group)
}

/// `[json length u32 LE][json][bytes]`, as `viewState` answers.
fn unpack(packed: &[u8]) -> (Value, Vec<u8>) {
    let len = u32::from_le_bytes(packed[..4].try_into().unwrap()) as usize;
    (serde_json::from_slice(&packed[4..4 + len]).unwrap(), packed[4 + len..].to_vec())
}

/// Puts `view` (a packed `viewState`) back as the editor does once the image opens.
fn restore(doc: &mut PhotoDocument, view: &[u8]) {
    let (state, bytes) = unpack(view);
    doc.execute(RESTORE_VIEW_STATE, json!({ "state": state, "bytes": base64::engine::general_purpose::STANDARD.encode(bytes) })).unwrap();
}

/// A person's own view: a selection, the group expanded again, the note's popup open, frame 5.
fn look_around(doc: &mut PhotoDocument, group: u64) {
    doc.execute("select.rect", json!({ "x": 40, "y": 20, "width": 200, "height": 150, "ellipse": true })).unwrap();
    doc.execute("layer.setExpanded", json!({ "layer": group, "expanded": true })).unwrap();
    doc.execute("note.set", json!({ "index": 0, "open": true })).unwrap();
    doc.execute("timeline.setFrame", json!({ "frame": 5 })).unwrap();
}

#[test]
fn two_sessions_that_open_the_same_file_and_save_it_unedited_write_the_same_bytes() {
    let (file, group) = picture();
    let mut first = PhotoDocument::open(FILE, &file).unwrap();
    look_around(&mut first, group);
    let mut second = PhotoDocument::open(FILE, &file).unwrap();
    second.execute("select.rect", json!({ "x": 100, "y": 100, "width": 50, "height": 50 })).unwrap();
    second.execute("select.editInQuickMaskMode", json!({ "on": true })).unwrap();
    assert!(second.inspect().unwrap()["quickMask"].as_bool().unwrap(), "the control: the second is in Quick Mask mode");
    assert!(first.inspect().unwrap()["hasSelection"].as_bool().unwrap(), "the control: the first has a selection");
    let (a, b) = (host_save(&mut first), host_save(&mut second));
    assert_eq!(a, b, "two sessions' saves of the same image are the same bytes, the whole .kphoto");
    // And a third, which only opens and saves.
    let mut third = PhotoDocument::open(FILE, &file).unwrap();
    assert_eq!(host_save(&mut third), a);
}

#[test]
fn the_view_travels_through_the_view_state_and_putting_it_back_is_no_edit() {
    let (file, group) = picture();
    let mut first = PhotoDocument::open(FILE, &file).unwrap();
    look_around(&mut first, group);
    let saved = host_save(&mut first);
    first.mark_saved();
    let view = first.view_state().unwrap();
    let (state, bytes) = unpack(&view);
    assert_eq!(state["v"], 1, "{state}");
    assert!(state.get("selection").is_some() && !bytes.is_empty(), "{state}");
    assert_eq!(state["frame"], 5);
    assert!(serde_json::to_vec(&state).unwrap().len() <= kition_photo_format::view_state::JSON_LIMIT);

    // The next session, from the file and the kept view.
    let mut again = PhotoDocument::open(FILE, &saved).unwrap();
    let before = (again.revision(), again.dirty(), again.inspect().unwrap()["canUndo"].clone());
    assert_ne!(again.view_state().unwrap(), view, "the control: the file alone gives no view");
    restore(&mut again, &view);
    assert_eq!(again.view_state().unwrap(), view, "the same view");
    assert_eq!((again.revision(), again.dirty(), again.inspect().unwrap()["canUndo"].clone()), before, "no edit, no undo step");
    let inspected = again.inspect().unwrap();
    assert!(inspected["hasSelection"].as_bool().unwrap());
    assert_eq!(inspected["timeline"]["current"], 5);
    assert_eq!(host_save(&mut again), saved, "and the file is the same");

    // Nothing to keep: `null`, which deletes what the host kept; restoring it changes nothing.
    let mut plain = PhotoDocument::open(FILE, &saved).unwrap();
    let (nothing, none) = unpack(&plain.view_state().unwrap());
    assert!(nothing.is_null() && none.is_empty(), "{nothing}");
    plain.execute(RESTORE_VIEW_STATE, json!({ "state": null })).unwrap();
    // A damaged value is refused and changes nothing.
    let (state, mut bytes) = unpack(&view);
    bytes[0] ^= 1;
    let refused = plain.execute(RESTORE_VIEW_STATE, json!({ "state": state, "bytes": base64::engine::general_purpose::STANDARD.encode(&bytes) }));
    assert!(refused.is_err());
    assert!(unpack(&plain.view_state().unwrap()).0.is_null());
    assert!(plain.execute(RESTORE_VIEW_STATE, json!({ "state": state, "bytes": "not base64!" })).is_err());
}
