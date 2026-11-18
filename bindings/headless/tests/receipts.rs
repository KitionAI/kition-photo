//! What receipts say and cost, and how long a write takes (task card C-photo), printed for the
//! report (`--nocapture`): the bytes of a run's `changes` for the edits agents make most, on 5 and
//! 200 layers where the count matters; and the median of 10 writes (dev profile) plain, with
//! `basis`, after an unrelated change made in the file, and refused, on a small image and on the
//! largest real fixture (`web/public/sample/Sample.psd`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use std::time::Instant;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Image.kphoto";

async fn receipt(f: &Fixture, step: Value) -> Value {
    let ran = f.ok("run", json!({ "steps": [step] })).await;
    ran["changes"].clone()
}

fn size(value: &Value) -> usize {
    value.to_string().len()
}

#[tokio::test(flavor = "multi_thread")]
async fn receipt_bytes() {
    for layers in [5, 200] {
        let f = Fixture::new(FILE, layers);
        let (details, _) = f.inspect().await;
        let names = ids_by_name(&details);
        let (l0, l1) = (names["L0"], names["L1"]);
        let mut rows = Vec::new();
        if layers == 5 {
            let property = receipt(&f, json!({ "command": "layer.setProps", "params": { "layer": l1, "opacity": 0.5 } })).await;
            rows.push(("property (opacity)", property));
            let [x, y, ..] = block(1);
            let stroke = receipt(
                &f,
                json!({ "command": "paint.stroke", "params": { "layer": l1, "points": [[x, y, 1], [x + 100, y + 100, 1]], "size": 10, "color": "#000000" } }),
            )
            .await;
            rows.push(("paint stroke (one tile)", stroke));
            let fill = receipt(&f, json!({ "command": "edit.fill", "params": { "layer": l1, "color": "#00ff00", "region": { "rect": region(1) } } })).await;
            rows.push(("fill of a region", fill));
            let filter = receipt(&f, json!({ "command": "filter.blur.gaussianBlur", "params": { "layer": l1, "radius": 3 } })).await;
            rows.push(("filter on a full layer", filter));
            let whole = receipt(&f, json!({ "command": "edit.fill", "params": { "layer": l1, "color": "#00ff00", "region": "all" } })).await;
            rows.push(("fill of the whole canvas (1280 x 256, 5 tiles)", whole));
            let moved = receipt(&f, json!({ "command": "layer.translate", "params": { "layer": l0, "dx": 300, "dy": 0 } })).await;
            rows.push(("move layer (its pixels 300 px right)", moved));
        }
        let reorder = receipt(&f, json!({ "command": "layer.arrange.bringToFront", "params": { "layer": l0 } })).await;
        rows.push(("reorder: bottom to top", reorder));
        let width = details["width"].as_i64().unwrap();
        let resize =
            receipt(&f, json!({ "command": "image.canvasSize", "params": { "width": width - 128, "height": details["height"], "anchor": "center" } })).await;
        rows.push(("canvas resize", resize));
        for (what, changes) in rows {
            let text = changes.to_string();
            println!("RECEIPT {layers} layers | {what} | {} bytes | {}", size(&changes), text.chars().take(600).collect::<String>());
        }
        f.shutdown().await;
    }
}

/// The median of 10 runs of `write`, in milliseconds, with what runs before each untimed.
async fn timed(f: &Fixture, step: &Value, mode: &str, target: u64, other: u64) -> f64 {
    let mut samples = Vec::new();
    for n in 0..10 {
        // Whole steps of 1/255, which a PSD stores exactly (a PSD keeps 8 bits of opacity, and a
        // value it rounds reads as a change by others: `bindings/wasm/tests/saved.rs`).
        let opacity = f64::from(60 + n * 7) / 255.0;
        let mut step = step.clone();
        step["params"]["opacity"] = json!(opacity);
        let input = match mode {
            "plain" => json!({ "steps": [step] }),
            _ => {
                let (_, basis) = f.inspect().await;
                match mode {
                    "unrelated" => {
                        f.colleague(&[("layer.setProps", json!({ "layer": other, "opacity": f64::from(130 + n) / 255.0 }))]);
                    }
                    "refused" => {
                        f.colleague(&[("layer.setProps", json!({ "layer": target, "opacity": f64::from(230 - n) / 255.0 }))]);
                    }
                    _ => {}
                }
                json!({ "basis": basis, "steps": [step] })
            }
        };
        let start = Instant::now();
        let answer = f.call("run", input).await;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(answer.is_err(), mode == "refused", "{mode}: {answer:?}");
    }
    median(samples)
}

async fn timings(name: &str, bytes: &[u8], what: &str) {
    let f = Fixture::from_bytes(name, bytes, CallOptions::default());
    let (details, _) = f.inspect().await;
    let layers = layers_of(&details);
    let pixel: Vec<&Value> = layers.iter().filter(|layer| layer["kind"] == "Pixel").collect();
    let target = pixel.first().map_or_else(|| layers[0]["id"].as_u64().unwrap(), |layer| layer["id"].as_u64().unwrap());
    let other = layers.iter().map(|layer| layer["id"].as_u64().unwrap()).find(|id| *id != target).unwrap();
    let step = json!({ "command": "layer.setProps", "params": { "layer": target, "opacity": 0.5 } });
    let mut row = Vec::new();
    for mode in ["plain", "basis", "unrelated", "refused"] {
        row.push(format!("{mode} {:.0} ms", timed(&f, &step, mode, target, other).await));
    }
    println!("TIMING {what} ({} layers, {} x {} px, {} bytes): {}", layers.len(), details["width"], details["height"], bytes.len(), row.join(" | "));
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn write_times() {
    timings(FILE, &image(FILE, 8), "small .kphoto").await;
    let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/public/sample/Sample.psd");
    let bytes = std::fs::read(&sample).unwrap();
    let doc = PhotoDocument::open("Sample.psd", &bytes).unwrap();
    println!("Sample.psd: {}", doc.summary());
    timings("Sample.psd", &bytes, "Sample.psd").await;
    let kphoto = doc.save(Some("Sample.kphoto")).unwrap();
    timings("Sample.kphoto", &kphoto, "Sample as .kphoto").await;
}
