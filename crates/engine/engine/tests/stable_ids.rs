//! Layer ids are the addresses agents, commands and diagnostics use (KHP A46, A47, A53, A57): a
//! layer keeps its id each time its file is read. A PSD gives each layer the id the file stores for
//! it (`lyid`) and writes the id back; a `.kphoto` keeps the ids its manifest or its replica stores;
//! a document numbers its own layers, so what one open document holds never changes the ids of
//! another; and a layer copied into another document takes a fresh id there.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeSet;

use kition_photo_engine::Session;
use kition_photo_psd::PsdFile;
use kition_photo_psd::tagged::TaggedBlock;
use serde_json::{Value, json};

fn run(s: &mut Session, command: &str, params: Value) -> Value {
    s.execute(command, params).unwrap_or_else(|error| panic!("{command}: {error}"))
}

/// Every layer of the active document as `(name, kind, id)`, top to bottom, groups before their
/// contents: what `inspect` tells an agent.
fn ids(s: &Session) -> Vec<(String, String, u64)> {
    fn walk(layers: &Value, out: &mut Vec<(String, String, u64)>) {
        for layer in layers.as_array().into_iter().flatten() {
            out.push((layer["name"].as_str().unwrap().to_owned(), layer["kind"].as_str().unwrap().to_owned(), layer["id"].as_u64().unwrap()));
            walk(&layer["children"], out);
        }
    }
    let mut out = Vec::new();
    walk(&kition_photo_engine::inspect::document(s.active().unwrap())["layers"], &mut out);
    out
}

fn assert_unique(list: &[(String, String, u64)]) {
    let set: BTreeSet<u64> = list.iter().map(|(_, _, id)| *id).collect();
    assert_eq!(set.len(), list.len(), "ids are unique in a document: {list:?}");
}

fn id_of(s: &Session, name: &str) -> u64 {
    ids(s).into_iter().find(|(n, _, _)| n == name).unwrap_or_else(|| panic!("no layer {name}: {:?}", ids(s))).2
}

/// A document holding every kind of layer an agent can address: pixels (with a mask), a group,
/// an artboard, an adjustment, a fill, type, a shape and an object layer.
fn every_kind() -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 96, "height": 64, "background": "white"}));
    run(&mut s, "layer.new.layer", json!({"name": "Ink"}));
    run(&mut s, "paint.stroke", json!({"points": [[4, 4, 1], [40, 30, 1]], "size": 6, "color": "#ff0000"}));
    run(&mut s, "select.rect", json!({"x": 10, "y": 8, "width": 30, "height": 20}));
    let ink = id_of(&s, "Ink");
    run(&mut s, "layer.layerMask.revealSelection", json!({"layer": ink}));
    run(&mut s, "select.deselect", json!({}));
    run(&mut s, "layer.new.group", json!({"name": "Group"}));
    run(&mut s, "layer.new.layer", json!({"name": "Inside"}));
    run(&mut s, "paint.stroke", json!({"points": [[50, 40, 1], [60, 50, 1]], "size": 4, "color": "#00ff00"}));
    run(&mut s, "layer.newAdjustmentLayer.levels", json!({"gamma": 1.6, "inBlack": 20, "outWhite": 230}));
    run(&mut s, "layer.newFillLayer.solidColor", json!({"color": "#204060"}));
    run(&mut s, "type.create", json!({"text": "Title", "x": 4, "y": 30, "size": 14, "color": "#aa2200", "name": "Title"}));
    run(&mut s, "shape.create", json!({"kind": "ellipse", "rect": [4, 4, 30, 20], "fill": "#ff0000", "name": "Shape"}));
    run(&mut s, "layer.new.layer", json!({"name": "Art"}));
    run(&mut s, "paint.stroke", json!({"points": [[20, 20, 1], [30, 25, 1]], "size": 5, "color": "#0000ff"}));
    let art = id_of(&s, "Art");
    run(&mut s, "layer.objectLayers.convertToObjectLayer", json!({"layer": art}));
    run(&mut s, "layer.new.artboard", json!({"name": "Board"}));
    let all = ids(&s);
    assert_unique(&all);
    let kinds: BTreeSet<&str> = all.iter().map(|(_, kind, _)| kind.as_str()).collect();
    assert!(kinds.len() >= 7, "every kind of layer: {all:?}");
    s
}

fn psd(s: &Session) -> Vec<u8> {
    kition_photo_io::export(&s.active().unwrap().doc, "image.psd", &kition_photo_io::ExportOptions::default()).unwrap().bytes
}

fn opened(name: &str, bytes: &[u8]) -> Session {
    let mut s = Session::new();
    let index = s.open_file(name, bytes).unwrap();
    assert!(s.set_active(index));
    s
}

/// A session that has other documents open, and has edited them, before it opens `name`.
fn opened_after_others(name: &str, bytes: &[u8]) -> Session {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 40, "height": 30, "background": "white"}));
    for k in 0..5 {
        run(&mut s, "layer.new.layer", json!({"name": format!("Other {k}")}));
    }
    let index = s.open_file("other.psd", &psd(&every_kind())).unwrap();
    assert!(s.set_active(index));
    run(&mut s, "layer.new.group", json!({"name": "Elsewhere"}));
    let index = s.open_file(name, bytes).unwrap();
    assert!(s.set_active(index));
    s
}

/// `bytes` with each layer record storing the PSD layer id `id(k)` (`lyid`).
fn with_layer_ids(bytes: &[u8], id: impl Fn(usize) -> u32) -> Vec<u8> {
    let mut file = PsdFile::from_bytes(bytes).unwrap();
    for (k, record) in file.layers_mut().iter_mut().enumerate() {
        record.blocks.retain(|block| &block.key != b"lyid");
        record.blocks.push(TaggedBlock::layer_id(id(k)));
    }
    file.to_bytes().unwrap()
}

#[test]
fn every_kind_of_layer_keeps_its_id_through_psd_save_and_reopen() {
    let s = every_kind();
    let want = ids(&s);
    let bytes = psd(&s);
    let once = opened("image.psd", &bytes);
    assert_eq!(ids(&once), want, "saved as PSD and reopened");
    assert_eq!(ids(&opened_after_others("image.psd", &bytes)), want, "reopened in a session with other documents open");
    assert_eq!(ids(&opened("image.psd", &psd(&once))), want, "saved and reopened twice");
}

#[test]
fn a_psd_keeps_exactly_the_layer_ids_it_stores() {
    let bytes = with_layer_ids(&psd(&every_kind()), |k| 1000 + 7 * k as u32);
    let stored: BTreeSet<u64> = PsdFile::from_bytes(&bytes).unwrap().layers().iter().filter_map(|r| r.layer_id()).map(u64::from).collect();
    let s = opened("stored.psd", &bytes);
    let got = ids(&s);
    assert!(got.iter().all(|(_, _, id)| stored.contains(id)), "every layer has the id its record stores: {got:?}");
    assert_unique(&got);
    assert_eq!(ids(&opened_after_others("stored.psd", &bytes)), got, "whatever else is open");
    assert_eq!(ids(&opened("again.psd", &psd(&s))), got, "export writes the ids back");
    // A layer made after opening takes an id no layer has, and keeps it through a save.
    let mut s = s;
    run(&mut s, "layer.new.layer", json!({"name": "New"}));
    let after = ids(&s);
    assert_unique(&after);
    assert_eq!(ids(&opened("again.psd", &psd(&s))), after);
}

#[test]
fn layers_without_a_stored_id_number_the_same_on_every_open() {
    let mut file = PsdFile::from_bytes(&psd(&every_kind())).unwrap();
    for record in file.layers_mut() {
        record.blocks.retain(|block| &block.key != b"lyid");
    }
    let bytes = file.to_bytes().unwrap();
    let first = ids(&opened("plain.psd", &bytes));
    assert_unique(&first);
    assert_eq!(ids(&opened_after_others("plain.psd", &bytes)), first);
}

#[test]
fn a_kphoto_keeps_its_ids_with_and_without_a_replica() {
    let mut s = every_kind();
    let want = ids(&s);
    // Without a replica: what a bundle written without one (or by another tool) holds.
    let plain = kition_photo_io::export(&s.active().unwrap().doc, "image.kphoto", &kition_photo_io::ExportOptions::default()).unwrap().bytes;
    assert!(kition_photo_format::read_kdm(&plain).unwrap().is_none());
    assert_eq!(ids(&opened("image.kphoto", &plain)), want, "a .kphoto without a replica");
    assert_eq!(ids(&opened_after_others("image.kphoto", &plain)), want);
    // With the replica the edits made: the document opens from the replica.
    s.version().unwrap();
    let bundled = s.active().unwrap().save_kphoto().unwrap();
    assert!(kition_photo_format::read_kdm(&bundled).unwrap().is_some());
    let reopened = opened("image.kphoto", &bundled);
    assert_eq!(ids(&reopened), want, "a .kphoto with its replica");
    assert_eq!(ids(&opened_after_others("image.kphoto", &bundled)), want);
    assert_eq!(ids(&opened("image.kphoto", &reopened.active().unwrap().save_kphoto().unwrap())), want, "saved again");
    // The replica alone (a host-stored replica, or a sync room's state).
    let mut s = s;
    let snapshot = s.snapshot().unwrap();
    let blobs = s.blobs().unwrap().clone();
    let mut room = Session::new();
    run(&mut room, "file.new", json!({"width": 20, "height": 20, "background": "white"}));
    room.open_snapshot(&snapshot, blobs, "image.psd").unwrap();
    assert_eq!(ids(&room), want, "opened from its replica alone");
}

#[test]
fn a_failed_run_rolled_back_by_reopening_leaves_inspect_identical() {
    let bytes = psd(&every_kind());
    let mut s = opened("image.psd", &bytes);
    let lasting = |s: &Session| {
        let mut state = kition_photo_engine::inspect::document(s.active().unwrap());
        for key in ["history", "historyIndex", "historyRedo", "canUndo", "canRedo", "revision"] {
            state.as_object_mut().unwrap().remove(key);
        }
        state
    };
    let before = lasting(&s);
    // A run whose last step fails: the server rolls the document back by closing it and opening
    // its file again.
    run(&mut s, "layer.new.layer", json!({"name": "Draft"}));
    run(&mut s, "layer.new.group", json!({"name": "Draft group"}));
    assert!(s.execute("layer.setProps", json!({"layer": 999_999, "opacity": 0.5})).is_err());
    let index = s.active_index().unwrap();
    s.close(index);
    let index = s.open_file("image.psd", &bytes).unwrap();
    assert!(s.set_active(index));
    assert_eq!(lasting(&s), before);
}

#[test]
fn a_new_document_numbers_its_layers_whatever_else_is_open() {
    let fresh = || {
        let mut s = Session::new();
        run(&mut s, "file.new", json!({"width": 64, "height": 48, "background": "white"}));
        s
    };
    let mut alone = fresh();
    let mut busy = opened_after_others("image.psd", &psd(&every_kind()));
    run(&mut busy, "file.new", json!({"width": 64, "height": 48, "background": "white"}));
    assert_eq!(ids(&busy), ids(&alone), "the new document's Background");
    for s in [&mut alone, &mut busy] {
        run(s, "layer.new.layer", json!({"name": "One"}));
        run(s, "layer.new.group", json!({"name": "Two"}));
    }
    assert_eq!(ids(&busy), ids(&alone), "and the layers made in it");
}

#[test]
fn a_layer_copied_into_another_document_takes_a_fresh_id_there() {
    let mut s = Session::new();
    let source = s.open_file("source.psd", &psd(&every_kind())).unwrap();
    let target = s.open_file("target.psd", &psd(&every_kind())).unwrap();
    assert!(s.set_active(target));
    let target_before = ids(&s);
    assert!(s.set_active(source));
    let source_ids = ids(&s);
    let ink = id_of(&s, "Ink");
    // The two documents hold the same ids: a copy keeping its id would clash.
    assert!(target_before.iter().any(|(_, _, id)| *id == ink));
    let copied = run(&mut s, "layer.copyToDocument", json!({"document": target, "layers": [ink]}));
    let new = copied["layers"][0].as_u64().unwrap();
    assert_eq!(s.active_index(), Some(target));
    let after = ids(&s);
    assert_unique(&after);
    assert!(!target_before.iter().any(|(_, _, id)| *id == new), "{new} is a fresh id in the target: {target_before:?}");
    assert!(after.iter().any(|(name, _, id)| *id == new && name == "Ink"), "the answer names the copy: {after:?}");
    assert!(target_before.iter().all(|layer| after.contains(layer)), "the target's layers keep their ids");
    assert_eq!(s.active().unwrap().active_layer.map(|l| l.0), Some(new));
    assert!(s.set_active(source));
    assert_eq!(ids(&s), source_ids, "the source is unchanged");

    // Pasting pixels copied from the source makes a new layer with a fresh id too.
    run(&mut s, "select.all", json!({}));
    run(&mut s, "layer.select", json!({"layer": ink}));
    run(&mut s, "edit.copy", json!({}));
    assert!(s.set_active(target));
    let before_paste = ids(&s);
    run(&mut s, "edit.paste", json!({}));
    let pasted = ids(&s);
    assert_unique(&pasted);
    assert_eq!(pasted.len(), before_paste.len() + 1);
    assert!(before_paste.iter().all(|layer| pasted.contains(layer)));
}

#[test]
fn layers_brought_out_of_a_object_layer_take_fresh_ids() {
    let mut s = Session::new();
    run(&mut s, "file.new", json!({"width": 64, "height": 48, "background": "white"}));
    for name in ["A", "B", "C"] {
        run(&mut s, "layer.new.layer", json!({"name": name}));
        run(&mut s, "paint.stroke", json!({"points": [[4, 4, 1], [30, 20, 1]], "size": 4, "color": "#ff0000"}));
    }
    let a = id_of(&s, "A");
    let b = id_of(&s, "B");
    run(&mut s, "layer.select", json!({"layer": a}));
    run(&mut s, "layer.select", json!({"layer": b, "mode": "add"}));
    let smart = run(&mut s, "layer.objectLayers.convertToObjectLayer", json!({}));
    let smart = smart["layer"].as_u64().or_else(|| s.active().unwrap().active_layer.map(|l| l.0)).unwrap();
    run(&mut s, "layer.new.layer", json!({"name": "D"}));
    run(&mut s, "layer.duplicate", json!({"layer": smart}));
    let copy = s.active().unwrap().active_layer.unwrap().0;
    assert_ne!(copy, smart);
    let before = ids(&s);
    run(&mut s, "layer.objectLayers.convertToLayers", json!({"layer": smart}));
    let after = ids(&s);
    assert_unique(&after);
    let kept: Vec<_> = before.iter().filter(|(_, _, id)| *id != smart).collect();
    assert!(kept.iter().all(|layer| after.contains(layer)), "the other layers keep their ids: {before:?} -> {after:?}");
    // Its copy holds the same contents: they come out under ids of their own as well.
    run(&mut s, "layer.objectLayers.convertToLayers", json!({"layer": copy}));
    let both = ids(&s);
    assert_unique(&both);
    let kept: Vec<_> = after.iter().filter(|(_, _, id)| *id != copy).collect();
    assert!(kept.iter().all(|layer| both.contains(layer)), "{after:?} -> {both:?}");
}
