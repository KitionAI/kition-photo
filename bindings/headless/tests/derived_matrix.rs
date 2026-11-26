//! C8-photo-3: a merge/bake reads rendering resources outside its addressed layers.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Derived.kphoto";

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    doc.as_object_mut().unwrap().remove("id");
    doc.as_object_mut().unwrap().remove("selection");
    doc
}

fn fixture(pattern: bool) -> Vec<u8> {
    let mut doc = image_doc(FILE, 3);
    let id = ids_by_name(&doc.inspect().unwrap())["L2"];
    if pattern {
        doc.execute("layer.newFillLayer.pattern", json!({"pattern":"Checkerboard"})).unwrap();
        doc.execute("layer.delete", json!({})).unwrap();
        doc.execute("layer.select", json!({"layer":id})).unwrap();
        doc.execute("layer.layerStyle.patternOverlay", json!({"pattern":"Checkerboard","opacity":100})).unwrap();
    } else {
        doc.execute("layer.layerStyle.dropShadow", json!({"useGlobalLight":true,"distance":8,"size":0,"opacity":100})).unwrap();
    }
    doc.save(Some(FILE)).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn derived_resource_matrix() {
    let mut wrong = Vec::new();
    let mut false_refusals = Vec::new();
    let mut wrong_reasons = Vec::new();
    let mut applied = 0;
    let mut necessary = 0;
    for command in ["layer.mergeDown", "layer.stampDown", "file.scripts.flattenAllLayerEffects"] {
        for pattern in [false, true] {
            let bytes = fixture(pattern);
            for relevant in [true, false] {
                let label = format!("{command}/{} /{relevant}", if pattern { "pattern" } else { "global light" });
                // Necessity is fixed before the first run: baking uses the changed resource;
                // a note does not affect its result or its destination.
                let expected_refusal = relevant;
                let f = Fixture::from_bytes(FILE, &bytes, CallOptions::default());
                let (details, basis) = f.inspect().await;
                let id = ids_by_name(&details)["L2"];
                if relevant && pattern {
                    let mut other = kition_photo_format::load_from_bytes(&std::fs::read(f.file()).unwrap()).unwrap();
                    let pattern = other.patterns.first_mut().unwrap();
                    pattern.surface.fill_rect(kition_photo_engine::doc::Rect::from_xywh(0, 0, pattern.width, pattern.height), &[1.0, 0.0, 0.0]);
                    std::fs::write(f.file(), kition_photo_format::save_to_bytes(&other, &Default::default()).unwrap()).unwrap();
                } else if relevant {
                    f.colleague(&[("layer.layerStyle.globalLight", json!({"angle":0,"altitude":60}))]);
                } else {
                    f.colleague(&[("note.add", json!({"x":10,"y":10,"text":"Colleague"}))]);
                }
                let before = std::fs::read(f.file()).unwrap();
                let params = if command.starts_with("layer.") { json!({"layer":id}) } else { json!({}) };
                let result = f.call("run", json!({"basis":basis,"steps":[{"command":command,"params":params}]})).await;
                match (result, expected_refusal) {
                    (Ok(_), true) => wrong.push(label),
                    (Err(error), true) => {
                        assert_eq!(error["error"]["code"], "conflict", "{label}: {error}");
                        if !error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == "changed") {
                            wrong_reasons.push(label);
                        }
                        assert_eq!(std::fs::read(f.file()).unwrap(), before);
                        necessary += 1;
                    }
                    (Err(error), false) => false_refusals.push(format!("{label}: {error}")),
                    (Ok(_), false) => {
                        let mut reference = PhotoDocument::open(FILE, &before).unwrap();
                        let state = reference.editor_state();
                        reference.execute(command, params).unwrap();
                        reference.restore_editor_state(&state).unwrap();
                        assert_eq!(saved(&std::fs::read(f.file()).unwrap()), saved(&reference.save(Some(FILE)).unwrap()), "{label}: saved merge");
                        applied += 1;
                    }
                }
                f.shutdown().await;
            }
        }
    }
    println!(
        "DERIVED cells=12 applied={applied} necessary={necessary} wrong={} false={} reason={}\nwrong={wrong:?}\nfalse={false_refusals:?}",
        wrong.len(),
        false_refusals.len(),
        wrong_reasons.len()
    );
    assert!(wrong.is_empty() && false_refusals.is_empty() && wrong_reasons.is_empty());
}
