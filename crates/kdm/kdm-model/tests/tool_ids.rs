//! Tool ids (KitObjectModel OM9, task card C-photo): the object model names every layer by its
//! numeric layer id (`Layer.number`, the id the kit's inspect lists and its commands take), so
//! receipts, queries, batches and commands speak one id space. A layer mask and a channel's pixels
//! are parts of what holds them (`owns`), so a change of them is told on the layer or channel.
//! A layer's non-transparent bounds are a derived value (`~bounds`), and nothing else is a place:
//! the stack is the `layers` tree.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use kdm_model::kition_kit::kdm::Origin;
use kdm_model::kition_kit::kdm::tool_ids::ToolIds;
use kdm_model::kition_kit::session::KitModel;
use kdm_model::{PhotoModel, PhotoSession};
use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{Document, Layer, LayerId, Size};

fn document() -> Document {
    let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
    for (n, name) in [(4, "Back"), (9, "Ink"), (12, "Top")] {
        let mut layer = Layer::raster(name, PixelFormat::RGBA8);
        layer.id = LayerId(n);
        doc.layers.push(layer);
    }
    doc
}

/// Every layer's tool id, by its layer id.
fn named(session: &PhotoSession) -> Vec<(u64, String)> {
    let tools = ToolIds::of_replica(session.replica());
    let ids = session.model().ids();
    let mut out: Vec<(u64, String)> = session
        .document()
        .walk()
        .iter()
        .map(|(_, _, layer)| {
            let object = ids.object(layer.id).unwrap_or_else(|| panic!("layer {} has no object", layer.id.0));
            (layer.id.0, tools.tool_id(object).unwrap_or_else(|| panic!("layer {} ({object}) has no tool id", layer.id.0)).to_owned())
        })
        .collect();
    out.sort();
    out
}

#[test]
fn the_schema_declares_the_layer_number_as_the_tool_id_and_what_layers_hold() {
    let schema = PhotoModel::schema();
    let layer = schema.types.get("Layer").expect("a Layer type");
    let tool = layer.tool_id.as_ref().expect("Layer declares a tool id");
    assert_eq!((tool.field.as_str(), tool.prefix.as_str(), tool.parent.as_deref()), ("number", "", None));
    let number = layer.fields.get("number").expect("Layer.number");
    assert!(number.read_only && number.optional, "Layer.number is read-only and optional: only the kit writes it");
    assert!(layer.fields["mask"].owns, "a layer's mask is part of the layer");
    assert!(schema.types["Channel"].fields["surface"].owns, "a channel's pixels are part of the channel");
    assert!(layer.derived.contains_key("bounds"), "a layer's non-transparent bounds are derived");
    assert!(schema.types.values().all(|ty| ty.fields.values().all(|field| !field.position)), "no field says where a layer is alone");
}

#[test]
fn layers_are_named_by_the_ids_the_kit_lists_and_keep_them_after_a_reorder_and_a_new_replica() {
    let doc = document();
    let mut session = PhotoSession::create(&doc).unwrap();
    let want = vec![(4, "4".to_owned()), (9, "9".to_owned()), (12, "12".to_owned())];
    assert_eq!(named(&session), want);
    let mut after = doc.clone();
    after.layers.reverse();
    after.layers[0].name = "Renamed".into();
    session.record(&Origin::person("person:test"), &after).unwrap();
    assert_eq!(named(&session), want);
    let reopened = PhotoSession::open_snapshot(&session.snapshot().unwrap(), session.blobs().clone()).unwrap();
    assert_eq!(named(&reopened), want, "a new replica names every layer as before");
}
