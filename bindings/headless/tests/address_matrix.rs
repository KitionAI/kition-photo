//! C8-photo-3: necessity expectations independently reviewed after the first run.
//! Each cell has a fresh server and an independent writer which really rewrites the file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Address.kphoto";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Address {
    Id,
    Name,
    Top,
    Active,
    Point,
    Rect,
    Selection,
}

const ADDRESSES: [Address; 7] = [Address::Id, Address::Name, Address::Top, Address::Active, Address::Point, Address::Rect, Address::Selection];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Same,
    Other,
    Rename,
    Delete,
    Reorder,
    InsertAbove,
    InsertBelow,
    Move,
    Hide,
    Lock,
    PaintInside,
    PaintOutside,
    Mask,
    Selection,
    Resize,
    Crop,
    Depth,
}

const CHANGES: [Change; 17] = [
    Change::Same,
    Change::Other,
    Change::Rename,
    Change::Delete,
    Change::Reorder,
    Change::InsertAbove,
    Change::InsertBelow,
    Change::Move,
    Change::Hide,
    Change::Lock,
    Change::PaintInside,
    Change::PaintOutside,
    Change::Mask,
    Change::Selection,
    Change::Resize,
    Change::Crop,
    Change::Depth,
];

fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

fn pixels(address: Address) -> bool {
    matches!(address, Address::Rect | Address::Selection)
}

/// Only the addressed property, address resolution, or pixels/format read by a fill matter.
/// The target is L3 (top), its neighbour L2, and an unrelated layer L0, each in its own tile.
fn necessity(address: Address, change: Change, scope: usize) -> Option<&'static str> {
    use Change::*;
    let target = scope == 0;
    if target && change == Delete {
        // The observed layer is gone, even though the point now picks Background.
        return Some("deleted");
    }
    if address == Address::Name && target && change == Rename {
        return Some("relocated");
    }
    if address == Address::Top && matches!((change, target), (Reorder, _) | (InsertAbove, true)) {
        return Some("relocated");
    }
    if address == Address::Point && target && matches!(change, Hide | Lock | Mask) {
        return Some("relocated");
    }
    if pixels(address) {
        // Resize/crop keep the origin and the entire addressed 40x40 area. Neither changes
        // the pixels being assigned. Translation does change pixels inside that area.
        // same-field decision: overlapping pixel assignments hold their old value.
        if change == Depth || target && matches!(change, Move | Lock | PaintInside) {
            return Some("changed");
        }
    } else if target && change == Same {
        // same-field decision: absolute opacity assignments still hold opacity.
        return Some("changed");
    }
    // The 16px move leaves L3 under its centre point. An opacity write still lands on L3.
    None
}

fn edits(change: Change, id: u64, target: u64, scope: usize) -> Vec<(&'static str, Value)> {
    use Change::*;
    let props = |p: Value| vec![("layer.setProps", p)];
    match change {
        Same => props(json!({"layer":id,"opacity":0.4})),
        Other => props(json!({"layer":id,"fill":0.7})),
        Rename => props(json!({"layer":id,"name":"Colleague"})),
        Delete => vec![("layer.delete", json!({"layer":id}))],
        Reorder => {
            vec![("layer.moveTo", json!({"layer":id,"target":if scope == 0 { id - 1 } else { target },"position":if scope == 0 { "below" } else { "above" }}))]
        }
        InsertAbove | InsertBelow => vec![
            ("layer.select", json!({"layer":id})),
            ("layer.new.layer", json!({"name":"Inserted"})),
            ("layer.moveTo", json!({"layer":6,"target":id,"position":if change == InsertAbove {"above"} else {"below"}})),
        ],
        Move => vec![("layer.translate", json!({"layer":id,"dx":16,"dy":0}))],
        Hide => props(json!({"layer":id,"visible":false})),
        Lock => props(json!({"layer":id,"locked":true})),
        PaintInside | PaintOutside => {
            let [x, y, ..] = region(if scope == 0 {
                3
            } else if scope == 1 {
                2
            } else {
                0
            });
            let x = if change == PaintOutside { x + 70 } else { x };
            vec![
                ("layer.select", json!({"layer":id})),
                ("select.rect", json!({"x":x,"y":y,"width":8,"height":8})),
                ("edit.fill", json!({"color":"#123456"})),
                ("edit.deselectAll", json!({})),
            ]
        }
        Mask => vec![("layer.layerMask.hideAll", json!({"layer":id}))],
        Selection => vec![("select.rect", json!({"x":1,"y":1,"width":3,"height":3}))],
        Resize => vec![("image.canvasSize", json!({"width":1300,"height":256,"anchor":"topLeft"}))],
        Crop => vec![("image.crop", json!({"x":0,"y":0,"width":1270,"height":250}))],
        Depth => vec![("image.mode.bits16", json!({}))],
    }
}

/// The persisted document, including every layer field and every surface's content hashes.
/// The replica is deliberately excluded: independent reference edits have no replica history.
fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    // Selection is editing-session state and is restored by the host before saving.
    doc.as_object_mut().unwrap().remove("selection");
    // DocId is an engine-session handle; layer ids and all persisted content remain compared.
    doc.as_object_mut().unwrap().remove("id");
    doc
}

#[derive(Default, Debug)]
struct Totals {
    cells: usize,
    applied: usize,
    necessary: usize,
    wrong: usize,
    false_refusals: usize,
    wrong_reasons: usize,
    failed: usize,
    bad_merge: usize,
}

/// The former SDK-gap partition retains its independent necessity expectations.
/// Session continuity and historical comparisons now satisfy them.
fn historical_cases(address: Address, change: Change, scope: usize) -> bool {
    (address == Address::Active && (scope == 0 && matches!(change, Change::Delete | Change::InsertAbove) || change == Change::Reorder))
        || (address == Address::Point
            && (scope < 2 && change == Change::Reorder
                || scope == 0
                    && matches!(
                        change,
                        Change::Delete | Change::Move | Change::PaintInside | Change::PaintOutside | Change::Resize | Change::Crop | Change::Depth
                    )))
        || (pixels(address) && scope == 0 && change == Change::PaintOutside)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Partition {
    Strict,
    Sdk,
    Kit,
}

fn partition(address: Address, change: Change, scope: usize) -> Partition {
    if pixels(address) && matches!(change, Change::Resize | Change::Crop) {
        // Effective clipping must remain unchanged, including after canvas edits.
        Partition::Kit
    } else if historical_cases(address, change, scope) {
        Partition::Sdk
    } else {
        Partition::Strict
    }
}

async fn matrix(which: Partition) -> Totals {
    let original = image(FILE, 4);
    let mut totals = Totals::default();
    for address in ADDRESSES {
        for change in CHANGES {
            for scope in 0..3 {
                // Global edits have one cell, rather than three copies of the same change.
                if scope > 0 && matches!(change, Change::Selection | Change::Resize | Change::Crop | Change::Depth) {
                    continue;
                }
                if partition(address, change, scope) != which {
                    continue;
                }
                let f = Fixture::from_bytes(FILE, &original, CallOptions::default());
                let (details, basis) = f.inspect().await;
                let ids = ids_by_name(&details);
                let target = ids["L3"];
                let id = [target, ids["L2"], ids["L0"]][scope];
                let expected = necessity(address, change, scope);
                f.colleague(&edits(change, id, target, scope));
                let before = std::fs::read(f.file()).unwrap();
                let mut reference = PhotoDocument::open(FILE, &before).unwrap();
                let mut params = if pixels(address) { json!({"color":"#00ff00"}) } else { json!({"opacity":0.6}) };
                let command = if pixels(address) { "edit.fill" } else { "layer.setProps" };
                let mut steps = Vec::new();
                let [x, y, w, h] = region(3);
                match address {
                    Address::Id | Address::Rect | Address::Selection => params["layer"] = json!(target),
                    Address::Name => params["layer"] = json!("L3"),
                    Address::Top => params["layer"] = json!("#1"),
                    Address::Point => params["at"] = json!(middle(3)),
                    Address::Active => {}
                }
                if address == Address::Rect {
                    params["region"] = json!({"rect":[x,y,w,h]});
                } else if address == Address::Selection {
                    steps.push(step("select.rect", json!({"x":x,"y":y,"width":w,"height":h})));
                }
                steps.push(step(command, params));
                let outcome = f.call("run", json!({"basis":basis,"steps":steps})).await;
                let label = format!("{address:?}/{change:?}/{scope}");
                totals.cells += 1;
                match (&outcome, expected) {
                    (Ok(_), Some(_)) => {
                        totals.wrong += 1;
                        println!("WRONG {label}");
                    }
                    (Err(error), Some(reason)) if error["error"]["code"] == "conflict" => {
                        totals.necessary += 1;
                        if !error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == reason) {
                            totals.wrong_reasons += 1;
                            println!("REASON {label} expected {reason}: {error}");
                        }
                        assert_eq!(std::fs::read(f.file()).unwrap(), before, "refused {label} changed the file");
                    }
                    (Err(error), None) if error["error"]["code"] == "conflict" => {
                        totals.false_refusals += 1;
                        println!("FALSE {label}: {}", error["conflicts"]);
                    }
                    (Err(error), _) => {
                        totals.failed += 1;
                        println!("FAILED {label}: {error}");
                    }
                    (Ok(_), None) => {
                        totals.applied += 1;
                        // Resolve the originally observed target by id, never by the current address.
                        let state = reference.editor_state();
                        reference.execute("layer.select", json!({"layer":target})).unwrap();
                        if pixels(address) {
                            reference.execute("select.rect", json!({"x":x,"y":y,"width":w,"height":h})).unwrap();
                        }
                        reference.execute(command, if pixels(address) { json!({"color":"#00ff00"}) } else { json!({"layer":target,"opacity":0.6}) }).unwrap();
                        reference.restore_editor_state(&state).unwrap();
                        let expected = saved(&reference.save_sealed(Some(FILE)).unwrap());
                        let actual = saved(&std::fs::read(f.file()).unwrap());
                        if actual != expected {
                            totals.bad_merge += 1;
                            let fields: Vec<_> = actual.as_object().unwrap().keys().filter(|key| actual[*key] != expected[*key]).collect();
                            println!("MERGE {label}: {fields:?}");
                        }
                    }
                }
                f.shutdown().await;
            }
        }
    }
    println!("ADDRESS {which:?} {totals:?}");
    totals
}

#[tokio::test(flavor = "multi_thread")]
async fn address_matrix() {
    let totals = matrix(Partition::Strict).await;
    assert_eq!((totals.cells, totals.applied, totals.necessary), (281, 255, 26));
    assert_eq!((totals.wrong, totals.false_refusals, totals.wrong_reasons, totals.failed, totals.bad_merge), (0, 0, 0, 0, 0), "{totals:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn address_reload_and_historical_pixels() {
    let totals = matrix(Partition::Sdk).await;
    assert_eq!(
        (totals.cells, totals.applied, totals.necessary, totals.wrong, totals.false_refusals, totals.wrong_reasons, totals.failed, totals.bad_merge),
        (16, 14, 2, 0, 0, 0, 0, 0),
        "Necessity expectations remain in necessity(). {totals:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unchanged_canvas_region() {
    let totals = matrix(Partition::Kit).await;
    assert_eq!(
        (totals.cells, totals.applied, totals.necessary, totals.wrong, totals.false_refusals, totals.wrong_reasons, totals.failed, totals.bad_merge),
        (4, 4, 0, 0, 0, 0, 0, 0),
        "The kit needs premises on effective clipping, rather than unconditional canvas dimensions. {totals:?}"
    );
}
