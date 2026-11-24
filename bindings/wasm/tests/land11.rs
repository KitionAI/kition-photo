//! C8-land-11p: numeric model limits and saved batch values.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kition_kit::KitDocument;
use kition_kit::session::model::Exchange;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[test]
fn batch_limits_refuse_without_changing_the_document() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    let mut exchange = Exchange::new();
    for (target, field, value) in [
        (json!("1"), "opacity", json!(-0.01)),
        (json!("1"), "opacity", json!(1.01)),
        (json!("1"), "fillOpacity", json!(101)),
        (json!("image"), "width", json!(0)),
        (json!("image"), "height", json!(300_001)),
        (json!("image"), "dpi", json!(0)),
        (json!("image"), "dpi", json!(30_001)),
    ] {
        let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
        let result = exchange.batch(&mut doc, &json!({"ops":[{"op":"set","target":target,"field":field,"value":value}]}), false);
        assert!(result.is_err(), "accepted {field} = {value}: {result:?}");
        assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
    }
}

#[test]
fn batch_values_are_the_values_the_file_stores() {
    for name in ["art.psd", "art.psb", "art.kphoto"] {
        let mut doc = PhotoDocument::create(name, 32, 24).unwrap();
        let mut exchange = Exchange::new();
        exchange
            .batch(
                &mut doc,
                &json!({"ops":[
                    {"op":"set","target":"image","field":"dpi","value":72.12345},
                    {"op":"set","target":"1","field":"opacity","value":0.37},
                    {"op":"set","target":"1","field":"fillOpacity","value":0.63}
                ]}),
                false,
            )
            .unwrap();
        let mut again = PhotoDocument::open(name, &doc.save_sealed(None).unwrap()).unwrap();
        let schema = kdm_model::PhotoModel::schema();
        let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
        let after = Exchange::new().replica(&mut again).unwrap().state_json().unwrap();
        let diff = kition_kit::kdm::reread::diff_paired(&schema, &before, &after).to_json();
        for list in ["created", "changed", "moved", "deleted"] {
            assert_eq!(diff[list], json!([]), "{name}: {diff}");
        }
        assert_eq!(doc.inspect().unwrap()["layers"][0]["opacity"], again.inspect().unwrap()["layers"][0]["opacity"]);
    }
}

use kition_kit::session::KitModel;

#[test]
fn owned_surfaces_delete_with_only_their_owner() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    doc.execute("layer.layerMask.revealAll", json!({"layer":1})).unwrap();
    doc.execute("channel.new", json!({"name":"Alpha"})).unwrap();
    let mut exchange = Exchange::new();
    let replica = exchange.replica(&mut doc).unwrap();
    assert_eq!(replica.objects_of_type("Surface").len(), 2);
    let channel = replica.objects_of_type("Channel").pop().unwrap();
    exchange.batch(&mut doc, &json!({"ops":[{"op":"delete","target":"1"}]}), false).unwrap();
    let replica = exchange.replica(&mut doc).unwrap();
    assert!(replica.objects_of_type("Layer").is_empty());
    assert_eq!(replica.objects_of_type("Surface").len(), 1);
    assert!(replica.exists(&channel));
    assert_eq!(doc.inspect().unwrap()["channels"]["alpha"].as_array().unwrap().len(), 1);
    exchange.batch(&mut doc, &json!({"ops":[{"op":"delete","target":channel.to_string()}]}), false).unwrap();
    let replica = exchange.replica(&mut doc).unwrap();
    assert!(replica.objects_of_type("Channel").is_empty());
    assert!(replica.objects_of_type("Surface").is_empty());
    let mut reopened = PhotoDocument::open("art.kphoto", &doc.save_sealed(None).unwrap()).unwrap();
    assert!(Exchange::new().replica(&mut reopened).unwrap().objects_of_type("Surface").is_empty());
}

#[test]
fn command_descriptions_state_the_model_limits() {
    let doc = PhotoDocument::create("art.psd", 32, 24).unwrap();
    for (id, field, range) in [
        ("file.new", "resolution", "1 to 30000"),
        ("image.imageSize", "width", "1..300000"),
        ("image.imageSize", "height", "1..300000"),
        ("image.imageSize", "resolution", "1..10000"),
        ("image.canvasSize", "width", "1..300000"),
        ("image.canvasSize", "height", "1..300000"),
    ] {
        let command = doc.commands().into_iter().find(|c| c.id == id).unwrap();
        let schema = command.schema.unwrap();
        assert!(schema[field]["description"].as_str().unwrap_or_default().contains(range), "{id}.{field}: {}", schema[field]);
    }
}

#[test]
fn batch_accepts_the_new_document_resolution_limit() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    let mut exchange = Exchange::new();
    exchange.batch(&mut doc, &json!({"ops":[{"op":"set","target":"image","field":"dpi","value":30000}]}), false).unwrap();
    assert_eq!(doc.inspect().unwrap()["resolution"], json!(30000.0));
}
