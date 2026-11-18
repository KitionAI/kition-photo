//! Round trip: the repository's sample PSD → import (replica) → load (`Document`) → export
//! (PSD bytes) → re-import. The same layer tree, properties and pixels come back.

use kdm_model::PhotoModel;
use kition_kit::kdm::ActorId;
use kition_kit::session::{KitModel, MemoryBlobs};
use kition_photo_doc::{Layer, LayerContent, Surface};
use kition_photo_geom::Rect;

fn sample_bytes() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../web/public/sample/Sample.psd");
    std::fs::read(path).unwrap()
}

fn assert_surfaces_equal(canvas: Rect, a: &Surface, b: &Surface) {
    assert_eq!(a.format(), b.format(), "pixel format");
    assert_eq!(a.read_region(canvas), b.read_region(canvas), "decoded pixels differ");
}

fn assert_same_tree(canvas: Rect, before: &[Layer], after: &[Layer]) {
    assert_eq!(before.len(), after.len(), "layer count");
    for (a, b) in before.iter().zip(after) {
        assert_eq!(a.name, b.name, "layer name");
        assert_eq!(a.visible, b.visible, "layer visible");
        assert_eq!(a.blend, b.blend, "layer blend");
        assert_eq!(a.opacity, b.opacity, "layer opacity");
        assert_eq!(a.fill_opacity, b.fill_opacity, "layer fillOpacity");
        assert_eq!(a.clipped, b.clipped, "layer clipped");
        assert_eq!(a.content.kind_name(), b.content.kind_name(), "layer kind");
        assert_eq!(a.mask.is_some(), b.mask.is_some(), "layer mask presence");
        match (&a.content, &b.content) {
            (LayerContent::Raster(sa), LayerContent::Raster(sb)) => assert_surfaces_equal(canvas, sa, sb),
            (LayerContent::Group(ga), LayerContent::Group(gb)) => assert_same_tree(canvas, &ga.children, &gb.children),
            (LayerContent::Text(ta), LayerContent::Text(tb)) => {
                assert_eq!(ta.text, tb.text, "text content");
                assert_eq!(ta.font_family, tb.font_family, "text font");
                assert_eq!(ta.size_pt, tb.size_pt, "text size");
            }
            (LayerContent::Shape(pa), LayerContent::Shape(pb)) => assert_eq!(pa.path, pb.path, "shape path"),
            _ => {}
        }
    }
}

#[test]
fn sample_psd_round_trips_through_the_replica() {
    let bytes = sample_bytes();
    let mut blobs = MemoryBlobs::default();
    let actor = ActorId::from_u64(1).unwrap();

    let replica = PhotoModel::import("Sample.psd", &bytes, &mut blobs, actor).unwrap();
    let model = PhotoModel::load(&replica, &blobs).unwrap();
    assert!(!model.document.layers.is_empty(), "the sample has layers");

    let exported = model.export(&replica, "Sample.psd", &blobs).unwrap();

    let actor2 = ActorId::from_u64(2).unwrap();
    let replica2 = PhotoModel::import("Sample.psd", &exported, &mut blobs, actor2).unwrap();
    let model2 = PhotoModel::load(&replica2, &blobs).unwrap();

    assert_eq!(model.document.size, model2.document.size);
    assert_eq!(model.document.mode, model2.document.mode);
    assert_eq!(model.document.depth, model2.document.depth);
    let canvas = Rect::new(0, 0, model.document.size.width as i32, model.document.size.height as i32);
    assert_same_tree(canvas, &model.document.layers, &model2.document.layers);
}

/// Every field whose engine type is an enum is an `enum` field listing the engine's values, as the
/// replica writes them, so the object model describes them and refuses any other value before the
/// image sees it (KHP A48, KitObjectModel OM4).
#[test]
fn enum_fields_list_the_engine_values() {
    use kition_kit::kdm::FieldKind;
    use kition_photo_color::{BlendMode, ColorMode, SampleType};

    // Exhaustive, so a new variant fails to compile here until the schema lists it.
    fn mode(mode: ColorMode) -> ColorMode {
        match mode {
            ColorMode::Bitmap
            | ColorMode::Grayscale
            | ColorMode::Indexed
            | ColorMode::Rgb
            | ColorMode::Cmyk
            | ColorMode::Lab
            | ColorMode::Multichannel
            | ColorMode::Duotone => mode,
        }
    }
    let modes = [
        ColorMode::Bitmap,
        ColorMode::Grayscale,
        ColorMode::Indexed,
        ColorMode::Rgb,
        ColorMode::Cmyk,
        ColorMode::Lab,
        ColorMode::Multichannel,
        ColorMode::Duotone,
    ]
    .map(mode);
    let blends: Vec<BlendMode> = std::iter::once(BlendMode::PassThrough).chain(BlendMode::LAYER_MODES).collect();
    let names = |values: Vec<serde_json::Value>| values.into_iter().map(|value| value.as_str().unwrap().to_owned()).collect::<Vec<_>>();

    let schema = PhotoModel::schema();
    for (ty, field, engine) in [
        ("Image", "mode", names(modes.iter().map(|m| serde_json::to_value(m).unwrap()).collect())),
        ("Image", "depth", names(SampleType::ALL.iter().map(|d| serde_json::to_value(d).unwrap()).collect())),
        ("Layer", "blend", names(blends.iter().map(|b| serde_json::to_value(b).unwrap()).collect())),
    ] {
        let def = schema.field(ty, field).unwrap();
        assert_eq!(def.kind, FieldKind::Enum, "{ty}.{field}");
        assert_eq!(def.values, engine, "{ty}.{field}");
    }
}
