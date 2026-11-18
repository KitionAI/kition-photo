use kition_photo_color::{ColorMode, PixelFormat, SampleType};
use kition_photo_doc::{AlphaChannel, Document, Guide, NamedPath, Note, Size};
use kition_photo_io::{ExportOptions, export, import};
use kition_photo_psd::PsdFile;
use kition_photo_raster::Surface;

#[test]
fn resource_keys_survive_psd_and_native_saves_and_foreign_edits_invalidate_them() {
    let mut doc = Document::with_background("keys", Size::new(16, 16), ColorMode::Rgb, SampleType::U8, kition_photo_color::Color::WHITE);
    doc.channels.push(AlphaChannel::new("Alpha", Surface::new(PixelFormat::GRAY8)));
    doc.paths.push(NamedPath { name: "Path".to_owned(), ..Default::default() });
    doc.notes.push(Note { text: "Note".to_owned(), ..Default::default() });
    doc.guides.vertical.push(Guide::new(5.0));
    let keys = |doc: &Document| vec![doc.channels[0].key.clone(), doc.paths[0].key.clone(), doc.notes[0].key.clone(), doc.guides.vertical[0].key.clone()];
    assert!(keys(&doc).iter().all(Option::is_some));
    let native = kition_photo_format::save_to_bytes(&doc, &Default::default()).unwrap();
    let zip = kition_photo_format::zip::ZipReader::new(&native).unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    assert_eq!(manifest["document"]["guides"]["vertical"], serde_json::json!([5.0]), "native guide coordinates remain scalar values");
    assert_eq!(keys(&kition_photo_format::load_from_bytes(&native).unwrap()), keys(&doc));
    let bytes = export(&doc, "keys.psd", &ExportOptions::default()).unwrap().bytes;
    let back = import("keys.psd", &bytes).unwrap().document;
    assert_eq!(keys(&back), keys(&doc));
    let mut foreign = PsdFile::from_bytes(&bytes).unwrap();
    foreign.resources.iter_mut().find(|r| r.id == 1032).unwrap().data[19] ^= 32;
    let changed = import("foreign.psd", &foreign.to_bytes().unwrap()).unwrap().document;
    assert_eq!(keys(&changed), vec![None; 4]);
}
