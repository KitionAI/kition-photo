#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod common;
use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    for field in ["id", "selection"] {
        doc.as_object_mut().unwrap().remove(field);
    }
    doc
}

async fn precision(class: &str) {
    for related in [true, false] {
        let mut doc = PhotoDocument::create("Precision.kphoto", 32, 32).unwrap();
        let id = doc.execute("layer.new.layer", json!({"name":"Target"})).unwrap()["layer"].clone();
        let other = doc.execute("layer.new.layer", json!({"name":"Other"})).unwrap()["layer"].clone();
        doc.execute("layer.select", json!({"layer":id})).unwrap();
        let (command, params, change) = match class {
            "mask" => {
                doc.execute("layer.layerMask.revealAll", json!({"layer":id})).unwrap();
                (
                    "layer.layerMask.enabled",
                    json!({"layer":id,"enabled":false}),
                    if related {
                        vec![("layer.layerMask.enabled", json!({"layer":id,"enabled":false}))]
                    } else {
                        vec![("edit.fill", json!({"layer":id,"target":"mask","color":"#555555","region":{"rect":[1,1,4,4]}}))]
                    },
                )
            }
            "vector" => {
                doc.execute("layer.vectorMask.add", json!({"layer":id,"path":{"subpaths":[]}})).unwrap();
                (
                    "layer.vectorMask.edit",
                    json!({"layer":id,"density":50}),
                    if related {
                        vec![("layer.vectorMask.edit", json!({"layer":id,"density":50}))]
                    } else {
                        vec![("layer.setLabelColor", json!({"layer":id,"color":"red"}))]
                    },
                )
            }
            _ => (
                "layer.linkLayers",
                json!({}),
                if related {
                    vec![("layer.select", json!({"layer":id})), ("layer.select", json!({"layer":other,"mode":"add"})), ("layer.linkLayers", json!({}))]
                } else {
                    vec![("layer.setLabelColor", json!({"layer":id,"color":"red"}))]
                },
            ),
        };
        let f = Fixture::from_bytes("Precision.kphoto", &doc.save(None).unwrap(), CallOptions::default());
        let (_, basis) = f.inspect().await;
        f.colleague(&change);
        let before = std::fs::read(f.file()).unwrap();
        let mut step = json!({"command":command,"params":params});
        if class == "links" {
            step["on"] = json!([id, other]);
        }
        let result = f.call("run", json!({"steps":[step],"basis":basis})).await;
        if related {
            let error = result.expect_err(&format!("{class}: same-field write must refuse"));
            assert_eq!(error["error"]["code"], "conflict", "{class}: {error}");
            assert!(error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == "changed"), "{class}: {error}");
            assert_eq!(std::fs::read(f.file()).unwrap(), before);
        } else {
            result.unwrap_or_else(|e| panic!("{class}: unrelated path must merge: {e}"));
            let mut reference = PhotoDocument::open("Precision.kphoto", &before).unwrap();
            if class == "links" {
                reference.execute("layer.select", json!({"layer":id})).unwrap();
                reference.execute("layer.select", json!({"layer":other,"mode":"add"})).unwrap();
            }
            reference.execute(command, params).unwrap();
            assert_eq!(saved(&std::fs::read(f.file()).unwrap()), saved(&reference.save(None).unwrap()), "{class}: complete saved merge");
        }
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn mask_flags() {
    precision("mask").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn vector_mask_properties() {
    precision("vector").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn layer_links() {
    precision("links").await;
}
