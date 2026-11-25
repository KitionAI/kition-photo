//! C8-land-12-photo: saved receipts and adopted SDK contracts over MCP.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod common;
use common::*;
use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

fn saved(bytes: &[u8]) -> serde_json::Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    for field in ["id", "selection"] {
        doc.as_object_mut().unwrap().remove(field);
    }
    doc
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_creation_is_refused_instead_of_receipted_and_lost() {
    for (kind, parent) in [("shape", json!(null)), ("raster", json!("1"))] {
        let doc = PhotoDocument::create("art.psd", 32, 24).unwrap();
        let f = Fixture::from_bytes("art.psd", &doc.save(None).unwrap(), Default::default());
        let before = layers_of(&on_disk(&f.file()).inspect().unwrap());
        let result = f
            .call(
                "batch",
                json!({"ops":[{"op":"create","type":"Layer","in":"root.layers","parent":parent,"fields":{
                    "name":"New", "kind":kind, "visible":true, "blend":"Normal", "opacity":1, "fillOpacity":1, "clipped":false
                }}]}),
            )
            .await;
        assert!(result.is_err(), "invalid {kind} creation reported as success: {result:?}");
        assert_eq!(layers_of(&on_disk(&f.file()).inspect().unwrap()), before);
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn rectangle_guard_compares_the_addressed_pixels_inside_a_tile() {
    for inside in [false, true] {
        let f = Fixture::new("art.kphoto", 1);
        let (seen, basis) = f.inspect().await;
        let id = ids_by_name(&seen)["L0"];
        let rect = if inside { [80, 80, 8, 8] } else { [150, 150, 8, 8] };
        f.colleague(&[
            ("select.rect", json!({"x":rect[0],"y":rect[1],"width":rect[2],"height":rect[3]})),
            ("edit.fill", json!({"layer":id,"color":"#000000"})),
            ("edit.deselectAll", json!({})),
        ]);
        let before = std::fs::read(f.file()).unwrap();
        let params = json!({"layer":id,"color":"#00ff00","region":{"rect":[70,70,40,40]}});
        let answer = f.call("run", json!({"basis":basis,"steps":[{"command":"edit.fill","params":params}]})).await;
        if inside {
            assert_conflict(&answer.expect_err("same addressed pixels changed"), &id.to_string(), "changed");
            assert_eq!(std::fs::read(f.file()).unwrap(), before);
        } else {
            answer.unwrap_or_else(|e| panic!("unrelated pixels must merge: {e}"));
            let mut reference = PhotoDocument::open("art.kphoto", &before).unwrap();
            reference.execute("select.rect", json!({"x":70,"y":70,"width":40,"height":40})).unwrap();
            reference.execute("edit.fill", json!({"layer":id,"color":"#00ff00"})).unwrap();
            assert_eq!(saved(&std::fs::read(f.file()).unwrap()), saved(&reference.save_sealed(None).unwrap()));
        }
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn json_comp_names_keep_same_field_and_deletion_reasons() {
    for deleted in [false, true] {
        let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
        doc.execute("layerComp.new", json!({"name":"Comp"})).unwrap();
        let f = Fixture::from_bytes("art.kphoto", &doc.save_sealed(None).unwrap(), Default::default());
        let (_, basis) = f.inspect().await;
        f.colleague(&[if deleted { ("layerComp.delete", json!({"comp":"Comp"})) } else { ("layerComp.rename", json!({"comp":"Comp","name":"Away"})) }]);
        let answer = f.refused("run", json!({"basis":basis,"steps":[{"command":"layerComp.rename","params":{"comp":"Comp","name":"Final"}}]})).await;
        assert_conflict(&answer, "image", if deleted { "deleted" } else { "changed" });
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn json_pattern_sources_keep_rename_and_deletion_reasons() {
    for deleted in [false, true] {
        let mut doc = image_doc("art.kphoto", 1);
        doc.execute("layer.newFillLayer.pattern", json!({"pattern":"Checkerboard"})).unwrap();
        let f = Fixture::from_bytes("art.kphoto", &doc.save_sealed(None).unwrap(), Default::default());
        let (seen, basis) = f.inspect().await;
        let id = ids_by_name(&seen)["L0"];
        let mut other = kition_photo_format::load_from_bytes(&std::fs::read(f.file()).unwrap()).unwrap();
        if deleted {
            other.patterns.clear();
        } else {
            other.patterns.first_mut().unwrap().name = "Away".into();
        }
        std::fs::write(f.file(), kition_photo_format::save_to_bytes(&other, &Default::default()).unwrap()).unwrap();
        let answer = f
            .refused(
                "run",
                json!({"basis":basis,"steps":[{"command":"edit.fill","params":{
                    "layer":id,"contents":"pattern","pattern":"Checkerboard"
                }}]}),
            )
            .await;
        assert_conflict(&answer, "image", if deleted { "deleted" } else { "relocated" });
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn json_text_style_ids_still_need_a_resource_identity_contract() {
    for prefix in ["type.characterStyle", "type.paragraphStyle"] {
        for deleted in [false, true] {
            let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
            let id = doc.execute(&format!("{prefix}.new"), json!({"name":"Style","fromSelection":false})).unwrap()["id"].clone();
            let f = Fixture::from_bytes("art.kphoto", &doc.save_sealed(None).unwrap(), Default::default());
            let (_, basis) = f.inspect().await;
            let command = format!("{prefix}.{}", if deleted { "delete" } else { "rename" });
            let params = if deleted { json!({"id":id}) } else { json!({"id":id,"name":"Away"}) };
            f.colleague(&[(&command, params)]);
            let answer = f.refused("run", json!({"basis":basis,"steps":[{"command":format!("{prefix}.rename"),"params":{"id":id,"name":"Final"}}]})).await;
            assert_conflict(&answer, "image", "changed");
            f.shutdown().await;
        }
    }
}
