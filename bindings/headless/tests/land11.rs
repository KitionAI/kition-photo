//! C8-land-11p adoption exercised through the real MCP guard and file reread.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod common;
use common::*;
use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn non_target_layer_names_refuse_rename_and_deletion() {
    for deleted in [false, true] {
        let f = Fixture::new("art.kphoto", 2);
        let (details, basis) = f.inspect().await;
        let id = ids_by_name(&details)["L0"];
        f.colleague(&[if deleted { ("layer.delete", json!({"layer":id})) } else { ("layer.setProps", json!({"layer":id,"name":"Away"})) }]);
        // Measurement's non-target `with` uses render::layer_named, including whole names.
        let refused = f
            .refused(
                "run",
                json!({"basis":basis,"steps":[
                    {"command":"image.measure","params":{"layer":1,"with":"L0"}},
                    {"command":"layer.setProps","params":{"layer":1,"opacity":0.5}}
                ]}),
            )
            .await;
        assert_conflict(&refused, &id.to_string(), if deleted { "deleted" } else { "relocated" });
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn batch_group_delete_matches_command_and_receipt() {
    for name in ["art.kphoto", "art.psd"] {
        let mut doc = PhotoDocument::create(name, 32, 24).unwrap();
        let group = doc.execute("layer.new.group", json!({"name":"Group"})).unwrap()["layer"].as_u64().unwrap();
        let child = doc.execute("layer.new.layer", json!({"name":"Child"})).unwrap()["layer"].as_u64().unwrap();
        doc.execute("layer.layerMask.revealAll", json!({"layer":child})).unwrap();
        let bytes = doc.save(None).unwrap();
        let f = Fixture::from_bytes(name, &bytes, Default::default());
        let refused = f.refused("batch", json!({"ops":[{"op":"delete","target":group.to_string()}]})).await;
        assert_eq!(refused["error"]["code"], "invalid-value", "{refused}");
        assert!(refused["error"]["message"].as_str().unwrap().contains("has children"), "{refused}");
        assert!(layer_on_disk(&f.file(), group).is_some());
        let receipt = f
            .ok(
                "batch",
                json!({"ops":[
                    {"op":"delete","target":child.to_string()},
                    {"op":"delete","target":group.to_string()}
                ]}),
            )
            .await;
        assert!(ids(&receipt["changes"]["deleted"]).contains(&group.to_string()), "{receipt}");
        assert!(ids(&receipt["changes"]["deleted"]).contains(&child.to_string()), "{receipt}");
        assert!(receipt["changes"]["changed"].as_array().unwrap().is_empty(), "{receipt}");
        let mut command = PhotoDocument::open(name, &bytes).unwrap();
        command.execute("layer.delete", json!({"layer":group})).unwrap();
        assert_eq!(layers_of(&on_disk(&f.file()).inspect().unwrap()), layers_of(&command.inspect().unwrap()));
        let queried = f.ok("query", json!({"follow":"layers"})).await;
        assert_eq!(ids(&queried["objects"]), vec!["1"], "deleted tree nodes must be hidden: {queried}");
        println!("GROUP DELETE {name}: {receipt}; reread {queried}");
        f.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stored_path_name_is_held_even_without_a_model_object() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    doc.execute("path.set", json!({"name":"Clip","path":{"subpaths":[{"closed":true,"knots":[{"anchor":[1,1]},{"anchor":[10,1]},{"anchor":[10,10]}]}]}}))
        .unwrap();
    let f = Fixture::from_bytes("art.kphoto", &doc.save(None).unwrap(), Default::default());
    let (_, basis) = f.inspect().await;
    f.colleague(&[("path.rename", json!({"name":"Clip","to":"Away"}))]);
    let refused = f.refused("run", json!({"basis":basis,"steps":[{"command":"path.rename","params":{"name":"Clip","to":"Final"}}]})).await;
    // SDK cannot classify names held inside JSON; its whole-register premise is conservative.
    assert_conflict(&refused, "image", "changed");
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn point_coordinates_are_not_layer_ids() {
    let f = Fixture::new("art.kphoto", 2);
    let (_, basis) = f.inspect().await;
    f.colleague(&[("layer.setProps", json!({"layer":2,"name":"Away"}))]);
    let answer = f.ok("run", json!({"basis":basis,"steps":[{"command":"layer.setProps","params":{"at":[2,2],"opacity":0.5}}]})).await;
    assert_eq!(ids(&answer["changes"]["changed"]), vec!["1"]);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn channel_names_are_scoped_and_hold_their_resource() {
    for deleted in [false, true] {
        let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
        doc.execute("layer.setProps", json!({"layer":1,"name":"Shared"})).unwrap();
        doc.execute("channel.new", json!({"name":"Shared"})).unwrap();
        let f = Fixture::from_bytes("art.kphoto", &doc.save(None).unwrap(), Default::default());
        let (_, basis) = f.inspect().await;
        f.colleague(&[("layer.setProps", json!({"layer":1,"name":"Other"}))]);
        f.ok("run", json!({"basis":basis,"steps":[{"command":"channel.rename","params":{"channel":"Shared","name":"Alpha"}}]})).await;
        let (_, basis) = f.inspect().await;
        f.colleague(&[if deleted { ("channel.delete", json!({"channel":"Alpha"})) } else { ("channel.rename", json!({"channel":"Alpha","name":"Away"})) }]);
        let refused = f.refused("run", json!({"basis":basis,"steps":[{"command":"channel.rename","params":{"channel":"Alpha","name":"Final"}}]})).await;
        println!("CHANNEL NAME deleted={deleted}: {refused}");
        assert_eq!(refused["error"]["code"], "conflict", "{refused}");
        assert_conflict(&refused, "image", if deleted { "deleted" } else { "changed" });
        f.shutdown().await;
    }
}
