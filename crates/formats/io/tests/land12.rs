//! C8-land-12-photo: an empty layered save must not hide a flattened image.
use kition_photo_psd::{Compression, Version, layer::LayerInfo, testgen};

#[test]
fn flattened_psd_with_an_empty_layer_table_keeps_its_pixels() {
    let mut file = testgen::small(Version::Psd, Compression::Raw);
    file.layer_info = Some(LayerInfo::default());
    let imported = kition_photo_io::import("flat.psd", &file.to_bytes().unwrap()).unwrap();
    assert_eq!(imported.document.layers.len(), 1);
}

#[test]
fn empty_layered_images_stay_empty_at_every_depth() {
    use kition_photo_doc::{ColorMode, Document, SampleType};
    use kition_photo_geom::Size;
    for name in ["empty.psd", "empty.psb"] {
        for mode in [ColorMode::Rgb, ColorMode::Grayscale, ColorMode::Cmyk] {
            for depth in [SampleType::U8, SampleType::U16, SampleType::F32] {
                let doc = Document::new("Empty", Size::new(4, 3), mode, depth);
                let bytes = kition_photo_io::export(&doc, name, &Default::default()).unwrap().bytes;
                let back = kition_photo_io::import(name, &bytes).unwrap().document;
                assert!(back.layers.is_empty(), "{name}: {mode:?} {depth:?}");
                assert_eq!((back.mode, back.depth), (mode, depth));
            }
        }
    }
}
