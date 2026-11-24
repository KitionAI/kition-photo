//! C8-photo-3: necessities are fixed before running these independent-writer pairs.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::dispatch::{Gate, Refusal, Role};
use kition_kit::{CallContext, KitDocument};
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Inputs.kphoto";
type Steps = Vec<(&'static str, Value)>;

struct Agent;
impl Gate for Agent {
    fn role(&self) -> Role {
        Role::Agent
    }
    fn check(&self, _: &str, _: &Value) -> Result<(), Refusal> {
        Ok(())
    }
}

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    doc.as_object_mut().unwrap().remove("id");
    doc.as_object_mut().unwrap().remove("selection");
    doc
}

fn same_saved_merge(before: &[u8], actual: &[u8], reference: &[u8]) -> bool {
    let actual = saved(actual);
    let mut reference = saved(reference);
    let retained = saved(before)["channels"].as_array().unwrap().len();
    let actual_channels = actual["channels"].as_array().unwrap();
    let reference_channels = reference["channels"].as_array().unwrap();
    if actual_channels.len() != reference_channels.len() {
        return false;
    }
    // A calculation allocates a fresh opaque resource key. Bind only newly created
    // channel identities; retained identities and every other saved field still compare.
    let keys: std::collections::BTreeSet<_> = actual_channels.iter().filter_map(|c| c["key"].as_str()).collect();
    assert_eq!(keys.len(), actual_channels.len(), "channel keys must stay unique and present");
    for channel in reference_channels.iter().skip(retained) {
        assert!(channel["key"].as_str().is_some_and(|s| !s.is_empty()));
    }
    for index in retained..actual_channels.len() {
        reference["channels"][index]["key"] = actual_channels[index]["key"].clone();
    }
    actual == reference
}

fn run(doc: &mut PhotoDocument, steps: &Steps) {
    for (command, params) in steps {
        doc.execute(command, params.clone()).unwrap_or_else(|e| panic!("setup {command}: {e}"));
    }
}

struct Pair {
    name: &'static str,
    bytes: Vec<u8>,
    command: &'static str,
    params: Value,
    on: Vec<Value>,
    change: Steps,
    // Live clipping/adjustment references do not bake the pixels below them. Their saved
    // result merges a colleague's pixels; a computed snapshot must refuse changed inputs.
    refuse: bool,
}

fn pair(name: &'static str) -> Pair {
    let mut doc = PhotoDocument::create(FILE, 128, 96).unwrap();
    for i in 0..4 {
        run(
            &mut doc,
            &vec![
                ("layer.new.layer", json!({"name":format!("L{i}")})),
                ("select.rect", json!({"x":8+i*24,"y":12+i*8,"width":16,"height":24})),
                ("edit.fill", json!({"color":"#dd3344"})),
                ("select.deselect", json!({})),
            ],
        );
    }
    let ids = ids_by_name(&doc.inspect().unwrap());
    let (target, source) = (ids["L3"], ids["L2"]);
    let mut out = Pair {
        name,
        bytes: vec![],
        command: name,
        params: json!({"layer":target}),
        on: vec![],
        change: vec![("layer.translate", json!({"layer":source,"dx":4,"dy":2}))],
        refuse: true,
    };
    match name {
        "layer.mergeVisible" | "layer.flattenImage" | "layer.stampVisible" => out.params = json!({}),
        "layer.createClippingMask" | "layer.releaseClippingMask" => {
            if name == "layer.releaseClippingMask" {
                doc.execute("layer.createClippingMask", json!({"layer":target})).unwrap();
            }
            out.refuse = false;
        }
        "layer.newAdjustmentLayer.invert" => {
            out.params = json!({});
            out.refuse = false;
        }
        "selection.fill" | "selection.stroke" | "selection.filter" => {
            out.command = match name {
                "selection.fill" => "edit.fill",
                "selection.stroke" => "edit.stroke",
                _ => "filter.blur.gaussianBlur",
            };
            out.params = match name {
                "selection.fill" => json!({"layer":target,"region":{"layer":source},"color":"#00dd55"}),
                "selection.stroke" => json!({"layer":target,"region":{"layer":source},"color":"#00dd55","width":3}),
                _ => json!({"layer":target,"region":{"layer":source},"radius":2}),
            };
        }
        "smart fill surroundings" => {
            out.command = "edit.smartFill";
            out.params = json!({"layer":target,"region":{"rect":[82,38,6,6]}});
            out.change = vec![
                ("layer.select", json!({"layer":target})),
                ("select.rect", json!({"x":96,"y":36,"width":3,"height":3})),
                ("edit.fill", json!({"color":"#0000ff"})),
                ("select.deselect", json!({})),
            ];
        }
        "layer.align.leftEdges" | "layer.distribute.horizontally" => {
            out.params = json!({});
            out.on = vec![json!(ids["L1"]), json!(source), json!(target)];
        }
        "linked transform" => {
            doc.set_context(&CallContext { on: vec![json!(target), json!(source)], ..Default::default() }).unwrap();
            doc.execute("layer.linkLayers", json!({})).unwrap();
            out.command = "layer.translate";
            out.params = json!({"layer":target,"dx":8,"dy":0});
        }
        "apply layer" | "calculate layer" | "apply channel" | "calculate channel" => {
            let channel = name.ends_with("channel");
            if channel {
                doc.execute("select.rect", json!({"x":20,"y":10,"width":24,"height":24})).unwrap();
                doc.execute("select.saveSelection", json!({"name":"Source"})).unwrap();
                doc.execute("select.deselect", json!({})).unwrap();
                out.change = vec![("channel.rename", json!({"channel":"Source","name":"Changed"}))];
            }
            let source = if channel { json!({"channel":"Source"}) } else { json!({"layer":source,"channel":"red"}) };
            if name.starts_with("apply") {
                out.command = "image.applyImage";
                out.params = json!({"layer":target,"source":source});
            } else {
                out.command = "image.calculations";
                out.params = json!({"source1":source,"source2":{"layer":target,"channel":"red"},"name":"Result"});
            }
        }
        "character style" | "paragraph style" => {
            let layer = doc.execute("type.create", json!({"text":"C8","x":10,"y":40,"size":12})).unwrap()["layer"].clone();
            let paragraph = name == "paragraph style";
            let new = if paragraph { "type.paragraphStyle.new" } else { "type.characterStyle.new" };
            let id = doc.execute(new, json!({"name":"Named","fromSelection":false,"attrs":{"size":24}})).unwrap()["id"].clone();
            out.command = if paragraph { "type.paragraphStyle.apply" } else { "type.characterStyle.apply" };
            out.params = json!({"layer":layer,"id":id});
            out.change = vec![(if paragraph { "type.paragraphStyle.set" } else { "type.characterStyle.set" }, json!({"id":id,"attrs":{"size":32}}))];
        }
        "stored pattern" => {
            doc.execute("layer.newFillLayer.pattern", json!({"pattern":"Checkerboard"})).unwrap();
            doc.execute("layer.delete", json!({})).unwrap();
            out.command = "edit.fill";
            out.params = json!({"layer":target,"contents":"pattern","pattern":"Checkerboard"});
            out.change = vec![("pattern.rename", json!({"pattern":"Checkerboard","name":"Changed"}))];
        }
        "smart contents" => {
            let target = doc.execute("layer.objectLayers.convertToObjectLayer", json!({"layer":target})).unwrap()["layer"].clone();
            out.command = "layer.rasterize.objectLayer";
            out.params = json!({"layer":target});
            out.change = vec![
                ("layer.objectLayers.editContents", json!({"layer":target})),
                ("edit.fill", json!({"color":"#0000ff"})),
                ("layer.objectLayers.saveContents", json!({})),
                ("file.close", json!({})),
            ];
        }
        _ => panic!("unknown pair {name}"),
    }
    out.bytes = doc.save_sealed(Some(FILE)).unwrap();
    out
}

async fn matrix(waiting: bool) -> (usize, usize, usize, usize, usize, usize, usize, usize) {
    let mut wrong = vec![];
    let mut false_refusals = vec![];
    let mut reasons = vec![];
    let mut failures = vec![];
    let mut merges = vec![];
    let (mut cells, mut applied, mut necessary) = (0, 0, 0);
    for family in [
        "layer.mergeVisible",
        "layer.flattenImage",
        "layer.stampVisible",
        "layer.createClippingMask",
        "layer.releaseClippingMask",
        "layer.newAdjustmentLayer.invert",
        "selection.fill",
        "selection.stroke",
        "selection.filter",
        "smart fill surroundings",
        "layer.align.leftEdges",
        "layer.distribute.horizontally",
        "linked transform",
        "apply layer",
        "calculate layer",
        "apply channel",
        "calculate channel",
        "character style",
        "paragraph style",
        "stored pattern",
        "smart contents",
    ] {
        let pair = pair(family);
        for related in [true, false] {
            let relocated = related
                && matches!(pair.name, "apply channel" | "calculate channel" | "stored pattern" | "selection.fill" | "selection.stroke" | "selection.filter");
            if relocated != waiting {
                continue;
            }
            let f = Fixture::from_bytes(FILE, &pair.bytes, CallOptions::default());
            let (_, basis) = f.inspect().await;
            let label = format!("{}/{related}", pair.name);
            let unrelated = vec![("notes.add", json!({"x":1,"y":1,"text":"Colleague"}))];
            f.colleague(if related { &pair.change } else { &unrelated });
            let before = std::fs::read(f.file()).unwrap();
            let expected = if related && pair.refuse {
                Some(match pair.name {
                    // The source name or sourced region no longer denotes what was observed.
                    "apply channel" | "calculate channel" | "stored pattern" | "selection.fill" | "selection.stroke" | "selection.filter" => "relocated",
                    _ => "changed",
                })
            } else {
                None
            };
            let mut step = json!({"command":pair.command,"params":pair.params});
            if !pair.on.is_empty() {
                step["on"] = json!(pair.on);
            }
            let result = f.call("run", json!({"basis":basis,"steps":[step]})).await;
            cells += 1;
            match (&result, expected) {
                (Ok(_), Some(_)) => wrong.push(label),
                (Err(e), Some(reason)) if e["error"]["code"] == "conflict" => {
                    necessary += 1;
                    if !e["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == reason) {
                        reasons.push(format!("{label} expected {reason}: {e}"));
                    }
                    assert_eq!(std::fs::read(f.file()).unwrap(), before);
                }
                (Err(e), None) if e["error"]["code"] == "conflict" => false_refusals.push(format!("{label}: {e}")),
                (Err(e), _) => failures.push(format!("{label}: {e}")),
                (Ok(_), None) => {
                    let mut reference = PhotoDocument::open(FILE, &before).unwrap();
                    let state = reference.editor_state();
                    reference.set_context(&CallContext { on: pair.on.clone(), ..Default::default() }).unwrap();
                    kition_kit::dispatch::with_gate(std::sync::Arc::new(Agent), || reference.execute(pair.command, pair.params.clone())).unwrap();
                    reference.restore_editor_state(&state).unwrap();
                    if !same_saved_merge(&before, &std::fs::read(f.file()).unwrap(), &reference.save_sealed(Some(FILE)).unwrap()) {
                        merges.push(label);
                    }
                    applied += 1;
                }
            }
            f.shutdown().await;
        }
    }
    println!(
        "DATA waiting={waiting} cells={cells} applied={applied} necessary={necessary} wrong={} false={} reason={} failed={} merge={}\nwrong={wrong:?}\nfalse={false_refusals:?}\nreason={reasons:?}\nfailed={failures:?}\nmerge={merges:?}",
        wrong.len(),
        false_refusals.len(),
        reasons.len(),
        failures.len(),
        merges.len()
    );
    (cells, applied, necessary, wrong.len(), false_refusals.len(), reasons.len(), failures.len(), merges.len())
}

#[tokio::test(flavor = "multi_thread")]
async fn derived_data_matrix() {
    assert_eq!(matrix(false).await, (36, 24, 12, 0, 0, 0, 0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn derived_address_reasons() {
    assert_eq!(matrix(true).await, (6, 0, 6, 0, 0, 0, 0, 0));
}
