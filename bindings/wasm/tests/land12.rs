//! C8-land-12-photo: receipts describe the model that saving actually keeps.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
use kition_kit::{KitDocument, session::KitModel, session::model::Exchange};
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

fn saved_diff(doc: &mut PhotoDocument, exchange: &mut Exchange, name: &str) -> Value {
    let before = exchange.replica(doc).unwrap().state_json().unwrap();
    let mut again = PhotoDocument::open(name, &doc.save(None).unwrap()).unwrap();
    let after = Exchange::new().replica(&mut again).unwrap().state_json().unwrap();
    kition_kit::kdm::reread::diff_paired(&kdm_model::PhotoModel::schema(), &before, &after).to_json()
}

#[test]
fn created_layer_has_stored_id_and_defaults_before_the_receipt() {
    for name in ["art.psd", "art.kphoto"] {
        let mut doc = PhotoDocument::create(name, 32, 24).unwrap();
        let mut exchange = Exchange::new();
        let receipt = exchange
            .batch(
                &mut doc,
                &json!({"ops":[{"op":"create","type":"Layer","in":"image.layers","fields":{
                    "name":"New", "kind":"raster", "visible":true, "blend":"Normal", "opacity":1, "fillOpacity":1, "clipped":false
                }}]}),
                false,
            )
            .unwrap();
        println!("CREATE {name}: {receipt}");
        let diff = saved_diff(&mut doc, &mut exchange, name);
        for list in ["created", "changed", "moved", "deleted"] {
            assert_eq!(diff[list], json!([]), "{name}: {diff}");
        }
    }
}

#[test]
fn mask_member_limits_refuse_atomically() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    doc.execute("layer.layerMask.revealAll", json!({"layer":1})).unwrap();
    let mut exchange = Exchange::new();
    for (field, value) in [("mask.props.density", json!(2)), ("mask.props.feather", json!(-1))] {
        let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
        let answer = exchange.batch(&mut doc, &json!({"ops":[{"op":"set","target":"1","field":field,"value":value}]}), false);
        assert!(answer.is_err(), "accepted {field}: {answer:?}");
        assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
    }
}

#[test]
fn sdk_tree_delete_reproduction() {
    use kition_kit::kdm::{
        ActorId, DocId, Replica, Schema,
        batch::{self, ReplicaTarget},
    };
    let schema = Schema::parse(
        &json!({"kdm":1,"kit":"kit.probe","schemaVersion":1,"root":"Image","types":{
            "Image":{"fields":{"layers":{"kind":"tree","of":"Layer"}}}, "Layer":{"fields":{}}
        }})
        .to_string(),
    )
    .unwrap();
    let mut replica = Replica::create(std::sync::Arc::new(schema), DocId::new(), ActorId::random()).unwrap();
    batch::run(
        &mut ReplicaTarget(&mut replica),
        &json!({"ops":[
            {"op":"create","type":"Image","id":"image"},
            {"op":"create","type":"Layer","id":"group","in":"image.layers"},
            {"op":"create","type":"Layer","id":"child","in":"image.layers","parent":"group"}
        ]}),
    )
    .unwrap();
    let result = batch::run(&mut ReplicaTarget(&mut replica), &json!({"ops":[{"op":"delete","target":"group"}]}));
    assert!(result.as_ref().err().is_some_and(|error| error.message.contains("has children")), "{result:?}");
}

#[test]
fn sdk_enforces_nested_member_limits_atomically() {
    use kition_kit::kdm::{
        ActorId, DocId, Replica, Schema,
        batch::{self, ReplicaTarget},
    };
    let schema = Schema::parse(
        &json!({"kdm":1,"kit":"kit.probe","schemaVersion":1,"root":"Image","types":{
            "Image":{"fields":{"props":{"kind":"json","members":{"mask.density":{"minimum":0,"maximum":1}}}}}
        }})
        .to_string(),
    )
    .unwrap();
    let mut replica = Replica::create(std::sync::Arc::new(schema), DocId::new(), ActorId::random()).unwrap();
    let before = replica.state_json().unwrap();
    let result = batch::run(
        &mut ReplicaTarget(&mut replica),
        &json!({"ops":[
            {"op":"create","type":"Image","id":"image","fields":{"props":{"mask":{"density":2}}}}
        ]}),
    );
    assert!(result.as_ref().err().is_some_and(|e| e.message.contains("minimum/maximum")), "{result:?}");
    assert_eq!(replica.state_json().unwrap(), before);
    batch::run(&mut ReplicaTarget(&mut replica), &json!({"ops":[{"op":"create","type":"Image","id":"image","fields":{"props":{"mask":{"density":0.5}}}}]}))
        .unwrap();
    assert_eq!(replica.state_json().unwrap()["objects"]["image"]["props"]["mask"]["density"], 0.5);
}

#[test]
fn deleting_the_last_layer_stays_deleted_on_disk() {
    for name in ["art.psd", "art.psb", "art.kphoto"] {
        let mut doc = PhotoDocument::create(name, 32, 24).unwrap();
        let mut exchange = Exchange::new();
        exchange.batch(&mut doc, &json!({"ops":[{"op":"delete","target":"1"}]}), false).unwrap();
        let diff = saved_diff(&mut doc, &mut exchange, name);
        for list in ["created", "changed", "moved", "deleted"] {
            assert_eq!(diff[list], json!([]), "{name}: {diff}");
        }
        assert_eq!(doc.inspect().unwrap()["layers"], json!([]));
    }
}

#[test]
fn incomplete_non_raster_creation_refuses_before_commit() {
    let mut doc = PhotoDocument::create("art.psd", 32, 24).unwrap();
    let mut exchange = Exchange::new();
    let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
    let result = exchange.batch(
        &mut doc,
        &json!({"ops":[{"op":"create","type":"Layer","in":"image.layers","fields":{
            "name":"Shape", "kind":"shape", "visible":true, "blend":"Normal", "opacity":1, "fillOpacity":1, "clipped":false
        }}]}),
        false,
    );
    assert!(result.is_err(), "incomplete shape accepted: {result:?}");
    assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
}

#[test]
fn creation_under_a_non_group_refuses_before_commit() {
    let mut doc = PhotoDocument::create("art.psd", 32, 24).unwrap();
    let mut exchange = Exchange::new();
    let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
    let result = exchange.batch(
        &mut doc,
        &json!({"ops":[{"op":"create","type":"Layer","in":"image.layers","parent":"1","fields":{
            "name":"Child", "kind":"raster", "visible":true, "blend":"Normal", "opacity":1, "fillOpacity":1, "clipped":false
        }}]}),
        false,
    );
    assert!(result.is_err(), "child of raster accepted: {result:?}");
    assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
}

#[test]
fn malformed_group_props_refuse_without_panicking() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    let mut exchange = Exchange::new();
    let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
    let result = exchange.batch(
        &mut doc,
        &json!({"ops":[{"op":"create","type":"Layer","in":"image.layers","fields":{
            "name":"Group", "kind":"group", "visible":true, "blend":"PassThrough", "opacity":1, "fillOpacity":1, "clipped":false,"props":2
        }}]}),
        false,
    );
    assert!(result.is_err(), "malformed group accepted: {result:?}");
    assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
}

#[test]
fn unplaced_layer_creation_refuses_instead_of_disappearing_on_save() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    let mut exchange = Exchange::new();
    let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
    let result = exchange.batch(
        &mut doc,
        &json!({"ops":[{"op":"create","type":"Layer","fields":{
            "name":"Unplaced", "kind":"raster", "visible":true, "blend":"Normal", "opacity":1, "fillOpacity":1, "clipped":false
        }}]}),
        false,
    );
    assert!(result.is_err(), "unplaced layer accepted: {result:?}");
    assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
}

#[test]
fn command_schemas_expose_numeric_limits() {
    let doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    for (id, field, min, max) in [
        ("file.new", "resolution", json!(1), json!(30000)),
        ("layer.vectorMask.edit", "density", json!(0), json!(100)),
        ("layer.vectorMask.edit", "feather", json!(0), Value::Null),
        ("layer.layerStyle.dropShadow", "opacity", json!(0), json!(100)),
        ("layer.layerStyle.dropShadow", "spread", json!(0), json!(100)),
        ("layer.layerStyle.gradientOverlay", "scale", json!(1), json!(1000)),
    ] {
        let schema = doc.commands().into_iter().find(|command| command.id == id).unwrap().schema.unwrap();
        assert_eq!(schema[field]["minimum"].as_f64(), min.as_f64(), "{id}.{field}: {}", schema[field]);
        assert_eq!(schema[field]["maximum"].as_f64(), max.as_f64(), "{id}.{field}: {}", schema[field]);
    }
}

#[test]
fn nested_vector_mask_and_effect_limits_refuse_atomically() {
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24).unwrap();
    doc.execute("layer.vectorMask.add", json!({"layer":1,"path":{"subpaths":[]}})).unwrap();
    doc.execute("layer.layerStyle.dropShadow", json!({"layer":1})).unwrap();
    let mut exchange = Exchange::new();
    for (field, value) in
        [("props.extra.vector_mask.density", json!(2)), ("props.extra.vector_mask.feather", json!(-1)), ("effects.items.0.DropShadow.common.opacity", json!(2))]
    {
        let before = exchange.replica(&mut doc).unwrap().state_json().unwrap();
        let result = exchange.batch(&mut doc, &json!({"ops":[{"op":"set","target":"1","field":field,"value":value}]}), false);
        assert!(result.is_err(), "invalid {field} accepted: {result:?}");
        assert_eq!(exchange.replica(&mut doc).unwrap().state_json().unwrap(), before);
    }
}

#[test]
fn json_mask_values_are_quantized_before_the_receipt() {
    for name in ["art.kphoto", "art.psd"] {
        let mut doc = PhotoDocument::create(name, 32, 24).unwrap();
        doc.execute("layer.vectorMask.revealAll", json!({"layer":1})).unwrap();
        doc.execute("layer.layerMask.revealAll", json!({"layer":1})).unwrap();
        let mut exchange = Exchange::new();
        exchange
            .batch(
                &mut doc,
                &json!({"ops":[
                    {"op":"set","target":"1","field":"props.extra.vector_mask.density","value":0.37123456},
                    {"op":"set","target":"1","field":"mask.props.density","value":0.37123456},
                    {"op":"set","target":"1","field":"mask.props.feather","value":0.123456789}
                ]}),
                false,
            )
            .unwrap();
        let replica = exchange.replica(&mut doc).unwrap();
        let layer = replica.objects_of_type("Layer").pop().unwrap();
        let props = replica.get(&layer, "props").unwrap();
        let density = props.pointer("/extra/vector_mask/density").unwrap().as_f64().unwrap();
        assert_eq!(density, f64::from(density as f32), "the receipt must say the f32 the engine holds");
        let diff = saved_diff(&mut doc, &mut exchange, name);
        for list in ["created", "changed", "moved", "deleted"] {
            assert_eq!(diff[list], json!([]), "{name}: {diff}");
        }
    }
}

#[test]
fn command_created_layer_defaults_match_saved_file() {
    let mut doc = PhotoDocument::open("art.psd", include_bytes!("../../../conformance/basic.psd")).unwrap();
    doc.execute("layer.new.layer", json!({"name":"New"})).unwrap();
    let diff = saved_diff(&mut doc, &mut Exchange::new(), "art.psd");
    assert_eq!(diff["changed"], json!([]), "{diff}");
}
