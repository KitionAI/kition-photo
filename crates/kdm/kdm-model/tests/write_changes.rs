//! `write_changes`: one stroke (a direct tile edit — `engine::Session` is a higher layer than
//! `kdm-model`, so this reaches for the simplest input that still exercises the diff: a new
//! `Document` value with one tile touched) writes only the changed tile and nothing else.

use kdm_model::{Nodes, write_changes, write_document};
use kition_kit::kdm::{ActorId, DocId, Origin, Replica};
use kition_kit::session::{IdMap, KitModel, MemoryBlobs};
use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{Document, Layer, LayerContent, Size};
use kition_photo_geom::TileCoord;

fn one_layer_document() -> Document {
    let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
    doc.layers.push(Layer::raster("Layer 1", PixelFormat::RGBA8));
    doc.layers.push(Layer::raster("Layer 2", PixelFormat::RGBA8));
    doc
}

#[test]
fn write_changes_touches_only_the_painted_tile() {
    let before = one_layer_document();
    let mut blobs = MemoryBlobs::default();
    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    let (mut replica, ()) =
        Replica::create_with(kdm_model::PhotoModel::schema(), DocId::new(), ActorId::from_u64(1).unwrap(), &Origin::system("import"), |tx| {
            write_document(tx, &before, &mut ids, &mut nodes, &mut blobs).map(|_| ())
        })
        .unwrap();
    replica.take_changed();

    let layer_object = ids.object(before.layers[0].id).unwrap().clone();
    let other_object = ids.object(before.layers[1].id).unwrap().clone();
    let tiles_before = replica.map_entries(&layer_object, "tiles").unwrap();
    assert!(tiles_before.is_empty(), "a fresh raster layer starts with no tiles");

    let mut after = before.clone();
    let LayerContent::Raster(surface) = &mut after.layers[0].content else { unreachable!() };
    surface.tile_mut(TileCoord { tx: 0, ty: 0 }).bytes_mut()[0] = 200;

    let wrote = replica.transact(&Origin::person("user:test"), |tx| write_changes(tx, &before, &after, &mut ids, &mut nodes, &mut blobs)).unwrap();
    assert!(wrote);

    let changed = replica.take_changed();
    assert_eq!(changed, std::collections::BTreeSet::from([layer_object.clone()]), "only the painted layer's object changed");

    let tiles_after = replica.map_entries(&layer_object, "tiles").unwrap();
    assert_eq!(tiles_after.len(), 1, "exactly one tile was written");
    assert_eq!(tiles_after[0].0, "0,0");

    // The other layer, and every other field of the painted layer, are untouched.
    assert!(replica.map_entries(&other_object, "tiles").unwrap().is_empty());
    assert_eq!(replica.get(&layer_object, "name").unwrap(), serde_json::json!("Layer 1"));
    assert_eq!(replica.get(&layer_object, "opacity").unwrap().as_f64(), Some(1.0));
}

fn names(layers: &[kition_photo_doc::Layer]) -> Vec<&str> {
    layers.iter().map(|layer| layer.name.as_str()).collect()
}

#[test]
fn write_changes_places_a_new_layer_inside_an_existing_group() {
    let before = {
        let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
        doc.layers.push(Layer::group("Group 1", vec![Layer::raster("Child A", PixelFormat::RGBA8)]));
        doc.layers.push(Layer::raster("Top", PixelFormat::RGBA8));
        doc
    };
    let mut blobs = MemoryBlobs::default();
    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    let (mut replica, ()) =
        Replica::create_with(kdm_model::PhotoModel::schema(), DocId::new(), ActorId::from_u64(1).unwrap(), &Origin::system("import"), |tx| {
            write_document(tx, &before, &mut ids, &mut nodes, &mut blobs).map(|_| ())
        })
        .unwrap();

    let mut after = before.clone();
    let LayerContent::Group(group) = &mut after.layers[0].content else { unreachable!() };
    group.children.push(Layer::raster("Child B", PixelFormat::RGBA8));

    let wrote = replica.transact(&Origin::person("user:test"), |tx| write_changes(tx, &before, &after, &mut ids, &mut nodes, &mut blobs)).unwrap();
    assert!(wrote);

    let model = kdm_model::PhotoModel::load(&replica, &blobs).unwrap();
    assert_eq!(names(&model.document.layers), vec!["Group 1", "Top"]);
    let LayerContent::Group(group) = &model.document.layers[0].content else { unreachable!() };
    assert_eq!(names(&group.children), vec!["Child A", "Child B"], "the new layer landed inside the existing group, after its sibling");
}

#[test]
fn write_changes_reorders_within_and_across_groups() {
    let before = {
        let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
        doc.layers.push(Layer::group("Group 1", vec![Layer::raster("A", PixelFormat::RGBA8), Layer::raster("B", PixelFormat::RGBA8)]));
        doc.layers.push(Layer::group("Group 2", vec![Layer::raster("C", PixelFormat::RGBA8)]));
        doc
    };
    let mut blobs = MemoryBlobs::default();
    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    let (mut replica, ()) =
        Replica::create_with(kdm_model::PhotoModel::schema(), DocId::new(), ActorId::from_u64(1).unwrap(), &Origin::system("import"), |tx| {
            write_document(tx, &before, &mut ids, &mut nodes, &mut blobs).map(|_| ())
        })
        .unwrap();

    // Swap A and B within Group 1 (a pure reorder), and move C out of Group 2 into Group 1,
    // after A and B (a cross-group move) — Group 2 ends up empty.
    let mut after = before.clone();
    let LayerContent::Group(group2) = &mut after.layers[1].content else { unreachable!() };
    let c = group2.children.remove(0);
    let LayerContent::Group(group1) = &mut after.layers[0].content else { unreachable!() };
    group1.children.swap(0, 1);
    group1.children.push(c);

    let wrote = replica.transact(&Origin::person("user:test"), |tx| write_changes(tx, &before, &after, &mut ids, &mut nodes, &mut blobs)).unwrap();
    assert!(wrote);

    let model = kdm_model::PhotoModel::load(&replica, &blobs).unwrap();
    assert_eq!(names(&model.document.layers), vec!["Group 1", "Group 2"]);
    let LayerContent::Group(group1) = &model.document.layers[0].content else { unreachable!() };
    assert_eq!(names(&group1.children), vec!["B", "A", "C"]);
    let LayerContent::Group(group2) = &model.document.layers[1].content else { unreachable!() };
    assert!(group2.children.is_empty());
}

/// kition-kdm refuses to delete a tree node's object while the node has live children (kition-sdk
/// C8-fix-10): a group deleted with what it holds goes children first, and a child moved out of a
/// group deleted in the same change is moved before the group goes.
#[test]
fn write_changes_deletes_a_group_with_what_it_holds_and_keeps_what_moved_out() {
    let before = {
        let mut doc = Document::new("t", Size { width: 300, height: 300 }, ColorMode::Rgb, SampleType::U8);
        let inner = Layer::group("Inner", vec![Layer::raster("Deep", PixelFormat::RGBA8)]);
        doc.layers.push(Layer::group("Outer", vec![Layer::raster("A", PixelFormat::RGBA8), inner, Layer::raster("Kept", PixelFormat::RGBA8)]));
        doc.layers.push(Layer::raster("Top", PixelFormat::RGBA8));
        doc
    };
    let mut blobs = MemoryBlobs::default();
    let mut ids = IdMap::default();
    let mut nodes = Nodes::default();
    let (mut replica, ()) =
        Replica::create_with(kdm_model::PhotoModel::schema(), DocId::new(), ActorId::from_u64(1).unwrap(), &Origin::system("import"), |tx| {
            write_document(tx, &before, &mut ids, &mut nodes, &mut blobs).map(|_| ())
        })
        .unwrap();

    // Delete Outer (and A, Inner and Deep with it), but move Kept to the top level first.
    let mut after = before.clone();
    let LayerContent::Group(outer) = &mut after.layers[0].content else { unreachable!() };
    let kept = outer.children.pop().unwrap();
    after.layers.remove(0);
    after.layers.insert(0, kept);

    let wrote = replica.transact(&Origin::person("user:test"), |tx| write_changes(tx, &before, &after, &mut ids, &mut nodes, &mut blobs)).unwrap();
    assert!(wrote);

    let model = kdm_model::PhotoModel::load(&replica, &blobs).unwrap();
    assert_eq!(names(&model.document.layers), vec!["Kept", "Top"]);
    assert_eq!(replica.objects_of_type("Layer").len(), 2, "the deleted layers' objects are gone");
}
