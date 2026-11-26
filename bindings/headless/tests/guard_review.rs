//! Adversarial premise regressions, C-photo review. Each call uses an inspect before an
//! independent writer changes the file; controls change a fact the command does not read.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "Review.kphoto";

fn fixture_doc() -> PhotoDocument {
    image_doc(FILE, 3)
}

fn fixture(doc: PhotoDocument) -> Fixture {
    Fixture::from_bytes(FILE, &doc.save(Some(FILE)).unwrap(), CallOptions::default())
}

fn step(command: &str, params: Value) -> Value {
    json!({ "command": command, "params": params })
}

async fn check(f: &Fixture, basis: &str, command: &str, params: Value, refuses: bool) {
    let before = std::fs::read(f.file()).unwrap();
    let result = f.call("run", json!({ "basis": basis, "steps": [step(command, params)] })).await;
    if refuses {
        let error = result.expect_err("a changed premise must refuse");
        assert_eq!(error["error"]["code"], "conflict", "{command}: {error}");
        assert_eq!(std::fs::read(f.file()).unwrap(), before, "a refused call writes nothing");
    } else {
        result.unwrap_or_else(|e| panic!("{command}: unrelated edit refused: {e}"));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_region_source_is_a_premise_but_its_opacity_is_not() {
    for relevant in [false, true] {
        let f = fixture(fixture_doc());
        let (details, basis) = f.inspect().await;
        let ids = ids_by_name(&details);
        let (target, source) = (ids["L0"], ids["L1"]);
        f.colleague(&[if relevant {
            ("layer.translate", json!({ "layer": source, "dx": 300, "dy": 0 }))
        } else {
            ("layer.setProps", json!({ "layer": source, "opacity": 0.5 }))
        }]);
        check(&f, &basis, "edit.fill", json!({ "layer": target, "color": "#00ff00", "region": { "layer": source } }), relevant).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_filter_canvas_and_pixels_are_premises_but_display_mask_is_not() {
    for kind in 0..3 {
        let mut doc = fixture_doc();
        let id = ids_by_name(&doc.inspect().unwrap())["L0"];
        doc.execute("layer.layerMask.revealAll", json!({ "layer": id })).unwrap();
        let f = fixture(doc);
        let (_, basis) = f.inspect().await;
        let changes = match kind {
            0 => vec![("image.canvasSize", json!({ "width": 1000, "height": 256, "anchor": "topLeft" }))],
            1 => vec![("layer.select", json!({ "layer": id })), ("edit.fill", json!({ "color": "#000000" }))],
            _ => vec![("layer.select", json!({ "layer": id })), ("edit.fill", json!({ "target": "mask", "color": "#000000" }))],
        };
        f.colleague(&changes);
        check(&f, &basis, "filter.blur.gaussianBlur", json!({ "layer": id, "radius": 2 }), kind != 2).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_channel_pixels_are_held_without_holding_unrelated_layer_pixels() {
    for (command, extra) in [("image.adjustments.invert", json!({})), ("paint.bucket", json!({ "x": 10, "y": 10, "color": "#00ff00" }))] {
        for relevant in [false, true] {
            let mut doc = fixture_doc();
            doc.execute("select.rect", json!({ "x": 0, "y": 0, "width": 50, "height": 50 })).unwrap();
            doc.execute("select.saveSelection", json!({ "name": "Alpha" })).unwrap();
            doc.execute("edit.deselectAll", json!({})).unwrap();
            let f = fixture(doc);
            let (details, basis) = f.inspect().await;
            let id = ids_by_name(&details)["L0"];
            f.colleague(&[
                ("layer.select", json!({ "layer": id })),
                ("edit.fill", json!({ "color": "#000000", "target": if relevant { json!({ "channel": 0 }) } else { json!("pixels") } })),
            ]);
            let mut params = extra.clone();
            params["layer"] = json!(id);
            params["target"] = json!({ "channel": 0 });
            check(&f, &basis, command, params, relevant).await;
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_mask_pixels_are_held_when_the_mask_is_the_target() {
    for relevant in [false, true] {
        let mut doc = fixture_doc();
        let id = ids_by_name(&doc.inspect().unwrap())["L0"];
        doc.execute("layer.layerMask.revealAll", json!({ "layer": id })).unwrap();
        let f = fixture(doc);
        let (_, basis) = f.inspect().await;
        f.colleague(&[
            ("layer.select", json!({ "layer": id })),
            ("edit.fill", json!({ "color": "#000000", "target": if relevant { "mask" } else { "pixels" } })),
        ]);
        check(&f, &basis, "image.adjustments.invert", json!({ "layer": id, "target": "mask" }), relevant).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_pattern_contents_are_doc_state_premises() {
    for relevant in [false, true] {
        let mut doc = fixture_doc();
        doc.execute("layer.newFillLayer.pattern", json!({ "pattern": "Checkerboard" })).unwrap();
        let f = fixture(doc);
        let (details, basis) = f.inspect().await;
        let id = ids_by_name(&details)["L0"];
        if relevant {
            // An independent writer redefines the stored pattern using other existing pixel blobs.
            let mut other = kition_photo_format::load_from_bytes(&std::fs::read(f.file()).unwrap()).unwrap();
            let pattern = other.patterns.first_mut().unwrap();
            let rect = kition_photo_engine::doc::Rect::from_xywh(0, 0, pattern.width, pattern.height);
            pattern.surface.fill_rect(rect, &[1.0, 0.0, 0.0]);
            std::fs::write(f.file(), kition_photo_format::save_to_bytes(&other, &Default::default()).unwrap()).unwrap();
        } else {
            f.colleague(&[("layer.setProps", json!({ "layer": id, "name": "Renamed" }))]);
        }
        check(&f, &basis, "edit.fill", json!({ "layer": id, "contents": "pattern", "pattern": "Checkerboard" }), relevant).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_nested_apply_image_source_holds_the_source_layer() {
    for relevant in [false, true] {
        let f = fixture(fixture_doc());
        let (details, basis) = f.inspect().await;
        let ids = ids_by_name(&details);
        f.colleague(&[("layer.select", json!({ "layer": if relevant { ids["L1"] } else { ids["L2"] } })), ("edit.fill", json!({ "color": "#000000" }))]);
        check(&f, &basis, "image.applyImage", json!({ "layer": ids["L0"], "source": { "layer": ids["L1"], "channel": "rgb" } }), relevant).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_hdr_toning_holds_all_layers_not_the_active_layer() {
    let f = fixture(fixture_doc());
    let (details, basis) = f.inspect().await;
    let id = ids_by_name(&details)["L0"];
    f.colleague(&[("layer.setProps", json!({ "layer": id, "opacity": 0.5 }))]);
    check(&f, &basis, "image.adjustments.hdrToning", json!({ "detail": 10 }), true).await;
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_apply_image_default_merged_source_holds_composite_inputs() {
    let f = fixture(fixture_doc());
    let (details, basis) = f.inspect().await;
    let ids = ids_by_name(&details);
    f.colleague(&[("layer.setProps", json!({ "layer": ids["L1"], "opacity": 0.5 }))]);
    check(&f, &basis, "image.applyImage", json!({ "layer": ids["L0"] }), true).await;
    f.shutdown().await;
}

async fn merged_input(command: &str, extra: Value, source_key: &str) {
    for merged in [false, true] {
        let f = fixture(fixture_doc());
        let (details, basis) = f.inspect().await;
        let ids = ids_by_name(&details);
        f.colleague(&[("layer.setProps", json!({ "layer": ids["L1"], "opacity": 0.5 }))]);
        let mut params = extra.clone();
        params["layer"] = json!(ids["L0"]);
        if !merged {
            params[source_key] = json!(ids["L0"]);
        }
        check(&f, &basis, command, params, merged).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_displace_merged_input_holds_contributing_layers() {
    merged_input("filter.distort.displace", json!({ "mapDocument": 0 }), "mapLayer").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_match_color_merged_input_holds_contributing_layers() {
    merged_input("image.adjustments.matchColor", json!({ "source": 0 }), "sourceLayer").await;
}

async fn sampled_paint(command: &str, extra: Value, sample_key: &str, sampled: Value, current: Value) {
    for all in [false, true] {
        let f = fixture(fixture_doc());
        let (details, basis) = f.inspect().await;
        let ids = ids_by_name(&details);
        f.colleague(&[("layer.setProps", json!({ "layer": ids["L1"], "opacity": 0.5 }))]);
        let mut params = extra.clone();
        params["layer"] = json!(ids["L0"]);
        params[sample_key] = if all { sampled.clone() } else { current.clone() };
        check(&f, &basis, command, params, all).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_bucket_sample_all_layers_holds_the_composite() {
    sampled_paint("paint.bucket", json!({ "x": 320, "y": 80, "color": "#00ff00" }), "sampleAllLayers", json!(true), json!(false)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_mixer_resolved_brush_sampling_holds_the_composite() {
    sampled_paint(
        "paint.mixerBrush",
        json!({ "points": [[320, 80], [330, 80]], "size": 8 }),
        "brush",
        json!({ "mixer": { "sampleAllLayers": true } }),
        json!({ "mixer": { "sampleAllLayers": false } }),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_clone_sample_all_layers_holds_the_composite() {
    sampled_paint("paint.cloneStamp", json!({ "points": [[70, 80]], "source": [320, 80], "size": 8 }), "sampleLayer", json!("all"), json!("current")).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_focus_sample_all_layers_holds_the_composite() {
    sampled_paint("paint.blur", json!({ "points": [[320, 80]], "size": 8 }), "sampleAllLayers", json!(true), json!(false)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn review_composite_sampling_holds_masks_clipping_and_adjustments_but_raw_fill_does_not() {
    for change in ["mask", "clipping", "adjustment"] {
        for sample in [false, true] {
            let mut doc = fixture_doc();
            let ids = ids_by_name(&doc.inspect().unwrap());
            doc.execute("layer.layerMask.revealAll", json!({ "layer": ids["L1"] })).unwrap();
            let f = fixture(doc);
            let (_, basis) = f.inspect().await;
            let before = std::fs::read(f.file()).unwrap();
            let edits = match change {
                "mask" => vec![("layer.select", json!({ "layer": ids["L1"] })), ("edit.fill", json!({ "target": "mask", "color": "#000000" }))],
                "clipping" => vec![("layer.setProps", json!({ "layer": ids["L1"], "clipped": true }))],
                _ => vec![("layer.select", json!({ "layer": ids["L1"] })), ("layer.newAdjustmentLayer.invert", json!({}))],
            };
            f.colleague(&edits);
            assert_ne!(std::fs::read(f.file()).unwrap(), before, "the {change} edit must change the file");
            let (command, params) = if sample {
                ("paint.bucket", json!({ "layer": ids["L0"], "x": 320, "y": 80, "color": "#00ff00", "sampleAllLayers": true }))
            } else {
                ("edit.fill", json!({ "layer": ids["L0"], "color": "#00ff00" }))
            };
            check(&f, &basis, command, params, sample).await;
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_editable_filter_stack_is_held_but_display_opacity_is_not() {
    for stack in [false, true] {
        let mut doc = fixture_doc();
        let old = ids_by_name(&doc.inspect().unwrap())["L0"];
        let id = doc.execute("layer.objectLayers.convertToObjectLayer", json!({ "layer": old })).unwrap()["layer"].clone();
        let f = fixture(doc);
        let (_, basis) = f.inspect().await;
        f.colleague(&[if stack {
            ("filter.blur.gaussianBlur", json!({ "layer": id, "radius": 2 }))
        } else {
            ("layer.setProps", json!({ "layer": id, "opacity": 0.5 }))
        }]);
        check(&f, &basis, "filter.blur.gaussianBlur", json!({ "layer": id, "radius": 3 }), stack).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_linked_move_holds_the_layers_it_also_moves_but_fill_does_not() {
    for moving in [false, true] {
        let mut doc = fixture_doc();
        let ids = ids_by_name(&doc.inspect().unwrap());
        doc.set_context(&kition_kit::CallContext { on: vec![json!(ids["L0"]), json!(ids["L1"])], ..Default::default() }).unwrap();
        doc.execute("layer.linkLayers", json!({})).unwrap();
        let f = fixture(doc);
        let (_, basis) = f.inspect().await;
        f.colleague(&[("layer.select", json!({ "layer": ids["L1"] })), ("edit.fill", json!({ "color": "#000000" }))]);
        let (command, params) = if moving {
            ("layer.translate", json!({ "layer": ids["L0"], "dx": 10, "dy": 0 }))
        } else {
            ("edit.fill", json!({ "layer": ids["L0"], "color": "#00ff00" }))
        };
        check(&f, &basis, command, params, moving).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_reparenting_holds_inherited_locks_but_not_group_display_opacity() {
    for locks in [false, true] {
        let mut doc = fixture_doc();
        let target = ids_by_name(&doc.inspect().unwrap())["L0"];
        let group = doc.execute("layer.new.group", json!({ "name": "Group" })).unwrap()["layer"].clone();
        doc.execute(
            "layer.setProps",
            if locks { json!({ "layer": group, "locks": { "transparency": true } }) } else { json!({ "layer": group, "opacity": 0.5 }) },
        )
        .unwrap();
        let f = fixture(doc);
        let (_, basis) = f.inspect().await;
        f.colleague(&[("layer.moveTo", json!({ "layer": target, "target": group, "position": "into" }))]);
        check(&f, &basis, "edit.fill", json!({ "layer": target, "color": "#00ff00" }), locks).await;
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn review_comp_and_style_definitions_are_held_beside_unrelated_metadata() {
    for style in [false, true] {
        for relevant in [false, true] {
            let mut doc = fixture_doc();
            let key = if style {
                doc.execute("text.add", json!({ "text": "Type", "x": 10, "y": 30, "size": 14 })).unwrap();
                doc.execute("text.characterStyle.new", json!({ "name": "Style", "attrs": { "size": 20 }, "fromSelection": false })).unwrap()["id"].clone()
            } else {
                doc.execute("layerComp.new", json!({ "name": "Comp" })).unwrap()["comp"].clone()
            };
            let f = fixture(doc);
            let (_, basis) = f.inspect().await;
            let change = match (style, relevant) {
                (true, true) => ("text.characterStyle.set", json!({ "id": key, "attrs": { "size": 30 } })),
                (true, false) => ("layerComp.new", json!({ "name": "Unrelated" })),
                (false, true) => ("layerComp.rename", json!({ "comp": key, "name": "Changed" })),
                (false, false) => ("text.characterStyle.new", json!({ "name": "Unrelated", "fromSelection": false })),
            };
            f.colleague(&[change]);
            let (command, params) = if style {
                ("text.characterStyle.rename", json!({ "id": key, "name": "Agent" }))
            } else {
                ("layerComp.rename", json!({ "comp": key, "name": "Agent" }))
            };
            check(&f, &basis, command, params, relevant).await;
            f.shutdown().await;
        }
    }
}
