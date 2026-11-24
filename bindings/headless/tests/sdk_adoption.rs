#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod common;
use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn raw_field_contracts_keep_same_field_decisions_and_merge_other_fields() {
    for related in [true, false] {
        let f = Fixture::new("Raw.kphoto", 2);
        let (view, basis) = f.inspect().await;
        let id = ids_by_name(&view)["L0"];
        f.colleague(&[("layer.setProps", if related { json!({"layer":id,"opacity":0.5}) } else { json!({"layer":id,"name":"Outside"}) })]);
        let before = std::fs::read(f.file()).unwrap();
        let result = f.call("batch", json!({"basis":basis,"ops":[{"op":"set","target":id,"field":"opacity","value":0.5}]})).await;
        if related {
            let error = result.expect_err("same-field raw assignment");
            assert_eq!(error["error"]["code"], "conflict", "{error}");
            assert!(error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == "changed"), "{error}");
            assert_eq!(std::fs::read(f.file()).unwrap(), before);
        } else {
            result.unwrap_or_else(|e| panic!("unrelated raw write: {e}"));
            let layer = layer_on_disk(&f.file(), id).unwrap();
            assert_eq!(layer["name"], "Outside");
            assert_eq!(layer["opacity"], 0.5);
        }
        f.shutdown().await;
    }
}
#[tokio::test(flavor = "multi_thread")]
async fn several_channel_sources_merge_private_coverage_on_the_image() {
    for pixels in [false, true] {
        for source in ["First", "Second", "Unrelated"] {
            let mut doc = PhotoDocument::create("Sources.kphoto", 32, 32).unwrap();
            for (name, x) in [("First", 1), ("Second", 10), ("Unrelated", 20)] {
                doc.execute("select.rect", json!({"x":x,"y":1,"width":5,"height":5})).unwrap();
                doc.execute("select.saveSelection", json!({"name":name})).unwrap();
            }
            doc.execute("select.deselect", json!({})).unwrap();
            let f = Fixture::from_bytes("Sources.kphoto", &doc.save_sealed(None).unwrap(), CallOptions::default());
            let (_, basis) = f.inspect().await;
            if pixels {
                f.colleague(&[
                    ("select.rect", json!({"x":2,"y":2,"width":8,"height":8})),
                    ("select.saveSelection", json!({"channel":source,"operation":"replace"})),
                    ("select.deselect", json!({})),
                ]);
            } else {
                f.colleague(&[("channel.rename", json!({"channel":source,"name":format!("Changed {source}")}))]);
            }
            let before = std::fs::read(f.file()).unwrap();
            let result = f
                .call(
                    "run",
                    json!({"basis":basis,"steps":[{"command":"image.calculations","params":{
            "source1":{"channel":"First"},"source2":{"channel":"Second"},"name":"Result"}}]}),
                )
                .await;
            if source == "Unrelated" {
                result.unwrap_or_else(|e| panic!("unrelated channel must merge: {e}"));
            } else {
                let error = result.expect_err("either source rename must refuse");
                assert_eq!(error["error"]["code"], "conflict", "{error}");
                let reason = if pixels { "changed" } else { "relocated" };
                assert!(error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == reason), "{error}");
                assert_eq!(std::fs::read(f.file()).unwrap(), before);
            }
            f.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn public_receipts_omit_private_derived_premises() {
    let f = Fixture::new("Receipt.kphoto", 2);
    let (view, basis) = f.inspect().await;
    let id = ids_by_name(&view)["L0"];
    let result = f.call("run", json!({"basis":basis,"steps":[{"command":"layer.translate","params":{"layer":id,"dx":8,"dy":0}}]})).await.unwrap();
    let changes = result["changes"].to_string();
    assert!(changes.contains("~bounds"), "proven public bounds change: {result}");
    for private in ["~pickState", "~canvasCoordinates", "~subtreeLabels", "~channelContents", "~inheritedLocks", "~linkGroups"] {
        assert!(!changes.contains(private), "private premise leaked: {result}");
    }
    let model = f.kits.described(kit().id).unwrap();
    assert_eq!(model["types"]["Layer"]["derived"]["pickState"]["premiseOnly"], true, "{}", model["types"]["Layer"]["derived"]);
    f.colleague(&[("layer.new.layer", json!({"name":"Outside"}))]);
    let created = f
        .ok(
            "run",
            json!({"basis":result["revision"],"steps":[{
                "command":"layer.setProps","params":{"layer":id,"opacity":0.5}
            }]}),
        )
        .await;
    for private in ["~pickState", "~canvasCoordinates", "~subtreeLabels", "~channelContents", "~inheritedLocks", "~linkGroups"] {
        assert!(!created["others"].to_string().contains(private), "private creation premise leaked: {created}");
    }
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn point_addresses_compare_masks_with_raw_surface_changes() {
    let mut failures = Vec::new();
    for change in ["create", "update", "delete"] {
        for related in [true, false] {
            let mut doc = image_doc("PointMasks.kphoto", 4);
            let ids = ids_by_name(&doc.inspect().unwrap());
            let target = ids["L3"];
            let changed = if related { target } else { ids["L2"] };
            match change {
                "update" => {
                    doc.execute("layer.layerMask.revealAll", json!({"layer":changed})).unwrap();
                }
                "delete" => {
                    doc.execute("layer.layerMask.hideAll", json!({"layer":changed})).unwrap();
                }
                _ => {}
            }
            let f = Fixture::from_bytes("PointMasks.kphoto", &doc.save_sealed(None).unwrap(), CallOptions::default());
            let (_, basis) = f.inspect().await;
            f.colleague(&[match change {
                "delete" => ("layer.layerMask.delete", json!({"layer":changed})),
                "update" => ("edit.fill", json!({"layer":changed,"target":"mask","color":"#000000","region":{"rect":block(if related {3} else {2})}})),
                _ => ("layer.layerMask.hideAll", json!({"layer":changed})),
            }]);
            let before = std::fs::read(f.file()).unwrap();
            let result = f
                .call(
                    "run",
                    json!({"basis":basis,"steps":[{
                        "command":"layer.setProps","params":{"at":middle(3),"opacity":0.625}
                    }]}),
                )
                .await;
            if related {
                match result {
                    Err(error) => {
                        if error["error"]["code"] != "conflict" || !error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == "relocated") {
                            failures.push(format!("{change}/related: {error}"));
                        }
                    }
                    Ok(answer) => failures.push(format!("{change}/related applied: {answer}")),
                }
                assert!(std::fs::read(f.file()).unwrap() == before, "{change}/related must not write");
            } else {
                match result {
                    Err(error) => failures.push(format!("{change}/unrelated: {error}")),
                    Ok(_) => {
                        let mut reference = PhotoDocument::open("PointMasks.kphoto", &before).unwrap();
                        reference.execute("layer.setProps", json!({"layer":target,"opacity":0.625})).unwrap();
                        let stored = |bytes: &[u8]| {
                            let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
                            let manifest: serde_json::Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
                            let mut doc = manifest["document"].clone();
                            // Session handles and per-person selection are not document content.
                            doc.as_object_mut().unwrap().remove("id");
                            doc.as_object_mut().unwrap().remove("selection");
                            doc
                        };
                        assert_eq!(stored(&std::fs::read(f.file()).unwrap()), stored(&reference.save_sealed(None).unwrap()), "{change}/unrelated saved merge");
                    }
                }
            }
            f.shutdown().await;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_returns_a_completion_without_a_document_view() {
    let f = Fixture::new("Close.kphoto", 1);
    let (_, basis) = f.inspect().await;
    let before = std::fs::read(f.file()).unwrap();
    let result = f.call("run", json!({"basis":basis,"steps":[{"command":"file.close","params":{}}]})).await.unwrap();
    assert_eq!(result["completed"][0]["result"]["closed"], true, "{result}");
    assert!(result.get("document").is_none(), "closed document has no view: {result}");
    assert_eq!(std::fs::read(f.file()).unwrap(), before);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn c8_centered_canvas_resize_changes_the_region_coordinate_frame() {
    let f = Fixture::new("Coordinates.kphoto", 20);
    let (view, basis) = f.inspect().await;
    let target = ids_by_name(&view)["L11"];
    f.colleague(&[("image.canvasSize", json!({"width":1152,"height":1024,"anchor":"center"}))]);
    let before = std::fs::read(f.file()).unwrap();
    let error = f
        .call(
            "run",
            json!({"basis":basis,"steps":[{"command":"edit.fill","params":{
        "layer":target,"color":"#00ff00","region":{"rect":region(11)}}}]}),
        )
        .await
        .expect_err("shifted canvas coordinates must refuse");
    assert!(error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == "changed"), "{error}");
    assert_eq!(std::fs::read(f.file()).unwrap(), before);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn c8_point_comparison_reads_tiles_from_prior_agent_steps() {
    let f = Fixture::new("PointSteps.kphoto", 20);
    let (view, basis) = f.inspect().await;
    let target = ids_by_name(&view)["L10"];
    f.colleague(&[("layer.select", json!({"layer":target})), ("layer.new.layer", json!({"name":"Inserted"}))]);
    f.ok(
        "run",
        json!({"basis":basis,"steps":[{"command":"edit.fill","params":{
        "layer":target,"color":"#00ff00","region":{"rect":region(10)}}}]}),
    )
    .await;
    f.ok(
        "run",
        json!({"basis":basis,"steps":[{"command":"layer.setProps","params":{
        "at":middle(10),"opacity":0.625}}]}),
    )
    .await;
    assert_eq!(layer_on_disk(&f.file(), target).unwrap()["opacity"], 0.625);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn c8_point_comparison_keeps_outside_pixels_after_an_agent_fill() {
    for moved in [false, true] {
        let f = Fixture::new("OutsidePointSteps.kphoto", 20);
        let (view, basis) = f.inspect().await;
        let target = ids_by_name(&view)["L15"];
        f.colleague(&if moved {
            vec![("layer.translate", json!({"layer":target,"dx":16,"dy":0}))]
        } else {
            vec![
                ("select.rect", json!({"x":828,"y":60,"width":12,"height":12})),
                ("edit.fill", json!({"layer":target,"color":"#202020"})),
                ("select.deselect", json!({})),
            ]
        });
        f.ok(
            "run",
            json!({"basis":basis,"steps":[{"command":"edit.fill","params":{
            "layer":target,"color":"#00ff00","region":{"rect":[90,850,8,8]}}}]}),
        )
        .await;
        f.ok(
            "run",
            json!({"basis":basis,"steps":[{"command":"layer.setProps","params":{
            "at":middle(15),"opacity":0.625}}]}),
        )
        .await;
        assert_eq!(layer_on_disk(&f.file(), target).unwrap()["opacity"], 0.625);
        f.shutdown().await;
    }
}
